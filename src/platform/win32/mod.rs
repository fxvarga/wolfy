//! Win32 platform implementation

pub mod apps;
pub mod dpi;
pub mod event;
pub mod filewatcher;
pub mod hotkey;
pub mod icon;
pub mod image;
pub mod render;
pub mod shortcut;
pub mod wallpaper;
pub mod window;

pub use apps::{discover_all_apps, AppEntry};
pub use dpi::*;
pub use event::{
    post_quit, run_message_loop, translate_message, Event, KeyCode, Modifiers, MouseButton,
};
pub use filewatcher::PollingFileWatcher;
pub use hotkey::{
    is_toggle_hotkey, parse_hotkey_string, register_hotkey, unregister_hotkey, HotkeyConfig,
    DEFAULT_TOGGLE_HOTKEY, HOTKEY_ID_TOGGLE,
};
pub use icon::{CachedIcon, IconLoader};
pub use image::{ImageLoader, LoadedImage};
pub use render::Renderer;
pub use shortcut::{parse_lnk, ShortcutInfo};
pub use wallpaper::get_wallpaper_path;
pub use window::{
    clear_window_callback, create_window, destroy_window, get_client_size, hide_window,
    invalidate_window, is_window_visible, register_window_class, reposition_window,
    set_window_callback, set_window_opacity, show_window, toggle_window, unregister_window_class,
    WindowConfig,
};
