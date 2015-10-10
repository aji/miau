//! Logger implementation

extern crate log;

struct DumbLogger;

impl log::Log for DumbLogger {
    fn enabled(&self, metadata: &log::LogMetadata) -> bool {
        metadata.level() <= log::LogLevel::Debug
    }

    fn log(&self, record: &log::LogRecord) {
        if self.enabled(record.metadata()) {
            println!(
                "{} {}@{}: {}",
                record.level(),
                record.location().file(),
                record.location().line(),
                record.args()
            )
        }
    }
}

pub fn init() -> Result<(), log::SetLoggerError> {
    log::set_logger(|max_log_level| {
        max_log_level.set(log::LogLevelFilter::Debug);
        Box::new(DumbLogger)
    })
}
