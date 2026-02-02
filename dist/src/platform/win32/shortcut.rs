//! Windows shortcut (.lnk) file parser using COM interfaces
//!
//! Uses IShellLinkW and IPersistFile to read shortcut properties.

use std::path::Path;

use windows::core::{Interface, GUID, PCWSTR};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, IPersistFile, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
    STGM_READ,
};
use windows::Win32::UI::Shell::IShellLinkW;

/// CLSID for ShellLink COM object
const CLSID_SHELL_LINK: GUID = GUID::from_u128(0x00021401_0000_0000_C000_000000000046);

/// Information extracted from a .lnk shortcut file
#[derive(Debug, Clone, Default)]
pub struct ShortcutInfo {
    /// Target path (the executable or file the shortcut points to)
    pub target_path: String,
    /// Description/comment from the shortcut
    pub description: String,
    /// Working directory
    pub working_directory: String,
    /// Command line arguments
    pub arguments: String,
    /// Icon location (path to icon file or exe)
    pub icon_path: String,
    /// Icon index within the icon file
    pub icon_index: i32,
}

impl ShortcutInfo {
    /// Check if the target path exists
    pub fn target_exists(&self) -> bool {
        if self.target_path.is_empty() {
            return false;
        }
        Path::new(&self.target_path).exists()
    }

    /// Get the best path for icon extraction
    /// Prefers explicit icon path, falls back to target path
    pub fn icon_source(&self) -> &str {
        if !self.icon_path.is_empty() && Path::new(&self.icon_path).exists() {
            &self.icon_path
        } else if !self.target_path.is_empty() {
            &self.target_path
        } else {
            ""
        }
    }
}

/// Parse a .lnk shortcut file and extract its properties
///
/// # Arguments
/// * `lnk_path` - Path to the .lnk file
///
/// # Returns
/// * `Ok(ShortcutInfo)` - Successfully parsed shortcut information
/// * `Err(String)` - Error message if parsing failed
pub fn parse_lnk(lnk_path: &Path) -> Result<ShortcutInfo, String> {
    unsafe {
        // Initialize COM (safe to call multiple times)
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

        // Create IShellLinkW instance
        let shell_link: IShellLinkW =
            CoCreateInstance(&CLSID_SHELL_LINK, None, CLSCTX_INPROC_SERVER)
                .map_err(|e| format!("Failed to create ShellLink: {:?}", e))?;

        // Query for IPersistFile interface
        let persist_file: IPersistFile = shell_link
            .cast()
            .map_err(|e| format!("Failed to get IPersistFile: {:?}", e))?;

        // Convert path to wide string
        let path_str = lnk_path.to_string_lossy();
        let path_wide: Vec<u16> = path_str.encode_utf16().chain(std::iter::once(0)).collect();

        // Load the .lnk file
        persist_file
            .Load(PCWSTR(path_wide.as_ptr()), STGM_READ)
            .map_err(|e| format!("Failed to load .lnk file: {:?}", e))?;

        // Extract shortcut information
        let mut info = ShortcutInfo::default();

        // Get target path
        let mut target_buf = [0u16; 260];
        if shell_link
            .GetPath(&mut target_buf, std::ptr::null_mut(), 0)
            .is_ok()
        {
            info.target_path = wstr_to_string(&target_buf);
        }

        // Get description
        let mut desc_buf = [0u16; 260];
        if shell_link.GetDescription(&mut desc_buf).is_ok() {
            info.description = wstr_to_string(&desc_buf);
        }

        // Get working directory
        let mut workdir_buf = [0u16; 260];
        if shell_link.GetWorkingDirectory(&mut workdir_buf).is_ok() {
            info.working_directory = wstr_to_string(&workdir_buf);
        }

        // Get arguments
        let mut args_buf = [0u16; 1024];
        if shell_link.GetArguments(&mut args_buf).is_ok() {
            info.arguments = wstr_to_string(&args_buf);
        }

        // Get icon location
        let mut icon_buf = [0u16; 260];
        let mut icon_index: i32 = 0;
        if shell_link
            .GetIconLocation(&mut icon_buf, &mut icon_index)
            .is_ok()
        {
            info.icon_path = wstr_to_string(&icon_buf);
            info.icon_index = icon_index;
        }

        Ok(info)
    }
}

/// Convert a null-terminated wide string to a Rust String
fn wstr_to_string(wstr: &[u16]) -> String {
    let len = wstr.iter().position(|&c| c == 0).unwrap_or(wstr.len());
    String::from_utf16_lossy(&wstr[..len])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shortcut_info_default() {
        let info = ShortcutInfo::default();
        assert!(info.target_path.is_empty());
        assert!(info.description.is_empty());
        assert!(!info.target_exists());
    }

    #[test]
    fn test_shortcut_info_target_exists() {
        let mut info = ShortcutInfo::default();

        // Non-existent path
        info.target_path = "C:\\NonExistent\\fake.exe".to_string();
        assert!(!info.target_exists());

        // Existing path (notepad should exist on Windows)
        info.target_path = "C:\\Windows\\System32\\notepad.exe".to_string();
        assert!(info.target_exists());
    }

    #[test]
    fn test_shortcut_info_icon_source() {
        let mut info = ShortcutInfo::default();

        // No paths set
        assert!(info.icon_source().is_empty());

        // Only target path
        info.target_path = "C:\\Windows\\System32\\notepad.exe".to_string();
        assert_eq!(info.icon_source(), "C:\\Windows\\System32\\notepad.exe");

        // Icon path set (but doesn't exist, so falls back to target)
        info.icon_path = "C:\\NonExistent\\icon.ico".to_string();
        assert_eq!(info.icon_source(), "C:\\Windows\\System32\\notepad.exe");

        // Icon path set and exists
        info.icon_path = "C:\\Windows\\System32\\notepad.exe".to_string();
        assert_eq!(info.icon_source(), "C:\\Windows\\System32\\notepad.exe");
    }

    #[test]
    fn test_wstr_to_string() {
        // Empty string
        let empty: [u16; 1] = [0];
        assert_eq!(wstr_to_string(&empty), "");

        // Normal string
        let hello: [u16; 6] = [72, 101, 108, 108, 111, 0]; // "Hello\0"
        assert_eq!(wstr_to_string(&hello), "Hello");

        // String with extra buffer space
        let mut buf = [0u16; 10];
        buf[0] = 'H' as u16;
        buf[1] = 'i' as u16;
        assert_eq!(wstr_to_string(&buf), "Hi");
    }
}
