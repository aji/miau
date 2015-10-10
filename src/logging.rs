//! Logger implementation

extern crate log;

use environment::Env;

static DEFAULT_LOG_LEVEL: log::LogLevelFilter = log::LogLevelFilter::Debug;

struct DumbLogger;

impl log::Log for DumbLogger {
    fn enabled(&self, _: &log::LogMetadata) -> bool { true }

    fn log(&self, record: &log::LogRecord) {
        println!(
            "{:5} {}@{}: {}",
            record.level(),
            record.location().file(),
            record.location().line(),
            record.args()
        );
    }
}

pub fn init(env: &Env) -> Result<(), log::SetLoggerError> {
    use log::LogLevelFilter::*;

    let log_level = match env.conf_str("log.level") {
        Some(value) => match value {
            "trace" => Trace,
            "debug" => Debug,
            "info"  => Info,
            "warn"  => Warn,
            "error" => Error,
            "off"   => Off,
            _ => {
                println!("warning: {} is not a valid log level name", value);
                println!("warning: log level defaults to {}", DEFAULT_LOG_LEVEL);
                DEFAULT_LOG_LEVEL
            },
        },
        None => {
            println!("warning: log level defaults to {}", DEFAULT_LOG_LEVEL);
            DEFAULT_LOG_LEVEL
        },
    };

    log::set_logger(|max_log_level| {
        max_log_level.set(log_level);
        Box::new(DumbLogger)
    })
}
