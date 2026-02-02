//! Parser infrastructure - Theme parser placeholder
//!
//! This module would integrate with the existing LALRPOP-based theme parser.
//! For the clean architecture migration, we'll define the interface here
//! and bridge to the existing parser.

use crate::domain::entities::Theme;
use crate::domain::errors::DomainError;
use crate::domain::repositories::ThemeRepository;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Rasi theme gateway - loads .rasi theme files
pub struct RasiThemeGateway {
    /// Directory containing theme files
    themes_dir: PathBuf,
    /// Cached themes
    cache: HashMap<String, Theme>,
}

impl RasiThemeGateway {
    /// Create a new Rasi theme gateway
    pub fn new(themes_dir: PathBuf) -> Self {
        Self {
            themes_dir,
            cache: HashMap::new(),
        }
    }

    /// Find theme file by name
    fn find_theme_file(&self, name: &str) -> Option<PathBuf> {
        // Try exact name
        let exact = self.themes_dir.join(name);
        if exact.exists() {
            return Some(exact);
        }

        // Try with .rasi extension
        let with_ext = self.themes_dir.join(format!("{}.rasi", name));
        if with_ext.exists() {
            return Some(with_ext);
        }

        // Try in current directory
        let current = PathBuf::from(format!("{}.rasi", name));
        if current.exists() {
            return Some(current);
        }

        None
    }

    /// Parse a theme file (placeholder - would integrate with existing parser)
    fn parse_theme_file(&self, path: &Path) -> Result<Theme, DomainError> {
        // This would integrate with the existing LALRPOP parser
        // For now, return a minimal theme with just the name
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        Ok(Theme::new(name))
    }
}

impl ThemeRepository for RasiThemeGateway {
    fn load(&self, name: &str) -> Result<Theme, DomainError> {
        // Check cache first
        if let Some(theme) = self.cache.get(name) {
            return Ok(theme.clone());
        }

        // Find and parse theme file
        let path = self
            .find_theme_file(name)
            .ok_or_else(|| DomainError::NotFound(format!("Theme not found: {}", name)))?;

        self.parse_theme_file(&path)
    }

    fn list_available(&self) -> Result<Vec<String>, DomainError> {
        let mut themes = Vec::new();

        if self.themes_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(&self.themes_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().map(|e| e == "rasi").unwrap_or(false) {
                        if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                            themes.push(name.to_string());
                        }
                    }
                }
            }
        }

        // Always include default
        if !themes.contains(&"default".to_string()) {
            themes.push("default".to_string());
        }

        themes.sort();
        Ok(themes)
    }

    fn exists(&self, name: &str) -> bool {
        self.find_theme_file(name).is_some() || name == "default"
    }

    fn reload(&mut self, name: &str) -> Result<Theme, DomainError> {
        self.cache.remove(name);
        self.load(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_theme_exists() {
        let gateway = RasiThemeGateway::new(PathBuf::from("."));
        assert!(gateway.exists("default"));
    }

    #[test]
    fn test_list_themes() {
        let gateway = RasiThemeGateway::new(PathBuf::from("."));
        let themes = gateway.list_available().unwrap();
        assert!(themes.contains(&"default".to_string()));
    }
}
