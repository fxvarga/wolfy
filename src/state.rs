//! Shared application state between multiple windows
//!
//! This module provides the shared state that is used across all Wolfy windows:
//! - Launcher (app search with task panel)
//! - ThemePicker (grid of HyDE themes)
//! - WallpaperPicker (grid of wallpapers)

use std::path::PathBuf;

use crate::history::History;
use crate::log::exe_dir;
use crate::widget::ElementData;

/// Get the HyDE themes directory path
pub fn hyde_themes_dir() -> Option<PathBuf> {
    // First, try relative to exe: ../hyde/themes/ (for portable installs)
    let exe_relative = exe_dir().join("../hyde/themes");
    if exe_relative.is_dir() {
        log!("Found HyDE themes at exe-relative path: {:?}", exe_relative);
        return Some(exe_relative);
    }

    // Try standard HyDE location: ~/.config/hyde/themes/
    if let Some(home) = std::env::var_os("HOME") {
        let path = PathBuf::from(home).join(".config/hyde/themes");
        if path.is_dir() {
            return Some(path);
        }
    }
    // Windows fallback: %USERPROFILE%\.config\hyde\themes
    if let Some(profile) = std::env::var_os("USERPROFILE") {
        let path = PathBuf::from(profile).join(".config/hyde/themes");
        if path.is_dir() {
            return Some(path);
        }
    }
    None
}

/// Represents a HyDE theme with its metadata
#[derive(Clone, Debug)]
pub struct HydeTheme {
    /// Theme name (directory name)
    pub name: String,
    /// Full path to theme directory
    pub path: PathBuf,
    /// Path to first wallpaper (used as thumbnail)
    pub thumbnail: Option<PathBuf>,
}

/// Check if a path is an image file based on extension
pub fn is_image_file(path: &std::path::Path) -> bool {
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return false;
    };
    matches!(
        ext.to_lowercase().as_str(),
        "png" | "jpg" | "jpeg" | "webp" | "bmp" | "gif"
    )
}

/// Find the first image file in a directory (sorted alphabetically)
pub fn find_first_image(dir: &std::path::Path) -> Option<PathBuf> {
    if !dir.is_dir() {
        return None;
    }

    let Ok(entries) = std::fs::read_dir(dir) else {
        return None;
    };

    let mut images: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && is_image_file(p))
        .collect();

    images.sort_by(|a, b| {
        let a_name = a.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let b_name = b.file_name().and_then(|n| n.to_str()).unwrap_or("");
        a_name.cmp(b_name)
    });

    images.into_iter().next()
}

/// Scan for HyDE themes in the themes directory
pub fn scan_hyde_themes() -> Vec<HydeTheme> {
    let Some(themes_dir) = hyde_themes_dir() else {
        log!("HyDE themes directory not found");
        return Vec::new();
    };

    log!("Scanning HyDE themes from: {:?}", themes_dir);

    let mut themes = Vec::new();

    let Ok(entries) = std::fs::read_dir(&themes_dir) else {
        log!("Failed to read themes directory");
        return Vec::new();
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };

        // Look for a thumbnail - prefer theme preview, fallback to first wallpaper
        let wallpapers_dir = path.join("wallpapers");
        let thumbnail = find_first_image(&wallpapers_dir);

        themes.push(HydeTheme {
            name: name.to_string(),
            path: path.clone(),
            thumbnail,
        });
    }

    // Sort alphabetically
    themes.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    log!("Found {} HyDE themes", themes.len());
    themes
}

/// Scan wallpapers for a specific theme
pub fn scan_theme_wallpapers(theme_name: &str) -> Vec<PathBuf> {
    let Some(themes_dir) = hyde_themes_dir() else {
        return Vec::new();
    };

    let wallpapers_dir = themes_dir.join(theme_name).join("wallpapers");
    if !wallpapers_dir.is_dir() {
        log!("Wallpapers directory not found for theme: {}", theme_name);
        return Vec::new();
    }

    log!("Scanning wallpapers from: {:?}", wallpapers_dir);

    let mut wallpapers = Vec::new();

    let Ok(entries) = std::fs::read_dir(&wallpapers_dir) else {
        return Vec::new();
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() && is_image_file(&path) {
            wallpapers.push(path);
        }
    }

    // Sort by filename
    wallpapers.sort_by(|a, b| {
        let a_name = a.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let b_name = b.file_name().and_then(|n| n.to_str()).unwrap_or("");
        a_name.cmp(b_name)
    });

    log!(
        "Found {} wallpapers for theme: {}",
        wallpapers.len(),
        theme_name
    );
    wallpapers
}

/// Shared application state across all windows
pub struct AppState {
    /// Currently selected HyDE theme name (set by ThemePicker, read by WallpaperPicker)
    pub current_theme: Option<String>,

    /// Usage history for app launcher
    pub history: History,

    /// Discovered applications (cached)
    pub all_apps: Vec<ElementData>,

    /// Cached HyDE themes (scanned once at startup)
    pub hyde_themes: Vec<HydeTheme>,

    /// Whether themes have been scanned yet
    pub themes_loaded: bool,
}

impl AppState {
    /// Create new shared state
    pub fn new() -> Self {
        Self {
            current_theme: None,
            history: History::load_default(),
            all_apps: Vec::new(),
            hyde_themes: Vec::new(),
            themes_loaded: false,
        }
    }

    /// Load HyDE themes if not already loaded
    pub fn ensure_themes_loaded(&mut self) {
        if !self.themes_loaded {
            self.hyde_themes = scan_hyde_themes();
            self.themes_loaded = true;
        }
    }

    /// Set the currently selected theme
    pub fn set_current_theme(&mut self, theme_name: Option<String>) {
        log!("AppState: setting current theme to {:?}", theme_name);
        self.current_theme = theme_name;
    }

    /// Get wallpapers for the currently selected theme
    pub fn get_current_theme_wallpapers(&self) -> Vec<PathBuf> {
        match &self.current_theme {
            Some(theme_name) => scan_theme_wallpapers(theme_name),
            None => Vec::new(),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_state_creation() {
        let state = AppState::new();
        assert!(state.current_theme.is_none());
        assert!(!state.themes_loaded);
        assert!(state.hyde_themes.is_empty());
    }

    #[test]
    fn test_is_image_file() {
        use std::path::Path;
        assert!(is_image_file(Path::new("test.png")));
        assert!(is_image_file(Path::new("test.jpg")));
        assert!(is_image_file(Path::new("test.JPEG")));
        assert!(is_image_file(Path::new("test.webp")));
        assert!(!is_image_file(Path::new("test.txt")));
        assert!(!is_image_file(Path::new("test")));
    }
}
