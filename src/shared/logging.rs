//! Logging utilities

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

/// Simple file-based logger
pub struct Logger {
    file: Mutex<Option<File>>,
    path: PathBuf,
    enabled: bool,
}

impl Logger {
    /// Create a new logger
    pub fn new(path: PathBuf) -> Self {
        Self {
            file: Mutex::new(None),
            path,
            enabled: true,
        }
    }

    /// Enable or disable logging
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Log a message
    pub fn log(&self, message: &str) {
        if !self.enabled {
            return;
        }

        let mut file_guard = self.file.lock().unwrap();

        // Lazy open file
        if file_guard.is_none() {
            if let Ok(f) = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.path)
            {
                *file_guard = Some(f);
            }
        }

        if let Some(ref mut f) = *file_guard {
            let timestamp = chrono_timestamp();
            let _ = writeln!(f, "[{}] {}", timestamp, message);
        }
    }

    /// Log with format
    pub fn logf(&self, args: std::fmt::Arguments) {
        self.log(&args.to_string());
    }
}

impl Default for Logger {
    fn default() -> Self {
        Self::new(PathBuf::from("wolfy.log"))
    }
}

/// Get current timestamp as string
fn chrono_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Simple timestamp format
    format!("{}", now)
}

/// Global logger instance (thread-safe)
static LOGGER: std::sync::OnceLock<Logger> = std::sync::OnceLock::new();

/// Initialize the global logger
pub fn init_logger(path: PathBuf) {
    let _ = LOGGER.set(Logger::new(path));
}

/// Get the global logger
pub fn logger() -> &'static Logger {
    LOGGER.get_or_init(Logger::default)
}

/// Log a message to the global logger
pub fn log(message: &str) {
    logger().log(message);
}

/// Log macro for convenient logging
#[macro_export]
macro_rules! log_msg {
    ($($arg:tt)*) => {
        $crate::shared::logging::log(&format!($($arg)*))
    };
}
