//! Domain layer error types
//!
//! All errors that can occur in domain layer operations.

use thiserror::Error;

/// Main domain error type
#[derive(Error, Debug)]
pub enum DomainError {
    /// Entity not found
    #[error("Entity not found: {0}")]
    NotFound(String),

    /// Invalid entity state
    #[error("Invalid state: {0}")]
    InvalidState(String),

    /// Validation failed
    #[error("Validation error: {0}")]
    ValidationError(String),

    /// Search error
    #[error("Search error: {0}")]
    SearchError(String),

    /// Theme resolution error
    #[error("Theme error: {0}")]
    ThemeError(String),

    /// IO error (wrapped)
    #[error("IO error: {0}")]
    IoError(String),

    /// Parse error
    #[error("Parse error: {0}")]
    ParseError(String),
}

impl From<std::io::Error> for DomainError {
    fn from(err: std::io::Error) -> Self {
        DomainError::IoError(err.to_string())
    }
}
