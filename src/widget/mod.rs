//! Widget system for Wolfy
//!
//! Widgets are UI components that can be rendered and handle events.

pub mod textbox;

use crate::platform::win32::Renderer;
use crate::platform::Event;
use crate::theme::types::{Color, LayoutContext, Rect};

pub use textbox::Textbox;

/// Widget rendering state
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WidgetState {
    #[default]
    Normal,
    Focused,
    Disabled,
}

impl WidgetState {
    /// Get the CSS state suffix for theme lookups
    pub fn as_suffix(&self) -> Option<&'static str> {
        match self {
            WidgetState::Normal => None,
            WidgetState::Focused => Some("focused"),
            WidgetState::Disabled => Some("disabled"),
        }
    }
}

/// Widget style properties resolved from theme
#[derive(Clone, Debug)]
pub struct WidgetStyle {
    pub background_color: Color,
    pub text_color: Color,
    pub border_color: Color,
    pub border_width: f32,
    pub border_radius: f32,
    pub padding_top: f32,
    pub padding_right: f32,
    pub padding_bottom: f32,
    pub padding_left: f32,
    pub font_family: String,
    pub font_size: f32,
    pub placeholder_color: Color,
    pub cursor_color: Color,
    pub selection_color: Color,
}

impl Default for WidgetStyle {
    fn default() -> Self {
        Self {
            background_color: Color::from_hex("#2d2d2d").unwrap_or(Color::BLACK),
            text_color: Color::WHITE,
            border_color: Color::from_hex("#555555").unwrap_or(Color::WHITE),
            border_width: 1.0,
            border_radius: 4.0,
            padding_top: 8.0,
            padding_right: 12.0,
            padding_bottom: 8.0,
            padding_left: 12.0,
            font_family: "Segoe UI".to_string(),
            font_size: 16.0,
            placeholder_color: Color::from_hex("#888888").unwrap_or(Color::WHITE),
            cursor_color: Color::WHITE,
            selection_color: Color::from_hex("#264f78").unwrap_or(Color::BLUE),
        }
    }
}

/// Result from handling an event
#[derive(Clone, Debug, Default)]
pub struct EventResult {
    /// Widget wants to be repainted
    pub needs_repaint: bool,
    /// Event was consumed (don't propagate)
    pub consumed: bool,
    /// Text changed (for textbox)
    pub text_changed: bool,
    /// Submit action triggered (Enter pressed)
    pub submit: bool,
    /// Cancel action triggered (Escape pressed)
    pub cancel: bool,
}

impl EventResult {
    pub fn none() -> Self {
        Self::default()
    }

    pub fn consumed() -> Self {
        Self {
            consumed: true,
            ..Default::default()
        }
    }

    pub fn repaint() -> Self {
        Self {
            needs_repaint: true,
            consumed: true,
            ..Default::default()
        }
    }
}

/// Widget trait for UI components
pub trait Widget {
    /// Handle an event
    fn handle_event(&mut self, event: &Event, ctx: &LayoutContext) -> EventResult;

    /// Render the widget
    fn render(
        &self,
        renderer: &mut Renderer,
        rect: Rect,
        ctx: &LayoutContext,
    ) -> Result<(), windows::core::Error>;

    /// Get the current widget state
    fn state(&self) -> WidgetState;

    /// Set the widget state
    fn set_state(&mut self, state: WidgetState);

    /// Get the widget's style
    fn style(&self) -> &WidgetStyle;

    /// Set the widget's style
    fn set_style(&mut self, style: WidgetStyle);
}
