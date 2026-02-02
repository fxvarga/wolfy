//! Application Layer - Use Cases and Business Workflows
//!
//! This layer orchestrates domain entities and defines application-specific workflows.
//! It contains:
//! - **Use Cases**: Single-purpose operations (LaunchApp, SearchApps, SwitchTheme)
//! - **Ports**: Interfaces for external dependencies (rendering, runtime, file system)
//! - **Services**: Application-level coordination services
//! - **DTOs**: Data transfer objects for layer boundaries
//!
//! # Clean Architecture Rules
//! - Depends only on the domain layer
//! - Defines ports that infrastructure implements
//! - Contains no framework-specific code

pub mod dto;
pub mod ports;
pub mod services;
pub mod use_cases;

// Re-export commonly used types
pub use dto::*;
pub use ports::*;
pub use services::*;
pub use use_cases::*;
