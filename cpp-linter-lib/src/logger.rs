//! A module to initialize and customize the logger object used in (most) stdout.

// non-std crates
use log::{Level, LevelFilter, Metadata, Record, SetLoggerError};

struct SimpleLogger;

impl log::Log for SimpleLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Debug
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            println!("{}: {}", record.level(), record.args());
        }
    }

    fn flush(&self) {}
}

/// A private constant to manage the application's logger object.
static LOGGER: SimpleLogger = SimpleLogger;

/// A function to initialize the private `LOGGER`.
///
/// The logging level defaults to [`LevelFilter::Info`].
/// Returns a [`SetLoggerError`] if the `LOGGER` is already initialized.
pub fn init() -> Result<(), SetLoggerError> {
    log::set_logger(&LOGGER).map(|()| log::set_max_level(LevelFilter::Info))
}

/// This prints a line to indicate the beginning of a related group of log statements.
///
/// This function may or may not get moved to [crate::rest_api::RestApiClient] trait
/// if/when platforms other than GitHub are supported.
pub fn start_log_group(name: String) {
    println!("::group::{}", name);
}

/// This prints a line to indicate the ending of a related group of log statements.
///
/// This function may or may not get moved to [crate::rest_api::RestApiClient] trait
/// if/when platforms other than GitHub are supported.
pub fn end_log_group() {
    println!("::endgroup::");
}

#[cfg(test)]
mod tests {
    use super::{end_log_group, start_log_group};

    #[test]
    fn issue_log_grouping_stdout() {
        start_log_group(String::from("a dumb test"));
        end_log_group();
    }
}
