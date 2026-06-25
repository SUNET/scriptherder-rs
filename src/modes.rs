//! Operating modes (wrap/ls/check/lastlog), mirroring
//! `../src/scriptherder.py:1123-1280`. Each returns a process exit code.

use chrono::{Local, TimeZone};
use scriptherder::checkstatus::CheckStatus;
use scriptherder::cli::{RawArgs, RawMode};
use scriptherder::jobs_list::JobsList;
use scriptherder::table::{Align, ColumnMeta, DataTable};
use std::collections::HashMap;
use std::io::IsTerminal;

/// Map a Nagios status level to its exit code. Unknown/FAIL → 3 (defensive).
pub fn level_to_code(level: &str) -> i32 {
    match level {
        "OK" => 0,
        "WARNING" => 1,
        "CRITICAL" => 2,
        _ => 3,
    }
}

/// `mode_wrap` (../src/scriptherder.py:1123): run a command and persist its state.
pub fn wrap(args: &RawArgs) -> i32 {
    let (name, umask, random_sleep, cmd) = match &args.mode {
        RawMode::Wrap {
            name,
            umask,
            random_sleep,
            cmd,
            ..
        } => (name.clone(), umask.clone(), *random_sleep, cmd.clone()),
        _ => unreachable!("wrap called with non-wrap mode"),
    };

    let mut job = match scriptherder::job::Job::new(&name, cmd.clone()) {
        Ok(j) => j,
        Err(e) => {
            log::error!("Failed creating job: {} ({})", e.reason(), e.filename());
            return 1;
        }
    };

    if random_sleep > 0 {
        let seconds = rand::random::<f64>() * random_sleep as f64;
        log::debug!("Sleeping for {seconds:.2} seconds");
        std::thread::sleep(std::time::Duration::from_secs_f64(seconds));
    }

    log::debug!("Invoking '{}'", cmd.join(" "));
    job.run();
    log::debug!("Finished, exit status {:?}", job.exit_status());

    // Record what the job's status evaluates to at the time of execution.
    let mut checkstatus = CheckStatus::new(true, args.checkdir.clone(), HashMap::new());
    let job_name = job.name();
    let check = checkstatus.get_check(&job_name).ok().cloned();
    if let Some(check) = check {
        job.check(&check);
        let msg = format!(
            "Job {job_name:?} check status is {} ({})",
            job.check_status().unwrap_or("UNKNOWN"),
            job.check_reason().unwrap_or("")
        );
        if job.is_ok() {
            log::info!("{msg}");
        } else {
            log::warn!("{msg}");
        }
    }

    if let Err(e) = job.save_to_file(&args.datadir, None, &umask) {
        log::error!("Failed saving job to file: {e}");
        return 1;
    }
    0
}

/// `mode_ls` (../src/scriptherder.py:1153): list saved job states in a table.
pub fn ls(args: &RawArgs, names: &[String]) -> i32 {
    let jobs = match JobsList::from_dir(&args.datadir, &args.checkdir, names, true, &[]) {
        Ok(j) => j,
        Err(e) => {
            log::error!("Failed loading jobs: {} ({})", e.reason(), e.filename());
            return 1;
        }
    };
    let last_of_each = jobs.last_of_each();

    // Which indices to display.
    let chosen: Vec<usize> = if names.is_empty() {
        println!(
            "\n=== Showing the last execution of each job, use 'ls ALL' to see all executions\n"
        );
        last_of_each.clone()
    } else {
        (0..jobs.jobs.len()).collect()
    };

    let fields = vec![
        ColumnMeta::new("Start time", Align::Right),
        ColumnMeta::new("Duration", Align::Left),
        ColumnMeta::new("Age", Align::Left),
        ColumnMeta::new("Status", Align::Left),
        ColumnMeta::new("Criteria", Align::Left),
        ColumnMeta::new("Name", Align::Left),
        ColumnMeta::new("Filename", Align::Left),
    ];
    let mut data = DataTable::new(fields);

    let is_tty = std::io::stdout().is_terminal();

    for &i in &chosen {
        let start = match jobs.jobs[i].start_time() {
            Some(st) => {
                let secs = st.trunc() as i64;
                match Local.timestamp_opt(secs, 0).single() {
                    Some(dt) => dt.format("%Y-%m-%d %X").to_string(),
                    None => "***".to_string(),
                }
            }
            None => "***".to_string(),
        };
        data.push(&start);
        data.push(&jobs.jobs[i].duration_str());
        data.push(&format!("{} ago", jobs.jobs[i].age()));

        let (level, msg): (String, String) = if last_of_each.contains(&i) {
            // For the last instance of each job, evaluate full check-mode status.
            let this = &jobs.jobs[i];
            // Re-construct a one-job list; loaded jobs already passed version checks.
            let single = scriptherder::job::Job::from_data(this.data.clone())
                .expect("loaded job data round-trips");
            let mut cs = CheckStatus::new(false, args.checkdir.clone(), HashMap::new());
            cs.check_jobs(JobsList::from_jobs(vec![single], false));
            let (lvl, m) = cs.aggregate_status();
            (lvl, m.unwrap_or_default())
        } else {
            let exit = jobs.jobs[i].exit_status();
            let level = if exit != Some(0) { "Non-zero" } else { "-" };
            (
                level.to_string(),
                format!(
                    "exit={}, age={}",
                    exit.map(|e| e.to_string())
                        .unwrap_or_else(|| "None".to_string()),
                    jobs.jobs[i].age()
                ),
            )
        };

        // ANSI coloring matching ../src/scriptherder.py:1201-1216.
        let (color1, color2, reset) = if level != "OK" && level != "-" && is_tty {
            let bold = "\x1b[;1m".to_string();
            let c1 = if level == "CRITICAL" {
                "\x1b[1;31m".to_string()
            } else {
                bold.clone()
            };
            (c1, bold, "\x1b[0;0m".to_string())
        } else {
            (String::new(), String::new(), String::new())
        };

        data.push(&format!("{color1}{level}{reset}"));
        data.push(&format!("{color2}{msg}{reset}"));
        data.push(&jobs.jobs[i].name());
        data.push(jobs.jobs[i].filename().unwrap_or(""));
        data.new_line();
    }

    print!("{}", data.render());
    0
}

/// `mode_check` (../src/scriptherder.py:1222): Nagios-style aggregate status.
pub fn run_check(args: &RawArgs, names: &[String], exclude: &[String]) -> i32 {
    let jobs = match JobsList::from_dir(&args.datadir, &args.checkdir, names, true, exclude) {
        Ok(j) => j,
        Err(e) => {
            println!(
                "UNKNOWN: Failed loading check from file '{}' ({})",
                e.filename(),
                e.reason()
            );
            return 3;
        }
    };
    let mut status = CheckStatus::new(false, args.checkdir.clone(), HashMap::new());
    status.check_jobs(jobs);
    let (level, msg) = status.aggregate_status();
    println!("{}: {}", level, msg.unwrap_or_default());
    level_to_code(&level)
}

/// `mode_lastlog` (../src/scriptherder.py:1244): print last (or last-failed) output.
pub fn lastlog(args: &RawArgs, names: &[String], fail_status: bool) -> i32 {
    let jobs = match JobsList::from_dir(&args.datadir, &args.checkdir, names, true, &[]) {
        Ok(j) => j,
        Err(e) => {
            log::error!("Failed loading jobs: {} ({})", e.reason(), e.filename());
            return 1;
        }
    };

    if jobs.jobs.is_empty() {
        // Python returns None here; `__main__` then exits 1.
        println!("No jobs found");
        return 1;
    }

    let last_of_each = jobs.last_of_each();
    let view: Vec<usize> = last_of_each
        .iter()
        .copied()
        .filter(|&i| {
            let job = &jobs.jobs[i];
            match job.output_filename() {
                Some(fname) if std::path::Path::new(fname).is_file() => {
                    if fail_status {
                        job.exit_status() != Some(0)
                    } else {
                        true
                    }
                }
                _ => false,
            }
        })
        .collect();

    if !view.is_empty() {
        for &i in &view {
            let display = jobs.jobs[i].display();
            let fname = match jobs.jobs[i].output_filename() {
                Some(f) => f.to_string(),
                None => continue,
            };
            match std::fs::read(&fname) {
                Ok(bytes) => {
                    println!("=== Script output of {display:?}");
                    use std::io::Write;
                    let _ = std::io::stdout().write_all(&bytes);
                    println!("=== End of script output\n");
                }
                Err(e) => log::warn!("Failed reading {fname}: {e}"),
            }
        }
    } else {
        // Names in insertion order, matching Python `by_name.keys()`.
        let names: Vec<String> = jobs.by_name_ordered().into_iter().map(|(n, _)| n).collect();
        println!(
            "No script output found for {} with fail_status={}",
            names.join(", "),
            if fail_status { "True" } else { "False" }
        );
    }
    // Python: `sys.exit(int(not bool(view_jobs)))` → 0 if output shown, else 1.
    if view.is_empty() {
        1
    } else {
        0
    }
}
