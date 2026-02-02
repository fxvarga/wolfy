//! ThemeResolver - CSS-like theme property resolution
//!
//! Resolves theme properties using CSS-like cascading and inheritance.

use crate::domain::entities::theme::{Theme, ThemeValue};
use crate::domain::value_objects::Color;

/// Service for resolving theme properties with inheritance
#[derive(Clone, Debug)]
pub struct ThemeResolver {
    /// Default property values
    defaults: Theme,
}

impl ThemeResolver {
    /// Create a new theme resolver with default values
    pub fn new() -> Self {
        let mut defaults = Theme::new("defaults");

        // Set default values
        defaults.set_property("*", "background-color", ThemeValue::Color(Color::BLACK));
        defaults.set_property("*", "text-color", ThemeValue::Color(Color::WHITE));
        defaults.set_property("*", "border-color", ThemeValue::Color(Color::TRANSPARENT));
        defaults.set_property("*", "border-radius", ThemeValue::Number(0.0));
        defaults.set_property("*", "font-size", ThemeValue::Number(14.0));
        defaults.set_property("*", "opacity", ThemeValue::Number(1.0));

        Self { defaults }
    }

    /// Resolve a property value for a selector with fallback chain
    pub fn resolve<'a>(&'a self, theme: &'a Theme, selector: &str, property: &str) -> Option<&'a ThemeValue> {
        // 1. Try exact selector match
        if let Some(value) = theme.get_property(selector, property) {
            return Some(value);
        }

        // 2. Try parent selectors (e.g., "window listview" -> "listview" -> "*")
        let parts: Vec<&str> = selector.split_whitespace().collect();
        for i in (0..parts.len()).rev() {
            let parent = parts[i..].join(" ");
            if let Some(value) = theme.get_property(&parent, property) {
                return Some(value);
            }
        }

        // 3. Try widget type only (last part of selector)
        if let Some(widget_type) = parts.last() {
            if let Some(value) = theme.get_property(widget_type, property) {
                return Some(value);
            }
        }

        // 4. Try wildcard selector
        if let Some(value) = theme.get_property("*", property) {
            return Some(value);
        }

        // 5. Try defaults
        if let Some(value) = self.defaults.get_property("*", property) {
            return Some(value);
        }

        None
    }

    /// Resolve a color property with fallback
    pub fn resolve_color(&self, theme: &Theme, selector: &str, property: &str, default: Color) -> Color {
        match self.resolve(theme, selector, property) {
            Some(ThemeValue::Color(c)) => *c,
            _ => default,
        }
    }

    /// Resolve a number property with fallback
    pub fn resolve_number(&self, theme: &Theme, selector: &str, property: &str, default: f64) -> f64 {
        match self.resolve(theme, selector, property) {
            Some(ThemeValue::Number(n)) => *n,
            _ => default,
        }
    }

    /// Resolve a string property with fallback
    pub fn resolve_string(&self, theme: &Theme, selector: &str, property: &str, default: &str) -> String {
        match self.resolve(theme, selector, property) {
            Some(ThemeValue::String(s)) => s.clone(),
            _ => default.to_string(),
        }
    }

    /// Resolve a boolean property with fallback
    pub fn resolve_bool(&self, theme: &Theme, selector: &str, property: &str, default: bool) -> bool {
        match self.resolve(theme, selector, property) {
            Some(ThemeValue::Boolean(b)) => *b,
            _ => default,
        }
    }

    /// Get all properties for a selector (merged from theme and defaults)
    pub fn get_all_properties(&self, theme: &Theme, selector: &str) -> Vec<(String, ThemeValue)> {
        let mut props = Vec::new();

        // Start with defaults
        if let Some(default_props) = self.get_selector_props(&self.defaults, "*") {
            props.extend(default_props);
        }

        // Add theme wildcard
        if let Some(theme_wild) = self.get_selector_props(theme, "*") {
            props.extend(theme_wild);
        }

        // Add widget type props
        let parts: Vec<&str> = selector.split_whitespace().collect();
        if let Some(widget_type) = parts.last() {
            if let Some(type_props) = self.get_selector_props(theme, widget_type) {
                props.extend(type_props);
            }
        }

        // Add exact selector props
        if let Some(exact_props) = self.get_selector_props(theme, selector) {
            props.extend(exact_props);
        }

        props
    }

    /// Helper to get properties from a specific selector
    fn get_selector_props(&self, theme: &Theme, selector: &str) -> Option<Vec<(String, ThemeValue)>> {
        // This would need access to theme internals - simplified for now
        // In a real implementation, Theme would expose this
        None
    }
}

impl Default for ThemeResolver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_exact_match() {
        let resolver = ThemeResolver::new();
        let mut theme = Theme::new("test");

        theme.set_property("window", "background-color", ThemeValue::Color(Color::BLUE));

        let color = resolver.resolve_color(&theme, "window", "background-color", Color::BLACK);
        assert_eq!(color, Color::BLUE);
    }

    #[test]
    fn test_resolve_fallback_to_default() {
        let resolver = ThemeResolver::new();
        let theme = Theme::new("test");

        // Property not set, should fall back to default
        let color = resolver.resolve_color(&theme, "window", "nonexistent", Color::RED);
        assert_eq!(color, Color::RED);
    }

    #[test]
    fn test_resolve_wildcard() {
        let resolver = ThemeResolver::new();
        let mut theme = Theme::new("test");

        theme.set_property("*", "font-size", ThemeValue::Number(16.0));

        let size = resolver.resolve_number(&theme, "textbox", "font-size", 12.0);
        assert_eq!(size, 16.0);
    }

    #[test]
    fn test_resolve_parent_selector() {
        let resolver = ThemeResolver::new();
        let mut theme = Theme::new("test");

        theme.set_property("listview", "text-color", ThemeValue::Color(Color::GREEN));

        // "window listview" should find "listview" property
        let color = resolver.resolve_color(&theme, "window listview", "text-color", Color::WHITE);
        assert_eq!(color, Color::GREEN);
    }
}
