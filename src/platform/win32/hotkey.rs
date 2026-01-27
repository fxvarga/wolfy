//! Global hotkey registration for Windows

use windows::core::Error;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS, MOD_ALT, MOD_NOREPEAT,
};
use windows::Win32::UI::WindowsAndMessaging::WM_HOTKEY;

/// Hotkey identifier for toggle (Alt+Space)
pub const HOTKEY_ID_TOGGLE: i32 = 1;

/// Virtual key code for Space
const VK_SPACE: u32 = 0x20;

/// Register the global hotkey (Alt+Space)
///
/// # Arguments
/// * `hwnd` - The window that will receive WM_HOTKEY messages
///
/// # Returns
/// * `Ok(())` if registration succeeds
/// * `Err` if registration fails (e.g., another app has the hotkey)
pub fn register_hotkey(hwnd: HWND) -> Result<(), Error> {
    unsafe {
        // MOD_ALT | MOD_NOREPEAT: Alt key, don't repeat when held
        let modifiers = HOT_KEY_MODIFIERS(MOD_ALT.0 | MOD_NOREPEAT.0);
        RegisterHotKey(hwnd, HOTKEY_ID_TOGGLE, modifiers, VK_SPACE)?;
        Ok(())
    }
}

/// Unregister the global hotkey
pub fn unregister_hotkey(hwnd: HWND) {
    unsafe {
        let _ = UnregisterHotKey(hwnd, HOTKEY_ID_TOGGLE);
    }
}

/// Check if a WM_HOTKEY message is for our toggle hotkey
///
/// # Arguments
/// * `msg` - The message type
/// * `wparam` - The WPARAM containing the hotkey ID
///
/// # Returns
/// * `true` if this is our toggle hotkey
pub fn is_toggle_hotkey(msg: u32, wparam: usize) -> bool {
    msg == WM_HOTKEY && wparam as i32 == HOTKEY_ID_TOGGLE
}

/// Custom hotkey configuration
#[derive(Clone, Copy, Debug)]
pub struct HotkeyConfig {
    pub id: i32,
    pub modifiers: u32,
    pub vk_code: u32,
}

impl HotkeyConfig {
    /// Create a new hotkey config
    pub const fn new(id: i32, modifiers: u32, vk_code: u32) -> Self {
        Self {
            id,
            modifiers,
            vk_code,
        }
    }

    /// Register this hotkey
    pub fn register(&self, hwnd: HWND) -> Result<(), Error> {
        unsafe {
            let mods = HOT_KEY_MODIFIERS(self.modifiers | MOD_NOREPEAT.0);
            RegisterHotKey(hwnd, self.id, mods, self.vk_code)?;
            Ok(())
        }
    }

    /// Unregister this hotkey
    pub fn unregister(&self, hwnd: HWND) {
        unsafe {
            let _ = UnregisterHotKey(hwnd, self.id);
        }
    }

    /// Check if a message is for this hotkey
    pub fn matches(&self, msg: u32, wparam: usize) -> bool {
        msg == WM_HOTKEY && wparam as i32 == self.id
    }
}

/// Default toggle hotkey (Alt+Space)
pub const DEFAULT_TOGGLE_HOTKEY: HotkeyConfig =
    HotkeyConfig::new(HOTKEY_ID_TOGGLE, MOD_ALT.0, VK_SPACE);
