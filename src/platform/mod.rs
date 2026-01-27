//! Platform abstraction layer
//!
//! Currently only Windows (win32) is supported.

#[cfg(target_os = "windows")]
pub mod win32;

#[cfg(target_os = "windows")]
pub use win32::*;
