//! Global hotkey registration for Windows

use windows::core::Error;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT,
};
use windows::Win32::UI::WindowsAndMessaging::WM_HOTKEY;

/// Hotkey identifier for toggle
pub const HOTKEY_ID_TOGGLE: i32 = 1;

// Virtual key codes
const VK_SPACE: u32 = 0x20;
const VK_0: u32 = 0x30; // 0-9 are 0x30-0x39
const VK_A: u32 = 0x41; // A-Z are 0x41-0x5A

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

/// Parse a hotkey string like "alt+space", "ctrl+space", "alt+r", "ctrl+1"
/// Returns None if the format is invalid
///
/// Supported formats:
/// - "alt+space" or "ctrl+space" - Modifier + Space key
/// - "alt+a" through "alt+z" or "ctrl+a" through "ctrl+z" - Modifier + letter
/// - "alt+0" through "alt+9" or "ctrl+0" through "ctrl+9" - Modifier + number
pub fn parse_hotkey_string(s: &str) -> Option<HotkeyConfig> {
    let s = s.trim().to_lowercase();

    // Determine modifier and key part
    let (modifier, key_part) = if s.starts_with("ctrl+") {
        (MOD_CONTROL.0, &s[5..])
    } else if s.starts_with("alt+") {
        (MOD_ALT.0, &s[4..])
    } else {
        return None;
    };

    let vk_code = match key_part {
        "space" => VK_SPACE,
        // Single letter a-z
        k if k.len() == 1 => {
            let c = k.chars().next()?;
            if c.is_ascii_lowercase() {
                VK_A + (c as u32 - 'a' as u32)
            } else if c.is_ascii_digit() {
                VK_0 + (c as u32 - '0' as u32)
            } else {
                return None;
            }
        }
        _ => return None,
    };

    Some(HotkeyConfig::new(HOTKEY_ID_TOGGLE, modifier, vk_code))
}

/// Get a human-readable name for a hotkey config
impl HotkeyConfig {
    /// Get a display name like "Alt+Space", "Ctrl+Space", "Alt+R"
    pub fn display_name(&self) -> String {
        let key_name = match self.vk_code {
            VK_SPACE => "Space".to_string(),
            vk if vk >= VK_A && vk <= VK_A + 25 => {
                let c = (b'A' + (vk - VK_A) as u8) as char;
                c.to_string()
            }
            vk if vk >= VK_0 && vk <= VK_0 + 9 => {
                let c = (b'0' + (vk - VK_0) as u8) as char;
                c.to_string()
            }
            _ => format!("0x{:02X}", self.vk_code),
        };

        if self.modifiers & MOD_CONTROL.0 != 0 {
            format!("Ctrl+{}", key_name)
        } else if self.modifiers & MOD_ALT.0 != 0 {
            format!("Alt+{}", key_name)
        } else {
            key_name
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_alt_space() {
        let config = parse_hotkey_string("alt+space").unwrap();
        assert_eq!(config.vk_code, VK_SPACE);
        assert_eq!(config.modifiers, MOD_ALT.0);
        assert_eq!(config.display_name(), "Alt+Space");
    }

    #[test]
    fn test_parse_ctrl_space() {
        let config = parse_hotkey_string("ctrl+space").unwrap();
        assert_eq!(config.vk_code, VK_SPACE);
        assert_eq!(config.modifiers, MOD_CONTROL.0);
        assert_eq!(config.display_name(), "Ctrl+Space");
    }

    #[test]
    fn test_parse_alt_letter() {
        let config = parse_hotkey_string("alt+r").unwrap();
        assert_eq!(config.vk_code, VK_A + 17); // 'r' is 17th letter (0-indexed)
        assert_eq!(config.display_name(), "Alt+R");
    }

    #[test]
    fn test_parse_ctrl_letter() {
        let config = parse_hotkey_string("ctrl+r").unwrap();
        assert_eq!(config.vk_code, VK_A + 17);
        assert_eq!(config.display_name(), "Ctrl+R");
    }

    #[test]
    fn test_parse_alt_number() {
        let config = parse_hotkey_string("alt+1").unwrap();
        assert_eq!(config.vk_code, VK_0 + 1);
        assert_eq!(config.display_name(), "Alt+1");
    }

    #[test]
    fn test_parse_ctrl_number() {
        let config = parse_hotkey_string("ctrl+1").unwrap();
        assert_eq!(config.vk_code, VK_0 + 1);
        assert_eq!(config.display_name(), "Ctrl+1");
    }

    #[test]
    fn test_parse_case_insensitive() {
        let config1 = parse_hotkey_string("Alt+Space").unwrap();
        let config2 = parse_hotkey_string("ALT+SPACE").unwrap();
        let config3 = parse_hotkey_string("alt+space").unwrap();
        assert_eq!(config1.vk_code, config2.vk_code);
        assert_eq!(config2.vk_code, config3.vk_code);

        let config4 = parse_hotkey_string("Ctrl+Space").unwrap();
        let config5 = parse_hotkey_string("CTRL+SPACE").unwrap();
        assert_eq!(config4.vk_code, config5.vk_code);
        assert_eq!(config4.modifiers, config5.modifiers);
    }

    #[test]
    fn test_parse_invalid() {
        assert!(parse_hotkey_string("shift+space").is_none()); // shift not supported
        assert!(parse_hotkey_string("alt+").is_none());
        assert!(parse_hotkey_string("ctrl+").is_none());
        assert!(parse_hotkey_string("space").is_none());
        assert!(parse_hotkey_string("alt+foo").is_none());
        assert!(parse_hotkey_string("ctrl+foo").is_none());
    }

    #[test]
    fn test_display_name() {
        assert_eq!(DEFAULT_TOGGLE_HOTKEY.display_name(), "Alt+Space");
    }
}
