//! Win32 platform implementation

pub mod dpi;
pub mod event;
pub mod hotkey;
pub mod render;
pub mod window;

pub use dpi::*;
pub use event::{post_quit, run_message_loop, translate_message, Event, KeyCode, Modifiers};
pub use hotkey::{is_toggle_hotkey, register_hotkey, unregister_hotkey, HOTKEY_ID_TOGGLE};
pub use render::Renderer;
pub use window::{
    clear_window_callback, create_window, destroy_window, get_client_size, hide_window,
    invalidate_window, is_window_visible, register_window_class, reposition_window,
    set_window_callback, set_window_opacity, show_window, toggle_window, unregister_window_class,
    WindowConfig,
};
