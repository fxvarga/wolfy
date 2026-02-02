//! SearchView - Interface for search UI components

use crate::adapters::presenters::search_presenter::ListItemViewModel;

/// Interface for search view
pub trait SearchView: Send + Sync {
    /// Display search results
    fn display_items(&mut self, items: Vec<ListItemViewModel>);

    /// Highlight an item
    fn highlight_item(&mut self, index: usize);

    /// Clear the display
    fn clear(&mut self);

    /// Set the search query text
    fn set_query(&mut self, query: &str);

    /// Get the current query text
    fn get_query(&self) -> String;

    /// Set the cursor position
    fn set_cursor(&mut self, position: usize);

    /// Show loading indicator
    fn show_loading(&mut self, loading: bool);

    /// Show error message
    fn show_error(&mut self, message: &str);

    /// Request redraw
    fn request_redraw(&mut self);
}

/// Null implementation for testing
pub struct NullSearchView {
    items: Vec<ListItemViewModel>,
    query: String,
    highlighted: usize,
}

impl NullSearchView {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            query: String::new(),
            highlighted: 0,
        }
    }

    pub fn items(&self) -> &[ListItemViewModel] {
        &self.items
    }
}

impl Default for NullSearchView {
    fn default() -> Self {
        Self::new()
    }
}

impl SearchView for NullSearchView {
    fn display_items(&mut self, items: Vec<ListItemViewModel>) {
        self.items = items;
    }

    fn highlight_item(&mut self, index: usize) {
        self.highlighted = index;
    }

    fn clear(&mut self) {
        self.items.clear();
        self.query.clear();
    }

    fn set_query(&mut self, query: &str) {
        self.query = query.to_string();
    }

    fn get_query(&self) -> String {
        self.query.clone()
    }

    fn set_cursor(&mut self, _position: usize) {}

    fn show_loading(&mut self, _loading: bool) {}

    fn show_error(&mut self, _message: &str) {}

    fn request_redraw(&mut self) {}
}
