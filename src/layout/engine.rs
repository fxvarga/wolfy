//! Layout engine - builds widget trees from theme definitions

use std::collections::HashMap;

use crate::theme::tree::ThemeTree;
use crate::widget::{Container, Panel, Textbox, Widget};

/// Factory function type for creating widgets
pub type WidgetFactory = fn(&str, &ThemeTree) -> Box<dyn Widget>;

/// Registry of widget factories
pub struct WidgetRegistry {
    factories: HashMap<String, WidgetFactory>,
}

impl Default for WidgetRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl WidgetRegistry {
    /// Create a new registry with default widget factories
    pub fn new() -> Self {
        let mut registry = Self {
            factories: HashMap::new(),
        };

        // Register built-in widget types
        registry.register("textbox", |name, theme| {
            let style = crate::widget::WidgetStyle::from_theme_textbox(theme, None);
            Box::new(Textbox::new().with_style(style))
        });

        registry.register("inputbar", |name, theme| {
            // inputbar is typically a container for the textbox
            let mut container = Container::new(name);
            container.load_from_theme(theme);
            Box::new(container)
        });

        registry.register("panel", |name, theme| {
            let mut panel = Panel::new(name);
            panel.load_from_theme(theme);
            Box::new(panel)
        });

        // Generic container (box)
        registry.register("box", |name, theme| {
            let mut container = Container::new(name);
            container.load_from_theme(theme);
            Box::new(container)
        });

        registry
    }

    /// Register a widget factory
    pub fn register(&mut self, widget_type: &str, factory: WidgetFactory) {
        self.factories.insert(widget_type.to_string(), factory);
    }

    /// Create a widget by name, returns None if unknown type
    pub fn create(&self, name: &str, theme: &ThemeTree) -> Option<Box<dyn Widget>> {
        // First check if there's a specific factory for this name
        if let Some(factory) = self.factories.get(name) {
            return Some(factory(name, theme));
        }

        // Check if this is a container (has children property)
        let children = theme.get_children(name);
        if !children.is_empty() {
            // It's a container
            let mut container = Container::new(name);
            container.load_from_theme(theme);
            return Some(Box::new(container));
        }

        // Check if it has a background-image (likely a panel)
        if theme.get_image(name, None, "background-image").is_some() {
            let mut panel = Panel::new(name);
            panel.load_from_theme(theme);
            return Some(Box::new(panel));
        }

        // Default: create a panel (simple colored background)
        let mut panel = Panel::new(name);
        panel.load_from_theme(theme);
        Some(Box::new(panel))
    }
}

/// Layout engine that builds widget trees from themes
pub struct LayoutEngine {
    registry: WidgetRegistry,
}

impl Default for LayoutEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl LayoutEngine {
    /// Create a new layout engine
    pub fn new() -> Self {
        Self {
            registry: WidgetRegistry::new(),
        }
    }

    /// Create with a custom registry
    pub fn with_registry(registry: WidgetRegistry) -> Self {
        Self { registry }
    }

    /// Build a widget tree from a theme, starting at the given root widget
    pub fn build_widget_tree(&self, theme: &ThemeTree, root_name: &str) -> Option<Box<dyn Widget>> {
        self.build_widget_recursive(theme, root_name)
    }

    /// Recursively build widget and its children
    fn build_widget_recursive(&self, theme: &ThemeTree, name: &str) -> Option<Box<dyn Widget>> {
        // Get children names from theme
        let children_names = theme.get_children(name);

        if children_names.is_empty() {
            // Leaf widget - just create it
            self.registry.create(name, theme)
        } else {
            // Container widget - create and add children
            let mut container = Container::new(name);
            container.load_from_theme(theme);

            for child_name in children_names {
                if let Some(child) = self.build_widget_recursive(theme, &child_name) {
                    container.add_child(child);
                } else {
                    crate::log!("Warning: Failed to create child widget '{}'", child_name);
                }
            }

            Some(Box::new(container))
        }
    }

    /// Get the registry for modification
    pub fn registry_mut(&mut self) -> &mut WidgetRegistry {
        &mut self.registry
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_creation() {
        let registry = WidgetRegistry::new();
        assert!(registry.factories.contains_key("textbox"));
        assert!(registry.factories.contains_key("panel"));
    }

    #[test]
    fn test_build_simple_tree() {
        let theme = ThemeTree::parse(
            r#"
            mainbox {
                orientation: horizontal;
                children: [ "panel-left", "panel-right" ];
            }
            
            panel-left {
                background-color: #ff0000;
                expand: true;
            }
            
            panel-right {
                background-color: #00ff00;
                expand: true;
            }
        "#,
        )
        .unwrap();

        let engine = LayoutEngine::new();
        let tree = engine.build_widget_tree(&theme, "mainbox");

        assert!(tree.is_some());
        let root = tree.unwrap();
        assert_eq!(root.widget_name(), "mainbox");
    }

    #[test]
    fn test_build_nested_tree() {
        let theme = ThemeTree::parse(
            r#"
            mainbox {
                orientation: horizontal;
                children: [ "wallpaper-panel", "listbox" ];
            }
            
            wallpaper-panel {
                background-image: url("auto", width);
                expand: true;
            }
            
            listbox {
                orientation: vertical;
                children: [ "inputbar", "listview" ];
                expand: true;
            }
            
            inputbar {
                background-color: #2d2d2d;
            }
            
            listview {
                expand: true;
            }
        "#,
        )
        .unwrap();

        let engine = LayoutEngine::new();
        let tree = engine.build_widget_tree(&theme, "mainbox");

        assert!(tree.is_some());
    }
}
