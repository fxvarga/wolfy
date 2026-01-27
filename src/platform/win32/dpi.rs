//! DPI awareness utilities for Windows

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::HiDpi::*;

/// Enable per-monitor DPI awareness (call early in main)
pub fn enable_dpi_awareness() -> Result<(), windows::core::Error> {
    unsafe {
        // Try V2 first (Windows 10 1703+)
        if SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2).is_ok() {
            return Ok(());
        }
        // Fall back to V1
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE)
    }
}

/// Get DPI for a specific window
pub fn get_window_dpi(hwnd: HWND) -> u32 {
    unsafe { GetDpiForWindow(hwnd) }
}

/// Get scale factor (1.0 = 100%, 1.5 = 150%, 2.0 = 200%)
pub fn get_scale_factor(hwnd: HWND) -> f32 {
    get_window_dpi(hwnd) as f32 / 96.0
}

/// Scale a pixel value by DPI
pub fn scale_px(px: i32, dpi: u32) -> i32 {
    ((px as f64) * (dpi as f64) / 96.0).round() as i32
}

/// Unscale a pixel value (convert from physical to logical)
pub fn unscale_px(px: i32, dpi: u32) -> i32 {
    ((px as f64) * 96.0 / (dpi as f64)).round() as i32
}

/// DPI information struct
#[derive(Clone, Copy, Debug)]
pub struct DpiInfo {
    pub dpi: u32,
    pub scale_factor: f32,
}

impl DpiInfo {
    pub fn for_window(hwnd: HWND) -> Self {
        let dpi = get_window_dpi(hwnd);
        Self {
            dpi,
            scale_factor: dpi as f32 / 96.0,
        }
    }

    pub fn default_96() -> Self {
        Self {
            dpi: 96,
            scale_factor: 1.0,
        }
    }
}
