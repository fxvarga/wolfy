//! Frequency-based usage history tracking (rofi-inspired)
//!
//! Stores app launch counts in a simple text file format:
//! ```text
//! 15 firefox.exe
//! 8 notepad.exe
//! 3 calculator
//! ```
//!
//! Higher counts mean more frequently used apps, which appear first in the list.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

/// Maximum number of entries to keep in history
const MAX_HISTORY_SIZE: usize = 100;

/// Frequency-based usage history
#[derive(Debug, Default)]
pub struct History {
    /// Map from app ID to launch count
    entries: HashMap<String, u32>,
    /// Path to the history file
    path: PathBuf,
}

impl History {
    /// Create a new empty history
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            path: PathBuf::new(),
        }
    }

    /// Load history from the default location (%APPDATA%\wolfy\history.txt)
    pub fn load_default() -> Self {
        if let Some(app_data) = dirs::data_dir() {
            let history_path = app_data.join("wolfy").join("history.txt");
            Self::load(&history_path)
        } else {
            crate::log!("Could not determine app data directory for history");
            Self::new()
        }
    }

    /// Load history from a specific file
    pub fn load(path: &Path) -> Self {
        let mut history = Self {
            entries: HashMap::new(),
            path: path.to_path_buf(),
        };

        // Try to read the history file
        if let Ok(file) = File::open(path) {
            let reader = BufReader::new(file);
            for line in reader.lines().map_while(Result::ok) {
                // Parse "count app_id" format
                if let Some((count_str, app_id)) = line.split_once(' ') {
                    if let Ok(count) = count_str.parse::<u32>() {
                        history.entries.insert(app_id.to_string(), count);
                    }
                }
            }
            crate::log!(
                "Loaded {} history entries from {:?}",
                history.entries.len(),
                path
            );
        }

        history
    }

    /// Save history to file
    pub fn save(&self) {
        if self.path.as_os_str().is_empty() {
            return;
        }

        // Ensure directory exists
        if let Some(parent) = self.path.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                crate::log!("Failed to create history directory: {:?}", e);
                return;
            }
        }

        // Sort entries by count (descending) and take top MAX_HISTORY_SIZE
        let mut entries: Vec<_> = self.entries.iter().collect();
        entries.sort_by(|a, b| b.1.cmp(a.1));
        entries.truncate(MAX_HISTORY_SIZE);

        // Write to file
        match File::create(&self.path) {
            Ok(mut file) => {
                for (app_id, count) in entries {
                    if let Err(e) = writeln!(file, "{} {}", count, app_id) {
                        crate::log!("Failed to write history entry: {:?}", e);
                        break;
                    }
                }
                crate::log!(
                    "Saved {} history entries to {:?}",
                    self.entries.len().min(MAX_HISTORY_SIZE),
                    self.path
                );
            }
            Err(e) => {
                crate::log!("Failed to create history file: {:?}", e);
            }
        }
    }

    /// Record a launch (increment count for app_id)
    pub fn record_launch(&mut self, app_id: &str) {
        let count = self.entries.entry(app_id.to_string()).or_insert(0);
        *count += 1;
        crate::log!("Recorded launch for '{}', count now: {}", app_id, *count);

        // Save immediately
        self.save();
    }

    /// Get the launch count for an app (None if never launched)
    pub fn get_count(&self, app_id: &str) -> Option<u32> {
        self.entries.get(app_id).copied()
    }

    /// Get the sort index for an app (higher = more frequent, None = never used)
    /// This is used for sorting: apps with higher sort index appear first
    pub fn sort_index(&self, app_id: &str) -> i32 {
        match self.entries.get(app_id) {
            Some(count) => *count as i32,
            None => -1, // Not in history
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_history_new() {
        let history = History::new();
        assert!(history.entries.is_empty());
        assert_eq!(history.get_count("test"), None);
    }

    #[test]
    fn test_history_record_launch() {
        let mut history = History::new();
        assert_eq!(history.get_count("notepad.exe"), None);

        history.record_launch("notepad.exe");
        assert_eq!(history.get_count("notepad.exe"), Some(1));

        history.record_launch("notepad.exe");
        assert_eq!(history.get_count("notepad.exe"), Some(2));

        history.record_launch("calc.exe");
        assert_eq!(history.get_count("calc.exe"), Some(1));
    }

    #[test]
    fn test_history_sort_index() {
        let mut history = History::new();

        // Not in history = -1
        assert_eq!(history.sort_index("unknown"), -1);

        // In history = count
        history.record_launch("app1");
        history.record_launch("app1");
        history.record_launch("app1");
        assert_eq!(history.sort_index("app1"), 3);

        history.record_launch("app2");
        assert_eq!(history.sort_index("app2"), 1);
    }

    #[test]
    fn test_history_load_save() {
        // Create a temp file with history data
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "5 firefox.exe").unwrap();
        writeln!(temp_file, "3 notepad.exe").unwrap();
        writeln!(temp_file, "1 calc.exe").unwrap();
        temp_file.flush().unwrap();

        // Load history
        let history = History::load(temp_file.path());
        assert_eq!(history.get_count("firefox.exe"), Some(5));
        assert_eq!(history.get_count("notepad.exe"), Some(3));
        assert_eq!(history.get_count("calc.exe"), Some(1));
        assert_eq!(history.get_count("unknown"), None);
    }
}
