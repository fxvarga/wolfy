//! Event types and Win32 message translation

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::*;

/// Keyboard key codes (matching Win32 virtual key codes)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum KeyCode {
    // Letters
    A = 0x41,
    B = 0x42,
    C = 0x43,
    D = 0x44,
    E = 0x45,
    F = 0x46,
    G = 0x47,
    H = 0x48,
    I = 0x49,
    J = 0x4A,
    K = 0x4B,
    L = 0x4C,
    M = 0x4D,
    N = 0x4E,
    O = 0x4F,
    P = 0x50,
    Q = 0x51,
    R = 0x52,
    S = 0x53,
    T = 0x54,
    U = 0x55,
    V = 0x56,
    W = 0x57,
    X = 0x58,
    Y = 0x59,
    Z = 0x5A,

    // Numbers
    Num0 = 0x30,
    Num1 = 0x31,
    Num2 = 0x32,
    Num3 = 0x33,
    Num4 = 0x34,
    Num5 = 0x35,
    Num6 = 0x36,
    Num7 = 0x37,
    Num8 = 0x38,
    Num9 = 0x39,

    // Function keys
    F1 = 0x70,
    F2 = 0x71,
    F3 = 0x72,
    F4 = 0x73,
    F5 = 0x74,
    F6 = 0x75,
    F7 = 0x76,
    F8 = 0x77,
    F9 = 0x78,
    F10 = 0x79,
    F11 = 0x7A,
    F12 = 0x7B,

    // Navigation
    Left = 0x25,
    Up = 0x26,
    Right = 0x27,
    Down = 0x28,
    Home = 0x24,
    End = 0x23,
    PageUp = 0x21,
    PageDown = 0x22,

    // Editing
    Backspace = 0x08,
    Tab = 0x09,
    Enter = 0x0D,
    Escape = 0x1B,
    Space = 0x20,
    Delete = 0x2E,
    Insert = 0x2D,

    // Modifiers (for detecting state)
    Shift = 0x10,
    Control = 0x11,
    Alt = 0x12,

    // Misc
    CapsLock = 0x14,
    NumLock = 0x90,
    ScrollLock = 0x91,

    // Unknown key
    Unknown = 0,
}

impl KeyCode {
    /// Convert from Win32 virtual key code
    pub fn from_vk(vk: u32) -> Self {
        match vk {
            0x41 => KeyCode::A,
            0x42 => KeyCode::B,
            0x43 => KeyCode::C,
            0x44 => KeyCode::D,
            0x45 => KeyCode::E,
            0x46 => KeyCode::F,
            0x47 => KeyCode::G,
            0x48 => KeyCode::H,
            0x49 => KeyCode::I,
            0x4A => KeyCode::J,
            0x4B => KeyCode::K,
            0x4C => KeyCode::L,
            0x4D => KeyCode::M,
            0x4E => KeyCode::N,
            0x4F => KeyCode::O,
            0x50 => KeyCode::P,
            0x51 => KeyCode::Q,
            0x52 => KeyCode::R,
            0x53 => KeyCode::S,
            0x54 => KeyCode::T,
            0x55 => KeyCode::U,
            0x56 => KeyCode::V,
            0x57 => KeyCode::W,
            0x58 => KeyCode::X,
            0x59 => KeyCode::Y,
            0x5A => KeyCode::Z,
            0x30 => KeyCode::Num0,
            0x31 => KeyCode::Num1,
            0x32 => KeyCode::Num2,
            0x33 => KeyCode::Num3,
            0x34 => KeyCode::Num4,
            0x35 => KeyCode::Num5,
            0x36 => KeyCode::Num6,
            0x37 => KeyCode::Num7,
            0x38 => KeyCode::Num8,
            0x39 => KeyCode::Num9,
            0x70 => KeyCode::F1,
            0x71 => KeyCode::F2,
            0x72 => KeyCode::F3,
            0x73 => KeyCode::F4,
            0x74 => KeyCode::F5,
            0x75 => KeyCode::F6,
            0x76 => KeyCode::F7,
            0x77 => KeyCode::F8,
            0x78 => KeyCode::F9,
            0x79 => KeyCode::F10,
            0x7A => KeyCode::F11,
            0x7B => KeyCode::F12,
            0x25 => KeyCode::Left,
            0x26 => KeyCode::Up,
            0x27 => KeyCode::Right,
            0x28 => KeyCode::Down,
            0x24 => KeyCode::Home,
            0x23 => KeyCode::End,
            0x21 => KeyCode::PageUp,
            0x22 => KeyCode::PageDown,
            0x08 => KeyCode::Backspace,
            0x09 => KeyCode::Tab,
            0x0D => KeyCode::Enter,
            0x1B => KeyCode::Escape,
            0x20 => KeyCode::Space,
            0x2E => KeyCode::Delete,
            0x2D => KeyCode::Insert,
            0x10 => KeyCode::Shift,
            0x11 => KeyCode::Control,
            0x12 => KeyCode::Alt,
            0x14 => KeyCode::CapsLock,
            0x90 => KeyCode::NumLock,
            0x91 => KeyCode::ScrollLock,
            _ => KeyCode::Unknown,
        }
    }

    /// Check if this is a printable character key
    pub fn is_printable(&self) -> bool {
        let code = *self as u32;
        // A-Z: 0x41-0x5A, 0-9: 0x30-0x39, Space: 0x20
        (0x41..=0x5A).contains(&code) || (0x30..=0x39).contains(&code) || code == 0x20
    }
}

/// Modifier key state
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

/// Mouse button
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

impl Modifiers {
    /// Get current modifier state from Windows
    pub fn current() -> Self {
        use windows::Win32::UI::Input::KeyboardAndMouse::GetKeyState;
        unsafe {
            Self {
                shift: GetKeyState(0x10) < 0, // VK_SHIFT
                ctrl: GetKeyState(0x11) < 0,  // VK_CONTROL
                alt: GetKeyState(0x12) < 0,   // VK_MENU (Alt)
            }
        }
    }

    pub fn none() -> Self {
        Self::default()
    }

    pub fn ctrl_only() -> Self {
        Self {
            ctrl: true,
            ..Default::default()
        }
    }

    pub fn shift_only() -> Self {
        Self {
            shift: true,
            ..Default::default()
        }
    }
}

/// Application events
#[derive(Clone, Debug)]
pub enum Event {
    /// Key pressed
    KeyDown { key: KeyCode, modifiers: Modifiers },
    /// Key released
    KeyUp { key: KeyCode, modifiers: Modifiers },
    /// Character typed (after keyboard translation)
    Char(char),
    /// Global hotkey triggered
    Hotkey(i32),
    /// Mouse button pressed
    MouseDown { x: i32, y: i32, button: MouseButton },
    /// Mouse button released
    MouseUp { x: i32, y: i32, button: MouseButton },
    /// Mouse moved
    MouseMove { x: i32, y: i32 },
    /// Window needs repainting
    Paint,
    /// Window received focus
    FocusGained,
    /// Window lost focus
    FocusLost,
    /// DPI changed, new DPI value
    DpiChanged(u32),
    /// Window should close
    Close,
    /// Window is being destroyed
    Destroy,
    /// Timer tick
    Timer(usize),
    /// Unknown/unhandled message
    Unknown(u32),
}

/// Translate a Win32 message to an Event
pub fn translate_message(_hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> Option<Event> {
    match msg {
        WM_KEYDOWN | WM_SYSKEYDOWN => {
            let vk = wparam.0 as u32;
            let key = KeyCode::from_vk(vk);
            let modifiers = Modifiers::current();
            Some(Event::KeyDown { key, modifiers })
        }
        WM_KEYUP | WM_SYSKEYUP => {
            let vk = wparam.0 as u32;
            let key = KeyCode::from_vk(vk);
            let modifiers = Modifiers::current();
            Some(Event::KeyUp { key, modifiers })
        }
        WM_CHAR => {
            // wparam contains the UTF-16 code unit
            let code = wparam.0 as u16;
            if let Some(ch) = char::from_u32(code as u32) {
                // Filter out control characters except for useful ones
                // Note: ESC (0x1B) and CR (0x0D) are handled in wnd_proc to prevent
                // default dialog behavior from DefWindowProcW
                if ch >= ' ' || ch == '\t' {
                    return Some(Event::Char(ch));
                }
            }
            None
        }
        WM_HOTKEY => {
            let id = wparam.0 as i32;
            Some(Event::Hotkey(id))
        }
        WM_LBUTTONDOWN => {
            let x = (lparam.0 & 0xFFFF) as i16 as i32;
            let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
            Some(Event::MouseDown {
                x,
                y,
                button: MouseButton::Left,
            })
        }
        WM_LBUTTONUP => {
            let x = (lparam.0 & 0xFFFF) as i16 as i32;
            let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
            Some(Event::MouseUp {
                x,
                y,
                button: MouseButton::Left,
            })
        }
        WM_RBUTTONDOWN => {
            let x = (lparam.0 & 0xFFFF) as i16 as i32;
            let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
            Some(Event::MouseDown {
                x,
                y,
                button: MouseButton::Right,
            })
        }
        WM_RBUTTONUP => {
            let x = (lparam.0 & 0xFFFF) as i16 as i32;
            let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
            Some(Event::MouseUp {
                x,
                y,
                button: MouseButton::Right,
            })
        }
        WM_MBUTTONDOWN => {
            let x = (lparam.0 & 0xFFFF) as i16 as i32;
            let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
            Some(Event::MouseDown {
                x,
                y,
                button: MouseButton::Middle,
            })
        }
        WM_MBUTTONUP => {
            let x = (lparam.0 & 0xFFFF) as i16 as i32;
            let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
            Some(Event::MouseUp {
                x,
                y,
                button: MouseButton::Middle,
            })
        }
        WM_MOUSEMOVE => {
            let x = (lparam.0 & 0xFFFF) as i16 as i32;
            let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
            Some(Event::MouseMove { x, y })
        }
        WM_PAINT => Some(Event::Paint),
        WM_SETFOCUS => Some(Event::FocusGained),
        WM_KILLFOCUS => Some(Event::FocusLost),
        WM_DPICHANGED => {
            let dpi = (wparam.0 & 0xFFFF) as u32;
            Some(Event::DpiChanged(dpi))
        }
        WM_CLOSE => Some(Event::Close),
        WM_DESTROY => Some(Event::Destroy),
        WM_TIMER => {
            let timer_id = wparam.0;
            Some(Event::Timer(timer_id))
        }
        _ => None,
    }
}

/// Run the Windows message loop
///
/// # Arguments
/// * `on_message` - Callback for each message, return true to continue, false to exit
pub fn run_message_loop<F>(mut on_message: F)
where
    F: FnMut(&MSG) -> bool,
{
    unsafe {
        let mut msg = MSG::default();
        loop {
            let ret = GetMessageW(&mut msg, None, 0, 0);
            if ret.0 <= 0 {
                break;
            }

            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);

            if !on_message(&msg) {
                break;
            }
        }
    }
}

/// Peek for messages without blocking
/// Returns true if a message was available
pub fn peek_message(msg: &mut MSG) -> bool {
    unsafe { PeekMessageW(msg, None, 0, 0, PM_REMOVE).as_bool() }
}

/// Post a quit message
pub fn post_quit() {
    unsafe {
        PostQuitMessage(0);
    }
}
