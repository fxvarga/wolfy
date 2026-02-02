//! ThemeRepository - interface for theme loading and management
//!
//! This trait defines how to load, list, and watch themes.

use crate::domain::entities::Theme;
use crate::domain::errors::DomainError;

/// Repository interface for theme data
pub trait ThemeRepository: Send + Sync {
    /// Load a theme by name
    fn load(&self, name: &str) -> Result<Theme, DomainError>;

    /// List all available theme names
    fn list_available(&self) -> Result<Vec<String>, DomainError>;

    /// Check if a theme exists
    fn exists(&self, name: &str) -> bool;

    /// Reload a theme from disk
    fn reload(&mut self, name: &str) -> Result<Theme, DomainError>;

    /// Get the default theme name
    fn default_theme_name(&self) -> &str {
        "default"
    }
}

/// A null implementation for testing
pub struct NullThemeRepository;

impl ThemeRepository for NullThemeRepository {
    fn load(&self, name: &str) -> Result<Theme, DomainError> {
        Ok(Theme::new(name))
    }

    fn list_available(&self) -> Result<Vec<String>, DomainError> {
        Ok(vec!["default".to_string()])
    }

    fn exists(&self, _name: &str) -> bool {
        true
    }

    fn reload(&mut self, name: &str) -> Result<Theme, DomainError> {
        self.load(name)
    }
}
