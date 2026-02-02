//! AppRepository - interface for application discovery and access
//!
//! This trait defines how to discover, search, and access applications.
//! Implementations may use Start Menu scanning, database, or other sources.

use crate::domain::entities::AppItem;
use crate::domain::errors::DomainError;
use crate::domain::value_objects::SearchQuery;

/// Result of a scored search
#[derive(Clone, Debug)]
pub struct ScoredAppItem {
    /// The matched application
    pub item: AppItem,
    /// Match score (higher is better)
    pub score: i32,
}

/// Repository interface for application data
pub trait AppRepository: Send + Sync {
    /// Discover all available applications
    fn discover_all(&self) -> Result<Vec<AppItem>, DomainError>;

    /// Find an application by its ID
    fn find_by_id(&self, id: &str) -> Result<Option<AppItem>, DomainError>;

    /// Search applications using a query
    fn search(&self, query: &SearchQuery) -> Result<Vec<ScoredAppItem>, DomainError>;

    /// Refresh the application cache
    fn refresh(&mut self) -> Result<(), DomainError>;

    /// Get the number of discovered applications
    fn count(&self) -> usize;
}

/// A null implementation for testing
pub struct NullAppRepository;

impl AppRepository for NullAppRepository {
    fn discover_all(&self) -> Result<Vec<AppItem>, DomainError> {
        Ok(Vec::new())
    }

    fn find_by_id(&self, _id: &str) -> Result<Option<AppItem>, DomainError> {
        Ok(None)
    }

    fn search(&self, _query: &SearchQuery) -> Result<Vec<ScoredAppItem>, DomainError> {
        Ok(Vec::new())
    }

    fn refresh(&mut self) -> Result<(), DomainError> {
        Ok(())
    }

    fn count(&self) -> usize {
        0
    }
}
