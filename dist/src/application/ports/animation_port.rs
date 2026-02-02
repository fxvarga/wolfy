//! AnimationPort - interface for animation operations
//!
//! This port defines animation capabilities.

use crate::domain::entities::WindowState;

/// Animation easing type
#[derive(Clone, Copy, Debug, Default)]
pub enum Easing {
    Linear,
    EaseIn,
    #[default]
    EaseOut,
    EaseInOut,
    CubicBezier(f32, f32, f32, f32),
}

/// Animation operation error
#[derive(Debug, Clone)]
pub enum AnimationError {
    /// Animation not found
    NotFound(String),
    /// Invalid parameters
    InvalidParams(String),
}

impl std::fmt::Display for AnimationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AnimationError::NotFound(s) => write!(f, "Animation not found: {}", s),
            AnimationError::InvalidParams(s) => write!(f, "Invalid params: {}", s),
        }
    }
}

impl std::error::Error for AnimationError {}

/// Handle to a running animation
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AnimationHandle(pub u64);

/// Port interface for animation operations
pub trait AnimationPort: Send + Sync {
    /// Start a window show animation
    fn animate_show(
        &mut self,
        state: &mut WindowState,
        duration_ms: u32,
        easing: Easing,
    ) -> AnimationHandle;

    /// Start a window hide animation
    fn animate_hide(
        &mut self,
        state: &mut WindowState,
        duration_ms: u32,
        easing: Easing,
    ) -> AnimationHandle;

    /// Update all running animations
    /// Returns list of completed animation handles
    fn update(&mut self, delta_ms: f32) -> Vec<AnimationHandle>;

    /// Check if an animation is running
    fn is_animating(&self, handle: AnimationHandle) -> bool;

    /// Cancel an animation
    fn cancel(&mut self, handle: AnimationHandle);

    /// Cancel all animations
    fn cancel_all(&mut self);

    /// Get the progress of an animation (0.0 - 1.0)
    fn get_progress(&self, handle: AnimationHandle) -> Option<f32>;
}

/// A null animation port for testing
pub struct NullAnimationPort {
    next_handle: u64,
}

impl NullAnimationPort {
    pub fn new() -> Self {
        Self { next_handle: 1 }
    }
}

impl Default for NullAnimationPort {
    fn default() -> Self {
        Self::new()
    }
}

impl AnimationPort for NullAnimationPort {
    fn animate_show(
        &mut self,
        state: &mut WindowState,
        _duration_ms: u32,
        _easing: Easing,
    ) -> AnimationHandle {
        state.finish_showing();
        let handle = AnimationHandle(self.next_handle);
        self.next_handle += 1;
        handle
    }

    fn animate_hide(
        &mut self,
        state: &mut WindowState,
        _duration_ms: u32,
        _easing: Easing,
    ) -> AnimationHandle {
        state.finish_hiding();
        let handle = AnimationHandle(self.next_handle);
        self.next_handle += 1;
        handle
    }

    fn update(&mut self, _delta_ms: f32) -> Vec<AnimationHandle> {
        Vec::new()
    }

    fn is_animating(&self, _handle: AnimationHandle) -> bool {
        false
    }

    fn cancel(&mut self, _handle: AnimationHandle) {}

    fn cancel_all(&mut self) {}

    fn get_progress(&self, _handle: AnimationHandle) -> Option<f32> {
        None
    }
}
