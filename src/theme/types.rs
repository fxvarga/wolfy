//! Core theme types: Color, Distance, Padding, Border

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ParseError {
    #[error("Invalid hex color: {0}")]
    InvalidHexColor(String),
    #[error("Invalid number: {0}")]
    InvalidNumber(String),
    #[error("Unknown unit: {0}")]
    UnknownUnit(String),
}

/// RGBA color (0.0-1.0 range for D2D compatibility)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Default for Color {
    fn default() -> Self {
        Self::BLACK
    }
}

impl Color {
    pub const BLACK: Color = Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    pub const WHITE: Color = Color {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };
    pub const TRANSPARENT: Color = Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    };
    pub const RED: Color = Color {
        r: 1.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    pub const GREEN: Color = Color {
        r: 0.0,
        g: 1.0,
        b: 0.0,
        a: 1.0,
    };
    pub const BLUE: Color = Color {
        r: 0.0,
        g: 0.0,
        b: 1.0,
        a: 1.0,
    };

    /// Create color from RGB values (0-255)
    pub fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self {
            r: r as f32 / 255.0,
            g: g as f32 / 255.0,
            b: b as f32 / 255.0,
            a: 1.0,
        }
    }

    /// Create color from RGBA values (0-255)
    pub fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self {
            r: r as f32 / 255.0,
            g: g as f32 / 255.0,
            b: b as f32 / 255.0,
            a: a as f32 / 255.0,
        }
    }

    /// Create color from f32 values (0.0-1.0)
    pub fn from_f32(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// Parse hex color string (#RGB, #RGBA, #RRGGBB, #RRGGBBAA)
    pub fn from_hex(hex: &str) -> Result<Self, ParseError> {
        let hex = hex.trim_start_matches('#');

        match hex.len() {
            3 => {
                // #RGB -> #RRGGBB
                let r = parse_hex_digit(hex.chars().nth(0).unwrap())? * 17;
                let g = parse_hex_digit(hex.chars().nth(1).unwrap())? * 17;
                let b = parse_hex_digit(hex.chars().nth(2).unwrap())? * 17;
                Ok(Self::rgb(r, g, b))
            }
            4 => {
                // #RGBA -> #RRGGBBAA
                let r = parse_hex_digit(hex.chars().nth(0).unwrap())? * 17;
                let g = parse_hex_digit(hex.chars().nth(1).unwrap())? * 17;
                let b = parse_hex_digit(hex.chars().nth(2).unwrap())? * 17;
                let a = parse_hex_digit(hex.chars().nth(3).unwrap())? * 17;
                Ok(Self::rgba(r, g, b, a))
            }
            6 => {
                // #RRGGBB
                let r = parse_hex_byte(&hex[0..2])?;
                let g = parse_hex_byte(&hex[2..4])?;
                let b = parse_hex_byte(&hex[4..6])?;
                Ok(Self::rgb(r, g, b))
            }
            8 => {
                // #RRGGBBAA
                let r = parse_hex_byte(&hex[0..2])?;
                let g = parse_hex_byte(&hex[2..4])?;
                let b = parse_hex_byte(&hex[4..6])?;
                let a = parse_hex_byte(&hex[6..8])?;
                Ok(Self::rgba(r, g, b, a))
            }
            _ => Err(ParseError::InvalidHexColor(hex.to_string())),
        }
    }

    /// Convert to packed u32 (ARGB format)
    pub fn to_u32(&self) -> u32 {
        let r = (self.r * 255.0) as u32;
        let g = (self.g * 255.0) as u32;
        let b = (self.b * 255.0) as u32;
        let a = (self.a * 255.0) as u32;
        (a << 24) | (r << 16) | (g << 8) | b
    }
}

fn parse_hex_digit(c: char) -> Result<u8, ParseError> {
    match c.to_ascii_lowercase() {
        '0'..='9' => Ok(c as u8 - b'0'),
        'a'..='f' => Ok(c as u8 - b'a' + 10),
        _ => Err(ParseError::InvalidHexColor(c.to_string())),
    }
}

fn parse_hex_byte(s: &str) -> Result<u8, ParseError> {
    u8::from_str_radix(s, 16).map_err(|_| ParseError::InvalidHexColor(s.to_string()))
}

/// Distance unit types
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum DistanceUnit {
    #[default]
    Px, // Absolute pixels (DPI-scaled)
    Em,      // Relative to font size
    Percent, // Percentage of parent
    Mm,      // Physical millimeters
}

/// A distance value with unit
#[derive(Clone, Debug, PartialEq)]
pub struct Distance {
    pub value: f64,
    pub unit: DistanceUnit,
}

impl Default for Distance {
    fn default() -> Self {
        Self::px(0.0)
    }
}

impl Distance {
    pub fn px(value: f64) -> Self {
        Self {
            value,
            unit: DistanceUnit::Px,
        }
    }

    pub fn em(value: f64) -> Self {
        Self {
            value,
            unit: DistanceUnit::Em,
        }
    }

    pub fn percent(value: f64) -> Self {
        Self {
            value,
            unit: DistanceUnit::Percent,
        }
    }

    pub fn mm(value: f64) -> Self {
        Self {
            value,
            unit: DistanceUnit::Mm,
        }
    }

    /// Resolve to physical pixels given context
    pub fn to_pixels(&self, ctx: &LayoutContext) -> f32 {
        match self.unit {
            DistanceUnit::Px => (self.value as f32) * ctx.scale_factor,
            DistanceUnit::Em => (self.value as f32) * ctx.base_font_size * ctx.scale_factor,
            DistanceUnit::Percent => (self.value as f32 / 100.0) * ctx.parent_size,
            DistanceUnit::Mm => (self.value as f32) * ctx.dpi / 25.4,
        }
    }
}

/// Layout context for resolving distances
#[derive(Clone, Debug)]
pub struct LayoutContext {
    pub dpi: f32,
    pub scale_factor: f32,
    pub base_font_size: f32,
    pub parent_size: f32, // Width or height depending on orientation
}

impl Default for LayoutContext {
    fn default() -> Self {
        Self {
            dpi: 96.0,
            scale_factor: 1.0,
            base_font_size: 16.0,
            parent_size: 100.0,
        }
    }
}

/// Four-sided padding/margin
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Padding {
    pub top: Distance,
    pub right: Distance,
    pub bottom: Distance,
    pub left: Distance,
}

impl Padding {
    /// Create uniform padding on all sides
    pub fn uniform(d: Distance) -> Self {
        Self {
            top: d.clone(),
            right: d.clone(),
            bottom: d.clone(),
            left: d,
        }
    }

    /// Create from vertical and horizontal values
    pub fn symmetric(vertical: Distance, horizontal: Distance) -> Self {
        Self {
            top: vertical.clone(),
            bottom: vertical,
            left: horizontal.clone(),
            right: horizontal,
        }
    }

    /// Create from all four values
    pub fn new(top: Distance, right: Distance, bottom: Distance, left: Distance) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }

    /// Resolve all sides to pixels
    pub fn to_pixels(&self, ctx: &LayoutContext) -> ResolvedPadding {
        ResolvedPadding {
            top: self.top.to_pixels(ctx),
            right: self.right.to_pixels(ctx),
            bottom: self.bottom.to_pixels(ctx),
            left: self.left.to_pixels(ctx),
        }
    }
}

/// Resolved padding in pixels
#[derive(Clone, Copy, Debug, Default)]
pub struct ResolvedPadding {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

/// Border specification
#[derive(Clone, Debug)]
pub struct Border {
    pub width: Distance,
    pub color: Color,
    pub radius: Distance,
    pub style: LineStyle,
}

impl Default for Border {
    fn default() -> Self {
        Self {
            width: Distance::px(0.0),
            color: Color::TRANSPARENT,
            radius: Distance::px(0.0),
            style: LineStyle::Solid,
        }
    }
}

/// Line style for borders
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum LineStyle {
    #[default]
    Solid,
    Dashed,
}

/// Layout orientation for containers
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Orientation {
    #[default]
    Vertical,
    Horizontal,
}

impl Orientation {
    /// Parse from string (used in theme)
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "horizontal" => Some(Orientation::Horizontal),
            "vertical" => Some(Orientation::Vertical),
            _ => None,
        }
    }
}

/// Image scaling mode for background images
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ImageScale {
    /// No scaling, display at native size
    #[default]
    None,
    /// Scale to fit width, maintain aspect ratio
    Width,
    /// Scale to fit height, maintain aspect ratio  
    Height,
    /// Scale to fill both dimensions (may crop)
    Both,
}

impl ImageScale {
    /// Parse from string (used in theme)
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "none" => Some(ImageScale::None),
            "width" => Some(ImageScale::Width),
            "height" => Some(ImageScale::Height),
            "both" => Some(ImageScale::Both),
            _ => None,
        }
    }
}

/// Image source for background-image property
#[derive(Clone, Debug, PartialEq)]
pub struct ImageSource {
    /// Path to the image (can be "auto" for wallpaper detection)
    pub path: String,
    /// How to scale the image
    pub scale: ImageScale,
}

/// Rectangle for layout
#[derive(Clone, Copy, Debug, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Create a zero-sized rect at origin
    pub fn zero() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        }
    }

    /// Shrink rect by padding amounts
    pub fn inset(&self, padding: &ResolvedPadding) -> Self {
        Self {
            x: self.x + padding.left,
            y: self.y + padding.top,
            width: self.width - padding.left - padding.right,
            height: self.height - padding.top - padding.bottom,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_hex_parsing() {
        // 3-digit
        let c = Color::from_hex("#fff").unwrap();
        assert_eq!(c, Color::WHITE);

        // 6-digit
        let c = Color::from_hex("#ff0000").unwrap();
        assert_eq!(c.r, 1.0);
        assert_eq!(c.g, 0.0);
        assert_eq!(c.b, 0.0);

        // 8-digit with alpha
        let c = Color::from_hex("#ff000080").unwrap();
        assert!((c.a - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_distance_to_pixels() {
        let ctx = LayoutContext {
            dpi: 96.0,
            scale_factor: 1.5,
            base_font_size: 16.0,
            parent_size: 200.0,
        };

        // Px scales with scale_factor
        assert_eq!(Distance::px(10.0).to_pixels(&ctx), 15.0);

        // Em is relative to font size
        assert_eq!(Distance::em(1.0).to_pixels(&ctx), 24.0); // 16 * 1.5

        // Percent is relative to parent
        assert_eq!(Distance::percent(50.0).to_pixels(&ctx), 100.0);
    }
}
