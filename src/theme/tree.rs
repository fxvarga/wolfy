//! Theme tree with property resolution and inheritance

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::theme::ast::{Property, Rule, Selector, Stylesheet, Value};
use crate::theme::lexer::Lexer;
use crate::theme::types::{Color, Distance, Padding};

// Import the generated parser
use crate::theme::theme_parser;

/// Error types for theme operations
#[derive(Debug)]
pub enum ThemeError {
    IoError(std::io::Error),
    ParseError(String),
}

impl std::fmt::Display for ThemeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ThemeError::IoError(e) => write!(f, "IO error: {}", e),
            ThemeError::ParseError(e) => write!(f, "Parse error: {}", e),
        }
    }
}

impl std::error::Error for ThemeError {}

impl From<std::io::Error> for ThemeError {
    fn from(e: std::io::Error) -> Self {
        ThemeError::IoError(e)
    }
}

/// A node in the theme tree representing a widget's styling
#[derive(Debug, Clone, Default)]
pub struct ThemeNode {
    /// Properties for the base state
    pub properties: HashMap<String, Value>,
    /// Properties for specific states (e.g., "focused", "selected")
    pub states: HashMap<String, HashMap<String, Value>>,
}

impl ThemeNode {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a property for the base state
    pub fn set(&mut self, name: String, value: Value) {
        self.properties.insert(name, value);
    }

    /// Set a property for a specific state
    pub fn set_state(&mut self, state: &str, name: String, value: Value) {
        self.states
            .entry(state.to_string())
            .or_default()
            .insert(name, value);
    }

    /// Get a property value, checking state first, then base
    pub fn get(&self, name: &str, state: Option<&str>) -> Option<&Value> {
        // First check state-specific properties
        if let Some(state) = state {
            if let Some(state_props) = self.states.get(state) {
                if let Some(value) = state_props.get(name) {
                    return Some(value);
                }
            }
        }
        // Fall back to base properties
        self.properties.get(name)
    }
}

/// The complete theme tree with property resolution
#[derive(Debug, Default)]
pub struct ThemeTree {
    /// Global properties (from * selector)
    pub globals: HashMap<String, Value>,
    /// Named widget nodes
    pub widgets: HashMap<String, ThemeNode>,
}

impl ThemeTree {
    /// Create an empty theme tree
    pub fn new() -> Self {
        Self::default()
    }

    /// Parse a theme from a string
    pub fn parse(input: &str) -> Result<Self, ThemeError> {
        let lexer = Lexer::new(input);
        let stylesheet = theme_parser::StylesheetParser::new()
            .parse(lexer)
            .map_err(|e| ThemeError::ParseError(format!("{:?}", e)))?;

        let tree = Self::from_stylesheet(stylesheet);

        // Debug: log what we parsed (only in non-test builds to avoid issues)
        #[cfg(not(test))]
        {
            crate::log!(
                "Theme parsed: {} globals, {} widgets",
                tree.globals.len(),
                tree.widgets.len()
            );
            for (name, node) in &tree.widgets {
                crate::log!("  Widget '{}': {} properties", name, node.properties.len());
                for (prop, val) in &node.properties {
                    crate::log!("    {}: {:?}", prop, val);
                }
            }
        }

        Ok(tree)
    }

    /// Load a theme from a file
    pub fn load(path: &Path) -> Result<Self, ThemeError> {
        let content = fs::read_to_string(path)?;
        Self::parse(&content)
    }

    /// Build theme tree from parsed stylesheet
    pub fn from_stylesheet(stylesheet: Stylesheet) -> Self {
        let mut tree = Self::new();

        for rule in stylesheet.rules {
            for selector in &rule.selectors {
                match selector {
                    Selector::Universal => {
                        // Add to globals
                        for prop in &rule.properties {
                            tree.globals.insert(prop.name.clone(), prop.value.clone());
                        }
                    }
                    Selector::Element { name, state } => {
                        let node = tree
                            .widgets
                            .entry(name.clone())
                            .or_insert_with(ThemeNode::new);

                        for prop in &rule.properties {
                            match state {
                                Some(s) => node.set_state(s, prop.name.clone(), prop.value.clone()),
                                None => node.set(prop.name.clone(), prop.value.clone()),
                            }
                        }
                    }
                }
            }
        }

        tree
    }

    /// Get a value with inheritance: widget.state -> widget -> globals
    pub fn get_value(&self, widget: &str, state: Option<&str>, property: &str) -> Option<&Value> {
        // First, try widget-specific properties
        if let Some(node) = self.widgets.get(widget) {
            if let Some(value) = node.get(property, state) {
                // Check for "inherit" keyword
                if let Value::Ident(s) = value {
                    if s == "inherit" {
                        // Fall through to globals
                        return self.globals.get(property);
                    }
                }
                return Some(value);
            }
        }
        // Fall back to globals
        self.globals.get(property)
    }

    /// Get a color property with default
    pub fn get_color(
        &self,
        widget: &str,
        state: Option<&str>,
        property: &str,
        default: Color,
    ) -> Color {
        self.get_value(widget, state, property)
            .and_then(|v| v.as_color())
            .unwrap_or(default)
    }

    /// Get a distance property with default
    pub fn get_distance(
        &self,
        widget: &str,
        state: Option<&str>,
        property: &str,
        default: Distance,
    ) -> Distance {
        self.get_value(widget, state, property)
            .and_then(|v| v.as_distance())
            .unwrap_or(default)
    }

    /// Get a padding property with default
    pub fn get_padding(
        &self,
        widget: &str,
        state: Option<&str>,
        property: &str,
        default: Padding,
    ) -> Padding {
        self.get_value(widget, state, property)
            .and_then(|v| v.as_padding())
            .unwrap_or(default)
    }

    /// Get a string property with default
    pub fn get_string(
        &self,
        widget: &str,
        state: Option<&str>,
        property: &str,
        default: &str,
    ) -> String {
        self.get_value(widget, state, property)
            .and_then(|v| v.as_string())
            .unwrap_or_else(|| default.to_string())
    }

    /// Get a number property with default
    pub fn get_number(
        &self,
        widget: &str,
        state: Option<&str>,
        property: &str,
        default: f64,
    ) -> f64 {
        self.get_value(widget, state, property)
            .and_then(|v| v.as_number())
            .unwrap_or(default)
    }

    /// Get a boolean property with default
    pub fn get_bool(
        &self,
        widget: &str,
        state: Option<&str>,
        property: &str,
        default: bool,
    ) -> bool {
        self.get_value(widget, state, property)
            .and_then(|v| v.as_bool())
            .unwrap_or(default)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_theme() {
        let theme = ThemeTree::parse(
            r#"
            * {
                background-color: #1a1a2e;
                text-color: white;
            }
            
            textbox {
                padding: 10px;
                border-radius: 4px;
            }
            
            textbox.focused {
                border-color: #e94560;
            }
        "#,
        )
        .unwrap();

        // Check globals
        assert!(theme.globals.contains_key("background-color"));

        // Check widget
        assert!(theme.widgets.contains_key("textbox"));

        // Check state
        let textbox = theme.widgets.get("textbox").unwrap();
        assert!(textbox.states.contains_key("focused"));
    }

    #[test]
    fn test_property_resolution() {
        let theme = ThemeTree::parse(
            r#"
            * {
                text-color: white;
            }
            
            entry {
                text-color: #ff0000;
            }
            
            entry.focused {
                text-color: #00ff00;
            }
        "#,
        )
        .unwrap();

        // Global fallback
        let c = theme.get_color("unknown", None, "text-color", Color::BLACK);
        assert_eq!(c, Color::WHITE);

        // Widget override
        let c = theme.get_color("entry", None, "text-color", Color::BLACK);
        assert_eq!(c.r, 1.0);
        assert_eq!(c.g, 0.0);

        // State override
        let c = theme.get_color("entry", Some("focused"), "text-color", Color::BLACK);
        assert_eq!(c.r, 0.0);
        assert_eq!(c.g, 1.0);
    }

    #[test]
    fn test_plain_numbers() {
        // Test parsing plain numbers without units (like font-size: 24)
        let theme = ThemeTree::parse(
            r#"
            textbox {
                font-size: 24;
                border-width: 1;
                border-radius: 4.5;
            }
        "#,
        )
        .unwrap();

        // Check that numbers are parsed correctly
        let font_size = theme.get_number("textbox", None, "font-size", 16.0);
        assert_eq!(font_size, 24.0);

        let border_width = theme.get_number("textbox", None, "border-width", 0.0);
        assert_eq!(border_width, 1.0);

        let border_radius = theme.get_number("textbox", None, "border-radius", 0.0);
        assert_eq!(border_radius, 4.5);
    }

    #[test]
    fn test_default_rasi_format() {
        // Test with the exact format from default.rasi
        let theme = ThemeTree::parse(
            r#"
/* Wolfy Default Theme */

* {
    background-color: #1e1e1e;
    text-color: #d4d4d4;
}

textbox {
    background-color: #2d2d2d;
    text-color: #ffffff;
    border-width: 1;
    border-color: #3c3c3c;
    border-radius: 4;
    padding-top: 8;
    padding-right: 12;
    padding-bottom: 8;
    padding-left: 12;
    font-size: 24;
    placeholder-color: #808080;
    cursor-color: #ffffff;
    selection-color: #264f78;
}

textbox.focused {
    border-color: #007acc;
}
        "#,
        )
        .unwrap();

        // Check that font-size is parsed
        let font_size = theme.get_number("textbox", None, "font-size", 16.0);
        assert_eq!(font_size, 24.0, "font-size should be 24, got {}", font_size);

        // Check colors
        let bg = theme.get_color("textbox", None, "background-color", Color::BLACK);
        // #2d2d2d = rgb(45, 45, 45)
        assert!(
            (bg.r - 45.0 / 255.0).abs() < 0.01,
            "background-color red should be ~0.176"
        );

        // Check focused border color
        let focused_border =
            theme.get_color("textbox", Some("focused"), "border-color", Color::BLACK);
        // #007acc = rgb(0, 122, 204)
        assert!(
            focused_border.r < 0.01,
            "focused border-color red should be 0"
        );
        assert!(
            (focused_border.b - 204.0 / 255.0).abs() < 0.01,
            "focused border-color blue should be ~0.8"
        );
    }
}
