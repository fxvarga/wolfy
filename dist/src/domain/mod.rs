//! Domain Layer - Pure business logic with zero external dependencies
//!
//! This layer contains:
//! - **Entities**: Core business objects (AppItem, Theme, Task)
//! - **Value Objects**: Immutable values (Color, Rect, Dimensions, SearchQuery)
//! - **Repository Interfaces**: Abstractions for data access (no implementations)
//! - **Domain Services**: Complex operations that don't belong to a single entity
//! - **Domain Errors**: Error types for domain operations
//!
//! # Clean Architecture Rules
//! - Zero dependencies on external frameworks
//! - 100% testable without mocks
//! - Framework-agnostic business rules

pub mod entities;
pub mod errors;
pub mod repositories;
pub mod services;
pub mod value_objects;

// Re-export commonly used types
pub use entities::*;
pub use errors::DomainError;
pub use value_objects::*;
