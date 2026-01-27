//! Container widget - HBox/VBox layout for child widgets

use crate::platform::win32::Renderer;
use crate::platform::Event;
use crate::theme::tree::ThemeTree;
use crate::theme::types::{Color, LayoutContext, Orientation, Rect};

use super::base::{ArrangedBounds, Constraints, LayoutProps, MeasuredSize, Size};
use super::{EventResult, Widget, WidgetState, WidgetStyle};

/// A container that arranges children horizontally or vertically
pub struct Container {
    /// Widget name (for theme lookups)
    name: String,
    /// Child widgets
    children: Vec<Box<dyn Widget>>,
    /// Layout properties
    layout: LayoutProps,
    /// Visual style
    style: ContainerStyle,
    /// Widget state
    state: WidgetState,
    /// Cached child bounds from last arrange
    child_bounds: Vec<ArrangedBounds>,
}

/// Style for container widget
#[derive(Clone, Debug)]
pub struct ContainerStyle {
    pub background_color: Color,
    pub border_color: Color,
    pub border_width: f32,
    pub border_radius: f32,
}

impl Default for ContainerStyle {
    fn default() -> Self {
        Self {
            background_color: Color::TRANSPARENT,
            border_color: Color::TRANSPARENT,
            border_width: 0.0,
            border_radius: 0.0,
        }
    }
}

impl ContainerStyle {
    /// Load style from theme for a named widget
    pub fn from_theme(theme: &ThemeTree, name: &str, state: Option<&str>) -> Self {
        let default = Self::default();
        Self {
            background_color: theme.get_color(
                name,
                state,
                "background-color",
                default.background_color,
            ),
            border_color: theme.get_color(name, state, "border-color", default.border_color),
            border_width: theme.get_number(name, state, "border-width", default.border_width as f64)
                as f32,
            border_radius: theme.get_number(
                name,
                state,
                "border-radius",
                default.border_radius as f64,
            ) as f32,
        }
    }
}

impl Container {
    /// Create a new container with a name
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            children: Vec::new(),
            layout: LayoutProps::default(),
            style: ContainerStyle::default(),
            state: WidgetState::Normal,
            child_bounds: Vec::new(),
        }
    }

    /// Set orientation (horizontal or vertical)
    pub fn with_orientation(mut self, orientation: Orientation) -> Self {
        self.layout.orientation = orientation;
        self
    }

    /// Set spacing between children
    pub fn with_spacing(mut self, spacing: f32) -> Self {
        self.layout.spacing = spacing;
        self
    }

    /// Set whether this container expands to fill available space
    pub fn with_expand(mut self, expand: bool) -> Self {
        self.layout.expand = expand;
        self
    }

    /// Set padding
    pub fn with_padding(mut self, top: f32, right: f32, bottom: f32, left: f32) -> Self {
        self.layout.padding = (top, right, bottom, left);
        self
    }

    /// Set style
    pub fn with_style(mut self, style: ContainerStyle) -> Self {
        self.style = style;
        self
    }

    /// Add a child widget
    pub fn add_child(&mut self, child: Box<dyn Widget>) {
        self.children.push(child);
    }

    /// Get the widget name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get children (immutable)
    pub fn children(&self) -> &[Box<dyn Widget>] {
        &self.children
    }

    /// Get children (mutable)
    pub fn children_mut(&mut self) -> &mut [Box<dyn Widget>] {
        &mut self.children
    }

    /// Load layout properties from theme
    pub fn load_from_theme(&mut self, theme: &ThemeTree) {
        self.layout.orientation = theme.get_orientation(&self.name, Orientation::Vertical);
        self.layout.expand = theme.get_expand(&self.name, false);

        let spacing = theme.get_spacing(&self.name, crate::theme::types::Distance::px(0.0));
        self.layout.spacing = spacing.value as f32;

        // Load padding
        let default_padding = 0.0;
        self.layout.padding = (
            theme.get_number(&self.name, None, "padding-top", default_padding) as f32,
            theme.get_number(&self.name, None, "padding-right", default_padding) as f32,
            theme.get_number(&self.name, None, "padding-bottom", default_padding) as f32,
            theme.get_number(&self.name, None, "padding-left", default_padding) as f32,
        );

        // Load style
        self.style = ContainerStyle::from_theme(theme, &self.name, None);
    }

    /// Measure the container and its children
    pub fn measure(&self, constraints: Constraints, ctx: &LayoutContext) -> MeasuredSize {
        let (pad_t, pad_r, pad_b, pad_l) = self.layout.padding;
        let pad_h = pad_l + pad_r;
        let pad_v = pad_t + pad_b;

        // Adjust constraints for padding
        let inner_constraints = Constraints {
            min: Size::new(
                (constraints.min.width - pad_h).max(0.0),
                (constraints.min.height - pad_v).max(0.0),
            ),
            max: Size::new(
                (constraints.max.width - pad_h).max(0.0),
                (constraints.max.height - pad_v).max(0.0),
            ),
        };

        if self.children.is_empty() {
            return MeasuredSize::new(pad_h, pad_v);
        }

        let spacing = self.layout.spacing;
        let num_gaps = (self.children.len() - 1) as f32;
        let total_spacing = spacing * num_gaps;

        match self.layout.orientation {
            Orientation::Horizontal => {
                // Horizontal: children side by side
                let mut total_width = 0.0f32;
                let mut max_height = 0.0f32;

                for child in &self.children {
                    let child_size = child.measure(inner_constraints, ctx);
                    total_width += child_size.size.width;
                    max_height = max_height.max(child_size.size.height);
                }

                total_width += total_spacing;

                MeasuredSize::new(total_width + pad_h, max_height + pad_v)
            }
            Orientation::Vertical => {
                // Vertical: children stacked
                let mut max_width = 0.0f32;
                let mut total_height = 0.0f32;

                for child in &self.children {
                    let child_size = child.measure(inner_constraints, ctx);
                    max_width = max_width.max(child_size.size.width);
                    total_height += child_size.size.height;
                }

                total_height += total_spacing;

                MeasuredSize::new(max_width + pad_h, total_height + pad_v)
            }
        }
    }

    /// Arrange children within the given bounds
    pub fn arrange(&mut self, bounds: Rect, ctx: &LayoutContext) {
        let (pad_t, pad_r, pad_b, pad_l) = self.layout.padding;

        // Inner content area
        let content = Rect::new(
            bounds.x + pad_l,
            bounds.y + pad_t,
            bounds.width - pad_l - pad_r,
            bounds.height - pad_t - pad_b,
        );

        self.child_bounds.clear();

        if self.children.is_empty() {
            return;
        }

        let spacing = self.layout.spacing;

        // First pass: measure all children to determine sizes
        let constraints = Constraints::loose(Size::new(content.width, content.height));
        let measurements: Vec<MeasuredSize> = self
            .children
            .iter()
            .map(|c| c.measure(constraints, ctx))
            .collect();

        // Count expanding children
        let expanding_count = self
            .children
            .iter()
            .filter(|c| c.layout_props().expand)
            .count();

        match self.layout.orientation {
            Orientation::Horizontal => {
                let num_gaps = (self.children.len() - 1) as f32;
                let total_spacing = spacing * num_gaps;

                // Calculate total fixed width
                let fixed_width: f32 = measurements
                    .iter()
                    .zip(self.children.iter())
                    .filter(|(_, c)| !c.layout_props().expand)
                    .map(|(m, _)| m.size.width)
                    .sum();

                // Remaining space for expanding children
                let remaining = (content.width - fixed_width - total_spacing).max(0.0);
                let expand_width = if expanding_count > 0 {
                    remaining / expanding_count as f32
                } else {
                    0.0
                };

                // Position children
                let mut x = content.x;
                for (i, (child, measured)) in
                    self.children.iter().zip(measurements.iter()).enumerate()
                {
                    let width = if child.layout_props().expand {
                        expand_width
                    } else {
                        measured.size.width
                    };

                    self.child_bounds.push(ArrangedBounds::new(
                        x,
                        content.y,
                        width,
                        content.height,
                    ));

                    x += width;
                    if i < self.children.len() - 1 {
                        x += spacing;
                    }
                }
            }
            Orientation::Vertical => {
                let num_gaps = (self.children.len() - 1) as f32;
                let total_spacing = spacing * num_gaps;

                // Calculate total fixed height
                let fixed_height: f32 = measurements
                    .iter()
                    .zip(self.children.iter())
                    .filter(|(_, c)| !c.layout_props().expand)
                    .map(|(m, _)| m.size.height)
                    .sum();

                // Remaining space for expanding children
                let remaining = (content.height - fixed_height - total_spacing).max(0.0);
                let expand_height = if expanding_count > 0 {
                    remaining / expanding_count as f32
                } else {
                    0.0
                };

                // Position children
                let mut y = content.y;
                for (i, (child, measured)) in
                    self.children.iter().zip(measurements.iter()).enumerate()
                {
                    let height = if child.layout_props().expand {
                        expand_height
                    } else {
                        measured.size.height
                    };

                    self.child_bounds.push(ArrangedBounds::new(
                        content.x,
                        y,
                        content.width,
                        height,
                    ));

                    y += height;
                    if i < self.children.len() - 1 {
                        y += spacing;
                    }
                }
            }
        }

        // Recursively arrange children that are containers
        for (child, bounds) in self.children.iter_mut().zip(self.child_bounds.iter()) {
            child.arrange(bounds.rect, ctx);
        }
    }
}

impl Widget for Container {
    fn handle_event(&mut self, event: &Event, ctx: &LayoutContext) -> EventResult {
        // Propagate to children
        let mut result = EventResult::none();

        for child in &mut self.children {
            let child_result = child.handle_event(event, ctx);
            if child_result.consumed {
                result = child_result;
                break;
            }
            // Merge results
            result.needs_repaint |= child_result.needs_repaint;
            result.text_changed |= child_result.text_changed;
            result.submit |= child_result.submit;
            result.cancel |= child_result.cancel;
        }

        result
    }

    fn render(
        &self,
        renderer: &mut Renderer,
        rect: Rect,
        ctx: &LayoutContext,
    ) -> Result<(), windows::core::Error> {
        use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;

        let bounds = D2D_RECT_F {
            left: rect.x,
            top: rect.y,
            right: rect.x + rect.width,
            bottom: rect.y + rect.height,
        };

        // Draw background if not transparent
        if self.style.background_color.a > 0.0 {
            if self.style.border_radius > 0.0 {
                renderer.fill_rounded_rect(
                    bounds,
                    self.style.border_radius,
                    self.style.border_radius,
                    self.style.background_color,
                )?;
            } else {
                renderer.fill_rect(bounds, self.style.background_color)?;
            }
        }

        // Draw border if present
        if self.style.border_width > 0.0 && self.style.border_color.a > 0.0 {
            if self.style.border_radius > 0.0 {
                renderer.draw_rounded_rect(
                    bounds,
                    self.style.border_radius,
                    self.style.border_radius,
                    self.style.border_color,
                    self.style.border_width,
                )?;
            } else {
                renderer.draw_rect(bounds, self.style.border_color, self.style.border_width)?;
            }
        }

        // Render children at their arranged positions
        for (child, child_bounds) in self.children.iter().zip(self.child_bounds.iter()) {
            child.render(renderer, child_bounds.rect, ctx)?;
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
        // Containers don't use WidgetStyle directly, return default
        static DEFAULT: std::sync::OnceLock<WidgetStyle> = std::sync::OnceLock::new();
        DEFAULT.get_or_init(WidgetStyle::default)
    }

    fn set_style(&mut self, _style: WidgetStyle) {
        // Containers use ContainerStyle instead
    }

    fn measure(&self, constraints: Constraints, ctx: &LayoutContext) -> MeasuredSize {
        Container::measure(self, constraints, ctx)
    }

    fn arrange(&mut self, bounds: Rect, ctx: &LayoutContext) {
        Container::arrange(self, bounds, ctx)
    }

    fn layout_props(&self) -> &LayoutProps {
        &self.layout
    }

    fn widget_name(&self) -> &str {
        &self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_container_creation() {
        let container = Container::new("mainbox")
            .with_orientation(Orientation::Horizontal)
            .with_spacing(10.0);

        assert_eq!(container.name(), "mainbox");
        assert!(container.children().is_empty());
    }
}
