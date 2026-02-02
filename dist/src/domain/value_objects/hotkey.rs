//! Hotkey value object - keyboard shortcut representation
//!
//! Represents a keyboard shortcut with modifiers and key code.

use std::fmt;

/// Keyboard modifiers
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Modifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub win: bool,
}

impl Modifiers {
    pub const NONE: Modifiers = Modifiers {
        ctrl: false,
        alt: false,
        shift: false,
        win: false,
    };

    pub const CTRL: Modifiers = Modifiers {
        ctrl: true,
        alt: false,
        shift: false,
        win: false,
    };

    pub const ALT: Modifiers = Modifiers {
        ctrl: false,
        alt: true,
        shift: false,
        win: false,
    };

    pub const SHIFT: Modifiers = Modifiers {
        ctrl: false,
        alt: false,
        shift: true,
        win: false,
    };

    pub const WIN: Modifiers = Modifiers {
        ctrl: false,
        alt: false,
        shift: false,
        win: true,
    };

    /// Create modifiers with specified flags
    pub fn new(ctrl: bool, alt: bool, shift: bool, win: bool) -> Self {
        Self {
            ctrl,
            alt,
            shift,
            win,
        }
    }

    /// Check if no modifiers are set
    pub fn is_empty(&self) -> bool {
        !self.ctrl && !self.alt && !self.shift && !self.win
    }

    /// Combine with another modifier set
    pub fn with(self, other: Modifiers) -> Self {
        Self {
            ctrl: self.ctrl || other.ctrl,
            alt: self.alt || other.alt,
            shift: self.shift || other.shift,
            win: self.win || other.win,
        }
    }
}

impl fmt::Display for Modifiers {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts = Vec::new();
        if self.ctrl {
            parts.push("Ctrl");
        }
        if self.alt {
            parts.push("Alt");
        }
        if self.shift {
            parts.push("Shift");
        }
        if self.win {
            parts.push("Win");
        }
        write!(f, "{}", parts.join("+"))
    }
}

/// Virtual key codes (platform-independent)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeyCode {
    // Letters
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,

    // Numbers
    Num0,
    Num1,
    Num2,
    Num3,
    Num4,
    Num5,
    Num6,
    Num7,
    Num8,
    Num9,

    // Function keys
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,

    // Special keys
    Space,
    Enter,
    Tab,
    Backspace,
    Delete,
    Escape,
    Home,
    End,
    PageUp,
    PageDown,
    Insert,

    // Arrow keys
    Up,
    Down,
    Left,
    Right,

    // Punctuation
    Semicolon,
    Comma,
    Period,
    Slash,
    Backslash,
    Quote,
    Backtick,
    Minus,
    Equals,
    LeftBracket,
    RightBracket,

    // Unknown/other
    Unknown(u32),
}

impl KeyCode {
    /// Parse key code from string
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_uppercase().as_str() {
            "A" => Some(KeyCode::A),
            "B" => Some(KeyCode::B),
            "C" => Some(KeyCode::C),
            "D" => Some(KeyCode::D),
            "E" => Some(KeyCode::E),
            "F" => Some(KeyCode::F),
            "G" => Some(KeyCode::G),
            "H" => Some(KeyCode::H),
            "I" => Some(KeyCode::I),
            "J" => Some(KeyCode::J),
            "K" => Some(KeyCode::K),
            "L" => Some(KeyCode::L),
            "M" => Some(KeyCode::M),
            "N" => Some(KeyCode::N),
            "O" => Some(KeyCode::O),
            "P" => Some(KeyCode::P),
            "Q" => Some(KeyCode::Q),
            "R" => Some(KeyCode::R),
            "S" => Some(KeyCode::S),
            "T" => Some(KeyCode::T),
            "U" => Some(KeyCode::U),
            "V" => Some(KeyCode::V),
            "W" => Some(KeyCode::W),
            "X" => Some(KeyCode::X),
            "Y" => Some(KeyCode::Y),
            "Z" => Some(KeyCode::Z),
            "0" => Some(KeyCode::Num0),
            "1" => Some(KeyCode::Num1),
            "2" => Some(KeyCode::Num2),
            "3" => Some(KeyCode::Num3),
            "4" => Some(KeyCode::Num4),
            "5" => Some(KeyCode::Num5),
            "6" => Some(KeyCode::Num6),
            "7" => Some(KeyCode::Num7),
            "8" => Some(KeyCode::Num8),
            "9" => Some(KeyCode::Num9),
            "F1" => Some(KeyCode::F1),
            "F2" => Some(KeyCode::F2),
            "F3" => Some(KeyCode::F3),
            "F4" => Some(KeyCode::F4),
            "F5" => Some(KeyCode::F5),
            "F6" => Some(KeyCode::F6),
            "F7" => Some(KeyCode::F7),
            "F8" => Some(KeyCode::F8),
            "F9" => Some(KeyCode::F9),
            "F10" => Some(KeyCode::F10),
            "F11" => Some(KeyCode::F11),
            "F12" => Some(KeyCode::F12),
            "SPACE" => Some(KeyCode::Space),
            "ENTER" | "RETURN" => Some(KeyCode::Enter),
            "TAB" => Some(KeyCode::Tab),
            "BACKSPACE" => Some(KeyCode::Backspace),
            "DELETE" | "DEL" => Some(KeyCode::Delete),
            "ESCAPE" | "ESC" => Some(KeyCode::Escape),
            "HOME" => Some(KeyCode::Home),
            "END" => Some(KeyCode::End),
            "PAGEUP" | "PGUP" => Some(KeyCode::PageUp),
            "PAGEDOWN" | "PGDN" => Some(KeyCode::PageDown),
            "INSERT" | "INS" => Some(KeyCode::Insert),
            "UP" => Some(KeyCode::Up),
            "DOWN" => Some(KeyCode::Down),
            "LEFT" => Some(KeyCode::Left),
            "RIGHT" => Some(KeyCode::Right),
            _ => None,
        }
    }
}

impl fmt::Display for KeyCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            KeyCode::A => "A",
            KeyCode::B => "B",
            KeyCode::C => "C",
            KeyCode::D => "D",
            KeyCode::E => "E",
            KeyCode::F => "F",
            KeyCode::G => "G",
            KeyCode::H => "H",
            KeyCode::I => "I",
            KeyCode::J => "J",
            KeyCode::K => "K",
            KeyCode::L => "L",
            KeyCode::M => "M",
            KeyCode::N => "N",
            KeyCode::O => "O",
            KeyCode::P => "P",
            KeyCode::Q => "Q",
            KeyCode::R => "R",
            KeyCode::S => "S",
            KeyCode::T => "T",
            KeyCode::U => "U",
            KeyCode::V => "V",
            KeyCode::W => "W",
            KeyCode::X => "X",
            KeyCode::Y => "Y",
            KeyCode::Z => "Z",
            KeyCode::Num0 => "0",
            KeyCode::Num1 => "1",
            KeyCode::Num2 => "2",
            KeyCode::Num3 => "3",
            KeyCode::Num4 => "4",
            KeyCode::Num5 => "5",
            KeyCode::Num6 => "6",
            KeyCode::Num7 => "7",
            KeyCode::Num8 => "8",
            KeyCode::Num9 => "9",
            KeyCode::F1 => "F1",
            KeyCode::F2 => "F2",
            KeyCode::F3 => "F3",
            KeyCode::F4 => "F4",
            KeyCode::F5 => "F5",
            KeyCode::F6 => "F6",
            KeyCode::F7 => "F7",
            KeyCode::F8 => "F8",
            KeyCode::F9 => "F9",
            KeyCode::F10 => "F10",
            KeyCode::F11 => "F11",
            KeyCode::F12 => "F12",
            KeyCode::Space => "Space",
            KeyCode::Enter => "Enter",
            KeyCode::Tab => "Tab",
            KeyCode::Backspace => "Backspace",
            KeyCode::Delete => "Delete",
            KeyCode::Escape => "Escape",
            KeyCode::Home => "Home",
            KeyCode::End => "End",
            KeyCode::PageUp => "PageUp",
            KeyCode::PageDown => "PageDown",
            KeyCode::Insert => "Insert",
            KeyCode::Up => "Up",
            KeyCode::Down => "Down",
            KeyCode::Left => "Left",
            KeyCode::Right => "Right",
            KeyCode::Semicolon => ";",
            KeyCode::Comma => ",",
            KeyCode::Period => ".",
            KeyCode::Slash => "/",
            KeyCode::Backslash => "\\",
            KeyCode::Quote => "'",
            KeyCode::Backtick => "`",
            KeyCode::Minus => "-",
            KeyCode::Equals => "=",
            KeyCode::LeftBracket => "[",
            KeyCode::RightBracket => "]",
            KeyCode::Unknown(code) => return write!(f, "Unknown(0x{:X})", code),
        };
        write!(f, "{}", name)
    }
}

/// A keyboard hotkey (modifier + key combination)
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Hotkey {
    pub modifiers: Modifiers,
    pub key: KeyCode,
}

impl Hotkey {
    /// Create a new hotkey
    pub fn new(modifiers: Modifiers, key: KeyCode) -> Self {
        Self { modifiers, key }
    }

    /// Create a hotkey with just a key (no modifiers)
    pub fn key(key: KeyCode) -> Self {
        Self::new(Modifiers::NONE, key)
    }

    /// Create a Ctrl+Key hotkey
    pub fn ctrl(key: KeyCode) -> Self {
        Self::new(Modifiers::CTRL, key)
    }

    /// Create an Alt+Key hotkey
    pub fn alt(key: KeyCode) -> Self {
        Self::new(Modifiers::ALT, key)
    }

    /// Create a Shift+Key hotkey
    pub fn shift(key: KeyCode) -> Self {
        Self::new(Modifiers::SHIFT, key)
    }

    /// Create a Win+Key hotkey
    pub fn win(key: KeyCode) -> Self {
        Self::new(Modifiers::WIN, key)
    }

    /// Parse hotkey from string (e.g., "Ctrl+Alt+Space")
    pub fn parse(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split('+').map(|p| p.trim()).collect();
        if parts.is_empty() {
            return None;
        }

        let mut modifiers = Modifiers::NONE;
        let mut key = None;

        for part in parts {
            match part.to_uppercase().as_str() {
                "CTRL" | "CONTROL" => modifiers.ctrl = true,
                "ALT" => modifiers.alt = true,
                "SHIFT" => modifiers.shift = true,
                "WIN" | "SUPER" | "META" => modifiers.win = true,
                _ => key = KeyCode::from_name(part),
            }
        }

        key.map(|k| Hotkey::new(modifiers, k))
    }
}

impl fmt::Display for Hotkey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.modifiers.is_empty() {
            write!(f, "{}", self.key)
        } else {
            write!(f, "{}+{}", self.modifiers, self.key)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_modifiers_display() {
        assert_eq!(Modifiers::CTRL.to_string(), "Ctrl");
        assert_eq!(Modifiers::CTRL.with(Modifiers::ALT).to_string(), "Ctrl+Alt");
    }

    #[test]
    fn test_hotkey_creation() {
        let hk = Hotkey::ctrl(KeyCode::Space);

        assert!(hk.modifiers.ctrl);
        assert!(!hk.modifiers.alt);
        assert_eq!(hk.key, KeyCode::Space);
    }

    #[test]
    fn test_hotkey_parse() {
        let hk = Hotkey::parse("Ctrl+Alt+Space").unwrap();

        assert!(hk.modifiers.ctrl);
        assert!(hk.modifiers.alt);
        assert!(!hk.modifiers.shift);
        assert_eq!(hk.key, KeyCode::Space);
    }

    #[test]
    fn test_hotkey_display() {
        let hk = Hotkey::ctrl(KeyCode::A);
        assert_eq!(hk.to_string(), "Ctrl+A");

        let hk = Hotkey::key(KeyCode::Escape);
        assert_eq!(hk.to_string(), "Escape");
    }

    #[test]
    fn test_keycode_from_name() {
        assert_eq!(KeyCode::from_name("A"), Some(KeyCode::A));
        assert_eq!(KeyCode::from_name("F1"), Some(KeyCode::F1));
        assert_eq!(KeyCode::from_name("Space"), Some(KeyCode::Space));
        assert_eq!(KeyCode::from_name("invalid"), None);
    }
}
