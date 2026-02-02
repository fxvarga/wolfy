//! Animation system with bezier easing curves
//!
//! Provides smooth animations for window transitions with configurable easing.

use std::time::Instant;

/// Bezier easing function type
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Easing {
    /// Linear interpolation (no easing)
    Linear,
    /// Ease in (slow start)
    EaseIn,
    /// Ease out (slow end)
    EaseOut,
    /// Ease in and out (slow start and end)
    EaseInOut,
    /// Custom cubic bezier curve (x1, y1, x2, y2)
    CubicBezier(f32, f32, f32, f32),
}

impl Default for Easing {
    fn default() -> Self {
        Easing::EaseOut
    }
}

impl Easing {
    /// Standard easing presets (CSS-like)
    pub const EASE: Easing = Easing::CubicBezier(0.25, 0.1, 0.25, 1.0);
    pub const EASE_IN: Easing = Easing::CubicBezier(0.42, 0.0, 1.0, 1.0);
    pub const EASE_OUT: Easing = Easing::CubicBezier(0.0, 0.0, 0.58, 1.0);
    pub const EASE_IN_OUT: Easing = Easing::CubicBezier(0.42, 0.0, 0.58, 1.0);

    /// Smooth/gentle easing (good for fade-in)
    pub const EASE_OUT_CUBIC: Easing = Easing::CubicBezier(0.33, 1.0, 0.68, 1.0);
    pub const EASE_OUT_QUART: Easing = Easing::CubicBezier(0.25, 1.0, 0.5, 1.0);
    pub const EASE_OUT_EXPO: Easing = Easing::CubicBezier(0.16, 1.0, 0.3, 1.0);

    /// Bouncy/spring-like easing
    pub const EASE_OUT_BACK: Easing = Easing::CubicBezier(0.34, 1.56, 0.64, 1.0);

    /// Parse easing from string name
    pub fn from_name(name: &str) -> Self {
        match name.to_lowercase().replace('-', "_").as_str() {
            "linear" => Easing::Linear,
            "ease" => Easing::EASE,
            "ease_in" | "easein" => Easing::EASE_IN,
            "ease_out" | "easeout" => Easing::EASE_OUT,
            "ease_in_out" | "easeinout" => Easing::EASE_IN_OUT,
            "ease_out_cubic" | "easeoutcubic" => Easing::EASE_OUT_CUBIC,
            "ease_out_quart" | "easeoutquart" => Easing::EASE_OUT_QUART,
            "ease_out_expo" | "easeoutexpo" => Easing::EASE_OUT_EXPO,
            "ease_out_back" | "easeoutback" => Easing::EASE_OUT_BACK,
            _ => Easing::EASE_OUT_EXPO, // Default
        }
    }

    /// Calculate the eased value for a given progress (0.0 to 1.0)
    pub fn ease(&self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);

        match self {
            Easing::Linear => t,
            Easing::EaseIn => t * t,
            Easing::EaseOut => 1.0 - (1.0 - t) * (1.0 - t),
            Easing::EaseInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
                }
            }
            Easing::CubicBezier(x1, y1, x2, y2) => cubic_bezier(t, *x1, *y1, *x2, *y2),
        }
    }
}

/// Cubic bezier interpolation
/// Based on WebKit's implementation
fn cubic_bezier(t: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
    // For a cubic bezier curve from (0,0) to (1,1) with control points (x1,y1) and (x2,y2),
    // we need to find the y value for a given t (time/progress).
    //
    // The curve is defined parametrically as:
    // x(s) = 3(1-s)²s·x1 + 3(1-s)s²·x2 + s³
    // y(s) = 3(1-s)²s·y1 + 3(1-s)s²·y2 + s³
    //
    // We need to find s such that x(s) = t, then return y(s)

    // Use Newton-Raphson to find s for given x = t
    let mut s = t; // Initial guess

    for _ in 0..8 {
        let x = bezier_sample(s, x1, x2) - t;
        if x.abs() < 0.0001 {
            break;
        }
        let dx = bezier_derivative(s, x1, x2);
        if dx.abs() < 0.0001 {
            break;
        }
        s -= x / dx;
    }

    // Clamp s to valid range
    s = s.clamp(0.0, 1.0);

    // Return y value at s
    bezier_sample(s, y1, y2)
}

/// Sample a 1D bezier curve at parameter s
#[inline]
fn bezier_sample(s: f32, p1: f32, p2: f32) -> f32 {
    // B(s) = 3(1-s)²s·p1 + 3(1-s)s²·p2 + s³
    let s2 = s * s;
    let s3 = s2 * s;
    let one_minus_s = 1.0 - s;
    let one_minus_s2 = one_minus_s * one_minus_s;

    3.0 * one_minus_s2 * s * p1 + 3.0 * one_minus_s * s2 * p2 + s3
}

/// Derivative of 1D bezier curve at parameter s
#[inline]
fn bezier_derivative(s: f32, p1: f32, p2: f32) -> f32 {
    // B'(s) = 3(1-s)²·p1 + 6(1-s)s·(p2-p1) + 3s²·(1-p2)
    let one_minus_s = 1.0 - s;
    3.0 * one_minus_s * one_minus_s * p1
        + 6.0 * one_minus_s * s * (p2 - p1)
        + 3.0 * s * s * (1.0 - p2)
}

/// Animation state
#[derive(Debug, Clone)]
pub struct Animation {
    /// Start time of the animation
    start_time: Instant,
    /// Duration in milliseconds
    duration_ms: u32,
    /// Start value
    from: f32,
    /// End value
    to: f32,
    /// Easing function
    easing: Easing,
    /// Whether the animation is complete
    completed: bool,
}

impl Animation {
    /// Create a new animation
    pub fn new(from: f32, to: f32, duration_ms: u32, easing: Easing) -> Self {
        Self {
            start_time: Instant::now(),
            duration_ms,
            from,
            to,
            easing,
            completed: false,
        }
    }

    /// Create a fade-in animation (0.0 to 1.0)
    pub fn fade_in(duration_ms: u32, easing: Easing) -> Self {
        Self::new(0.0, 1.0, duration_ms, easing)
    }

    /// Create a fade-out animation (1.0 to 0.0)
    pub fn fade_out(duration_ms: u32, easing: Easing) -> Self {
        Self::new(1.0, 0.0, duration_ms, easing)
    }

    /// Get the current animated value
    pub fn value(&self) -> f32 {
        if self.completed {
            return self.to;
        }

        let elapsed = self.start_time.elapsed().as_millis() as f32;
        let duration = self.duration_ms as f32;

        if elapsed >= duration {
            return self.to;
        }

        let progress = elapsed / duration;
        let eased = self.easing.ease(progress);

        self.from + (self.to - self.from) * eased
    }

    /// Check if the animation is complete
    pub fn is_complete(&self) -> bool {
        if self.completed {
            return true;
        }
        self.start_time.elapsed().as_millis() >= self.duration_ms as u128
    }

    /// Mark the animation as complete
    pub fn complete(&mut self) {
        self.completed = true;
    }

    /// Reset the animation with new parameters
    pub fn reset(&mut self, from: f32, to: f32) {
        self.start_time = Instant::now();
        self.from = from;
        self.to = to;
        self.completed = false;
    }

    /// Get the progress (0.0 to 1.0)
    pub fn progress(&self) -> f32 {
        if self.completed {
            return 1.0;
        }
        let elapsed = self.start_time.elapsed().as_millis() as f32;
        let duration = self.duration_ms as f32;
        (elapsed / duration).min(1.0)
    }
}

/// Window animation state manager
#[derive(Debug)]
pub struct WindowAnimator {
    /// Current opacity animation (if any)
    pub opacity: Option<Animation>,
    /// Animation duration in ms
    pub fade_duration_ms: u32,
    /// Easing function for animations
    pub easing: Easing,
    /// Whether animations are enabled
    pub enabled: bool,
}

impl Default for WindowAnimator {
    fn default() -> Self {
        Self {
            opacity: None,
            fade_duration_ms: 150, // 150ms default
            easing: Easing::EASE_OUT_CUBIC,
            enabled: true,
        }
    }
}

impl WindowAnimator {
    /// Create a new animator with custom settings
    pub fn new(duration_ms: u32, easing: Easing) -> Self {
        Self {
            opacity: None,
            fade_duration_ms: duration_ms,
            easing,
            enabled: true,
        }
    }

    /// Start a fade-in animation
    pub fn start_fade_in(&mut self) {
        if self.enabled {
            self.opacity = Some(Animation::fade_in(self.fade_duration_ms, self.easing));
        }
    }

    /// Start a fade-out animation
    pub fn start_fade_out(&mut self) {
        if self.enabled {
            self.opacity = Some(Animation::fade_out(self.fade_duration_ms, self.easing));
        }
    }

    /// Get current opacity (1.0 if no animation)
    pub fn get_opacity(&self) -> f32 {
        self.opacity.as_ref().map(|a| a.value()).unwrap_or(1.0)
    }

    /// Check if any animation is running
    pub fn is_animating(&self) -> bool {
        self.opacity
            .as_ref()
            .map(|a| !a.is_complete())
            .unwrap_or(false)
    }

    /// Update animation state, returns true if still animating
    pub fn update(&mut self) -> bool {
        if let Some(ref anim) = self.opacity {
            if anim.is_complete() {
                self.opacity = None;
                return false;
            }
            return true;
        }
        false
    }

    /// Clear all animations
    pub fn clear(&mut self) {
        self.opacity = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linear_easing() {
        let easing = Easing::Linear;
        assert!((easing.ease(0.0) - 0.0).abs() < 0.001);
        assert!((easing.ease(0.5) - 0.5).abs() < 0.001);
        assert!((easing.ease(1.0) - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_ease_out() {
        let easing = Easing::EaseOut;
        // Ease out should be faster at the start
        assert!(easing.ease(0.5) > 0.5);
    }

    #[test]
    fn test_ease_in() {
        let easing = Easing::EaseIn;
        // Ease in should be slower at the start
        assert!(easing.ease(0.5) < 0.5);
    }

    #[test]
    fn test_cubic_bezier_endpoints() {
        let easing = Easing::CubicBezier(0.25, 0.1, 0.25, 1.0);
        assert!((easing.ease(0.0) - 0.0).abs() < 0.01);
        assert!((easing.ease(1.0) - 1.0).abs() < 0.01);
    }
}
