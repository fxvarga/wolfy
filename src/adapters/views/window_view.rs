//! WindowView - Interface for window UI

/// Interface for window view
pub trait WindowView: Send + Sync {
    /// Show the window
    fn show(&mut self);

    /// Hide the window
    fn hide(&mut self);

    /// Set window opacity
    fn set_opacity(&mut self, opacity: f32);

    /// Set window position
    fn set_position(&mut self, x: i32, y: i32);

    /// Set window size
    fn set_size(&mut self, width: u32, height: u32);

    /// Center window on screen
    fn center_on_screen(&mut self);

    /// Set window title
    fn set_title(&mut self, title: &str);

    /// Request focus
    fn focus(&mut self);

    /// Check if window is visible
    fn is_visible(&self) -> bool;

    /// Check if window has focus
    fn is_focused(&self) -> bool;

    /// Request redraw
    fn request_redraw(&mut self);

    /// Close the window
    fn close(&mut self);
}

/// Null implementation for testing
pub struct NullWindowView {
    visible: bool,
    focused: bool,
    opacity: f32,
    position: (i32, i32),
    size: (u32, u32),
    title: String,
}

impl NullWindowView {
    pub fn new() -> Self {
        Self {
            visible: false,
            focused: false,
            opacity: 1.0,
            position: (0, 0),
            size: (800, 600),
            title: String::new(),
        }
    }
}

impl Default for NullWindowView {
    fn default() -> Self {
        Self::new()
    }
}

impl WindowView for NullWindowView {
    fn show(&mut self) {
        self.visible = true;
    }

    fn hide(&mut self) {
        self.visible = false;
    }

    fn set_opacity(&mut self, opacity: f32) {
        self.opacity = opacity;
    }

    fn set_position(&mut self, x: i32, y: i32) {
        self.position = (x, y);
    }

    fn set_size(&mut self, width: u32, height: u32) {
        self.size = (width, height);
    }

    fn center_on_screen(&mut self) {
        // No-op in null implementation
    }

    fn set_title(&mut self, title: &str) {
        self.title = title.to_string();
    }

    fn focus(&mut self) {
        self.focused = true;
    }

    fn is_visible(&self) -> bool {
        self.visible
    }

    fn is_focused(&self) -> bool {
        self.focused
    }

    fn request_redraw(&mut self) {}

    fn close(&mut self) {
        self.visible = false;
    }
}
