//! SearchAppsUseCase - Search for applications
//!
//! Handles the business logic for searching applications.

use std::collections::HashMap;
use std::sync::Arc;

use crate::domain::entities::AppItem;
use crate::domain::errors::DomainError;
use crate::domain::repositories::{AppRepository, HistoryRepository};
use crate::domain::services::search_service::{SearchResult, SearchService};
use crate::domain::value_objects::SearchQuery;

/// Use case for searching applications
pub struct SearchAppsUseCase<R, H>
where
    R: AppRepository,
    H: HistoryRepository,
{
    app_repository: Arc<R>,
    history_repository: Arc<std::sync::Mutex<H>>,
    search_service: SearchService,
}

impl<R, H> SearchAppsUseCase<R, H>
where
    R: AppRepository,
    H: HistoryRepository,
{
    /// Create a new search apps use case
    pub fn new(
        app_repository: Arc<R>,
        history_repository: Arc<std::sync::Mutex<H>>,
    ) -> Self {
        Self {
            app_repository,
            history_repository,
            search_service: SearchService::new(),
        }
    }

    /// Create with custom search service
    pub fn with_search_service(
        app_repository: Arc<R>,
        history_repository: Arc<std::sync::Mutex<H>>,
        search_service: SearchService,
    ) -> Self {
        Self {
            app_repository,
            history_repository,
            search_service,
        }
    }

    /// Execute search with a query string
    pub fn execute(&self, query: &str) -> Result<Vec<SearchResult>, DomainError> {
        let search_query = SearchQuery::new(query);
        self.execute_query(&search_query)
    }

    /// Execute search with a SearchQuery
    pub fn execute_query(&self, query: &SearchQuery) -> Result<Vec<SearchResult>, DomainError> {
        // 1. Get all applications
        let apps = self.app_repository.discover_all()?;

        // 2. Get history frequency for boosting
        let history_frequency = self
            .history_repository
            .lock()
            .map(|h| h.get_frequency_map())
            .unwrap_or_default();

        // 3. Perform search with history boosting
        let results = self
            .search_service
            .search_with_history(&apps, query, &history_frequency);

        Ok(results)
    }

    /// Get top N results
    pub fn top_results(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>, DomainError> {
        let mut results = self.execute(query)?;
        results.truncate(limit);
        Ok(results)
    }

    /// Get all apps (empty query)
    pub fn all_apps(&self) -> Result<Vec<AppItem>, DomainError> {
        self.app_repository.discover_all()
    }

    /// Refresh the application cache
    pub fn refresh(&self) -> Result<(), DomainError> {
        // Note: This would need mutable access to the repository
        // For now, this is a placeholder
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::repositories::app_repository::NullAppRepository;
    use crate::domain::repositories::history_repository::NullHistoryRepository;
    use std::sync::Mutex;

    #[test]
    fn test_search_empty() {
        let app_repo = Arc::new(NullAppRepository);
        let history_repo = Arc::new(Mutex::new(NullHistoryRepository));

        let use_case = SearchAppsUseCase::new(app_repo, history_repo);

        let results = use_case.execute("test").unwrap();
        assert!(results.is_empty()); // NullAppRepository returns empty
    }
}
