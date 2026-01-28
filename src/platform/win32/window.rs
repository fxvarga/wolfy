//! Win32 window creation and management

use std::cell::RefCell;

use windows::core::{w, Error, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, InvalidateRect, MonitorFromWindow, HBRUSH, MONITORINFO,
    MONITOR_DEFAULTTOPRIMARY,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::GetDpiForSystem;
use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
use windows::Win32::UI::WindowsAndMessaging::*;

use super::dpi::{scale_px, DpiInfo};

/// Custom message to request window hide (deferred to avoid re-entrancy)
pub const WM_APP_HIDE: u32 = WM_APP + 1;

/// Window configuration
#[derive(Clone, Debug)]
pub struct WindowConfig {
    /// Logical width in pixels (at 96 DPI)
    pub width: i32,
    /// Logical height in pixels (at 96 DPI)
    pub height: i32,
    /// Vertical position: 0.0 = top, 0.5 = center, 1.0 = bottom
    pub vertical_position: f32,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            width: 600,
            height: 48,
            vertical_position: 0.5, // True center of screen
        }
    }
}

/// Thread-local storage for window procedure callback data
thread_local! {
    static WINDOW_CALLBACK: RefCell<Option<Box<dyn FnMut(HWND, u32, WPARAM, LPARAM) -> Option<LRESULT>>>> = RefCell::new(None);
}

/// Set the window procedure callback
pub fn set_window_callback<F>(callback: F)
where
    F: FnMut(HWND, u32, WPARAM, LPARAM) -> Option<LRESULT> + 'static,
{
    WINDOW_CALLBACK.with(|cb| {
        *cb.borrow_mut() = Some(Box::new(callback));
    });
}

/// Clear the window procedure callback
pub fn clear_window_callback() {
    WINDOW_CALLBACK.with(|cb| {
        *cb.borrow_mut() = None;
    });
}

/// Window procedure
unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // Log important messages for debugging
    match msg {
        WM_CLOSE | WM_DESTROY | WM_QUIT | WM_SYSCOMMAND | WM_KEYDOWN | WM_CHAR => {
            log!(
                "wnd_proc: msg=0x{:04X}, wparam=0x{:X}, lparam=0x{:X}",
                msg,
                wparam.0,
                lparam.0
            );
        }
        _ => {}
    }

    // Try to call the user callback first
    let result = WINDOW_CALLBACK.with(|cb| {
        if let Some(ref mut callback) = *cb.borrow_mut() {
            callback(hwnd, msg, wparam, lparam)
        } else {
            None
        }
    });

    if let Some(r) = result {
        log!("  -> callback returned LRESULT({})", r.0);
        return r;
    }

    log!("  -> callback returned None, using default handling");

    // Default handling
    match msg {
        WM_APP_HIDE => {
            // Deferred hide request - safe to call now
            log!("WM_APP_HIDE received - hiding window");
            let _ = ShowWindow(hwnd, SW_HIDE);
            LRESULT(0)
        }
        WM_SYSCOMMAND => {
            // Intercept SC_CLOSE (which can be triggered by ESC in some window configurations)
            let cmd = wparam.0 & 0xFFF0;
            log!("WM_SYSCOMMAND received: cmd=0x{:04X}", cmd);
            if cmd == SC_CLOSE as usize {
                log!("  SC_CLOSE - posting hide request");
                let _ = PostMessageW(hwnd, WM_APP_HIDE, WPARAM(0), LPARAM(0));
                return LRESULT(0);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_CHAR => {
            // Consume ESC and Enter WM_CHAR messages to prevent default dialog behavior
            // These are already handled via WM_KEYDOWN
            let ch = wparam.0 as u16;
            if ch == 0x1B || ch == 0x0D {
                // ESC (0x1B) or Enter/CR (0x0D) - consume without action
                log!("WM_CHAR: consuming ESC/Enter (ch=0x{:02X})", ch);
                return LRESULT(0);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_CLOSE => {
            // Don't close/destroy the window - just hide it
            // User can press Alt+Space to show it again
            log!("WM_CLOSE received - posting hide request");
            let _ = PostMessageW(hwnd, WM_APP_HIDE, WPARAM(0), LPARAM(0));
            LRESULT(0)
        }
        WM_DESTROY => {
            log!("WM_DESTROY received - posting quit message");
            PostQuitMessage(0);
            LRESULT(0)
        }
        WM_ERASEBKGND => {
            // Prevent background erase flickering - we handle all painting
            LRESULT(1)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

const WINDOW_CLASS_NAME: PCWSTR = w!("WolfyWindowClass");

/// Register the window class (call once at startup)
pub fn register_window_class() -> Result<(), Error> {
    unsafe {
        let hinstance = GetModuleHandleW(None)?;

        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinstance.into(),
            hIcon: HICON::default(),
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            hbrBackground: HBRUSH::default(), // No background brush - we paint everything
            lpszMenuName: PCWSTR::null(),
            lpszClassName: WINDOW_CLASS_NAME,
            hIconSm: HICON::default(),
        };

        let atom = RegisterClassExW(&wc);
        if atom == 0 {
            return Err(Error::from_win32());
        }

        Ok(())
    }
}

/// Unregister the window class (call at shutdown)
pub fn unregister_window_class() {
    unsafe {
        let _ = GetModuleHandleW(None).map(|h| {
            let _ = UnregisterClassW(WINDOW_CLASS_NAME, h);
        });
    }
}

/// Calculate window position and size for a given monitor
fn calculate_window_rect(config: &WindowConfig, dpi: u32) -> RECT {
    unsafe {
        // Get primary monitor info
        let monitor = MonitorFromWindow(HWND::default(), MONITOR_DEFAULTTOPRIMARY);
        let mut monitor_info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        let _ = GetMonitorInfoW(monitor, &mut monitor_info);

        let work_area = monitor_info.rcWork;
        let work_width = work_area.right - work_area.left;
        let work_height = work_area.bottom - work_area.top;

        // Scale dimensions by DPI
        let scaled_width = scale_px(config.width, dpi);
        let scaled_height = scale_px(config.height, dpi);

        // Center horizontally
        let x = work_area.left + (work_width - scaled_width) / 2;

        // Position vertically based on config
        let available_y = work_height - scaled_height;
        let y = work_area.top + (available_y as f32 * config.vertical_position) as i32;

        RECT {
            left: x,
            top: y,
            right: x + scaled_width,
            bottom: y + scaled_height,
        }
    }
}

/// Create the main window
pub fn create_window(config: &WindowConfig) -> Result<HWND, Error> {
    unsafe {
        let hinstance = GetModuleHandleW(None)?;

        // Get system DPI for initial window size
        let dpi = GetDpiForSystem();
        let rect = calculate_window_rect(config, dpi);

        let hwnd = CreateWindowExW(
            // Extended styles: topmost, tool window (no taskbar), layered for per-pixel alpha
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_LAYERED,
            WINDOW_CLASS_NAME,
            w!("Wolfy"),
            // Popup window (no frame), initially hidden
            WS_POPUP,
            rect.left,
            rect.top,
            rect.right - rect.left,
            rect.bottom - rect.top,
            None,
            None,
            hinstance,
            None,
        )?;

        Ok(hwnd)
    }
}

/// Set window opacity (0.0 = fully transparent, 1.0 = fully opaque)
pub fn set_window_opacity(hwnd: HWND, opacity: f32) {
    let alpha = (opacity.clamp(0.0, 1.0) * 255.0) as u8;
    log!("set_window_opacity: opacity={}, alpha={}", opacity, alpha);
    unsafe {
        let _ = SetLayeredWindowAttributes(hwnd, None, alpha, LWA_ALPHA);
    }
}

/// Show the window with animation
pub fn show_window(hwnd: HWND) {
    log!("show_window() called, hwnd={:?}", hwnd);
    unsafe {
        log!("  Calling ShowWindow(SW_SHOWNOACTIVATE)...");
        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        log!("  Calling SetForegroundWindow()...");
        let _ = SetForegroundWindow(hwnd);
        log!("  Calling SetFocus()...");
        let _ = SetFocus(hwnd);
        log!("  show_window() completed");
    }
}

/// Hide the window (posts a message to hide safely, avoiding re-entrancy)
pub fn hide_window(hwnd: HWND) {
    log!(
        "hide_window() called, hwnd={:?} - posting WM_APP_HIDE",
        hwnd
    );
    unsafe {
        let _ = PostMessageW(hwnd, WM_APP_HIDE, WPARAM(0), LPARAM(0));
    }
}

/// Hide the window immediately (use with caution - can cause re-entrancy)
pub fn hide_window_immediate(hwnd: HWND) {
    log!("hide_window_immediate() called, hwnd={:?}", hwnd);
    unsafe {
        let _ = ShowWindow(hwnd, SW_HIDE);
    }
    log!("  hide_window_immediate() completed");
}

/// Toggle window visibility, returns true if now visible
pub fn toggle_window(hwnd: HWND) -> bool {
    log!("toggle_window() called, hwnd={:?}", hwnd);
    unsafe {
        let is_visible = IsWindowVisible(hwnd).as_bool();
        log!("  IsWindowVisible() returned {}", is_visible);

        if is_visible {
            hide_window_immediate(hwnd);
            log!("  toggle_window() returning false (now hidden)");
            false
        } else {
            show_window(hwnd);
            log!("  toggle_window() returning true (now visible)");
            true
        }
    }
}

/// Check if window is visible
pub fn is_window_visible(hwnd: HWND) -> bool {
    unsafe { IsWindowVisible(hwnd).as_bool() }
}

/// Reposition window (e.g., after DPI change)
pub fn reposition_window(hwnd: HWND, config: &WindowConfig) {
    let dpi = DpiInfo::for_window(hwnd).dpi;
    let rect = calculate_window_rect(config, dpi);

    unsafe {
        let _ = SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            rect.left,
            rect.top,
            rect.right - rect.left,
            rect.bottom - rect.top,
            SWP_NOACTIVATE | SWP_NOZORDER,
        );
    }
}

/// Request window repaint
pub fn invalidate_window(hwnd: HWND) {
    unsafe {
        let _ = InvalidateRect(hwnd, None, false);
    }
}

/// Get window client area size
pub fn get_client_size(hwnd: HWND) -> (i32, i32) {
    unsafe {
        let mut rect = RECT::default();
        let result = GetClientRect(hwnd, &mut rect);
        let width = rect.right - rect.left;
        let height = rect.bottom - rect.top;
        log!(
            "get_client_size(hwnd={:?}): result={:?}, rect=({},{},{},{}), size={}x{}",
            hwnd,
            result,
            rect.left,
            rect.top,
            rect.right,
            rect.bottom,
            width,
            height
        );
        (width, height)
    }
}

/// Destroy the window
pub fn destroy_window(hwnd: HWND) {
    unsafe {
        let _ = DestroyWindow(hwnd);
    }
}
