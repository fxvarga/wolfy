//! Working directory history for interactive tasks
//!
//! Stores most-recently-used directories per task in a simple JSON file.
//! Location: %APPDATA%\wolfy\cwd_history.json

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter};
use std::path::PathBuf;

/// Maximum number of directories to remember per task
const MAX_DIRS_PER_TASK: usize = 10;

/// Working directory history storage
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CwdHistory {
    /// Map from task key (group:name) to list of directories (most recent first)
    #[serde(default)]
    tasks: HashMap<String, Vec<String>>,

    /// File path for persistence (not serialized)
    #[serde(skip)]
    path: PathBuf,
}

impl CwdHistory {
    /// Create a new empty history
    pub fn new() -> Self {
        Self {
            tasks: HashMap::new(),
            path: PathBuf::new(),
        }
    }

    /// Load history from the default location (%APPDATA%\wolfy\cwd_history.json)
    pub fn load_default() -> Self {
        if let Some(app_data) = dirs::data_dir() {
            let history_path = app_data.join("wolfy").join("cwd_history.json");
            Self::load(&history_path)
        } else {
            crate::log!("Could not determine app data directory for CWD history");
            Self::new()
        }
    }

    /// Load history from a specific file
    pub fn load(path: &PathBuf) -> Self {
        let mut history = Self {
            tasks: HashMap::new(),
            path: path.clone(),
        };

        if path.exists() {
            if let Ok(file) = File::open(path) {
                let reader = BufReader::new(file);
                if let Ok(data) = serde_json::from_reader::<_, CwdHistory>(reader) {
                    history.tasks = data.tasks;
                    crate::log!("Loaded CWD history with {} tasks from {:?}", history.tasks.len(), path);
                }
            }
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
                crate::log!("Failed to create CWD history directory: {:?}", e);
                return;
            }
        }

        match File::create(&self.path) {
            Ok(file) => {
                let writer = BufWriter::new(file);
                if let Err(e) = serde_json::to_writer_pretty(writer, self) {
                    crate::log!("Failed to write CWD history: {:?}", e);
                }
            }
            Err(e) => {
                crate::log!("Failed to create CWD history file: {:?}", e);
            }
        }
    }

    /// Get directories for a task (most recent first)
    pub fn get_dirs(&self, group: &str, name: &str) -> Vec<String> {
        let key = format!("{}:{}", group, name);
        self.tasks.get(&key).cloned().unwrap_or_default()
    }

    /// Add a directory to a task's history (moves to front if already exists)
    pub fn add_dir(&mut self, group: &str, name: &str, dir: &str) {
        let key = format!("{}:{}", group, name);
        let dirs = self.tasks.entry(key).or_insert_with(Vec::new);

        // Remove if already exists (we'll add to front)
        dirs.retain(|d| d != dir);

        // Add to front
        dirs.insert(0, dir.to_string());

        // Trim to max size
        dirs.truncate(MAX_DIRS_PER_TASK);

        // Save immediately
        self.save();
    }

    /// Get the most recent directory for a task, or home directory if none
    pub fn get_last_dir(&self, group: &str, name: &str) -> String {
        let dirs = self.get_dirs(group, name);
        if let Some(first) = dirs.first() {
            first.clone()
        } else {
            // Default to user's home directory
            dirs::home_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| "C:\\".to_string())
        }
    }

    /// Get all directories for a task, with home directory as fallback
    pub fn get_dirs_with_default(&self, group: &str, name: &str) -> Vec<String> {
        let mut dirs = self.get_dirs(group, name);

        // Add home directory if not in list
        let home = dirs::home_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "C:\\".to_string());

        if !dirs.iter().any(|d| d == &home) {
            dirs.push(home);
        }

        dirs
    }
}
