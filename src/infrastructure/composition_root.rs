//! CompositionRoot - Dependency Injection Container
//!
//! This module wires together all the dependencies for the application.
//! It creates and owns all the major components.

use std::path::PathBuf;
use std::sync::Arc;

use crate::adapters::controllers::{HotkeyController, SearchController, WindowController};
use crate::adapters::gateways::{FileHistoryGateway, MemoryAppGateway};
use crate::adapters::presenters::{SearchPresenter, ThemePresenter};
use crate::application::ports::filesystem_port::FileSystemPort;
use crate::application::ports::runtime_port::NullRuntimePort;
use crate::application::services::{ThemeManager, WindowManager};
use crate::domain::repositories::theme_repository::NullThemeRepository;
use crate::infrastructure::animation::WindowAnimator;
use crate::infrastructure::filesystem::StdFileSystem;
use crate::infrastructure::parser::RasiThemeGateway;

/// Application composition root - owns all dependencies
pub struct CompositionRoot {
    // Controllers
    pub window_controller: WindowController,
    pub search_controller: SearchController,
    pub hotkey_controller: HotkeyController,

    // Presenters
    pub search_presenter: SearchPresenter,
    pub theme_presenter: ThemePresenter,

    // Application services
    pub window_manager: WindowManager<WindowAnimator>,
    pub theme_manager: ThemeManager<RasiThemeGateway>,

    // Infrastructure
    pub file_system: StdFileSystem,
}

impl CompositionRoot {
    /// Create a new composition root with default configuration
    pub fn new() -> Self {
        Self::with_config(CompositionConfig::default())
    }

    /// Create with custom configuration
    pub fn with_config(config: CompositionConfig) -> Self {
        // Create infrastructure
        let file_system = StdFileSystem::new();
        let animator = WindowAnimator::new();

        // Create gateways
        let themes_dir = config.themes_dir.unwrap_or_else(|| file_system.exe_dir().join("themes"));
        let theme_gateway = RasiThemeGateway::new(themes_dir);

        // Create controllers
        let mut window_controller = WindowController::new();
        window_controller.set_escape_hides(config.escape_hides);

        let search_controller = SearchController::new()
            .with_debounce(config.search_debounce_ms);

        let mut hotkey_controller = HotkeyController::new();
        hotkey_controller.register_default_toggle();

        // Create presenters
        let search_presenter = SearchPresenter::new();
        let theme_presenter = ThemePresenter::new();

        // Create application services
        let window_manager = WindowManager::new(animator)
            .with_durations(config.show_duration_ms, config.hide_duration_ms);

        let theme_manager = ThemeManager::new(Arc::new(theme_gateway));

        Self {
            window_controller,
            search_controller,
            hotkey_controller,
            search_presenter,
            theme_presenter,
            window_manager,
            theme_manager,
            file_system,
        }
    }

    /// Get history file path
    pub fn history_path(&self) -> PathBuf {
        self.file_system.app_data_dir().join("history.txt")
    }

    /// Get themes directory
    pub fn themes_dir(&self) -> PathBuf {
        self.file_system.exe_dir().join("themes")
    }
}

impl Default for CompositionRoot {
    fn default() -> Self {
        Self::new()
    }
}

/// Configuration for the composition root
#[derive(Clone, Debug)]
pub struct CompositionConfig {
    /// Whether Escape hides the window (vs exiting)
    pub escape_hides: bool,
    /// Search debounce time in milliseconds
    pub search_debounce_ms: u32,
    /// Show animation duration in milliseconds
    pub show_duration_ms: u32,
    /// Hide animation duration in milliseconds
    pub hide_duration_ms: u32,
    /// Custom themes directory
    pub themes_dir: Option<PathBuf>,
    /// Custom history file path
    pub history_path: Option<PathBuf>,
}

impl Default for CompositionConfig {
    fn default() -> Self {
        Self {
            escape_hides: true,
            search_debounce_ms: 100,
            show_duration_ms: 200,
            hide_duration_ms: 150,
            themes_dir: None,
            history_path: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_composition_root_creation() {
        let root = CompositionRoot::new();

        // Verify components are created
        assert!(root.search_presenter.is_empty());
    }

    #[test]
    fn test_custom_config() {
        let config = CompositionConfig {
            escape_hides: false,
            search_debounce_ms: 200,
            show_duration_ms: 300,
            hide_duration_ms: 200,
            themes_dir: Some(PathBuf::from("./custom_themes")),
            history_path: None,
        };

        let _root = CompositionRoot::with_config(config);
        // Just verify it doesn't panic
    }
}
