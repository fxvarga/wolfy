//! Element widget - a single row item with optional icon and text

use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;

use crate::platform::win32::Renderer;
use crate::platform::Event;
use crate::theme::tree::ThemeTree;
use crate::theme::types::{Color, LayoutContext, Rect};

use super::base::{Constraints, LayoutProps, MeasuredSize};
use super::{EventResult, Widget, WidgetState, WidgetStyle};

/// Data for a single element/row
#[derive(Clone, Debug)]
pub struct ElementData {
    /// Display text
    pub text: String,
    /// Secondary text (e.g., path or description)
    pub subtext: Option<String>,
    /// Icon path (for future use)
    pub icon_path: Option<String>,
    /// User data (e.g., launch command)
    pub user_data: String,
}

impl ElementData {
    pub fn new(text: impl Into<String>, user_data: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            subtext: None,
            icon_path: None,
            user_data: user_data.into(),
        }
    }

    pub fn with_subtext(mut self, subtext: impl Into<String>) -> Self {
        self.subtext = Some(subtext.into());
        self
    }

    pub fn with_icon(mut self, icon_path: impl Into<String>) -> Self {
        self.icon_path = Some(icon_path.into());
        self
    }
}

/// Style for element widget
#[derive(Clone, Debug)]
pub struct ElementStyle {
    pub background_color: Color,
    pub background_color_selected: Color,
    pub background_color_hover: Color,
    pub text_color: Color,
    pub text_color_selected: Color,
    pub subtext_color: Color,
    pub font_family: String,
    pub font_size: f32,
    pub subtext_font_size: f32,
    pub padding_horizontal: f32,
    pub padding_vertical: f32,
    pub icon_size: f32,
    pub icon_spacing: f32,
    pub height: f32,
    pub border_radius: f32,
}

impl Default for ElementStyle {
    fn default() -> Self {
        Self {
            background_color: Color::TRANSPARENT,
            background_color_selected: Color::from_hex("#264f78").unwrap_or(Color::BLUE),
            background_color_hover: Color::from_hex("#3c3c3c").unwrap_or(Color::BLACK),
            text_color: Color::from_hex("#d4d4d4").unwrap_or(Color::WHITE),
            text_color_selected: Color::WHITE,
            subtext_color: Color::from_hex("#808080").unwrap_or(Color::WHITE),
            font_family: "Segoe UI".to_string(),
            font_size: 14.0,
            subtext_font_size: 11.0,
            padding_horizontal: 12.0,
            padding_vertical: 8.0,
            icon_size: 24.0,
            icon_spacing: 8.0,
            height: 40.0,
            border_radius: 0.0,
        }
    }
}

impl ElementStyle {
    /// Load style from theme
    pub fn from_theme(theme: &ThemeTree, state: Option<&str>) -> Self {
        let default = Self::default();
        let style = Self {
            background_color: theme.get_color(
                "element",
                state,
                "background-color",
                default.background_color,
            ),
            background_color_selected: theme.get_color(
                "element",
                Some("selected"),
                "background-color",
                default.background_color_selected,
            ),
            background_color_hover: theme.get_color(
                "element",
                Some("hover"),
                "background-color",
                default.background_color_hover,
            ),
            text_color: theme.get_color("element", state, "text-color", default.text_color),
            text_color_selected: theme.get_color(
                "element",
                Some("selected"),
                "text-color",
                default.text_color_selected,
            ),
            subtext_color: theme.get_color(
                "element",
                state,
                "subtext-color",
                default.subtext_color,
            ),
            font_family: theme.get_string("element", state, "font-family", &default.font_family),
            font_size: theme.get_number("element", state, "font-size", default.font_size as f64)
                as f32,
            subtext_font_size: theme.get_number(
                "element",
                state,
                "subtext-font-size",
                default.subtext_font_size as f64,
            ) as f32,
            padding_horizontal: theme.get_number(
                "element",
                state,
                "padding-horizontal",
                default.padding_horizontal as f64,
            ) as f32,
            padding_vertical: theme.get_number(
                "element",
                state,
                "padding-vertical",
                default.padding_vertical as f64,
            ) as f32,
            icon_size: theme.get_number("element", state, "icon-size", default.icon_size as f64)
                as f32,
            icon_spacing: theme.get_number(
                "element",
                state,
                "icon-spacing",
                default.icon_spacing as f64,
            ) as f32,
            height: theme.get_number("element", state, "height", default.height as f64) as f32,
            border_radius: theme.get_number(
                "element",
                state,
                "border-radius",
                default.border_radius as f64,
            ) as f32,
        };
        crate::log!(
            "ElementStyle::from_theme - border_radius={}, hover_bg=({},{},{},{})",
            style.border_radius,
            style.background_color_hover.r,
            style.background_color_hover.g,
            style.background_color_hover.b,
            style.background_color_hover.a
        );
        style
    }
}

/// A single element/row in a list
pub struct Element {
    /// Element data
    data: ElementData,
    /// Layout properties
    layout: LayoutProps,
    /// Visual style
    style: ElementStyle,
    /// Widget state
    state: WidgetState,
    /// Whether this element is selected
    selected: bool,
    /// Whether mouse is hovering
    hovered: bool,
}

impl Element {
    /// Create a new element
    pub fn new(data: ElementData) -> Self {
        Self {
            data,
            layout: LayoutProps::default(),
            style: ElementStyle::default(),
            state: WidgetState::Normal,
            selected: false,
            hovered: false,
        }
    }

    /// Set the style
    pub fn with_style(mut self, style: ElementStyle) -> Self {
        self.style = style;
        self
    }

    /// Get the element data
    pub fn data(&self) -> &ElementData {
        &self.data
    }

    /// Set selected state
    pub fn set_selected(&mut self, selected: bool) {
        self.selected = selected;
    }

    /// Check if selected
    pub fn is_selected(&self) -> bool {
        self.selected
    }

    /// Set hovered state
    pub fn set_hovered(&mut self, hovered: bool) {
        self.hovered = hovered;
    }

    /// Get the configured height
    pub fn height(&self) -> f32 {
        self.style.height
    }

    /// Update style (for hot-reload)
    pub fn update_style(&mut self, style: ElementStyle) {
        self.style = style;
    }
}

impl Widget for Element {
    fn handle_event(&mut self, _event: &Event, _ctx: &LayoutContext) -> EventResult {
        // Elements don't handle events directly - ListView manages selection
        EventResult::none()
    }

    fn render(
        &self,
        renderer: &mut Renderer,
        rect: Rect,
        _ctx: &LayoutContext,
    ) -> Result<(), windows::core::Error> {
        let bounds = D2D_RECT_F {
            left: rect.x,
            top: rect.y,
            right: rect.x + rect.width,
            bottom: rect.y + rect.height,
        };

        // Choose background color based on state
        let bg_color = if self.selected {
            self.style.background_color_selected
        } else if self.hovered {
            self.style.background_color_hover
        } else {
            self.style.background_color
        };

        // Draw background (rounded if border_radius > 0)
        if bg_color.a > 0.0 {
            // Scale border_radius with DPI
            let scaled_radius = self.style.border_radius * _ctx.scale_factor;
            log!(
                "Element::render bg - selected={}, hovered={}, border_radius={} (scaled={}), color=({},{},{},{})",
                self.selected,
                self.hovered,
                self.style.border_radius,
                scaled_radius,
                bg_color.r,
                bg_color.g,
                bg_color.b,
                bg_color.a
            );
            if scaled_radius > 0.0 {
                renderer.fill_rounded_rect(bounds, scaled_radius, scaled_radius, bg_color)?;
            } else {
                renderer.fill_rect(bounds, bg_color)?;
            }
        }

        // Choose text color
        let text_color = if self.selected {
            self.style.text_color_selected
        } else {
            self.style.text_color
        };

        // Calculate text position (leaving room for icon)
        let text_x =
            rect.x + self.style.padding_horizontal + self.style.icon_size + self.style.icon_spacing;
        let text_width = rect.width
            - self.style.padding_horizontal * 2.0
            - self.style.icon_size
            - self.style.icon_spacing;

        // Create text format fresh (like Textbox does)
        let format = match renderer.create_text_format(
            &self.style.font_family,
            self.style.font_size,
            false,
            false,
        ) {
            Ok(f) => f,
            Err(e) => {
                log!("Element::render - failed to create text format: {:?}", e);
                return Ok(());
            }
        };

        // Draw main text - always single line centered (no subtext)
        let text_rect = D2D_RECT_F {
            left: text_x,
            top: rect.y,
            right: text_x + text_width,
            bottom: rect.y + rect.height,
        };

        log!(
            "Element::render drawing '{}' at rect ({},{},{},{}) color=({},{},{},{})",
            self.data.text,
            text_rect.left,
            text_rect.top,
            text_rect.right,
            text_rect.bottom,
            text_color.r,
            text_color.g,
            text_color.b,
            text_color.a
        );
        renderer.draw_text(&self.data.text, &format, text_rect, text_color)?;

        // Draw icon if we have an icon path
        if let Some(ref icon_path) = self.data.icon_path {
            // Calculate icon position - vertically centered in the element
            let icon_size = self.style.icon_size;
            let icon_x = rect.x + self.style.padding_horizontal;
            let icon_y = rect.y + (rect.height - icon_size) / 2.0;

            let icon_rect = D2D_RECT_F {
                left: icon_x,
                top: icon_y,
                right: icon_x + icon_size,
                bottom: icon_y + icon_size,
            };

            renderer.draw_icon(icon_path, icon_rect, 1.0);
        }

        Ok(())
    }

    fn state(&self) -> WidgetState {
        self.state
    }

    fn set_state(&mut self, state: WidgetState) {
        self.state = state;
    }

    fn style(&self) -> &WidgetStyle {
        static DEFAULT: std::sync::OnceLock<WidgetStyle> = std::sync::OnceLock::new();
        DEFAULT.get_or_init(WidgetStyle::default)
    }

    fn set_style(&mut self, _style: WidgetStyle) {
        // Elements use ElementStyle
    }

    fn measure(&self, constraints: Constraints, _ctx: &LayoutContext) -> MeasuredSize {
        MeasuredSize::new(
            constraints.max.width,
            self.style.height.min(constraints.max.height),
        )
    }

    fn arrange(&mut self, _bounds: Rect, _ctx: &LayoutContext) {
        // Elements have no children
    }

    fn layout_props(&self) -> &LayoutProps {
        &self.layout
    }

    fn widget_name(&self) -> &str {
        "element"
    }
}
