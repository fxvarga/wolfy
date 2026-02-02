//! SearchController - Handles search input and updates

use crate::application::services::command_handler::AppCommand;

/// Controller for search input
pub struct SearchController {
    /// Current search text
    text: String,
    /// Cursor position
    cursor: usize,
    /// Debounce delay in milliseconds
    debounce_ms: u32,
    /// Time of last input
    last_input_time: Option<std::time::Instant>,
    /// Whether text has changed since last search
    dirty: bool,
}

impl SearchController {
    /// Create a new search controller
    pub fn new() -> Self {
        Self {
            text: String::new(),
            cursor: 0,
            debounce_ms: 100,
            last_input_time: None,
            dirty: false,
        }
    }

    /// Set debounce delay
    pub fn with_debounce(mut self, ms: u32) -> Self {
        self.debounce_ms = ms;
        self
    }

    /// Handle character input
    pub fn handle_char(&mut self, c: char) -> Option<AppCommand> {
        if c.is_control() {
            return None;
        }

        // Insert character at cursor
        self.text.insert(self.cursor, c);
        self.cursor += 1;
        self.dirty = true;
        self.last_input_time = Some(std::time::Instant::now());

        Some(AppCommand::Search(self.text.clone()))
    }

    /// Handle backspace
    pub fn handle_backspace(&mut self) -> Option<AppCommand> {
        if self.cursor > 0 {
            self.cursor -= 1;
            self.text.remove(self.cursor);
            self.dirty = true;
            self.last_input_time = Some(std::time::Instant::now());

            Some(AppCommand::Search(self.text.clone()))
        } else {
            None
        }
    }

    /// Handle delete
    pub fn handle_delete(&mut self) -> Option<AppCommand> {
        if self.cursor < self.text.len() {
            self.text.remove(self.cursor);
            self.dirty = true;
            self.last_input_time = Some(std::time::Instant::now());

            Some(AppCommand::Search(self.text.clone()))
        } else {
            None
        }
    }

    /// Clear the search text
    pub fn clear(&mut self) -> AppCommand {
        self.text.clear();
        self.cursor = 0;
        self.dirty = true;
        AppCommand::Clear
    }

    /// Set the search text directly
    pub fn set_text(&mut self, text: &str) -> AppCommand {
        self.text = text.to_string();
        self.cursor = self.text.len();
        self.dirty = true;
        AppCommand::Search(self.text.clone())
    }

    /// Move cursor left
    pub fn move_cursor_left(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    /// Move cursor right
    pub fn move_cursor_right(&mut self) {
        if self.cursor < self.text.len() {
            self.cursor += 1;
        }
    }

    /// Move cursor to start
    pub fn move_cursor_home(&mut self) {
        self.cursor = 0;
    }

    /// Move cursor to end
    pub fn move_cursor_end(&mut self) {
        self.cursor = self.text.len();
    }

    /// Get current text
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Get cursor position
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Check if text is empty
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Check if debounce time has elapsed
    pub fn should_search(&self) -> bool {
        if !self.dirty {
            return false;
        }

        if let Some(last_time) = self.last_input_time {
            last_time.elapsed().as_millis() >= self.debounce_ms as u128
        } else {
            false
        }
    }

    /// Mark search as executed
    pub fn mark_searched(&mut self) {
        self.dirty = false;
    }

    /// Get pending search command if debounce elapsed
    pub fn poll_search(&mut self) -> Option<AppCommand> {
        if self.should_search() {
            self.dirty = false;
            Some(AppCommand::Search(self.text.clone()))
        } else {
            None
        }
    }
}

impl Default for SearchController {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_char_input() {
        let mut controller = SearchController::new();

        controller.handle_char('h');
        controller.handle_char('e');
        controller.handle_char('l');
        controller.handle_char('l');
        controller.handle_char('o');

        assert_eq!(controller.text(), "hello");
        assert_eq!(controller.cursor(), 5);
    }

    #[test]
    fn test_backspace() {
        let mut controller = SearchController::new();
        controller.set_text("hello");

        controller.handle_backspace();
        assert_eq!(controller.text(), "hell");

        controller.handle_backspace();
        assert_eq!(controller.text(), "hel");
    }

    #[test]
    fn test_clear() {
        let mut controller = SearchController::new();
        controller.set_text("hello");

        controller.clear();
        assert!(controller.is_empty());
        assert_eq!(controller.cursor(), 0);
    }

    #[test]
    fn test_cursor_movement() {
        let mut controller = SearchController::new();
        controller.set_text("hello");

        controller.move_cursor_home();
        assert_eq!(controller.cursor(), 0);

        controller.move_cursor_right();
        assert_eq!(controller.cursor(), 1);

        controller.move_cursor_end();
        assert_eq!(controller.cursor(), 5);

        controller.move_cursor_left();
        assert_eq!(controller.cursor(), 4);
    }
}
