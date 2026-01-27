//! Simple file-based logging for debugging

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

static LOG_FILE: Mutex<Option<File>> = Mutex::new(None);

/// Get the directory where the executable is located
pub fn exe_dir() -> PathBuf {
    std::env::current_exe()
        .unwrap_or_else(|_| PathBuf::from("wolfy.exe"))
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Initialize logging to a file next to the executable
pub fn init() {
    let log_path = exe_dir().join("wolfy.log");

    if let Ok(file) = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&log_path)
    {
        if let Ok(mut guard) = LOG_FILE.lock() {
            *guard = Some(file);
        }
    }

    log("=== Wolfy Log Started ===");
}

/// Get current timestamp as milliseconds
fn timestamp() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

/// Log a message to the file
pub fn log(msg: &str) {
    if let Ok(mut guard) = LOG_FILE.lock() {
        if let Some(ref mut file) = *guard {
            let ts = timestamp();
            let _ = writeln!(file, "[{}] {}", ts, msg);
            let _ = file.flush();
        }
    }
}

/// Log a formatted message
#[macro_export]
macro_rules! log {
    ($($arg:tt)*) => {
        $crate::log::log(&format!($($arg)*))
    };
}

/// Log with function context
#[macro_export]
macro_rules! log_fn {
    ($fn_name:expr) => {
        $crate::log::log(&format!("-> {}", $fn_name))
    };
    ($fn_name:expr, $($arg:tt)*) => {
        $crate::log::log(&format!("-> {}: {}", $fn_name, format!($($arg)*)))
    };
}
