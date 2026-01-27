//! Widget system for Wolfy
//!
//! Widgets are UI components that can be rendered and handle events.

pub mod textbox;

use crate::platform::win32::Renderer;
use crate::platform::Event;
use crate::theme::tree::ThemeTree;
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
    // Window-level properties
    pub window_background_color: Color,
    pub window_opacity: f32, // 0.0 = fully transparent, 1.0 = opaque
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
            window_background_color: Color::from_hex("#1e1e1e").unwrap_or(Color::BLACK),
            window_opacity: 1.0,
        }
    }
}

impl WidgetStyle {
    /// Load textbox style from theme
    pub fn from_theme_textbox(theme: &ThemeTree, state: Option<&str>) -> Self {
        let default = Self::default();

        // Debug: log what we're extracting
        crate::log!("from_theme_textbox: state={:?}", state);
        let font_size_val = theme.get_value("textbox", state, "font-size");
        crate::log!("  font-size value from theme: {:?}", font_size_val);
        let font_size =
            theme.get_number("textbox", state, "font-size", default.font_size as f64) as f32;
        crate::log!("  font-size after get_number: {}", font_size);

        Self {
            background_color: theme.get_color(
                "textbox",
                state,
                "background-color",
                default.background_color,
            ),
            text_color: theme.get_color("textbox", state, "text-color", default.text_color),
            border_color: theme.get_color("textbox", state, "border-color", default.border_color),
            border_width: theme.get_number(
                "textbox",
                state,
                "border-width",
                default.border_width as f64,
            ) as f32,
            border_radius: theme.get_number(
                "textbox",
                state,
                "border-radius",
                default.border_radius as f64,
            ) as f32,
            padding_top: theme.get_number(
                "textbox",
                state,
                "padding-top",
                default.padding_top as f64,
            ) as f32,
            padding_right: theme.get_number(
                "textbox",
                state,
                "padding-right",
                default.padding_right as f64,
            ) as f32,
            padding_bottom: theme.get_number(
                "textbox",
                state,
                "padding-bottom",
                default.padding_bottom as f64,
            ) as f32,
            padding_left: theme.get_number(
                "textbox",
                state,
                "padding-left",
                default.padding_left as f64,
            ) as f32,
            font_family: theme.get_string("textbox", state, "font-family", &default.font_family),
            font_size,
            placeholder_color: theme.get_color(
                "textbox",
                state,
                "placeholder-color",
                default.placeholder_color,
            ),
            cursor_color: theme.get_color("textbox", state, "cursor-color", default.cursor_color),
            selection_color: theme.get_color(
                "textbox",
                state,
                "selection-color",
                default.selection_color,
            ),
            // Window-level properties from globals (*)
            window_background_color: {
                let color = theme.get_color(
                    "*",
                    None,
                    "background-color",
                    default.window_background_color,
                );
                crate::log!(
                    "  window_background_color from theme: r={}, g={}, b={}, a={}",
                    color.r,
                    color.g,
                    color.b,
                    color.a
                );
                color
            },
            window_opacity: {
                let opacity =
                    theme.get_number("*", None, "opacity", default.window_opacity as f64) as f32;
                crate::log!("  window_opacity from theme: {}", opacity);
                opacity
            },
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
