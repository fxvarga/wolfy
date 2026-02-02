//! Windows wallpaper detection and setting
//!
//! Detects the current desktop wallpaper path using:
//! 1. IDesktopWallpaper COM interface (Windows 8+, multi-monitor support)
//! 2. SystemParametersInfo fallback (Windows 7 compatible)
//! 3. Registry fallback

use std::path::PathBuf;

use windows::core::PCWSTR;
use windows::Win32::Foundation::MAX_PATH;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_APARTMENTTHREADED,
};
use windows::Win32::UI::Shell::{DesktopWallpaper, IDesktopWallpaper};
use windows::Win32::UI::WindowsAndMessaging::{
    SystemParametersInfoW, SPI_GETDESKWALLPAPER, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
};

/// Get the current desktop wallpaper path
///
/// This function tries multiple methods to detect the wallpaper:
/// 1. SystemParametersInfoW (classic API, safest)
/// 2. Registry (fallback)
/// 3. IDesktopWallpaper COM interface (skipped for now due to threading issues)
pub fn get_wallpaper_path() -> Option<PathBuf> {
    crate::log!("get_wallpaper_path() called");

    // Try SystemParametersInfo first (safest, no COM)
    if let Some(path) = get_wallpaper_via_spi() {
        crate::log!("get_wallpaper_path() returning SPI result");
        return Some(path);
    }

    // Fallback to registry
    if let Some(path) = get_wallpaper_via_registry() {
        crate::log!("get_wallpaper_path() returning registry result");
        return Some(path);
    }

    // Skip COM for now - it can cause issues with Direct2D's COM usage
    // if let Some(path) = get_wallpaper_via_com() {
    //     return Some(path);
    // }

    crate::log!("get_wallpaper_path() returning None");
    None
}

/// Get wallpaper for a specific monitor index (0-based)
/// Returns None if the monitor doesn't exist or has no wallpaper
pub fn get_wallpaper_for_monitor(monitor_index: u32) -> Option<PathBuf> {
    unsafe {
        // Initialize COM if not already done
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

        let wallpaper: Result<IDesktopWallpaper, _> =
            CoCreateInstance(&DesktopWallpaper, None, CLSCTX_ALL);

        if let Ok(wallpaper) = wallpaper {
            // Get monitor device path
            if let Ok(monitor_id) = wallpaper.GetMonitorDevicePathAt(monitor_index) {
                // Get wallpaper for this specific monitor
                if let Ok(path) = wallpaper.GetWallpaper(PCWSTR(monitor_id.as_ptr())) {
                    let path_str = path.to_string().ok()?;
                    if !path_str.is_empty() {
                        return Some(PathBuf::from(path_str));
                    }
                }
            }
        }
    }

    None
}

/// Get wallpaper using IDesktopWallpaper COM interface
fn get_wallpaper_via_com() -> Option<PathBuf> {
    unsafe {
        // Initialize COM if not already done
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

        let wallpaper: Result<IDesktopWallpaper, _> =
            CoCreateInstance(&DesktopWallpaper, None, CLSCTX_ALL);

        match wallpaper {
            Ok(wallpaper) => {
                // Get wallpaper for the primary monitor (NULL = primary)
                match wallpaper.GetWallpaper(PCWSTR::null()) {
                    Ok(path) => {
                        let path_str = path.to_string().ok()?;
                        if !path_str.is_empty() {
                            crate::log!("Wallpaper via COM: {}", path_str);
                            return Some(PathBuf::from(path_str));
                        }
                    }
                    Err(e) => {
                        crate::log!("IDesktopWallpaper::GetWallpaper failed: {:?}", e);
                    }
                }
            }
            Err(e) => {
                crate::log!("Failed to create IDesktopWallpaper: {:?}", e);
            }
        }
    }

    None
}

/// Get wallpaper using SystemParametersInfoW
fn get_wallpaper_via_spi() -> Option<PathBuf> {
    unsafe {
        let mut buffer: [u16; MAX_PATH as usize] = [0; MAX_PATH as usize];

        let result = SystemParametersInfoW(
            SPI_GETDESKWALLPAPER,
            buffer.len() as u32,
            Some(buffer.as_mut_ptr() as *mut _),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        );

        if result.is_ok() {
            // Find null terminator
            let len = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
            let path_str = String::from_utf16_lossy(&buffer[..len]);

            if !path_str.is_empty() {
                crate::log!("Wallpaper via SPI: {}", path_str);
                return Some(PathBuf::from(path_str));
            }
        }
    }

    None
}

/// Get wallpaper from registry (fallback)
fn get_wallpaper_via_registry() -> Option<PathBuf> {
    use windows::core::w;
    use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_SZ};

    unsafe {
        let mut buffer: [u16; MAX_PATH as usize] = [0; MAX_PATH as usize];
        let mut size = (buffer.len() * 2) as u32;

        let result = RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Control Panel\\Desktop"),
            w!("Wallpaper"),
            RRF_RT_REG_SZ,
            None,
            Some(buffer.as_mut_ptr() as *mut _),
            Some(&mut size),
        );

        if result.is_ok() {
            let len = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
            let path_str = String::from_utf16_lossy(&buffer[..len]);

            if !path_str.is_empty() {
                crate::log!("Wallpaper via registry: {}", path_str);
                return Some(PathBuf::from(path_str));
            }
        }
    }

    None
}

/// Check if the system is using a solid color instead of a wallpaper
pub fn is_solid_color_background() -> bool {
    get_wallpaper_path().is_none()
}

/// Get the number of monitors with wallpapers
pub fn get_monitor_count() -> u32 {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

        let wallpaper: Result<IDesktopWallpaper, _> =
            CoCreateInstance(&DesktopWallpaper, None, CLSCTX_ALL);

        if let Ok(wallpaper) = wallpaper {
            if let Ok(count) = wallpaper.GetMonitorDevicePathCount() {
                return count;
            }
        }
    }

    1 // Assume at least one monitor
}

/// Set the desktop wallpaper
///
/// Uses direct Win32 API call via SystemParametersInfoW for speed.
/// Falls back to PowerShell if the direct method fails.
/// Returns true if successful, false otherwise.
pub fn set_wallpaper(path: &str) -> bool {
    use std::path::Path;
    use windows::Win32::UI::WindowsAndMessaging::{
        SystemParametersInfoW, SPI_SETDESKWALLPAPER, SPIF_SENDWININICHANGE, SPIF_UPDATEINIFILE,
        SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
    };

    crate::log!("set_wallpaper() called with path: {}", path);

    // Normalize the path - resolve ../ and ./ segments
    let normalized_path = match Path::new(path).canonicalize() {
        Ok(p) => {
            // Remove the \\?\ prefix that canonicalize adds on Windows
            let s = p.to_string_lossy().to_string();
            if s.starts_with(r"\\?\") {
                s[4..].to_string()
            } else {
                s
            }
        }
        Err(e) => {
            crate::log!("set_wallpaper() failed to canonicalize path: {:?}", e);
            return false;
        }
    };

    crate::log!("set_wallpaper() normalized path: {}", normalized_path);

    // Verify the file exists
    if !Path::new(&normalized_path).exists() {
        crate::log!("set_wallpaper() failed: file does not exist");
        return false;
    }

    crate::log!("set_wallpaper() file exists, calling SystemParametersInfoW...");

    // Convert path to wide string for Windows API
    // Use PCWSTR wrapper for proper pointer handling
    let wide_path: Vec<u16> = normalized_path.encode_utf16().chain(std::iter::once(0)).collect();

    crate::log!("set_wallpaper() wide_path len={}, ptr={:?}", wide_path.len(), wide_path.as_ptr());

    // Try direct Win32 API call first (much faster than PowerShell)
    // Note: SystemParametersInfoW expects pvParam as a pointer to a null-terminated wide string
    // We use SPIF_UPDATEINIFILE only (not SPIF_SENDWININICHANGE) to avoid broadcast message issues
    let result = unsafe {
        SystemParametersInfoW(
            SPI_SETDESKWALLPAPER,
            0,
            Some(wide_path.as_ptr() as *mut std::ffi::c_void),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(SPIF_UPDATEINIFILE.0),
        )
    };

    crate::log!("set_wallpaper() SystemParametersInfoW returned: {:?}", result);

    if result.is_ok() {
        crate::log!("set_wallpaper() succeeded via Win32 API");
        return true;
    }

    crate::log!("set_wallpaper() Win32 API failed, falling back to PowerShell");

    // Fallback to PowerShell (hidden window, no profile for speed)
    set_wallpaper_powershell(&normalized_path)
}

/// Set wallpaper using PowerShell (fallback method)
fn set_wallpaper_powershell(path: &str) -> bool {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    const CREATE_NO_WINDOW: u32 = 0x08000000;

    // Escape the path for PowerShell
    let escaped_path = path.replace("'", "''");

    // PowerShell command to set wallpaper
    let ps_script = format!(
        r#"Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public class Wallpaper {{
    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    public static extern int SystemParametersInfo(int uAction, int uParam, string lpvParam, int fuWinIni);
}}
"@
[Wallpaper]::SystemParametersInfo(0x0014, 0, '{}', 0x01 -bor 0x02)"#,
        escaped_path
    );

    crate::log!("set_wallpaper() running PowerShell command (hidden)...");

    let result = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &ps_script])
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    match result {
        Ok(output) => {
            if output.status.success() {
                crate::log!("set_wallpaper() succeeded via PowerShell");
                true
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                crate::log!("set_wallpaper() PowerShell failed: {}", stderr);
                false
            }
        }
        Err(e) => {
            crate::log!("set_wallpaper() failed to run PowerShell: {:?}", e);
            false
        }
    }
}
