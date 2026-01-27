//! AST types for the theme parser

use crate::theme::types::{
    Color, Distance, DistanceUnit, ImageScale, ImageSource, Orientation, Padding,
};

/// A complete stylesheet
#[derive(Debug, Clone)]
pub struct Stylesheet {
    pub rules: Vec<Rule>,
}

/// A single rule: selector(s) + properties
#[derive(Debug, Clone)]
pub struct Rule {
    pub selectors: Vec<Selector>,
    pub properties: Vec<Property>,
}

/// Widget selector
#[derive(Debug, Clone, PartialEq)]
pub enum Selector {
    /// Universal selector: *
    Universal,
    /// Element selector with optional state: `textbox` or `textbox.focused`
    Element { name: String, state: Option<String> },
}

impl Selector {
    pub fn element(name: impl Into<String>) -> Self {
        Selector::Element {
            name: name.into(),
            state: None,
        }
    }

    pub fn element_with_state(name: impl Into<String>, state: impl Into<String>) -> Self {
        Selector::Element {
            name: name.into(),
            state: Some(state.into()),
        }
    }
}

/// A property: name-value pair
#[derive(Debug, Clone)]
pub struct Property {
    pub name: String,
    pub value: Value,
}

/// Property value
#[derive(Debug, Clone)]
pub enum Value {
    /// Color value
    Color(Color),
    /// Distance with unit
    Distance(Distance),
    /// Plain number (no unit)
    Number(f64),
    /// Quoted string
    String(String),
    /// Identifier (e.g., `inherit`, `bold`, color names)
    Ident(String),
    /// Boolean
    Boolean(bool),
    /// Padding shorthand (2 values: vertical horizontal)
    Padding2(Distance, Distance),
    /// Padding shorthand (4 values: top right bottom left)
    Padding4(Distance, Distance, Distance, Distance),
    /// Array of strings (for children property)
    Array(Vec<String>),
    /// Image source with scaling (for background-image)
    Image(ImageSource),
    /// Orientation (horizontal/vertical)
    Orientation(Orientation),
}

impl Value {
    /// Try to convert to Color
    pub fn as_color(&self) -> Option<Color> {
        match self {
            Value::Color(c) => Some(*c),
            Value::Ident(name) => named_color(name),
            _ => None,
        }
    }

    /// Try to convert to Distance
    pub fn as_distance(&self) -> Option<Distance> {
        match self {
            Value::Distance(d) => Some(d.clone()),
            Value::Number(n) => Some(Distance::px(*n)), // Default to px
            _ => None,
        }
    }

    /// Try to convert to Padding
    pub fn as_padding(&self) -> Option<Padding> {
        match self {
            Value::Distance(d) => Some(Padding::uniform(d.clone())),
            Value::Number(n) => Some(Padding::uniform(Distance::px(*n))),
            Value::Padding2(v, h) => Some(Padding::symmetric(v.clone(), h.clone())),
            Value::Padding4(t, r, b, l) => {
                Some(Padding::new(t.clone(), r.clone(), b.clone(), l.clone()))
            }
            _ => None,
        }
    }

    /// Try to convert to String
    pub fn as_string(&self) -> Option<String> {
        match self {
            Value::String(s) => Some(s.clone()),
            Value::Ident(s) => Some(s.clone()),
            _ => None,
        }
    }

    /// Try to convert to f64
    pub fn as_number(&self) -> Option<f64> {
        match self {
            Value::Number(n) => Some(*n),
            Value::Distance(d) => Some(d.value),
            _ => None,
        }
    }

    /// Try to convert to bool
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Boolean(b) => Some(*b),
            Value::Ident(s) if s == "true" => Some(true),
            Value::Ident(s) if s == "false" => Some(false),
            _ => None,
        }
    }

    /// Try to convert to array of strings
    pub fn as_array(&self) -> Option<&[String]> {
        match self {
            Value::Array(arr) => Some(arr),
            _ => None,
        }
    }

    /// Try to convert to ImageSource
    pub fn as_image(&self) -> Option<&ImageSource> {
        match self {
            Value::Image(img) => Some(img),
            _ => None,
        }
    }

    /// Try to convert to Orientation
    pub fn as_orientation(&self) -> Option<Orientation> {
        match self {
            Value::Orientation(o) => Some(*o),
            Value::Ident(s) => Orientation::from_str(s),
            _ => None,
        }
    }
}

/// Get color from CSS color name
fn named_color(name: &str) -> Option<Color> {
    match name.to_lowercase().as_str() {
        "black" => Some(Color::BLACK),
        "white" => Some(Color::WHITE),
        "red" => Some(Color::RED),
        "green" => Some(Color::GREEN),
        "blue" => Some(Color::BLUE),
        "transparent" => Some(Color::TRANSPARENT),

        // Extended colors
        "gray" | "grey" => Some(Color::rgb(128, 128, 128)),
        "silver" => Some(Color::rgb(192, 192, 192)),
        "maroon" => Some(Color::rgb(128, 0, 0)),
        "yellow" => Some(Color::rgb(255, 255, 0)),
        "olive" => Some(Color::rgb(128, 128, 0)),
        "lime" => Some(Color::rgb(0, 255, 0)),
        "aqua" | "cyan" => Some(Color::rgb(0, 255, 255)),
        "teal" => Some(Color::rgb(0, 128, 128)),
        "navy" => Some(Color::rgb(0, 0, 128)),
        "fuchsia" | "magenta" => Some(Color::rgb(255, 0, 255)),
        "purple" => Some(Color::rgb(128, 0, 128)),
        "orange" => Some(Color::rgb(255, 165, 0)),
        "pink" => Some(Color::rgb(255, 192, 203)),
        "brown" => Some(Color::rgb(165, 42, 42)),

        _ => None,
    }
}

/// Helper to create a Distance from number and unit token
pub fn make_distance(value: f64, unit: &str) -> Distance {
    match unit {
        "px" => Distance::px(value),
        "em" => Distance::em(value),
        "%" => Distance::percent(value),
        "mm" => Distance::mm(value),
        _ => Distance::px(value), // Default fallback
    }
}
