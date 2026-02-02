//! Interface Adapters Layer
//!
//! This layer converts data between the use case format and external formats.
//! It contains:
//! - **Controllers**: Handle input events, translate to use case calls
//! - **Presenters**: Format use case output for display
//! - **Gateways**: Repository implementations
//! - **Views**: View interfaces for UI components
//!
//! # Clean Architecture Rules
//! - Depends on application and domain layers
//! - Implements ports defined in application layer
//! - Provides interfaces that infrastructure implements

pub mod controllers;
pub mod gateways;
pub mod presenters;
pub mod views;

pub use controllers::*;
pub use gateways::*;
pub use presenters::*;
pub use views::*;
