//! Application Configuration

use std::path::PathBuf;

use crate::domain::value_objects::{Hotkey, KeyCode, Modifiers};

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
