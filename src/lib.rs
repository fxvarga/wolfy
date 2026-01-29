//! Wolfy library - exposes theme module for testing
//!
//! The main binary is in main.rs, but we expose the theme module
//! as a library so we can run tests without Windows dependencies.

#[macro_use]
extern crate lalrpop_util;

// Include the log module so the log! macro works
#[macro_use]
pub mod log;

// Expose theme module for testing
pub mod theme;

// Animation system (no Windows dependencies)
pub mod animation;

// Usage history tracking (no Windows dependencies)
pub mod history;

// Task runner configuration (no Windows dependencies)
pub mod tasks;

// Widget base types (no Windows dependencies)
pub mod widget {
    pub mod base;
    pub use base::*;
}
