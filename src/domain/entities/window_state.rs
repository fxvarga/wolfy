//! WindowState entity - represents the state of the application window
//!
//! Manages window visibility, position, size, and animation state.

/// Window visibility state
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowVisibility {
    /// Window is completely hidden
    Hidden,
    /// Window is animating to become visible
    Showing,
    /// Window is fully visible
    Shown,
    /// Window is animating to become hidden
    Hiding,
}

impl Default for WindowVisibility {
    fn default() -> Self {
        WindowVisibility::Hidden
    }
}

/// Window position on screen
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct WindowPosition {
    pub x: i32,
    pub y: i32,
}

impl WindowPosition {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

/// Window dimensions
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct WindowDimensions {
    pub width: u32,
    pub height: u32,
}

impl WindowDimensions {
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

/// Complete window state
#[derive(Clone, Debug, PartialEq)]
pub struct WindowState {
    /// Current visibility state
    pub visibility: WindowVisibility,
    /// Window position
    pub position: WindowPosition,
    /// Window dimensions
    pub dimensions: WindowDimensions,
    /// Current opacity (0.0-1.0)
    pub opacity: f32,
    /// Animation progress (0.0-1.0, only valid during Showing/Hiding)
    pub animation_progress: f32,
    /// Whether window has focus
    pub focused: bool,
    /// DPI scale factor
    pub dpi_scale: f32,
}

impl Default for WindowState {
    fn default() -> Self {
        Self {
            visibility: WindowVisibility::Hidden,
            position: WindowPosition::default(),
            dimensions: WindowDimensions::new(928, 600),
            opacity: 1.0,
            animation_progress: 0.0,
            focused: false,
            dpi_scale: 1.0,
        }
    }
}

impl WindowState {
    /// Create a new window state with given dimensions
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            dimensions: WindowDimensions::new(width, height),
            ..Default::default()
        }
    }

    /// Check if window is visible (shown or animating)
    pub fn is_visible(&self) -> bool {
        matches!(
            self.visibility,
            WindowVisibility::Shown | WindowVisibility::Showing | WindowVisibility::Hiding
        )
    }

    /// Check if window is fully shown
    pub fn is_shown(&self) -> bool {
        self.visibility == WindowVisibility::Shown
    }

    /// Check if window is completely hidden
    pub fn is_hidden(&self) -> bool {
        self.visibility == WindowVisibility::Hidden
    }

    /// Check if window is animating
    pub fn is_animating(&self) -> bool {
        matches!(
            self.visibility,
            WindowVisibility::Showing | WindowVisibility::Hiding
        )
    }

    /// Start show animation
    pub fn start_showing(&mut self) {
        self.visibility = WindowVisibility::Showing;
        self.animation_progress = 0.0;
    }

    /// Start hide animation
    pub fn start_hiding(&mut self) {
        self.visibility = WindowVisibility::Hiding;
        self.animation_progress = 0.0;
    }

    /// Complete show animation
    pub fn finish_showing(&mut self) {
        self.visibility = WindowVisibility::Shown;
        self.animation_progress = 1.0;
        self.opacity = 1.0;
    }

    /// Complete hide animation
    pub fn finish_hiding(&mut self) {
        self.visibility = WindowVisibility::Hidden;
        self.animation_progress = 0.0;
        self.opacity = 0.0;
    }

    /// Update animation progress
    pub fn update_animation(&mut self, progress: f32) {
        self.animation_progress = progress.clamp(0.0, 1.0);

        // Update opacity based on visibility state
        match self.visibility {
            WindowVisibility::Showing => {
                self.opacity = self.animation_progress;
            }
            WindowVisibility::Hiding => {
                self.opacity = 1.0 - self.animation_progress;
            }
            _ => {}
        }
    }

    /// Get effective opacity (considering animation)
    pub fn effective_opacity(&self) -> f32 {
        self.opacity
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_window_state_default() {
        let state = WindowState::default();

        assert!(state.is_hidden());
        assert!(!state.is_visible());
        assert!(!state.is_animating());
    }

    #[test]
    fn test_show_animation_lifecycle() {
        let mut state = WindowState::default();

        // Start showing
        state.start_showing();
        assert!(state.is_animating());
        assert!(state.is_visible());
        assert_eq!(state.visibility, WindowVisibility::Showing);

        // Mid-animation
        state.update_animation(0.5);
        assert_eq!(state.animation_progress, 0.5);
        assert!((state.opacity - 0.5).abs() < 0.001);

        // Finish showing
        state.finish_showing();
        assert!(state.is_shown());
        assert!(!state.is_animating());
        assert_eq!(state.opacity, 1.0);
    }

    #[test]
    fn test_hide_animation_lifecycle() {
        let mut state = WindowState::default();
        state.finish_showing(); // Start from shown state

        // Start hiding
        state.start_hiding();
        assert!(state.is_animating());
        assert_eq!(state.visibility, WindowVisibility::Hiding);

        // Mid-animation
        state.update_animation(0.5);
        assert!((state.opacity - 0.5).abs() < 0.001);

        // Finish hiding
        state.finish_hiding();
        assert!(state.is_hidden());
        assert_eq!(state.opacity, 0.0);
    }
}
