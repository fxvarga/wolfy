//! Application Configuration

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::domain::value_objects::{Hotkey, KeyCode, Modifiers};

// ============================================================================
// EXTENSIONS CONFIGURATION (extensions.toml)
// ============================================================================

/// Extensions configuration loaded from extensions.toml
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ExtensionsConfig {
    /// Wolfy metadata section
    #[serde(default)]
    pub wolfy: WolfyMeta,

    /// PR Reviews extension configuration
    #[serde(default)]
    pub pr_reviews: PrReviewsExtConfig,
}

/// Wolfy metadata
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WolfyMeta {
    /// Config version for compatibility
    #[serde(default = "default_version")]
    pub version: String,
}

impl Default for WolfyMeta {
    fn default() -> Self {
        Self {
            version: default_version(),
        }
    }
}

fn default_version() -> String {
    "0.1".to_string()
}

/// PR Reviews extension configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PrReviewsExtConfig {
    /// Whether the extension is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Directory containing PR review markdown files
    #[serde(default)]
    pub reviews_dir: Option<PathBuf>,
}

impl Default for PrReviewsExtConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            reviews_dir: None, // Will use built-in default if not specified
        }
    }
}

fn default_true() -> bool {
    true
}

impl ExtensionsConfig {
    /// Find extensions.toml in standard locations
    pub fn find_config_path() -> Option<PathBuf> {
        // Check in order: %APPDATA%/wolfy, exe dir, cwd
        let candidates = [
            dirs::config_dir().map(|p| p.join("wolfy").join("extensions.toml")),
            std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|d| d.join("extensions.toml"))),
            Some(PathBuf::from("extensions.toml")),
        ];

        for candidate in candidates.into_iter().flatten() {
            if candidate.exists() {
                return Some(candidate);
            }
        }
        None
    }

    /// Load configuration from file, returning defaults if not found
    pub fn load() -> Self {
        if let Some(path) = Self::find_config_path() {
            Self::load_from_path(&path).unwrap_or_default()
        } else {
            Self::default()
        }
    }

    /// Load configuration from a specific path
    pub fn load_from_path(path: &PathBuf) -> Result<Self, ExtensionsConfigError> {
        let content = std::fs::read_to_string(path)?;
        let config: ExtensionsConfig = toml::from_str(&content)?;
        Ok(config)
    }
}

/// Extensions configuration error
#[derive(Debug)]
pub enum ExtensionsConfigError {
    IoError(std::io::Error),
    ParseError(toml::de::Error),
}

impl std::fmt::Display for ExtensionsConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExtensionsConfigError::IoError(e) => write!(f, "IO error: {}", e),
            ExtensionsConfigError::ParseError(e) => write!(f, "Parse error: {}", e),
        }
    }
}

impl std::error::Error for ExtensionsConfigError {}

impl From<std::io::Error> for ExtensionsConfigError {
    fn from(e: std::io::Error) -> Self {
        ExtensionsConfigError::IoError(e)
    }
}

impl From<toml::de::Error> for ExtensionsConfigError {
    fn from(e: toml::de::Error) -> Self {
        ExtensionsConfigError::ParseError(e)
    }
}

// ============================================================================
// APPLICATION CONFIGURATION
// ============================================================================

/// Application configuration
#[derive(Clone, Debug)]
pub struct Config {
    /// Window width
    pub window_width: u32,
    /// Window height
    pub window_height: u32,
    /// Window opacity (0.0 - 1.0)
    pub window_opacity: f32,
    /// Theme name
    pub theme_name: String,
    /// Show animation duration in milliseconds
    pub show_duration_ms: u32,
    /// Hide animation duration in milliseconds
    pub hide_duration_ms: u32,
    /// Toggle hotkey
    pub toggle_hotkey: Hotkey,
    /// Whether to hide on focus loss
    pub hide_on_blur: bool,
    /// Maximum search results
    pub max_results: usize,
    /// Search debounce in milliseconds
    pub search_debounce_ms: u32,
    /// Font family
    pub font_family: String,
    /// Base font size
    pub font_size: f32,
    /// History file path
    pub history_path: Option<PathBuf>,
    /// Themes directory
    pub themes_dir: Option<PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            window_width: 928,
            window_height: 600,
            window_opacity: 0.95,
            theme_name: "default".to_string(),
            show_duration_ms: 200,
            hide_duration_ms: 150,
            toggle_hotkey: Hotkey::alt(KeyCode::Space),
            hide_on_blur: true,
            max_results: 50,
            search_debounce_ms: 100,
            font_family: "Segoe UI".to_string(),
            font_size: 14.0,
            history_path: None,
            themes_dir: None,
        }
    }
}

impl Config {
    /// Load configuration from file
    pub fn load(path: &PathBuf) -> Result<Self, ConfigError> {
        // Placeholder - would load from file
        Ok(Self::default())
    }

    /// Save configuration to file
    pub fn save(&self, path: &PathBuf) -> Result<(), ConfigError> {
        // Placeholder - would save to file
        Ok(())
    }

    /// Merge with command-line arguments
    pub fn with_args(mut self, args: &[String]) -> Self {
        // Placeholder - would parse args
        self
    }
}

/// Configuration error
#[derive(Debug)]
pub enum ConfigError {
    IoError(std::io::Error),
    ParseError(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::IoError(e) => write!(f, "IO error: {}", e),
            ConfigError::ParseError(s) => write!(f, "Parse error: {}", s),
        }
    }
}

impl std::error::Error for ConfigError {}

impl From<std::io::Error> for ConfigError {
    fn from(e: std::io::Error) -> Self {
        ConfigError::IoError(e)
    }
}
