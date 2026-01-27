//! Base widget types and traits for the layout system

use crate::theme::types::{LayoutContext, Orientation, Rect};

/// Size constraint for layout calculations
#[derive(Clone, Copy, Debug, Default)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

impl Size {
    pub const ZERO: Size = Size {
        width: 0.0,
        height: 0.0,
    };

    pub fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }

    /// Create a size with unconstrained dimensions (infinity)
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
}

/// Constraints for layout measurement
#[derive(Clone, Copy, Debug)]
pub struct Constraints {
    /// Minimum allowed size
    pub min: Size,
    /// Maximum allowed size
    pub max: Size,
}

impl Default for Constraints {
    fn default() -> Self {
        Self {
            min: Size::ZERO,
            max: Size::unconstrained(),
        }
    }
}

impl Constraints {
    /// Create tight constraints (exactly this size)
    pub fn tight(size: Size) -> Self {
        Self {
            min: size,
            max: size,
        }
    }

    /// Create loose constraints (0 to max)
    pub fn loose(max: Size) -> Self {
        Self {
            min: Size::ZERO,
            max,
        }
    }

    /// Create constraints with only max width bounded
    pub fn max_width(width: f32) -> Self {
        Self {
            min: Size::ZERO,
            max: Size::new(width, f32::INFINITY),
        }
    }

    /// Create constraints with only max height bounded
    pub fn max_height(height: f32) -> Self {
        Self {
            min: Size::ZERO,
            max: Size::new(f32::INFINITY, height),
        }
    }

    /// Constrain a size to these constraints
    pub fn constrain(&self, size: Size) -> Size {
        Size {
            width: size.width.max(self.min.width).min(self.max.width),
            height: size.height.max(self.min.height).min(self.max.height),
        }
    }

    /// Get the tightest size that satisfies these constraints
    pub fn smallest(&self) -> Size {
        self.min
    }

    /// Get the largest size that satisfies these constraints
    pub fn biggest(&self) -> Size {
        Size {
            width: if self.max.width.is_infinite() {
                0.0
            } else {
                self.max.width
            },
            height: if self.max.height.is_infinite() {
                0.0
            } else {
                self.max.height
            },
        }
    }
}

/// Layout properties extracted from theme
#[derive(Clone, Debug, Default)]
pub struct LayoutProps {
    /// Orientation for containers (horizontal/vertical)
    pub orientation: Orientation,
    /// Whether widget should expand to fill available space
    pub expand: bool,
    /// Spacing between children (for containers)
    pub spacing: f32,
    /// Fixed width (None = auto)
    pub fixed_width: Option<f32>,
    /// Fixed height (None = auto)
    pub fixed_height: Option<f32>,
    /// Padding (top, right, bottom, left)
    pub padding: (f32, f32, f32, f32),
}

impl LayoutProps {
    pub fn padding_horizontal(&self) -> f32 {
        self.padding.1 + self.padding.3
    }

    pub fn padding_vertical(&self) -> f32 {
        self.padding.0 + self.padding.2
    }
}

/// Measured size from a widget
#[derive(Clone, Copy, Debug, Default)]
pub struct MeasuredSize {
    /// Desired size
    pub size: Size,
    /// Baseline for text alignment (if applicable)
    pub baseline: Option<f32>,
}

impl MeasuredSize {
    pub fn new(width: f32, height: f32) -> Self {
        Self {
            size: Size::new(width, height),
            baseline: None,
        }
    }

    pub fn with_baseline(mut self, baseline: f32) -> Self {
        self.baseline = Some(baseline);
        self
    }
}

/// Arranged position and size for a widget
#[derive(Clone, Copy, Debug, Default)]
pub struct ArrangedBounds {
    pub rect: Rect,
}

impl ArrangedBounds {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            rect: Rect::new(x, y, width, height),
        }
    }

    pub fn from_rect(rect: Rect) -> Self {
        Self { rect }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constraints_constrain() {
        let constraints = Constraints {
            min: Size::new(10.0, 10.0),
            max: Size::new(100.0, 100.0),
        };

        // Within bounds
        let size = constraints.constrain(Size::new(50.0, 50.0));
        assert_eq!(size.width, 50.0);
        assert_eq!(size.height, 50.0);

        // Below minimum
        let size = constraints.constrain(Size::new(5.0, 5.0));
        assert_eq!(size.width, 10.0);
        assert_eq!(size.height, 10.0);

        // Above maximum
        let size = constraints.constrain(Size::new(200.0, 200.0));
        assert_eq!(size.width, 100.0);
        assert_eq!(size.height, 100.0);
    }

    #[test]
    fn test_layout_props_padding() {
        let props = LayoutProps {
            padding: (10.0, 20.0, 10.0, 20.0),
            ..Default::default()
        };

        assert_eq!(props.padding_horizontal(), 40.0);
        assert_eq!(props.padding_vertical(), 20.0);
    }
}
