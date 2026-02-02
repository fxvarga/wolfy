//! Application Ports - Interfaces for external dependencies
//!
//! Ports define the interfaces that infrastructure must implement.
//! They allow the application layer to remain framework-agnostic.

pub mod animation_port;
pub mod filesystem_port;
pub mod render_port;
pub mod runtime_port;

pub use animation_port::AnimationPort;
pub use filesystem_port::FileSystemPort;
pub use render_port::RenderPort;
pub use runtime_port::RuntimePort;
