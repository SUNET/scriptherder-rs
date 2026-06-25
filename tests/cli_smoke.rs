use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_scriptherder"))
}

#[test]
fn ls_empty_datadir_runs() {
    let dir = std::env::temp_dir().join(format!("sh_cli_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let out = bin()
        .arg("-d")
        .arg(&dir)
        .arg("--checkdir")
        .arg(&dir)
        .arg("ls")
        .output()
        .unwrap();
    assert!(out.status.success());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn wrap_then_check_roundtrip() {
    let dir = std::env::temp_dir().join(format!("sh_cli2_{}", std::process::id()));
    let checkdir = dir.join("check");
    std::fs::create_dir_all(&checkdir).unwrap();
    std::fs::write(
        checkdir.join("smoke.ini"),
        "[check]\nok = exit_status=0, max_age=8h\n",
    )
    .unwrap();
    // wrap a successful command
    let w = bin()
        .arg("-d")
        .arg(&dir)
        .arg("--checkdir")
        .arg(&checkdir)
        .arg("wrap")
        .arg("-N")
        .arg("smoke")
        .arg("--")
        .arg("/bin/true")
        .output()
        .unwrap();
    assert!(w.status.success());
    // check should report OK (exit 0)
    let c = bin()
        .arg("-d")
        .arg(&dir)
        .arg("--checkdir")
        .arg(&checkdir)
        .arg("check")
        .arg("smoke")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&c.stdout);
    assert!(stdout.starts_with("OK:"), "got: {stdout}");
    assert_eq!(c.status.code(), Some(0));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn lastlog_empty_datadir_no_jobs_exits_1() {
    let dir = std::env::temp_dir().join(format!("sh_cli3_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let out = bin()
        .arg("-d")
        .arg(&dir)
        .arg("--checkdir")
        .arg(&dir)
        .arg("lastlog")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(stdout.trim(), "No jobs found", "got: {stdout}");
    assert_eq!(out.status.code(), Some(1));
    std::fs::remove_dir_all(&dir).ok();
}

/// Run `wrap -N <name> -- <cmd...>` into the given dirs.
fn wrap_job(datadir: &std::path::Path, checkdir: &std::path::Path, name: &str, cmd: &[&str]) {
    let w = bin()
        .arg("-d")
        .arg(datadir)
        .arg("--checkdir")
        .arg(checkdir)
        .arg("wrap")
        .arg("-N")
        .arg(name)
        .arg("--")
        .args(cmd)
        .output()
        .unwrap();
    assert!(w.status.success(), "wrap {name} failed");
}

/// `check --exclude` drops a failing job, flipping the aggregate from CRITICAL to OK.
#[test]
fn check_exclude_removes_failing_job() {
    let dir = std::env::temp_dir().join(format!("sh_excl1_{}", std::process::id()));
    let checkdir = dir.join("check");
    std::fs::create_dir_all(&checkdir).unwrap();
    std::fs::write(checkdir.join("alpha.ini"), "[check]\nok = exit_status=0\n").unwrap();
    std::fs::write(checkdir.join("beta.ini"), "[check]\nok = exit_status=0\n").unwrap();
    wrap_job(&dir, &checkdir, "alpha", &["/bin/true"]);
    wrap_job(&dir, &checkdir, "beta", &["/bin/sh", "-c", "exit 1"]);

    // Without exclude: beta is failing → CRITICAL.
    let c = bin()
        .arg("-d")
        .arg(&dir)
        .arg("--checkdir")
        .arg(&checkdir)
        .arg("check")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&c.stdout);
    assert!(stdout.starts_with("CRITICAL:"), "got: {stdout}");
    assert_eq!(c.status.code(), Some(2));

    // Excluding beta leaves only alpha (OK).
    let c = bin()
        .arg("-d")
        .arg(&dir)
        .arg("--checkdir")
        .arg(&checkdir)
        .arg("check")
        .arg("--exclude")
        .arg("beta")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&c.stdout);
    assert!(stdout.starts_with("OK:"), "got: {stdout}");
    assert_eq!(c.status.code(), Some(0));
    std::fs::remove_dir_all(&dir).ok();
}

/// `check --exclude` also drops a not-running job synthesized from a check `.ini`.
#[test]
fn check_exclude_removes_not_running() {
    let dir = std::env::temp_dir().join(format!("sh_excl2_{}", std::process::id()));
    let checkdir = dir.join("check");
    std::fs::create_dir_all(&checkdir).unwrap();
    std::fs::write(checkdir.join("alpha.ini"), "[check]\nok = exit_status=0\n").unwrap();
    // stale.ini has no corresponding job → load_not_running synthesizes a CRITICAL job.
    std::fs::write(checkdir.join("stale.ini"), "[check]\nok = exit_status=0\n").unwrap();
    wrap_job(&dir, &checkdir, "alpha", &["/bin/true"]);

    // Without exclude: stale is not running → CRITICAL.
    let c = bin()
        .arg("-d")
        .arg(&dir)
        .arg("--checkdir")
        .arg(&checkdir)
        .arg("check")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&c.stdout);
    assert!(stdout.starts_with("CRITICAL:"), "got: {stdout}");
    assert_eq!(c.status.code(), Some(2));

    // Excluding stale leaves only alpha (OK).
    let c = bin()
        .arg("-d")
        .arg(&dir)
        .arg("--checkdir")
        .arg(&checkdir)
        .arg("check")
        .arg("--exclude")
        .arg("stale")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&c.stdout);
    assert!(stdout.starts_with("OK:"), "got: {stdout}");
    assert_eq!(c.status.code(), Some(0));
    std::fs::remove_dir_all(&dir).ok();
}
