//! Color value object - RGBA color representation
//!
//! Colors are immutable and use f32 values in the 0.0-1.0 range
//! for Direct2D compatibility.

use std::fmt;

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
    // Common color constants
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
    pub const YELLOW: Color = Color {
        r: 1.0,
        g: 1.0,
        b: 0.0,
        a: 1.0,
    };
    pub const CYAN: Color = Color {
        r: 0.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };
    pub const MAGENTA: Color = Color {
        r: 1.0,
        g: 0.0,
        b: 1.0,
        a: 1.0,
    };

    /// Create color from f32 values (0.0-1.0)
    pub fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self {
            r: r.clamp(0.0, 1.0),
            g: g.clamp(0.0, 1.0),
            b: b.clamp(0.0, 1.0),
            a: a.clamp(0.0, 1.0),
        }
    }

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
        Self::new(r, g, b, a)
    }

    /// Parse hex color string (#RGB, #RGBA, #RRGGBB, #RRGGBBAA)
    pub fn from_hex(hex: &str) -> Result<Self, ColorParseError> {
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
            _ => Err(ColorParseError::InvalidFormat(hex.to_string())),
        }
    }

    /// Convert to hex string (#RRGGBB or #RRGGBBAA if alpha < 1)
    pub fn to_hex(&self) -> String {
        let r = (self.r * 255.0).round() as u8;
        let g = (self.g * 255.0).round() as u8;
        let b = (self.b * 255.0).round() as u8;

        if (self.a - 1.0).abs() < 0.001 {
            format!("#{:02x}{:02x}{:02x}", r, g, b)
        } else {
            let a = (self.a * 255.0).round() as u8;
            format!("#{:02x}{:02x}{:02x}{:02x}", r, g, b, a)
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

    /// Create from packed u32 (ARGB format)
    pub fn from_u32(value: u32) -> Self {
        let a = ((value >> 24) & 0xFF) as f32 / 255.0;
        let r = ((value >> 16) & 0xFF) as f32 / 255.0;
        let g = ((value >> 8) & 0xFF) as f32 / 255.0;
        let b = (value & 0xFF) as f32 / 255.0;
        Self { r, g, b, a }
    }

    /// Blend this color with another using alpha
    pub fn blend(&self, other: &Color, alpha: f32) -> Self {
        let alpha = alpha.clamp(0.0, 1.0);
        Self {
            r: self.r + (other.r - self.r) * alpha,
            g: self.g + (other.g - self.g) * alpha,
            b: self.b + (other.b - self.b) * alpha,
            a: self.a + (other.a - self.a) * alpha,
        }
    }

    /// Create a color with modified alpha
    pub fn with_alpha(&self, alpha: f32) -> Self {
        Self {
            a: alpha.clamp(0.0, 1.0),
            ..*self
        }
    }

    /// Lighten the color by a factor (0.0-1.0)
    pub fn lighten(&self, factor: f32) -> Self {
        Self {
            r: (self.r + (1.0 - self.r) * factor).clamp(0.0, 1.0),
            g: (self.g + (1.0 - self.g) * factor).clamp(0.0, 1.0),
            b: (self.b + (1.0 - self.b) * factor).clamp(0.0, 1.0),
            a: self.a,
        }
    }

    /// Darken the color by a factor (0.0-1.0)
    pub fn darken(&self, factor: f32) -> Self {
        Self {
            r: (self.r * (1.0 - factor)).clamp(0.0, 1.0),
            g: (self.g * (1.0 - factor)).clamp(0.0, 1.0),
            b: (self.b * (1.0 - factor)).clamp(0.0, 1.0),
            a: self.a,
        }
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

/// Error parsing color from string
#[derive(Debug, Clone)]
pub enum ColorParseError {
    InvalidFormat(String),
    InvalidDigit(char),
}

impl fmt::Display for ColorParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ColorParseError::InvalidFormat(s) => write!(f, "Invalid color format: {}", s),
            ColorParseError::InvalidDigit(c) => write!(f, "Invalid hex digit: {}", c),
        }
    }
}

impl std::error::Error for ColorParseError {}

fn parse_hex_digit(c: char) -> Result<u8, ColorParseError> {
    match c.to_ascii_lowercase() {
        '0'..='9' => Ok(c as u8 - b'0'),
        'a'..='f' => Ok(c as u8 - b'a' + 10),
        _ => Err(ColorParseError::InvalidDigit(c)),
    }
}

fn parse_hex_byte(s: &str) -> Result<u8, ColorParseError> {
    u8::from_str_radix(s, 16).map_err(|_| ColorParseError::InvalidFormat(s.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_constants() {
        assert_eq!(Color::BLACK.r, 0.0);
        assert_eq!(Color::WHITE.r, 1.0);
        assert_eq!(Color::TRANSPARENT.a, 0.0);
    }

    #[test]
    fn test_color_rgb() {
        let c = Color::rgb(255, 128, 0);
        assert_eq!(c.r, 1.0);
        assert!((c.g - 0.502).abs() < 0.01);
        assert_eq!(c.b, 0.0);
        assert_eq!(c.a, 1.0);
    }

    #[test]
    fn test_color_from_hex_rgb() {
        let c = Color::from_hex("#ff0000").unwrap();
        assert_eq!(c.r, 1.0);
        assert_eq!(c.g, 0.0);
        assert_eq!(c.b, 0.0);
    }

    #[test]
    fn test_color_from_hex_short() {
        let c = Color::from_hex("#f00").unwrap();
        assert_eq!(c.r, 1.0);
        assert_eq!(c.g, 0.0);
        assert_eq!(c.b, 0.0);
    }

    #[test]
    fn test_color_from_hex_with_alpha() {
        let c = Color::from_hex("#ff000080").unwrap();
        assert_eq!(c.r, 1.0);
        assert!((c.a - 0.502).abs() < 0.01);
    }

    #[test]
    fn test_color_to_hex() {
        assert_eq!(Color::RED.to_hex(), "#ff0000");
        assert_eq!(Color::rgba(255, 0, 0, 128).to_hex(), "#ff000080");
    }

    #[test]
    fn test_color_blend() {
        let c1 = Color::BLACK;
        let c2 = Color::WHITE;
        let blended = c1.blend(&c2, 0.5);

        assert!((blended.r - 0.5).abs() < 0.001);
        assert!((blended.g - 0.5).abs() < 0.001);
        assert!((blended.b - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_color_with_alpha() {
        let c = Color::RED.with_alpha(0.5);
        assert_eq!(c.r, 1.0);
        assert_eq!(c.a, 0.5);
    }

    #[test]
    fn test_color_lighten_darken() {
        let c = Color::rgb(128, 128, 128);
        let lighter = c.lighten(0.5);
        let darker = c.darken(0.5);

        assert!(lighter.r > c.r);
        assert!(darker.r < c.r);
    }
}
