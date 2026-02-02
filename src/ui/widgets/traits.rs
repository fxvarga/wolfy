//! Widget traits - Interfaces for UI components

use crate::application::ports::RenderPort;
use crate::domain::value_objects::Rect;

/// Event result from widget event handling
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventResult {
    /// Event was handled, stop propagation
    Handled,
    /// Event was not handled, continue propagation
    NotHandled,
    /// Event was partially handled but should continue propagation
    Propagate,
}

/// Widget state flags
#[derive(Clone, Copy, Debug, Default)]
pub struct WidgetState {
    pub enabled: bool,
    pub visible: bool,
    pub focused: bool,
    pub hovered: bool,
    pub pressed: bool,
}

impl WidgetState {
    pub fn new() -> Self {
        Self {
            enabled: true,
            visible: true,
            focused: false,
            hovered: false,
            pressed: false,
        }
    }

    pub fn is_interactive(&self) -> bool {
        self.enabled && self.visible
    }
}

/// Size constraint for layout
#[derive(Clone, Copy, Debug, Default)]
pub struct Constraints {
    pub min_width: f32,
    pub min_height: f32,
    pub max_width: f32,
    pub max_height: f32,
}

impl Constraints {
    pub fn new(min_width: f32, min_height: f32, max_width: f32, max_height: f32) -> Self {
        Self {
            min_width,
            min_height,
            max_width,
            max_height,
        }
    }

    pub fn unbounded() -> Self {
        Self::new(0.0, 0.0, f32::INFINITY, f32::INFINITY)
    }

    pub fn tight(width: f32, height: f32) -> Self {
        Self::new(width, height, width, height)
    }
}

/// Measured size from layout
#[derive(Clone, Copy, Debug, Default)]
pub struct MeasuredSize {
    pub width: f32,
    pub height: f32,
}

impl MeasuredSize {
    pub fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }
}

/// Core widget trait
pub trait Widget {
    /// Measure the widget's desired size
    fn measure(&self, constraints: Constraints) -> MeasuredSize;

    /// Arrange the widget within bounds
    fn arrange(&mut self, bounds: Rect);

    /// Render the widget
    fn render(&self, renderer: &mut dyn RenderPort);

    /// Get widget bounds
    fn bounds(&self) -> Rect;

    /// Get widget state
    fn state(&self) -> WidgetState;

    /// Set widget state
    fn set_state(&mut self, state: WidgetState);

    /// Check if widget contains a point
    fn hit_test(&self, x: f32, y: f32) -> bool {
        self.bounds().contains(x, y)
    }
}

/// Container widget trait
pub trait Container: Widget {
    /// Add a child widget
    fn add_child(&mut self, child: Box<dyn Widget>);

    /// Remove a child widget
    fn remove_child(&mut self, index: usize) -> Option<Box<dyn Widget>>;

    /// Get child count
    fn child_count(&self) -> usize;

    /// Get child by index
    fn child(&self, index: usize) -> Option<&dyn Widget>;

    /// Get mutable child by index
    fn child_mut(&mut self, index: usize) -> Option<&mut dyn Widget>;
}

/// Focusable widget trait
pub trait Focusable: Widget {
    /// Focus the widget
    fn focus(&mut self);

    /// Remove focus
    fn blur(&mut self);

    /// Check if focused
    fn is_focused(&self) -> bool;

    /// Check if widget can receive focus
    fn can_focus(&self) -> bool;
}

/// Scrollable widget trait
pub trait Scrollable: Widget {
    /// Scroll by delta
    fn scroll(&mut self, delta: f32);

    /// Scroll to position
    fn scroll_to(&mut self, position: f32);

    /// Get current scroll position
    fn scroll_position(&self) -> f32;

    /// Get maximum scroll position
    fn max_scroll(&self) -> f32;

    /// Get visible content height
    fn viewport_height(&self) -> f32;

    /// Get total content height
    fn content_height(&self) -> f32;
}
