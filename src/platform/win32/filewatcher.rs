//! File watcher using polling (checks file modification time)

use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Simple polling-based file watcher
/// Checks file modification time to detect changes
pub struct PollingFileWatcher {
    path: PathBuf,
    last_modified: Option<SystemTime>,
}

impl PollingFileWatcher {
    /// Create a new file watcher for the given path
    pub fn new(path: &Path) -> Self {
        let last_modified = std::fs::metadata(path).and_then(|m| m.modified()).ok();

        Self {
            path: path.to_path_buf(),
            last_modified,
        }
    }

    /// Check if the file has been modified since last check
    /// Returns true if the file was modified (and updates internal state)
    pub fn check_modified(&mut self) -> bool {
        let current = std::fs::metadata(&self.path)
            .and_then(|m| m.modified())
            .ok();

        match (&self.last_modified, &current) {
            (Some(last), Some(curr)) if curr > last => {
                self.last_modified = current;
                true
            }
            (None, Some(_)) => {
                // File appeared or became readable
                self.last_modified = current;
                true
            }
            _ => false,
        }
    }

    /// Get the watched file path
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Reset the last modified time to current
    /// Useful after a reload to prevent immediate re-trigger
    pub fn reset(&mut self) {
        self.last_modified = std::fs::metadata(&self.path)
            .and_then(|m| m.modified())
            .ok();
    }
}
