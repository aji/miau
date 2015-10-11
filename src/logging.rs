//! Basic logger implementation that prints to standard output.

extern crate log;

use environment::Env;

static EXTERNAL_LOG_LEVEL: log::LogLevel = log::LogLevel::Debug;
static DEFAULT_LOG_LEVEL: log::LogLevelFilter = log::LogLevelFilter::Debug;

struct DumbLogger;

impl log::Log for DumbLogger {
    fn enabled(&self, _: &log::LogMetadata) -> bool { true }

    fn log(&self, record: &log::LogRecord) {
        if !record.location().module_path().starts_with("miau::") {
            if record.level() > EXTERNAL_LOG_LEVEL {
                return;
            }
        }

        println!(
            "{:5} {}@{}: {}",
            record.level(),
            record.location().module_path(),
            record.location().line(),
            record.args()
        );
    }
}

/// Configures the logging system with the configuration for the current
/// environment.
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
