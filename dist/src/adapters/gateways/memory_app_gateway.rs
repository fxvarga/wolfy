//! MemoryAppGateway - In-memory application repository

use crate::domain::entities::AppItem;
use crate::domain::errors::DomainError;
use crate::domain::repositories::app_repository::{AppRepository, ScoredAppItem};
use crate::domain::services::FuzzyMatcher;
use crate::domain::value_objects::SearchQuery;

/// In-memory application repository for testing
pub struct MemoryAppGateway {
    apps: Vec<AppItem>,
    matcher: FuzzyMatcher,
}

impl MemoryAppGateway {
    /// Create a new empty gateway
    pub fn new() -> Self {
        Self {
            apps: Vec::new(),
            matcher: FuzzyMatcher::new(),
        }
    }

    /// Create with initial apps
    pub fn with_apps(apps: Vec<AppItem>) -> Self {
        Self {
            apps,
            matcher: FuzzyMatcher::new(),
        }
    }

    /// Add an application
    pub fn add_app(&mut self, app: AppItem) {
        self.apps.push(app);
    }

    /// Remove an application by ID
    pub fn remove_app(&mut self, id: &str) {
        self.apps.retain(|app| app.id != id);
    }

    /// Clear all applications
    pub fn clear(&mut self) {
        self.apps.clear();
    }
}

impl Default for MemoryAppGateway {
    fn default() -> Self {
        Self::new()
    }
}

impl AppRepository for MemoryAppGateway {
    fn discover_all(&self) -> Result<Vec<AppItem>, DomainError> {
        Ok(self.apps.clone())
    }

    fn find_by_id(&self, id: &str) -> Result<Option<AppItem>, DomainError> {
        Ok(self.apps.iter().find(|app| app.id == id).cloned())
    }

    fn search(&self, query: &SearchQuery) -> Result<Vec<ScoredAppItem>, DomainError> {
        if query.is_empty() {
            return Ok(self
                .apps
                .iter()
                .map(|app| ScoredAppItem {
                    item: app.clone(),
                    score: 0,
                })
                .collect());
        }

        let mut results: Vec<ScoredAppItem> = self
            .apps
            .iter()
            .filter_map(|app| {
                let result = self.matcher.fuzzy_match(query.normalized(), &app.name);
                if result.matched {
                    Some(ScoredAppItem {
                        item: app.clone(),
                        score: result.score,
                    })
                } else {
                    None
                }
            })
            .collect();

        results.sort_by(|a, b| b.score.cmp(&a.score));
        results.truncate(query.max_results);

        Ok(results)
    }

    fn refresh(&mut self) -> Result<(), DomainError> {
        // No-op for in-memory repository
        Ok(())
    }

    fn count(&self) -> usize {
        self.apps.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_add_and_find() {
        let mut gateway = MemoryAppGateway::new();

        let app = AppItem::new("Chrome", PathBuf::from("/chrome"));
        let id = app.id.clone();

        gateway.add_app(app);

        let found = gateway.find_by_id(&id).unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "Chrome");
    }

    #[test]
    fn test_search() {
        let mut gateway = MemoryAppGateway::new();

        gateway.add_app(AppItem::new("Google Chrome", PathBuf::from("/chrome")));
        gateway.add_app(AppItem::new("Firefox", PathBuf::from("/firefox")));
        gateway.add_app(AppItem::new("Chrome Canary", PathBuf::from("/chrome-canary")));

        let query = SearchQuery::new("chrome");
        let results = gateway.search(&query).unwrap();

        assert_eq!(results.len(), 2); // Google Chrome and Chrome Canary
        assert!(results[0].item.name.to_lowercase().contains("chrome"));
    }
}
