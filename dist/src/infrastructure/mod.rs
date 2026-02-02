//! Infrastructure Layer
//!
//! This layer contains concrete implementations of all interfaces and
//! external framework integrations.
//! It contains:
//! - **Platform**: Windows-specific implementations (Win32, Direct2D)
//! - **FileSystem**: File system operations
//! - **Animation**: Animation system implementation
//! - **Parser**: Theme parser (LALRPOP-based)
//! - **CompositionRoot**: Dependency injection container
//!
//! # Clean Architecture Rules
//! - Implements ports defined in application layer
//! - Contains all framework-specific code
//! - No domain logic here - only technical implementations

pub mod animation;
pub mod composition_root;
pub mod filesystem;
pub mod parser;

// Platform-specific modules
#[cfg(windows)]
pub mod win32;

pub use composition_root::CompositionRoot;
