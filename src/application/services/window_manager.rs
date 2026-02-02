//! WindowManager - Manages window lifecycle and animations
//!
//! Coordinates window show/hide animations and state.

use crate::application::ports::animation_port::{AnimationHandle, AnimationPort, Easing};
use crate::domain::entities::window_state::{WindowState, WindowVisibility};

/// Manages application window state and animations
pub struct WindowManager<A>
where
    A: AnimationPort,
{
    animation_port: A,
    window_state: WindowState,
    current_animation: Option<AnimationHandle>,
    show_duration_ms: u32,
    hide_duration_ms: u32,
    easing: Easing,
}

impl<A> WindowManager<A>
where
    A: AnimationPort,
{
    /// Create a new window manager
    pub fn new(animation_port: A) -> Self {
        Self {
            animation_port,
            window_state: WindowState::default(),
            current_animation: None,
            show_duration_ms: 200,
            hide_duration_ms: 150,
            easing: Easing::EaseOut,
        }
    }

    /// Configure animation durations
    pub fn with_durations(mut self, show_ms: u32, hide_ms: u32) -> Self {
        self.show_duration_ms = show_ms;
        self.hide_duration_ms = hide_ms;
        self
    }

    /// Configure easing
    pub fn with_easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    /// Show the window (with animation)
    pub fn show(&mut self) {
        if self.window_state.is_shown() || self.window_state.visibility == WindowVisibility::Showing
        {
            return;
        }

        // Cancel any existing animation
        if let Some(handle) = self.current_animation.take() {
            self.animation_port.cancel(handle);
        }

        // Start show animation
        let handle = self.animation_port.animate_show(
            &mut self.window_state,
            self.show_duration_ms,
            self.easing,
        );
        self.current_animation = Some(handle);
    }

    /// Hide the window (with animation)
    pub fn hide(&mut self) {
        if self.window_state.is_hidden() || self.window_state.visibility == WindowVisibility::Hiding
        {
            return;
        }

        // Cancel any existing animation
        if let Some(handle) = self.current_animation.take() {
            self.animation_port.cancel(handle);
        }

        // Start hide animation
        let handle = self.animation_port.animate_hide(
            &mut self.window_state,
            self.hide_duration_ms,
            self.easing,
        );
        self.current_animation = Some(handle);
    }

    /// Toggle window visibility
    pub fn toggle(&mut self) {
        if self.window_state.is_visible() {
            self.hide();
        } else {
            self.show();
        }
    }

    /// Update animations
    pub fn update(&mut self, delta_ms: f32) {
        let completed = self.animation_port.update(delta_ms);

        if let Some(handle) = &self.current_animation {
            if completed.contains(handle) {
                self.current_animation = None;

                // Finalize state based on visibility
                match self.window_state.visibility {
                    WindowVisibility::Showing => self.window_state.finish_showing(),
                    WindowVisibility::Hiding => self.window_state.finish_hiding(),
                    _ => {}
                }
            }
        }
    }

    /// Check if window is visible
    pub fn is_visible(&self) -> bool {
        self.window_state.is_visible()
    }

    /// Check if window is animating
    pub fn is_animating(&self) -> bool {
        self.window_state.is_animating()
    }

    /// Get current opacity
    pub fn opacity(&self) -> f32 {
        self.window_state.effective_opacity()
    }

    /// Get current window state
    pub fn state(&self) -> &WindowState {
        &self.window_state
    }

    /// Get mutable window state
    pub fn state_mut(&mut self) -> &mut WindowState {
        &mut self.window_state
    }

    /// Set window position
    pub fn set_position(&mut self, x: i32, y: i32) {
        self.window_state.position.x = x;
        self.window_state.position.y = y;
    }

    /// Set window dimensions
    pub fn set_dimensions(&mut self, width: u32, height: u32) {
        self.window_state.dimensions.width = width;
        self.window_state.dimensions.height = height;
    }

    /// Set DPI scale
    pub fn set_dpi_scale(&mut self, scale: f32) {
        self.window_state.dpi_scale = scale;
    }

    /// Set focus state
    pub fn set_focused(&mut self, focused: bool) {
        self.window_state.focused = focused;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::ports::animation_port::NullAnimationPort;

    #[test]
    fn test_window_manager_show_hide() {
        let animation_port = NullAnimationPort::new();
        let mut manager = WindowManager::new(animation_port);

        assert!(!manager.is_visible());

        manager.show();
        assert!(manager.is_visible());

        manager.hide();
        assert!(!manager.is_visible());
    }

    #[test]
    fn test_window_manager_toggle() {
        let animation_port = NullAnimationPort::new();
        let mut manager = WindowManager::new(animation_port);

        manager.toggle();
        assert!(manager.is_visible());

        manager.toggle();
        assert!(!manager.is_visible());
    }
}
