//! Rect value object - Rectangle representation
//!
//! Rectangles are used for layout bounds and hit testing.

use super::dimensions::Size;

/// A rectangle defined by its bounds
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Rect {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl Rect {
    /// Create a new rectangle from bounds
    pub fn new(left: f32, top: f32, right: f32, bottom: f32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    /// Create a rectangle from position and size
    pub fn from_pos_size(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            left: x,
            top: y,
            right: x + width,
            bottom: y + height,
        }
    }

    /// Create a zero rectangle at origin
    pub fn zero() -> Self {
        Self::default()
    }

    /// Get width
    pub fn width(&self) -> f32 {
        self.right - self.left
    }

    /// Get height
    pub fn height(&self) -> f32 {
        self.bottom - self.top
    }

    /// Get size
    pub fn size(&self) -> Size {
        Size::new(self.width(), self.height())
    }

    /// Get center X coordinate
    pub fn center_x(&self) -> f32 {
        (self.left + self.right) / 2.0
    }

    /// Get center Y coordinate
    pub fn center_y(&self) -> f32 {
        (self.top + self.bottom) / 2.0
    }

    /// Get center point
    pub fn center(&self) -> (f32, f32) {
        (self.center_x(), self.center_y())
    }

    /// Check if a point is inside this rectangle
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.left && x < self.right && y >= self.top && y < self.bottom
    }

    /// Check if this rectangle intersects another
    pub fn intersects(&self, other: &Rect) -> bool {
        self.left < other.right
            && self.right > other.left
            && self.top < other.bottom
            && self.bottom > other.top
    }

    /// Get intersection with another rectangle
    pub fn intersection(&self, other: &Rect) -> Option<Rect> {
        let left = self.left.max(other.left);
        let top = self.top.max(other.top);
        let right = self.right.min(other.right);
        let bottom = self.bottom.min(other.bottom);

        if left < right && top < bottom {
            Some(Rect::new(left, top, right, bottom))
        } else {
            None
        }
    }

    /// Get union (bounding box) with another rectangle
    pub fn union(&self, other: &Rect) -> Rect {
        Rect::new(
            self.left.min(other.left),
            self.top.min(other.top),
            self.right.max(other.right),
            self.bottom.max(other.bottom),
        )
    }

    /// Inset rectangle by given amounts
    pub fn inset(&self, left: f32, top: f32, right: f32, bottom: f32) -> Rect {
        Rect::new(
            self.left + left,
            self.top + top,
            self.right - right,
            self.bottom - bottom,
        )
    }

    /// Inset rectangle uniformly
    pub fn inset_uniform(&self, amount: f32) -> Rect {
        self.inset(amount, amount, amount, amount)
    }

    /// Expand rectangle by given amounts
    pub fn expand(&self, left: f32, top: f32, right: f32, bottom: f32) -> Rect {
        Rect::new(
            self.left - left,
            self.top - top,
            self.right + right,
            self.bottom + bottom,
        )
    }

    /// Expand rectangle uniformly
    pub fn expand_uniform(&self, amount: f32) -> Rect {
        self.expand(amount, amount, amount, amount)
    }

    /// Translate rectangle by offset
    pub fn translate(&self, dx: f32, dy: f32) -> Rect {
        Rect::new(
            self.left + dx,
            self.top + dy,
            self.right + dx,
            self.bottom + dy,
        )
    }

    /// Check if rectangle is empty (zero or negative area)
    pub fn is_empty(&self) -> bool {
        self.width() <= 0.0 || self.height() <= 0.0
    }

    /// Get area
    pub fn area(&self) -> f32 {
        self.width() * self.height()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rect_dimensions() {
        let r = Rect::new(10.0, 20.0, 110.0, 70.0);

        assert_eq!(r.width(), 100.0);
        assert_eq!(r.height(), 50.0);
        assert_eq!(r.area(), 5000.0);
    }

    #[test]
    fn test_rect_from_pos_size() {
        let r = Rect::from_pos_size(10.0, 20.0, 100.0, 50.0);

        assert_eq!(r.left, 10.0);
        assert_eq!(r.top, 20.0);
        assert_eq!(r.right, 110.0);
        assert_eq!(r.bottom, 70.0);
    }

    #[test]
    fn test_rect_contains() {
        let r = Rect::new(0.0, 0.0, 100.0, 100.0);

        assert!(r.contains(50.0, 50.0));
        assert!(r.contains(0.0, 0.0));
        assert!(!r.contains(100.0, 100.0)); // Exclusive bounds
        assert!(!r.contains(-1.0, 50.0));
    }

    #[test]
    fn test_rect_intersects() {
        let r1 = Rect::new(0.0, 0.0, 100.0, 100.0);
        let r2 = Rect::new(50.0, 50.0, 150.0, 150.0);
        let r3 = Rect::new(200.0, 200.0, 300.0, 300.0);

        assert!(r1.intersects(&r2));
        assert!(!r1.intersects(&r3));
    }

    #[test]
    fn test_rect_intersection() {
        let r1 = Rect::new(0.0, 0.0, 100.0, 100.0);
        let r2 = Rect::new(50.0, 50.0, 150.0, 150.0);

        let i = r1.intersection(&r2).unwrap();

        assert_eq!(i.left, 50.0);
        assert_eq!(i.top, 50.0);
        assert_eq!(i.right, 100.0);
        assert_eq!(i.bottom, 100.0);
    }

    #[test]
    fn test_rect_inset() {
        let r = Rect::new(0.0, 0.0, 100.0, 100.0);
        let inset = r.inset_uniform(10.0);

        assert_eq!(inset.left, 10.0);
        assert_eq!(inset.top, 10.0);
        assert_eq!(inset.right, 90.0);
        assert_eq!(inset.bottom, 90.0);
    }

    #[test]
    fn test_rect_translate() {
        let r = Rect::new(0.0, 0.0, 100.0, 100.0);
        let moved = r.translate(50.0, 25.0);

        assert_eq!(moved.left, 50.0);
        assert_eq!(moved.top, 25.0);
        assert_eq!(moved.right, 150.0);
        assert_eq!(moved.bottom, 125.0);
    }

    #[test]
    fn test_rect_center() {
        let r = Rect::new(0.0, 0.0, 100.0, 50.0);

        assert_eq!(r.center(), (50.0, 25.0));
    }
}
