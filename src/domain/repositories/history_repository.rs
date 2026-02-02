//! HistoryRepository - interface for launch history tracking
//!
//! This trait defines how to track and query application usage history.

use std::collections::HashMap;

use crate::domain::errors::DomainError;

/// A single launch history entry
#[derive(Clone, Debug)]
pub struct LaunchRecord {
    /// Application ID
    pub app_id: String,
    /// Launch timestamp (Unix epoch seconds)
    pub timestamp: u64,
}

/// Repository interface for launch history
pub trait HistoryRepository: Send + Sync {
    /// Record a launch event
    fn record_launch(&mut self, app_id: &str) -> Result<(), DomainError>;

    /// Get the launch count for an application
    fn get_launch_count(&self, app_id: &str) -> u32;

    /// Get frequency map for all applications (for boosting search results)
    fn get_frequency_map(&self) -> HashMap<String, f32>;

    /// Get recent launches (most recent first)
    fn get_recent_launches(&self, limit: usize) -> Vec<LaunchRecord>;

    /// Clear all history
    fn clear(&mut self) -> Result<(), DomainError>;

    /// Save history to persistent storage
    fn save(&self) -> Result<(), DomainError>;

    /// Load history from persistent storage
    fn load(&mut self) -> Result<(), DomainError>;
}

/// A null implementation for testing
pub struct NullHistoryRepository;

impl HistoryRepository for NullHistoryRepository {
    fn record_launch(&mut self, _app_id: &str) -> Result<(), DomainError> {
        Ok(())
    }

    fn get_launch_count(&self, _app_id: &str) -> u32 {
        0
    }

    fn get_frequency_map(&self) -> HashMap<String, f32> {
        HashMap::new()
    }

    fn get_recent_launches(&self, _limit: usize) -> Vec<LaunchRecord> {
        Vec::new()
    }

    fn clear(&mut self) -> Result<(), DomainError> {
        Ok(())
    }

    fn save(&self) -> Result<(), DomainError> {
        Ok(())
    }

    fn load(&mut self) -> Result<(), DomainError> {
        Ok(())
    }
}
