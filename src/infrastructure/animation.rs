//! Animation infrastructure - Animation system implementation

use std::collections::HashMap;
use std::time::Instant;

use crate::application::ports::animation_port::{AnimationHandle, AnimationPort, Easing};
use crate::domain::entities::WindowState;

/// An active animation
struct ActiveAnimation {
    handle: AnimationHandle,
    start_time: Instant,
    duration_ms: u32,
    easing: Easing,
    animation_type: AnimationType,
}

/// Type of animation
enum AnimationType {
    Show,
    Hide,
}

/// Animation system implementation
pub struct WindowAnimator {
    next_handle: u64,
    animations: HashMap<u64, ActiveAnimation>,
}

impl WindowAnimator {
    /// Create a new window animator
    pub fn new() -> Self {
        Self {
            next_handle: 1,
            animations: HashMap::new(),
        }
    }

    /// Allocate a new handle
    fn next_handle(&mut self) -> AnimationHandle {
        let handle = AnimationHandle(self.next_handle);
        self.next_handle += 1;
        handle
    }

    /// Calculate eased progress
    fn ease(t: f32, easing: Easing) -> f32 {
        let t = t.clamp(0.0, 1.0);

        match easing {
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
            Easing::CubicBezier(x1, y1, x2, y2) => {
                // Simplified cubic bezier approximation
                Self::cubic_bezier_ease(t, x1, y1, x2, y2)
            }
        }
    }

    /// Cubic bezier easing (simplified)
    fn cubic_bezier_ease(t: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
        // Newton-Raphson approximation for cubic bezier
        let mut x = t;
        for _ in 0..5 {
            let x_bez = Self::bezier_at(x, x1, x2);
            let dx = (x_bez - t) / Self::bezier_derivative(x, x1, x2).max(0.001);
            x -= dx;
            x = x.clamp(0.0, 1.0);
        }
        Self::bezier_at(x, y1, y2)
    }

    fn bezier_at(t: f32, p1: f32, p2: f32) -> f32 {
        let t2 = t * t;
        let t3 = t2 * t;
        let mt = 1.0 - t;
        let mt2 = mt * mt;
        let mt3 = mt2 * mt;

        3.0 * mt2 * t * p1 + 3.0 * mt * t2 * p2 + t3
    }

    fn bezier_derivative(t: f32, p1: f32, p2: f32) -> f32 {
        let mt = 1.0 - t;
        3.0 * mt * mt * p1 + 6.0 * mt * t * (p2 - p1) + 3.0 * t * t * (1.0 - p2)
    }
}

impl Default for WindowAnimator {
    fn default() -> Self {
        Self::new()
    }
}

impl AnimationPort for WindowAnimator {
    fn animate_show(
        &mut self,
        state: &mut WindowState,
        duration_ms: u32,
        easing: Easing,
    ) -> AnimationHandle {
        let handle = self.next_handle();

        state.start_showing();

        self.animations.insert(
            handle.0,
            ActiveAnimation {
                handle,
                start_time: Instant::now(),
                duration_ms,
                easing,
                animation_type: AnimationType::Show,
            },
        );

        handle
    }

    fn animate_hide(
        &mut self,
        state: &mut WindowState,
        duration_ms: u32,
        easing: Easing,
    ) -> AnimationHandle {
        let handle = self.next_handle();

        state.start_hiding();

        self.animations.insert(
            handle.0,
            ActiveAnimation {
                handle,
                start_time: Instant::now(),
                duration_ms,
                easing,
                animation_type: AnimationType::Hide,
            },
        );

        handle
    }

    fn update(&mut self, _delta_ms: f32) -> Vec<AnimationHandle> {
        let now = Instant::now();
        let mut completed = Vec::new();

        // Collect animations to remove
        let to_remove: Vec<u64> = self
            .animations
            .iter()
            .filter_map(|(id, anim)| {
                let elapsed = now.duration_since(anim.start_time).as_millis() as f32;
                let duration = anim.duration_ms as f32;

                if elapsed >= duration {
                    completed.push(anim.handle);
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();

        // Remove completed animations
        for id in to_remove {
            self.animations.remove(&id);
        }

        completed
    }

    fn is_animating(&self, handle: AnimationHandle) -> bool {
        self.animations.contains_key(&handle.0)
    }

    fn cancel(&mut self, handle: AnimationHandle) {
        self.animations.remove(&handle.0);
    }

    fn cancel_all(&mut self) {
        self.animations.clear();
    }

    fn get_progress(&self, handle: AnimationHandle) -> Option<f32> {
        self.animations.get(&handle.0).map(|anim| {
            let elapsed = anim.start_time.elapsed().as_millis() as f32;
            let duration = anim.duration_ms as f32;
            let linear_progress = (elapsed / duration).clamp(0.0, 1.0);
            Self::ease(linear_progress, anim.easing)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_animator_creation() {
        let animator = WindowAnimator::new();
        assert!(animator.animations.is_empty());
    }

    #[test]
    fn test_animate_show() {
        let mut animator = WindowAnimator::new();
        let mut state = WindowState::default();

        let handle = animator.animate_show(&mut state, 200, Easing::EaseOut);

        assert!(animator.is_animating(handle));
        assert!(state.is_animating());
    }

    #[test]
    fn test_cancel_animation() {
        let mut animator = WindowAnimator::new();
        let mut state = WindowState::default();

        let handle = animator.animate_show(&mut state, 200, Easing::EaseOut);
        animator.cancel(handle);

        assert!(!animator.is_animating(handle));
    }

    #[test]
    fn test_easing_linear() {
        assert!((WindowAnimator::ease(0.0, Easing::Linear) - 0.0).abs() < 0.001);
        assert!((WindowAnimator::ease(0.5, Easing::Linear) - 0.5).abs() < 0.001);
        assert!((WindowAnimator::ease(1.0, Easing::Linear) - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_easing_ease_out() {
        let start = WindowAnimator::ease(0.0, Easing::EaseOut);
        let mid = WindowAnimator::ease(0.5, Easing::EaseOut);
        let end = WindowAnimator::ease(1.0, Easing::EaseOut);

        assert!((start - 0.0).abs() < 0.001);
        assert!(mid > 0.5); // Ease out is faster at start
        assert!((end - 1.0).abs() < 0.001);
    }
}
