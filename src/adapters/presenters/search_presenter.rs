//! SearchPresenter - Formats search results for display

use crate::domain::entities::AppItem;
use crate::domain::services::search_service::SearchResult;

/// View model for a list item
#[derive(Clone, Debug)]
pub struct ListItemViewModel {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub icon_path: Option<String>,
    pub is_selected: bool,
    pub matched_indices: Vec<usize>,
}

/// Presenter for search results
pub struct SearchPresenter {
    /// Current view models
    items: Vec<ListItemViewModel>,
    /// Selected index
    selected_index: usize,
}

impl SearchPresenter {
    /// Create a new search presenter
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            selected_index: 0,
        }
    }

    /// Present search results
    pub fn present_results(&mut self, results: &[SearchResult], selected_index: usize) {
        self.selected_index = selected_index;
        self.items = results
            .iter()
            .enumerate()
            .map(|(i, result)| self.create_view_model(&result.item, &result.matched_indices, i == selected_index))
            .collect();
    }

    /// Present app items (without match info)
    pub fn present_apps(&mut self, apps: &[AppItem], selected_index: usize) {
        self.selected_index = selected_index;
        self.items = apps
            .iter()
            .enumerate()
            .map(|(i, app)| self.create_view_model(app, &[], i == selected_index))
            .collect();
    }

    /// Create a view model from an app item
    fn create_view_model(
        &self,
        item: &AppItem,
        matched_indices: &[usize],
        is_selected: bool,
    ) -> ListItemViewModel {
        ListItemViewModel {
            id: item.id.clone(),
            title: item.name.clone(),
            subtitle: item
                .description
                .clone()
                .unwrap_or_else(|| item.path.to_string_lossy().to_string()),
            icon_path: item.icon_path.as_ref().map(|p| p.to_string_lossy().to_string()),
            is_selected,
            matched_indices: matched_indices.to_vec(),
        }
    }

    /// Get current view models
    pub fn items(&self) -> &[ListItemViewModel] {
        &self.items
    }

    /// Get selected index
    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    /// Update selection
    pub fn set_selected_index(&mut self, index: usize) {
        // Update old selection
        if self.selected_index < self.items.len() {
            self.items[self.selected_index].is_selected = false;
        }

        // Update new selection
        if index < self.items.len() {
            self.selected_index = index;
            self.items[index].is_selected = true;
        }
    }

    /// Clear results
    pub fn clear(&mut self) {
        self.items.clear();
        self.selected_index = 0;
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Get count
    pub fn count(&self) -> usize {
        self.items.len()
    }
}

impl Default for SearchPresenter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn test_results() -> Vec<SearchResult> {
        vec![
            SearchResult {
                item: AppItem::new("Chrome", PathBuf::from("/chrome")),
                score: 100,
                matched_indices: vec![0, 1],
            },
            SearchResult {
                item: AppItem::new("Firefox", PathBuf::from("/firefox")),
                score: 80,
                matched_indices: vec![],
            },
        ]
    }

    #[test]
    fn test_present_results() {
        let mut presenter = SearchPresenter::new();
        let results = test_results();

        presenter.present_results(&results, 0);

        assert_eq!(presenter.count(), 2);
        assert!(presenter.items()[0].is_selected);
        assert!(!presenter.items()[1].is_selected);
    }

    #[test]
    fn test_update_selection() {
        let mut presenter = SearchPresenter::new();
        let results = test_results();

        presenter.present_results(&results, 0);
        presenter.set_selected_index(1);

        assert!(!presenter.items()[0].is_selected);
        assert!(presenter.items()[1].is_selected);
    }
}
