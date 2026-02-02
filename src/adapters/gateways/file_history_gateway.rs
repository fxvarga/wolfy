//! FileHistoryGateway - File-based history repository

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use crate::domain::errors::DomainError;
use crate::domain::repositories::history_repository::{HistoryRepository, LaunchRecord};

/// Entry in the history file
#[derive(Clone, Debug)]
struct HistoryEntry {
    app_id: String,
    count: u32,
    last_launch: u64,
}

/// File-based history repository
pub struct FileHistoryGateway {
    path: PathBuf,
    entries: HashMap<String, HistoryEntry>,
    dirty: bool,
}

impl FileHistoryGateway {
    /// Create a new file history gateway
    pub fn new(path: PathBuf) -> Self {
        let mut gateway = Self {
            path,
            entries: HashMap::new(),
            dirty: false,
        };
        let _ = gateway.load();
        gateway
    }

    /// Get the history file path
    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    /// Get current timestamp
    fn now() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    /// Parse history from file content
    fn parse_content(content: &str) -> HashMap<String, HistoryEntry> {
        let mut entries = HashMap::new();

        for line in content.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 3 {
                if let (Ok(count), Ok(last_launch)) =
                    (parts[1].parse::<u32>(), parts[2].parse::<u64>())
                {
                    entries.insert(
                        parts[0].to_string(),
                        HistoryEntry {
                            app_id: parts[0].to_string(),
                            count,
                            last_launch,
                        },
                    );
                }
            }
        }

        entries
    }

    /// Serialize entries to file content
    fn serialize_entries(&self) -> String {
        self.entries
            .values()
            .map(|e| format!("{}\t{}\t{}", e.app_id, e.count, e.last_launch))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl HistoryRepository for FileHistoryGateway {
    fn record_launch(&mut self, app_id: &str) -> Result<(), DomainError> {
        let now = Self::now();

        let entry = self.entries.entry(app_id.to_string()).or_insert(HistoryEntry {
            app_id: app_id.to_string(),
            count: 0,
            last_launch: now,
        });

        entry.count += 1;
        entry.last_launch = now;
        self.dirty = true;

        // Auto-save
        self.save()?;

        Ok(())
    }

    fn get_launch_count(&self, app_id: &str) -> u32 {
        self.entries.get(app_id).map(|e| e.count).unwrap_or(0)
    }

    fn get_frequency_map(&self) -> HashMap<String, f32> {
        let max_count = self.entries.values().map(|e| e.count).max().unwrap_or(1) as f32;

        self.entries
            .iter()
            .map(|(id, entry)| (id.clone(), entry.count as f32 / max_count))
            .collect()
    }

    fn get_recent_launches(&self, limit: usize) -> Vec<LaunchRecord> {
        let mut entries: Vec<_> = self.entries.values().collect();
        entries.sort_by(|a, b| b.last_launch.cmp(&a.last_launch));

        entries
            .into_iter()
            .take(limit)
            .map(|e| LaunchRecord {
                app_id: e.app_id.clone(),
                timestamp: e.last_launch,
            })
            .collect()
    }

    fn clear(&mut self) -> Result<(), DomainError> {
        self.entries.clear();
        self.dirty = true;
        self.save()
    }

    fn save(&self) -> Result<(), DomainError> {
        if !self.dirty {
            return Ok(());
        }

        let content = self.serialize_entries();

        // Ensure parent directory exists
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(&self.path, content)?;

        Ok(())
    }

    fn load(&mut self) -> Result<(), DomainError> {
        if !self.path.exists() {
            return Ok(());
        }

        let content = fs::read_to_string(&self.path)?;
        self.entries = Self::parse_content(&content);
        self.dirty = false;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;
    use std::sync::atomic::{AtomicU32, Ordering};

    static TEST_COUNTER: AtomicU32 = AtomicU32::new(0);

    fn temp_history_path() -> PathBuf {
        let counter = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        temp_dir().join(format!("wolfy_test_history_{}_{}.txt", std::process::id(), counter))
    }

    #[test]
    fn test_record_launch() {
        let path = temp_history_path();
        let mut gateway = FileHistoryGateway::new(path.clone());

        gateway.record_launch("test_app").unwrap();
        assert_eq!(gateway.get_launch_count("test_app"), 1);

        gateway.record_launch("test_app").unwrap();
        assert_eq!(gateway.get_launch_count("test_app"), 2);

        // Cleanup
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_frequency_map() {
        let path = temp_history_path();
        let mut gateway = FileHistoryGateway::new(path.clone());

        gateway.record_launch("app1").unwrap();
        gateway.record_launch("app1").unwrap();
        gateway.record_launch("app2").unwrap();

        let freq = gateway.get_frequency_map();

        assert_eq!(freq.get("app1"), Some(&1.0)); // Most launched
        assert_eq!(freq.get("app2"), Some(&0.5)); // Half as much

        // Cleanup
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_save_load() {
        let path = temp_history_path();

        {
            let mut gateway = FileHistoryGateway::new(path.clone());
            gateway.record_launch("app1").unwrap();
            gateway.record_launch("app1").unwrap();
            gateway.record_launch("app2").unwrap();
        }

        {
            let gateway = FileHistoryGateway::new(path.clone());
            assert_eq!(gateway.get_launch_count("app1"), 2);
            assert_eq!(gateway.get_launch_count("app2"), 1);
        }

        // Cleanup
        let _ = fs::remove_file(&path);
    }
}
