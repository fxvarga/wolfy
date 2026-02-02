//! ThemeManager - Manages theme loading and switching
//!
//! Coordinates theme-related use cases and maintains current theme state.

use std::sync::Arc;

use crate::application::use_cases::SwitchThemeUseCase;
use crate::domain::entities::Theme;
use crate::domain::repositories::ThemeRepository;
use crate::domain::services::ThemeResolver;

/// Manages application themes
pub struct ThemeManager<R>
where
    R: ThemeRepository + 'static,
{
    switch_theme_use_case: SwitchThemeUseCase<R>,
    theme_resolver: ThemeResolver,
    current_theme: Option<Theme>,
    current_theme_name: String,
}

impl<R> ThemeManager<R>
where
    R: ThemeRepository,
{
    /// Create a new theme manager
    pub fn new(theme_repository: Arc<R>) -> Self {
        Self {
            switch_theme_use_case: SwitchThemeUseCase::new(theme_repository),
            theme_resolver: ThemeResolver::new(),
            current_theme: None,
            current_theme_name: String::new(),
        }
    }

    /// Load and set the default theme
    pub fn load_default(&mut self) -> Result<&Theme, String> {
        let theme = self
            .switch_theme_use_case
            .default_theme()
            .map_err(|e| e.to_string())?;

        self.current_theme_name = theme.name.clone();
        self.current_theme = Some(theme);
        Ok(self.current_theme.as_ref().unwrap())
    }

    /// Switch to a theme by name
    pub fn switch_theme(&mut self, name: &str) -> Result<&Theme, String> {
        let theme = self
            .switch_theme_use_case
            .execute(name)
            .map_err(|e| e.to_string())?;

        self.current_theme_name = theme.name.clone();
        self.current_theme = Some(theme);
        Ok(self.current_theme.as_ref().unwrap())
    }

    /// Get the current theme
    pub fn current_theme(&self) -> Option<&Theme> {
        self.current_theme.as_ref()
    }

    /// Get the current theme name
    pub fn current_theme_name(&self) -> &str {
        &self.current_theme_name
    }

    /// Get available theme names
    pub fn available_themes(&self) -> Vec<String> {
        self.switch_theme_use_case
            .available_themes()
            .unwrap_or_default()
    }

    /// Get the theme resolver
    pub fn resolver(&self) -> &ThemeResolver {
        &self.theme_resolver
    }

    /// Resolve a color from the current theme
    pub fn get_color(
        &self,
        selector: &str,
        property: &str,
        default: crate::domain::value_objects::Color,
    ) -> crate::domain::value_objects::Color {
        if let Some(theme) = &self.current_theme {
            self.theme_resolver.resolve_color(theme, selector, property, default)
        } else {
            default
        }
    }

    /// Resolve a number from the current theme
    pub fn get_number(&self, selector: &str, property: &str, default: f64) -> f64 {
        if let Some(theme) = &self.current_theme {
            self.theme_resolver.resolve_number(theme, selector, property, default)
        } else {
            default
        }
    }

    /// Resolve a string from the current theme
    pub fn get_string(&self, selector: &str, property: &str, default: &str) -> String {
        if let Some(theme) = &self.current_theme {
            self.theme_resolver.resolve_string(theme, selector, property, default)
        } else {
            default.to_string()
        }
    }
}
