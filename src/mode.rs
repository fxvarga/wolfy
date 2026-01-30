//! Operating modes for Wolfy
//!
//! Wolfy supports multiple modes triggered by different hotkeys:
//! - Launcher: App search with task panel (Ctrl+0)
//! - ThemePicker: Grid of Hyde themes (Ctrl+1)
//! - WallpaperPicker: Grid of wallpapers (Ctrl+2)

use crate::platform::win32::{HOTKEY_ID_LAUNCHER, HOTKEY_ID_THEME, HOTKEY_ID_WALLPAPER};

/// Operating mode for Wolfy
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Mode {
    /// App launcher with search and task panel
    #[default]
    Launcher,
    /// Grid view of Hyde themes for selection
    ThemePicker,
    /// Grid view of wallpapers for selection
    WallpaperPicker,
}

impl Mode {
    /// Get the hotkey ID associated with this mode
    pub fn hotkey_id(&self) -> i32 {
        match self {
            Mode::Launcher => HOTKEY_ID_LAUNCHER,
            Mode::ThemePicker => HOTKEY_ID_THEME,
            Mode::WallpaperPicker => HOTKEY_ID_WALLPAPER,
        }
    }

    /// Create a Mode from a hotkey ID
    pub fn from_hotkey_id(id: i32) -> Option<Mode> {
        match id {
            HOTKEY_ID_LAUNCHER => Some(Mode::Launcher),
            HOTKEY_ID_THEME => Some(Mode::ThemePicker),
            HOTKEY_ID_WALLPAPER => Some(Mode::WallpaperPicker),
            _ => None,
        }
    }

    /// Whether this mode shows the task panel sidebar
    pub fn has_task_panel(&self) -> bool {
        matches!(self, Mode::Launcher)
    }

    /// Whether this mode uses grid view (vs list view)
    pub fn uses_grid_view(&self) -> bool {
        matches!(self, Mode::ThemePicker | Mode::WallpaperPicker)
    }

    /// Whether this mode shows the wallpaper panel on the left
    pub fn has_wallpaper_panel(&self) -> bool {
        matches!(self, Mode::Launcher)
    }

    /// Get a display name for this mode
    pub fn display_name(&self) -> &'static str {
        match self {
            Mode::Launcher => "App Launcher",
            Mode::ThemePicker => "Theme Picker",
            Mode::WallpaperPicker => "Wallpaper Picker",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mode_from_hotkey_id() {
        assert_eq!(
            Mode::from_hotkey_id(HOTKEY_ID_LAUNCHER),
            Some(Mode::Launcher)
        );
        assert_eq!(
            Mode::from_hotkey_id(HOTKEY_ID_THEME),
            Some(Mode::ThemePicker)
        );
        assert_eq!(
            Mode::from_hotkey_id(HOTKEY_ID_WALLPAPER),
            Some(Mode::WallpaperPicker)
        );
        assert_eq!(Mode::from_hotkey_id(999), None);
    }

    #[test]
    fn test_mode_features() {
        assert!(Mode::Launcher.has_task_panel());
        assert!(!Mode::ThemePicker.has_task_panel());
        assert!(!Mode::WallpaperPicker.has_task_panel());

        assert!(!Mode::Launcher.uses_grid_view());
        assert!(Mode::ThemePicker.uses_grid_view());
        assert!(Mode::WallpaperPicker.uses_grid_view());

        assert!(Mode::Launcher.has_wallpaper_panel());
        assert!(!Mode::ThemePicker.has_wallpaper_panel());
        assert!(!Mode::WallpaperPicker.has_wallpaper_panel());
    }

    #[test]
    fn test_default_mode() {
        assert_eq!(Mode::default(), Mode::Launcher);
    }
}
