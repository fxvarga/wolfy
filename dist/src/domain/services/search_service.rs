//! SearchService - application search logic
//!
//! Combines fuzzy matching with history boosting for search results.

use std::collections::HashMap;

use crate::domain::entities::AppItem;
use crate::domain::services::fuzzy_matcher::FuzzyMatcher;
use crate::domain::value_objects::SearchQuery;

/// A search result with score
#[derive(Clone, Debug)]
pub struct SearchResult {
    /// The matched application
    pub item: AppItem,
    /// Total score (match score + history boost)
    pub score: i32,
    /// Indices of matched characters (for highlighting)
    pub matched_indices: Vec<usize>,
}

/// Service for searching applications
#[derive(Clone, Debug)]
pub struct SearchService {
    /// Fuzzy matcher instance
    matcher: FuzzyMatcher,
    /// Boost multiplier for history frequency
    history_boost_multiplier: f32,
}

impl SearchService {
    /// Create a new search service
    pub fn new() -> Self {
        Self {
            matcher: FuzzyMatcher::new(),
            history_boost_multiplier: 50.0,
        }
    }

    /// Set the history boost multiplier
    pub fn with_history_boost(mut self, multiplier: f32) -> Self {
        self.history_boost_multiplier = multiplier;
        self
    }

    /// Search applications with a query
    pub fn search(&self, items: &[AppItem], query: &SearchQuery) -> Vec<SearchResult> {
        self.search_with_history(items, query, &HashMap::new())
    }

    /// Search applications with history boosting
    pub fn search_with_history(
        &self,
        items: &[AppItem],
        query: &SearchQuery,
        history_frequency: &HashMap<String, f32>,
    ) -> Vec<SearchResult> {
        if query.is_empty() {
            // Return all items sorted by history
            return self.sort_by_history(items, history_frequency);
        }

        let mut results: Vec<SearchResult> = items
            .iter()
            .filter_map(|item| {
                // Try matching against name first
                let name_result = self.matcher.fuzzy_match(query.normalized(), &item.name);

                if name_result.matched {
                    let history_boost = history_frequency
                        .get(&item.id)
                        .copied()
                        .unwrap_or(0.0)
                        * self.history_boost_multiplier;

                    Some(SearchResult {
                        item: item.clone(),
                        score: name_result.score + history_boost as i32,
                        matched_indices: name_result.matched_indices,
                    })
                } else {
                    // Try matching against searchable text
                    let text = item.searchable_text();
                    let text_result = self.matcher.fuzzy_match(query.normalized(), &text);

                    if text_result.matched {
                        let history_boost = history_frequency
                            .get(&item.id)
                            .copied()
                            .unwrap_or(0.0)
                            * self.history_boost_multiplier;

                        // Lower score for non-name matches
                        Some(SearchResult {
                            item: item.clone(),
                            score: text_result.score / 2 + history_boost as i32,
                            matched_indices: Vec::new(), // Don't highlight description matches
                        })
                    } else {
                        None
                    }
                }
            })
            .collect();

        // Sort by score (highest first)
        results.sort_by(|a, b| b.score.cmp(&a.score));

        // Limit results
        results.truncate(query.max_results);

        results
    }

    /// Sort items by history frequency (for empty query)
    fn sort_by_history(
        &self,
        items: &[AppItem],
        history_frequency: &HashMap<String, f32>,
    ) -> Vec<SearchResult> {
        let mut results: Vec<SearchResult> = items
            .iter()
            .map(|item| {
                let history_score = history_frequency
                    .get(&item.id)
                    .copied()
                    .unwrap_or(0.0)
                    * self.history_boost_multiplier;

                SearchResult {
                    item: item.clone(),
                    score: history_score as i32,
                    matched_indices: Vec::new(),
                }
            })
            .collect();

        // Sort by score (history), then by name
        results.sort_by(|a, b| {
            b.score
                .cmp(&a.score)
                .then_with(|| a.item.name.cmp(&b.item.name))
        });

        results
    }

    /// Get the top N results for a query
    pub fn top_results(
        &self,
        items: &[AppItem],
        query: &SearchQuery,
        limit: usize,
    ) -> Vec<SearchResult> {
        let mut results = self.search(items, query);
        results.truncate(limit);
        results
    }
}

impl Default for SearchService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_apps() -> Vec<AppItem> {
        vec![
            AppItem::new("Chrome", "/chrome.exe"),
            AppItem::new("Firefox", "/firefox.exe"),
            AppItem::new("Visual Studio Code", "/code.exe"),
            AppItem::new("Visual Studio 2022", "/vs.exe"),
            AppItem::new("Notepad", "/notepad.exe"),
        ]
    }

    #[test]
    fn test_search_exact() {
        let service = SearchService::new();
        let apps = test_apps();
        let query = SearchQuery::new("Chrome");

        let results = service.search(&apps, &query);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].item.name, "Chrome");
    }

    #[test]
    fn test_search_fuzzy() {
        let service = SearchService::new();
        let apps = test_apps();
        let query = SearchQuery::new("vsc");

        let results = service.search(&apps, &query);

        assert!(!results.is_empty());
        assert!(results[0].item.name.contains("Visual"));
    }

    #[test]
    fn test_search_empty_query() {
        let service = SearchService::new();
        let apps = test_apps();
        let query = SearchQuery::empty();

        let results = service.search(&apps, &query);

        // All apps should be returned
        assert_eq!(results.len(), apps.len());
    }

    #[test]
    fn test_search_with_history() {
        let service = SearchService::new();
        let apps = test_apps();
        let query = SearchQuery::new("Visual");

        let mut history = HashMap::new();
        history.insert(apps[3].id.clone(), 1.0); // VS 2022

        let results = service.search_with_history(&apps, &query, &history);

        // VS 2022 should be boosted to the top
        assert!(!results.is_empty());
        assert_eq!(results[0].item.name, "Visual Studio 2022");
    }

    #[test]
    fn test_search_no_results() {
        let service = SearchService::new();
        let apps = test_apps();
        let query = SearchQuery::new("nonexistent");

        let results = service.search(&apps, &query);

        assert!(results.is_empty());
    }
}
