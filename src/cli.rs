use clap::{Parser, Subcommand};

pub const DEFAULT_DATADIR: &str = "/var/cache/scriptherder";
pub const DEFAULT_CHECKDIR: &str = "/etc/scriptherder/check";
pub const DEFAULT_UMASK: &str = "077";

#[derive(Parser)]
#[command(name = "scriptherder", about = "Script herder script")]
pub struct RawArgs {
    #[arg(long, default_value_t = false)]
    pub debug: bool,
    #[arg(short = 'd', long, default_value = DEFAULT_DATADIR)]
    pub datadir: String,
    #[arg(long, default_value = DEFAULT_CHECKDIR)]
    pub checkdir: String,
    #[command(subcommand)]
    pub mode: RawMode,
}

#[derive(Subcommand)]
pub enum RawMode {
    Wrap {
        #[arg(short = 'N', long, required = true)]
        name: String,
        #[arg(long, default_value = DEFAULT_UMASK)]
        umask: String,
        #[arg(long, default_value_t = false)]
        syslog: bool,
        #[arg(long = "random-sleep", default_value_t = 0)]
        random_sleep: u64,
        #[arg(required = true, num_args = 1.., trailing_var_arg = true)]
        cmd: Vec<String>,
    },
    Ls {
        names: Vec<String>,
    },
    Check {
        names: Vec<String>,
    },
    Lastlog {
        names: Vec<String>,
    },
    Lastfaillog {
        names: Vec<String>,
    },
}

/// Replicate Python argv preprocessing: drop a leading `--mode`, default empty to `ls`.
pub fn preprocess(mut args: Vec<String>) -> Vec<String> {
    if args.first().map(|s| s == "--mode").unwrap_or(false) {
        args.remove(0);
    }
    if args.is_empty() {
        args.push("ls".to_string());
    }
    args
}

pub fn parse() -> RawArgs {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let processed = preprocess(argv);
    let mut full = vec!["scriptherder".to_string()];
    full.extend(processed);
    let args = RawArgs::parse_from(full);
    if let RawMode::Wrap { umask, .. } = &args.mode {
        if umask.len() != 3 {
            eprintln!("error: Umask must be 3 digits (e.g. the default '{DEFAULT_UMASK}')");
            std::process::exit(2);
        }
    }
    args
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preprocess_strips_mode_and_defaults_ls() {
        assert_eq!(
            preprocess(vec!["--mode".into(), "ls".into()]),
            vec!["ls".to_string()]
        );
        assert_eq!(preprocess(vec![]), vec!["ls".to_string()]);
        assert_eq!(
            preprocess(vec!["check".into(), "foo".into()]),
            vec!["check".to_string(), "foo".to_string()]
        );
    }

    #[test]
    fn wrap_captures_trailing_hyphen_args() {
        // Verify wrap with `--` separator captures cmd including -v style flags
        let args = RawArgs::parse_from([
            "scriptherder",
            "wrap",
            "-N",
            "myscript",
            "--",
            "/bin/foo",
            "-v",
        ]);
        match args.mode {
            RawMode::Wrap { name, cmd, .. } => {
                assert_eq!(name, "myscript");
                assert_eq!(cmd, vec!["/bin/foo", "-v"]);
            }
            _ => panic!("expected Wrap"),
        }
    }

    #[test]
    fn global_flags_before_subcommand() {
        let args =
            RawArgs::parse_from(["scriptherder", "--debug", "-d", "/tmp/data", "ls", "job1"]);
        assert!(args.debug);
        assert_eq!(args.datadir, "/tmp/data");
        match args.mode {
            RawMode::Ls { names } => assert_eq!(names, vec!["job1"]),
            _ => panic!("expected Ls"),
        }
    }

    #[test]
    fn wrap_defaults() {
        let args = RawArgs::parse_from(["scriptherder", "wrap", "-N", "test", "--", "/bin/true"]);
        match args.mode {
            RawMode::Wrap {
                umask,
                syslog,
                random_sleep,
                cmd,
                ..
            } => {
                assert_eq!(umask, "077");
                assert!(!syslog);
                assert_eq!(random_sleep, 0);
                assert_eq!(cmd, vec!["/bin/true"]);
            }
            _ => panic!("expected Wrap"),
        }
    }

    #[test]
    fn preprocess_strips_mode_keeps_value_and_rest() {
        // --mode wrap foo → [wrap, foo]
        assert_eq!(
            preprocess(vec!["--mode".into(), "wrap".into(), "foo".into()]),
            vec!["wrap".to_string(), "foo".to_string()]
        );
    }

    #[test]
    fn check_with_names() {
        let args = RawArgs::parse_from(["scriptherder", "check", "jobA", "jobB"]);
        match args.mode {
            RawMode::Check { names } => assert_eq!(names, vec!["jobA", "jobB"]),
            _ => panic!("expected Check"),
        }
    }
}
