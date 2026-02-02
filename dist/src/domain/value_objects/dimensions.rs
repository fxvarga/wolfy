//! Dimension value objects - Size, Distance, Padding
//!
//! These represent measurements with optional units.

/// Distance unit types
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum DistanceUnit {
    /// Absolute pixels (DPI-scaled)
    #[default]
    Px,
    /// Relative to font size
    Em,
    /// Percentage of parent
    Percent,
    /// Physical millimeters
    Mm,
}

/// A distance value with unit
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Distance {
    pub value: f64,
    pub unit: DistanceUnit,
}

impl Distance {
    /// Create a zero distance
    pub const ZERO: Distance = Distance {
        value: 0.0,
        unit: DistanceUnit::Px,
    };

    /// Create distance in pixels
    pub fn px(value: f64) -> Self {
        Self {
            value,
            unit: DistanceUnit::Px,
        }
    }

    /// Create distance in ems
    pub fn em(value: f64) -> Self {
        Self {
            value,
            unit: DistanceUnit::Em,
        }
    }

    /// Create distance as percentage
    pub fn percent(value: f64) -> Self {
        Self {
            value,
            unit: DistanceUnit::Percent,
        }
    }

    /// Create distance in millimeters
    pub fn mm(value: f64) -> Self {
        Self {
            value,
            unit: DistanceUnit::Mm,
        }
    }

    /// Resolve to pixels given context
    pub fn to_px(&self, font_size: f64, parent_size: f64, dpi: f64) -> f64 {
        match self.unit {
            DistanceUnit::Px => self.value,
            DistanceUnit::Em => self.value * font_size,
            DistanceUnit::Percent => self.value * parent_size / 100.0,
            DistanceUnit::Mm => self.value * dpi / 25.4,
        }
    }

    /// Check if this is a zero distance
    pub fn is_zero(&self) -> bool {
        self.value == 0.0
    }
}

/// Size with width and height
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

impl Size {
    pub const ZERO: Size = Size {
        width: 0.0,
        height: 0.0,
    };

    /// Create a new size
    pub fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }

    /// Create a square size
    pub fn square(side: f32) -> Self {
        Self {
            width: side,
            height: side,
        }
    }

    /// Create an unconstrained size (infinity)
    pub fn unconstrained() -> Self {
        Self {
            width: f32::INFINITY,
            height: f32::INFINITY,
        }
    }

    /// Constrain this size to fit within max bounds
    pub fn constrain(&self, max: Size) -> Size {
        Size {
            width: self.width.min(max.width),
            height: self.height.min(max.height),
        }
    }

    /// Get area
    pub fn area(&self) -> f32 {
        self.width * self.height
    }

    /// Check if either dimension is zero
    pub fn is_empty(&self) -> bool {
        self.width <= 0.0 || self.height <= 0.0
    }
}

/// Padding/margin values for all four sides
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Padding {
    pub top: Distance,
    pub right: Distance,
    pub bottom: Distance,
    pub left: Distance,
}

impl Padding {
    /// Create zero padding
    pub fn zero() -> Self {
        Self::default()
    }

    /// Create uniform padding
    pub fn uniform(value: f64) -> Self {
        let d = Distance::px(value);
        Self {
            top: d.clone(),
            right: d.clone(),
            bottom: d.clone(),
            left: d,
        }
    }

    /// Create symmetric padding (vertical, horizontal)
    pub fn symmetric(vertical: f64, horizontal: f64) -> Self {
        Self {
            top: Distance::px(vertical),
            right: Distance::px(horizontal),
            bottom: Distance::px(vertical),
            left: Distance::px(horizontal),
        }
    }

    /// Create padding with all four values
    pub fn new(top: f64, right: f64, bottom: f64, left: f64) -> Self {
        Self {
            top: Distance::px(top),
            right: Distance::px(right),
            bottom: Distance::px(bottom),
            left: Distance::px(left),
        }
    }

    /// Get total horizontal padding
    pub fn horizontal(&self, font_size: f64, parent_width: f64, dpi: f64) -> f64 {
        self.left.to_px(font_size, parent_width, dpi)
            + self.right.to_px(font_size, parent_width, dpi)
    }

    /// Get total vertical padding
    pub fn vertical(&self, font_size: f64, parent_height: f64, dpi: f64) -> f64 {
        self.top.to_px(font_size, parent_height, dpi)
            + self.bottom.to_px(font_size, parent_height, dpi)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_distance_px() {
        let d = Distance::px(10.0);
        assert_eq!(d.to_px(16.0, 100.0, 96.0), 10.0);
    }

    #[test]
    fn test_distance_em() {
        let d = Distance::em(2.0);
        assert_eq!(d.to_px(16.0, 100.0, 96.0), 32.0);
    }

    #[test]
    fn test_distance_percent() {
        let d = Distance::percent(50.0);
        assert_eq!(d.to_px(16.0, 200.0, 96.0), 100.0);
    }

    #[test]
    fn test_size_constrain() {
        let s = Size::new(100.0, 100.0);
        let constrained = s.constrain(Size::new(50.0, 80.0));

        assert_eq!(constrained.width, 50.0);
        assert_eq!(constrained.height, 80.0);
    }

    #[test]
    fn test_padding_uniform() {
        let p = Padding::uniform(10.0);

        assert_eq!(p.top.value, 10.0);
        assert_eq!(p.right.value, 10.0);
        assert_eq!(p.bottom.value, 10.0);
        assert_eq!(p.left.value, 10.0);
    }

    #[test]
    fn test_padding_symmetric() {
        let p = Padding::symmetric(10.0, 20.0);

        assert_eq!(p.top.value, 10.0);
        assert_eq!(p.bottom.value, 10.0);
        assert_eq!(p.left.value, 20.0);
        assert_eq!(p.right.value, 20.0);
    }
}
