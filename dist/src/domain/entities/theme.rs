//! Theme entity - represents a UI theme configuration
//!
//! This is a domain representation of a theme, independent of
//! the parsing format (rasi, css, etc).

use std::collections::HashMap;

use crate::domain::value_objects::{Color, Distance, Padding};

/// A resolved theme configuration
#[derive(Clone, Debug)]
pub struct Theme {
    /// Theme name
    pub name: String,
    /// Theme properties organized by selector
    properties: HashMap<String, HashMap<String, ThemeValue>>,
}

/// A resolved theme value
#[derive(Clone, Debug, PartialEq)]
pub enum ThemeValue {
    /// Color value
    Color(Color),
    /// Distance value (with unit)
    Distance(Distance),
    /// Padding/margin (4 distances)
    Padding(Padding),
    /// String value
    String(String),
    /// Number value
    Number(f64),
    /// Boolean value
    Boolean(bool),
    /// List of values
    List(Vec<ThemeValue>),
    /// Reference to another property
    Reference(String),
    /// Inherited from parent
    Inherit,
}

impl Theme {
    /// Create a new empty theme
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            properties: HashMap::new(),
        }
    }

    /// Set a property value for a selector
    pub fn set_property(&mut self, selector: &str, property: &str, value: ThemeValue) {
        self.properties
            .entry(selector.to_string())
            .or_default()
            .insert(property.to_string(), value);
    }

    /// Get a property value for a selector
    pub fn get_property(&self, selector: &str, property: &str) -> Option<&ThemeValue> {
        self.properties
            .get(selector)
            .and_then(|props| props.get(property))
    }

    /// Get a color property with fallback
    pub fn get_color(&self, selector: &str, property: &str, default: Color) -> Color {
        match self.get_property(selector, property) {
            Some(ThemeValue::Color(c)) => *c,
            _ => default,
        }
    }

    /// Get a distance property with fallback
    pub fn get_distance(&self, selector: &str, property: &str, default: Distance) -> Distance {
        match self.get_property(selector, property) {
            Some(ThemeValue::Distance(d)) => d.clone(),
            _ => default,
        }
    }

    /// Get a number property with fallback
    pub fn get_number(&self, selector: &str, property: &str, default: f64) -> f64 {
        match self.get_property(selector, property) {
            Some(ThemeValue::Number(n)) => *n,
            _ => default,
        }
    }

    /// Get a string property with fallback
    pub fn get_string(&self, selector: &str, property: &str, default: &str) -> String {
        match self.get_property(selector, property) {
            Some(ThemeValue::String(s)) => s.clone(),
            _ => default.to_string(),
        }
    }

    /// Get a boolean property with fallback
    pub fn get_bool(&self, selector: &str, property: &str, default: bool) -> bool {
        match self.get_property(selector, property) {
            Some(ThemeValue::Boolean(b)) => *b,
            _ => default,
        }
    }

    /// Merge another theme into this one (other takes precedence)
    pub fn merge(&mut self, other: &Theme) {
        for (selector, props) in &other.properties {
            let entry = self.properties.entry(selector.clone()).or_default();
            for (key, value) in props {
                entry.insert(key.clone(), value.clone());
            }
        }
    }

    /// Get all selectors defined in this theme
    pub fn selectors(&self) -> impl Iterator<Item = &String> {
        self.properties.keys()
    }

    /// Check if a selector exists
    pub fn has_selector(&self, selector: &str) -> bool {
        self.properties.contains_key(selector)
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::new("default")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_theme_creation() {
        let theme = Theme::new("dark");
        assert_eq!(theme.name, "dark");
    }

    #[test]
    fn test_theme_properties() {
        let mut theme = Theme::new("test");

        theme.set_property("window", "background-color", ThemeValue::Color(Color::BLACK));
        theme.set_property("window", "width", ThemeValue::Number(800.0));

        assert_eq!(theme.get_color("window", "background-color", Color::WHITE), Color::BLACK);
        assert_eq!(theme.get_number("window", "width", 0.0), 800.0);
    }

    #[test]
    fn test_theme_fallbacks() {
        let theme = Theme::new("test");

        // Non-existent properties should return defaults
        assert_eq!(theme.get_color("window", "bg", Color::RED), Color::RED);
        assert_eq!(theme.get_number("window", "size", 42.0), 42.0);
        assert_eq!(theme.get_string("window", "font", "Arial"), "Arial");
    }

    #[test]
    fn test_theme_merge() {
        let mut base = Theme::new("base");
        base.set_property("window", "width", ThemeValue::Number(800.0));
        base.set_property("window", "height", ThemeValue::Number(600.0));

        let mut override_theme = Theme::new("override");
        override_theme.set_property("window", "width", ThemeValue::Number(1024.0));

        base.merge(&override_theme);

        assert_eq!(base.get_number("window", "width", 0.0), 1024.0);
        assert_eq!(base.get_number("window", "height", 0.0), 600.0);
    }
}
