//! Clock widget for displaying time on the wallpaper panel

use crate::theme::types::Color;

/// Clock position on the wallpaper panel (3x3 grid)
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ClockPosition {
    TopLeft,
    TopCenter,
    #[default]
    TopRight,
    MiddleLeft,
    MiddleCenter,
    MiddleRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

impl ClockPosition {
    /// Parse position from string (e.g., "top-right", "middle-center")
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "top-left" => ClockPosition::TopLeft,
            "top-center" => ClockPosition::TopCenter,
            "top-right" => ClockPosition::TopRight,
            "middle-left" => ClockPosition::MiddleLeft,
            "middle-center" | "center" => ClockPosition::MiddleCenter,
            "middle-right" => ClockPosition::MiddleRight,
            "bottom-left" => ClockPosition::BottomLeft,
            "bottom-center" => ClockPosition::BottomCenter,
            "bottom-right" => ClockPosition::BottomRight,
            _ => ClockPosition::TopRight, // Default
        }
    }

    /// Get horizontal alignment factor (0.0 = left, 0.5 = center, 1.0 = right)
    pub fn horizontal_align(&self) -> f32 {
        match self {
            ClockPosition::TopLeft | ClockPosition::MiddleLeft | ClockPosition::BottomLeft => 0.0,
            ClockPosition::TopCenter
            | ClockPosition::MiddleCenter
            | ClockPosition::BottomCenter => 0.5,
            ClockPosition::TopRight | ClockPosition::MiddleRight | ClockPosition::BottomRight => {
                1.0
            }
        }
    }

    /// Get vertical alignment factor (0.0 = top, 0.5 = middle, 1.0 = bottom)
    pub fn vertical_align(&self) -> f32 {
        match self {
            ClockPosition::TopLeft | ClockPosition::TopCenter | ClockPosition::TopRight => 0.0,
            ClockPosition::MiddleLeft
            | ClockPosition::MiddleCenter
            | ClockPosition::MiddleRight => 0.5,
            ClockPosition::BottomLeft
            | ClockPosition::BottomCenter
            | ClockPosition::BottomRight => 1.0,
        }
    }
}

/// Clock configuration loaded from theme
#[derive(Clone, Debug)]
pub struct ClockConfig {
    /// Whether the clock is enabled
    pub enabled: bool,
    /// Position on the wallpaper panel
    pub position: ClockPosition,
    /// Time format string (strftime format, e.g., "%H:%M:%S" or "%I:%M %p")
    pub time_format: String,
    /// Date format string (strftime format, empty = no date)
    pub date_format: String,
    /// Font family for the clock
    pub font_family: String,
    /// Font size for the time display
    pub font_size: f32,
    /// Font size for the date display
    pub date_font_size: f32,
    /// Text color
    pub text_color: Color,
    /// Shadow color
    pub shadow_color: Color,
    /// Shadow offset (x, y)
    pub shadow_offset: (f32, f32),
    /// Padding from panel edges
    pub padding: f32,
}

impl Default for ClockConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            position: ClockPosition::TopRight,
            time_format: "%H:%M:%S".to_string(),
            date_format: "%a, %b %d".to_string(),
            font_family: "Segoe UI".to_string(),
            font_size: 72.0,
            date_font_size: 24.0,
            text_color: Color::WHITE,
            shadow_color: Color::from_f32(0.0, 0.0, 0.0, 0.5),
            shadow_offset: (3.0, 3.0),
            padding: 24.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clock_position_from_str() {
        assert_eq!(ClockPosition::from_str("top-left"), ClockPosition::TopLeft);
        assert_eq!(
            ClockPosition::from_str("top-center"),
            ClockPosition::TopCenter
        );
        assert_eq!(
            ClockPosition::from_str("top-right"),
            ClockPosition::TopRight
        );
        assert_eq!(
            ClockPosition::from_str("middle-left"),
            ClockPosition::MiddleLeft
        );
        assert_eq!(
            ClockPosition::from_str("middle-center"),
            ClockPosition::MiddleCenter
        );
        assert_eq!(
            ClockPosition::from_str("center"),
            ClockPosition::MiddleCenter
        );
        assert_eq!(
            ClockPosition::from_str("middle-right"),
            ClockPosition::MiddleRight
        );
        assert_eq!(
            ClockPosition::from_str("bottom-left"),
            ClockPosition::BottomLeft
        );
        assert_eq!(
            ClockPosition::from_str("bottom-center"),
            ClockPosition::BottomCenter
        );
        assert_eq!(
            ClockPosition::from_str("bottom-right"),
            ClockPosition::BottomRight
        );
        // Invalid defaults to TopRight
        assert_eq!(ClockPosition::from_str("invalid"), ClockPosition::TopRight);
    }

    #[test]
    fn test_clock_position_alignment() {
        assert_eq!(ClockPosition::TopLeft.horizontal_align(), 0.0);
        assert_eq!(ClockPosition::TopLeft.vertical_align(), 0.0);

        assert_eq!(ClockPosition::MiddleCenter.horizontal_align(), 0.5);
        assert_eq!(ClockPosition::MiddleCenter.vertical_align(), 0.5);

        assert_eq!(ClockPosition::BottomRight.horizontal_align(), 1.0);
        assert_eq!(ClockPosition::BottomRight.vertical_align(), 1.0);
    }

    #[test]
    fn test_clock_config_default() {
        let config = ClockConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.position, ClockPosition::TopRight);
        assert_eq!(config.time_format, "%H:%M:%S");
        assert_eq!(config.font_size, 72.0);
    }
}
