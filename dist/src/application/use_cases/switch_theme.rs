//! SwitchThemeUseCase - Switch the application theme
//!
//! Handles the business logic for switching themes.

use std::sync::Arc;

use crate::domain::entities::Theme;
use crate::domain::errors::DomainError;
use crate::domain::repositories::ThemeRepository;

/// Error for theme switching
#[derive(Debug)]
pub enum ThemeError {
    /// Theme not found
    NotFound(String),
    /// Failed to load theme
    LoadFailed(String),
    /// Domain error
    Domain(DomainError),
}

impl std::fmt::Display for ThemeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ThemeError::NotFound(name) => write!(f, "Theme not found: {}", name),
            ThemeError::LoadFailed(msg) => write!(f, "Failed to load theme: {}", msg),
            ThemeError::Domain(e) => write!(f, "Domain error: {}", e),
        }
    }
}

impl std::error::Error for ThemeError {}

impl From<DomainError> for ThemeError {
    fn from(e: DomainError) -> Self {
        ThemeError::Domain(e)
    }
}

/// Callback for theme change events
pub trait ThemeChangeListener: Send + Sync {
    fn on_theme_changed(&mut self, theme: &Theme);
}

/// Use case for switching themes
pub struct SwitchThemeUseCase<R>
where
    R: ThemeRepository,
{
    theme_repository: Arc<R>,
    listeners: Vec<Arc<std::sync::Mutex<dyn ThemeChangeListener>>>,
}

impl<R> SwitchThemeUseCase<R>
where
    R: ThemeRepository,
{
    /// Create a new switch theme use case
    pub fn new(theme_repository: Arc<R>) -> Self {
        Self {
            theme_repository,
            listeners: Vec::new(),
        }
    }

    /// Add a theme change listener
    pub fn add_listener(&mut self, listener: Arc<std::sync::Mutex<dyn ThemeChangeListener>>) {
        self.listeners.push(listener);
    }

    /// Switch to a theme by name
    pub fn execute(&self, theme_name: &str) -> Result<Theme, ThemeError> {
        // 1. Check if theme exists
        if !self.theme_repository.exists(theme_name) {
            return Err(ThemeError::NotFound(theme_name.to_string()));
        }

        // 2. Load the theme
        let theme = self
            .theme_repository
            .load(theme_name)
            .map_err(|e| ThemeError::LoadFailed(e.to_string()))?;

        // 3. Notify listeners
        self.notify_listeners(&theme);

        Ok(theme)
    }

    /// Get available theme names
    pub fn available_themes(&self) -> Result<Vec<String>, ThemeError> {
        self.theme_repository
            .list_available()
            .map_err(ThemeError::from)
    }

    /// Get the default theme
    pub fn default_theme(&self) -> Result<Theme, ThemeError> {
        let name = self.theme_repository.default_theme_name();
        self.execute(name)
    }

    /// Notify all listeners of theme change
    fn notify_listeners(&self, theme: &Theme) {
        for listener in &self.listeners {
            if let Ok(mut l) = listener.lock() {
                l.on_theme_changed(theme);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::repositories::theme_repository::NullThemeRepository;

    #[test]
    fn test_switch_theme() {
        let theme_repo = Arc::new(NullThemeRepository);
        let use_case = SwitchThemeUseCase::new(theme_repo);

        let result = use_case.execute("default");
        assert!(result.is_ok());

        let theme = result.unwrap();
        assert_eq!(theme.name, "default");
    }

    #[test]
    fn test_available_themes() {
        let theme_repo = Arc::new(NullThemeRepository);
        let use_case = SwitchThemeUseCase::new(theme_repo);

        let themes = use_case.available_themes().unwrap();
        assert!(themes.contains(&"default".to_string()));
    }
}
