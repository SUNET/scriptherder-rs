use scriptherder::cli::{self, RawArgs, RawMode};
use std::io::{IsTerminal, Write};

mod modes;

/// Logger that writes to stderr and (for `wrap --syslog`) also to syslog.
/// Mirrors ../src/scriptherder.py:1361-1380.
struct ShLogger {
    stderr_level: log::LevelFilter,
    syslog: Option<std::sync::Mutex<syslog::Logger<syslog::LoggerBackend, syslog::Formatter3164>>>,
}

impl log::Log for ShLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= self.stderr_level || self.syslog.is_some()
    }

    fn log(&self, record: &log::Record) {
        // stderr handler honours its own level.
        if record.level() <= self.stderr_level {
            let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S,%3f");
            let _ = writeln!(
                std::io::stderr(),
                "{}: MainThread {} {}",
                now,
                record.level(),
                record.args()
            );
        }
        // syslog handler logs at INFO and above.
        if let Some(sl) = &self.syslog {
            if record.level() <= log::Level::Info {
                if let Ok(mut logger) = sl.lock() {
                    let msg = format!("{} {}", record.level(), record.args());
                    let _ = match record.level() {
                        log::Level::Error => logger.err(msg),
                        log::Level::Warn => logger.warning(msg),
                        _ => logger.info(msg),
                    };
                }
            }
        }
    }

    fn flush(&self) {
        let _ = std::io::stderr().flush();
    }
}

/// Build and install the global logger honouring `--debug`, TTY state and
/// `wrap --syslog` (../src/scriptherder.py:1361-1380).
fn init_logging(args: &RawArgs) {
    // --debug → DEBUG, else INFO; drop to ERROR when stderr is not a TTY and not debug.
    let stderr_level = if args.debug {
        log::LevelFilter::Debug
    } else if !std::io::stderr().is_terminal() {
        log::LevelFilter::Error
    } else {
        log::LevelFilter::Info
    };

    let want_syslog = matches!(&args.mode, RawMode::Wrap { syslog: true, .. });
    let syslog = if want_syslog {
        let name = match &args.mode {
            RawMode::Wrap { name, .. } => name.clone(),
            _ => "scriptherder".to_string(),
        };
        // SysLogHandler("/dev/log") with format "{name}: {LEVEL} {msg}".
        let formatter = syslog::Formatter3164 {
            facility: syslog::Facility::LOG_USER,
            hostname: None,
            process: name,
            pid: std::process::id(),
        };
        syslog::unix(formatter).ok().map(std::sync::Mutex::new)
    } else {
        None
    };

    // Global max level must allow whatever any handler wants.
    let max = if syslog.is_some() {
        std::cmp::max(stderr_level, log::LevelFilter::Info)
    } else {
        stderr_level
    };

    let logger = ShLogger {
        stderr_level,
        syslog,
    };
    if log::set_boxed_logger(Box::new(logger)).is_ok() {
        log::set_max_level(max);
    }
}

fn main() {
    let args = cli::parse();
    init_logging(&args);
    let code = match &args.mode {
        RawMode::Wrap { .. } => modes::wrap(&args),
        RawMode::Ls { names } => modes::ls(&args, names),
        RawMode::Check { names, exclude } => modes::run_check(&args, names, exclude),
        RawMode::Lastlog { names } => modes::lastlog(&args, names, false),
        RawMode::Lastfaillog { names } => modes::lastlog(&args, names, true),
    };
    std::process::exit(code);
}
