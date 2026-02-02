//! Widget factory - creates widgets dynamically from theme definitions
//!
//! Following rofi's pattern:
//! - Widget names/prefixes determine widget type
//! - Theme's `children` property defines the widget tree
//! - Known containers have default children if not specified

use crate::platform::win32::Renderer;
use crate::theme::tree::ThemeTree;
use crate::theme::types::{LayoutContext, Orientation, Rect};

use super::container::Container;
use super::listview::ListView;
use super::panel::Panel;
use super::textbox::Textbox;
use super::{ElementStyle, EventResult, ListViewStyle, Widget, WidgetStyle};
use crate::platform::Event;

/// Widget types that can be created
#[derive(Clone, Debug, PartialEq)]
pub enum WidgetType {
    /// Container (box) - horizontal or vertical layout
    Container,
    /// Panel - background with color/image
    Panel,
    /// Textbox - text input
    Textbox,
    /// ListView - scrollable list of elements  
    ListView,
    /// Dummy - spacer widget
    Dummy,
}

/// Default children for known container widgets
fn default_children(name: &str) -> Option<Vec<&'static str>> {
    match name {
        "window" => Some(vec!["mainbox"]),
        "mainbox" => Some(vec!["wallpaper-panel", "listbox"]),
        "listbox" => Some(vec!["dummy", "listview", "dummy"]),
        "inputbar" => Some(vec!["textbox"]),
        _ => None,
    }
}

/// Determine widget type from name (rofi-style prefix matching)
fn widget_type_from_name(name: &str) -> WidgetType {
    // Known special widgets
    match name {
        "listview" => return WidgetType::ListView,
        "textbox" | "entry" | "prompt" => return WidgetType::Textbox,
        "dummy" => return WidgetType::Dummy,
        _ => {}
    }

    // Prefix matching (rofi pattern)
    if name.starts_with("textbox") {
        WidgetType::Textbox
    } else if name.starts_with("listview") {
        WidgetType::ListView
    } else if name.ends_with("-panel") || name == "wallpaper-panel" {
        // Panels are for backgrounds (wallpaper, images)
        WidgetType::Panel
    } else {
        // Default: container (box)
        WidgetType::Container
    }
}

/// A node in the widget tree
pub struct WidgetNode {
    /// Widget name (for theme lookups)
    pub name: String,
    /// The actual widget
    pub widget: Box<dyn Widget>,
    /// Child nodes
    pub children: Vec<WidgetNode>,
    /// Cached bounds from last layout
    pub bounds: Option<Rect>,
}

impl WidgetNode {
    /// Create a new widget node
    pub fn new(name: impl Into<String>, widget: Box<dyn Widget>) -> Self {
        Self {
            name: name.into(),
            widget,
            children: Vec::new(),
            bounds: None,
        }
    }

    /// Add a child node
    pub fn add_child(&mut self, child: WidgetNode) {
        self.children.push(child);
    }

    /// Get a child by name (recursive)
    pub fn find_child(&self, name: &str) -> Option<&WidgetNode> {
        if self.name == name {
            return Some(self);
        }
        for child in &self.children {
            if let Some(found) = child.find_child(name) {
                return Some(found);
            }
        }
        None
    }

    /// Get a mutable child by name (recursive)
    pub fn find_child_mut(&mut self, name: &str) -> Option<&mut WidgetNode> {
        if self.name == name {
            return Some(self);
        }
        for child in &mut self.children {
            if let Some(found) = child.find_child_mut(name) {
                return Some(found);
            }
        }
        None
    }
}

/// Factory context for creating widgets
pub struct WidgetFactory<'a> {
    theme: &'a ThemeTree,
    /// Shared state that special widgets need access to
    listview_style: ListViewStyle,
    element_style: ElementStyle,
    textbox_style: WidgetStyle,
}

impl<'a> WidgetFactory<'a> {
    /// Create a new widget factory
    pub fn new(theme: &'a ThemeTree) -> Self {
        Self {
            theme,
            listview_style: ListViewStyle::from_theme(theme, None),
            element_style: ElementStyle::from_theme(theme, None),
            textbox_style: WidgetStyle::from_theme_textbox(theme, None),
        }
    }

    /// Build the complete widget tree starting from "window"
    pub fn build_tree(&self) -> WidgetNode {
        self.build_widget("window")
    }

    /// Build a widget and its children recursively
    fn build_widget(&self, name: &str) -> WidgetNode {
        let widget_type = widget_type_from_name(name);
        crate::log!("WidgetFactory: building '{}' as {:?}", name, widget_type);

        // Create the widget based on type
        let widget: Box<dyn Widget> = match widget_type {
            WidgetType::Container => {
                let mut container = Container::new(name);
                container.load_from_theme(self.theme);
                Box::new(container)
            }
            WidgetType::Panel => {
                let mut panel = Panel::new(name);
                panel.load_from_theme(self.theme);
                Box::new(panel)
            }
            WidgetType::Textbox => {
                let textbox = Textbox::new().with_style(self.textbox_style.clone());
                Box::new(textbox)
            }
            WidgetType::ListView => {
                let listview = ListView::new()
                    .with_style(self.listview_style.clone())
                    .with_element_style(self.element_style.clone());
                Box::new(listview)
            }
            WidgetType::Dummy => {
                // Dummy is just an expanding panel with no background
                let mut panel = Panel::new(name);
                panel.load_from_theme(self.theme);
                Box::new(panel)
            }
        };

        let mut node = WidgetNode::new(name, widget);

        // Get children from theme, or use defaults
        let children_names = self.theme.get_children(name);
        let children: Vec<String> = if children_names.is_empty() {
            // Use defaults if available
            default_children(name)
                .map(|v| v.into_iter().map(String::from).collect())
                .unwrap_or_default()
        } else {
            children_names
        };

        crate::log!("  '{}' children: {:?}", name, children);

        // Recursively build children (but not for leaf widgets)
        match widget_type {
            WidgetType::Container => {
                for child_name in children {
                    let child_node = self.build_widget(&child_name);
                    node.add_child(child_node);
                }
            }
            _ => {
                // Leaf widgets don't have children
            }
        }

        node
    }
}

/// The complete UI tree with layout and rendering
pub struct UITree {
    /// Root widget node
    root: WidgetNode,
    /// Cached layout context
    layout_ctx: LayoutContext,
}

impl UITree {
    /// Create a new UI tree from theme
    pub fn from_theme(theme: &ThemeTree, layout_ctx: LayoutContext) -> Self {
        let factory = WidgetFactory::new(theme);
        let root = factory.build_tree();
        Self { root, layout_ctx }
    }

    /// Rebuild the tree from theme (for hot-reload)
    pub fn rebuild(&mut self, theme: &ThemeTree) {
        let factory = WidgetFactory::new(theme);
        self.root = factory.build_tree();
    }

    /// Update the layout context
    pub fn set_layout_ctx(&mut self, ctx: LayoutContext) {
        self.layout_ctx = ctx;
    }

    /// Get a reference to a widget by name
    pub fn find_widget(&self, name: &str) -> Option<&WidgetNode> {
        self.root.find_child(name)
    }

    /// Get a mutable reference to a widget by name
    pub fn find_widget_mut(&mut self, name: &str) -> Option<&mut WidgetNode> {
        self.root.find_child_mut(name)
    }

    /// Handle an event, propagating through the tree
    pub fn handle_event(&mut self, event: &Event) -> EventResult {
        Self::handle_event_recursive(&mut self.root, event, &self.layout_ctx)
    }

    fn handle_event_recursive(
        node: &mut WidgetNode,
        event: &Event,
        layout_ctx: &LayoutContext,
    ) -> EventResult {
        // First propagate to children
        let mut result = EventResult::none();
        for child in &mut node.children {
            let child_result = Self::handle_event_recursive(child, event, layout_ctx);
            if child_result.consumed {
                return child_result;
            }
            result.needs_repaint |= child_result.needs_repaint;
            result.text_changed |= child_result.text_changed;
            result.submit |= child_result.submit;
            result.cancel |= child_result.cancel;
        }

        // Then let the widget handle it
        let widget_result = node.widget.handle_event(event, layout_ctx);
        result.needs_repaint |= widget_result.needs_repaint;
        result.consumed |= widget_result.consumed;
        result.text_changed |= widget_result.text_changed;
        result.submit |= widget_result.submit;
        result.cancel |= widget_result.cancel;

        result
    }

    /// Layout the tree within the given bounds
    pub fn layout(&mut self, bounds: Rect) {
        Self::layout_recursive(&mut self.root, bounds, &self.layout_ctx);
    }

    fn layout_recursive(node: &mut WidgetNode, bounds: Rect, layout_ctx: &LayoutContext) {
        node.bounds = Some(bounds);

        // Let the widget arrange itself and determine child bounds
        node.widget.arrange(bounds, layout_ctx);

        // For containers, we need to calculate child bounds based on orientation
        if !node.children.is_empty() {
            let child_bounds = Self::calculate_child_bounds(node, bounds);
            for (child, child_rect) in node.children.iter_mut().zip(child_bounds) {
                Self::layout_recursive(child, child_rect, layout_ctx);
            }
        }
    }

    /// Calculate bounds for each child of a container
    fn calculate_child_bounds(node: &WidgetNode, bounds: Rect) -> Vec<Rect> {
        let props = node.widget.layout_props();
        let (pad_t, pad_r, pad_b, pad_l) = props.padding;
        let spacing = props.spacing;

        // Content area (inside padding)
        let content = Rect::new(
            bounds.x + pad_l,
            bounds.y + pad_t,
            bounds.width - pad_l - pad_r,
            bounds.height - pad_t - pad_b,
        );

        if node.children.is_empty() {
            return vec![];
        }

        let num_children = node.children.len();
        let num_gaps = (num_children.saturating_sub(1)) as f32;
        let total_spacing = spacing * num_gaps;

        // Count expanding vs fixed children
        let mut expanding_count = 0;
        let mut fixed_size = 0.0;

        for child in &node.children {
            let child_props = child.widget.layout_props();
            if child_props.expand {
                expanding_count += 1;
            } else {
                // Get fixed size from layout props
                match props.orientation {
                    Orientation::Horizontal => {
                        fixed_size += child_props.fixed_width.unwrap_or(0.0);
                    }
                    Orientation::Vertical => {
                        fixed_size += child_props.fixed_height.unwrap_or(0.0);
                    }
                }
            }
        }

        // Calculate size for expanding children
        let available = match props.orientation {
            Orientation::Horizontal => content.width - fixed_size - total_spacing,
            Orientation::Vertical => content.height - fixed_size - total_spacing,
        };
        let expand_size = if expanding_count > 0 {
            available / expanding_count as f32
        } else {
            0.0
        };

        // Generate bounds for each child
        let mut result = Vec::with_capacity(num_children);
        let mut pos = match props.orientation {
            Orientation::Horizontal => content.x,
            Orientation::Vertical => content.y,
        };

        for (i, child) in node.children.iter().enumerate() {
            let child_props = child.widget.layout_props();
            let is_expand = child_props.expand;

            let child_rect = match props.orientation {
                Orientation::Horizontal => {
                    let width = if is_expand {
                        expand_size
                    } else {
                        child_props.fixed_width.unwrap_or(expand_size)
                    };
                    let rect = Rect::new(pos, content.y, width, content.height);
                    pos += width;
                    rect
                }
                Orientation::Vertical => {
                    let height = if is_expand {
                        expand_size
                    } else {
                        child_props.fixed_height.unwrap_or(expand_size)
                    };
                    let rect = Rect::new(content.x, pos, content.width, height);
                    pos += height;
                    rect
                }
            };

            result.push(child_rect);

            // Add spacing (except after last child)
            if i < num_children - 1 {
                pos += spacing;
            }
        }

        result
    }

    /// Render the tree
    pub fn render(&self, renderer: &mut Renderer) -> Result<(), windows::core::Error> {
        self.render_recursive(&self.root, renderer)
    }

    fn render_recursive(
        &self,
        node: &WidgetNode,
        renderer: &mut Renderer,
    ) -> Result<(), windows::core::Error> {
        // Render this widget at its bounds
        if let Some(bounds) = node.bounds {
            node.widget.render(renderer, bounds, &self.layout_ctx)?;
        }

        // Render children
        for child in &node.children {
            self.render_recursive(child, renderer)?;
        }

        Ok(())
    }

    /// Get the root node
    pub fn root(&self) -> &WidgetNode {
        &self.root
    }

    /// Get mutable root node
    pub fn root_mut(&mut self) -> &mut WidgetNode {
        &mut self.root
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_widget_type_from_name() {
        assert_eq!(widget_type_from_name("listview"), WidgetType::ListView);
        assert_eq!(widget_type_from_name("textbox"), WidgetType::Textbox);
        assert_eq!(widget_type_from_name("textbox-custom"), WidgetType::Textbox);
        assert_eq!(widget_type_from_name("wallpaper-panel"), WidgetType::Panel);
        assert_eq!(widget_type_from_name("image-panel"), WidgetType::Panel);
        assert_eq!(widget_type_from_name("mainbox"), WidgetType::Container);
        assert_eq!(widget_type_from_name("listbox"), WidgetType::Container);
        assert_eq!(widget_type_from_name("dummy"), WidgetType::Dummy);
    }

    #[test]
    fn test_default_children() {
        assert_eq!(default_children("window"), Some(vec!["mainbox"]));
        assert_eq!(
            default_children("mainbox"),
            Some(vec!["wallpaper-panel", "listbox"])
        );
        assert_eq!(default_children("unknown"), None);
    }
}
