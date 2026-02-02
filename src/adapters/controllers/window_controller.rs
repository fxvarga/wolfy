//! WindowController - Translates window events to use case calls

use crate::application::services::command_handler::AppCommand;
use crate::domain::value_objects::{Hotkey, KeyCode, Modifiers};

/// Window message types (platform-independent)
#[derive(Clone, Debug)]
pub enum WindowMessage {
    /// Key pressed
    KeyDown(KeyCode, Modifiers),
    /// Key released
    KeyUp(KeyCode, Modifiers),
    /// Character input
    CharInput(char),
    /// Mouse moved
    MouseMove(f32, f32),
    /// Mouse button pressed
    MouseDown(MouseButton, f32, f32),
    /// Mouse button released
    MouseUp(MouseButton, f32, f32),
    /// Mouse wheel scrolled
    MouseWheel(f32),
    /// Window gained focus
    FocusGained,
    /// Window lost focus
    FocusLost,
    /// Window resized
    Resize(u32, u32),
    /// Window moved
    Move(i32, i32),
    /// DPI changed
    DpiChanged(f32),
    /// Timer tick
    Timer(u32),
    /// Close requested
    CloseRequested,
    /// Hotkey triggered
    Hotkey(u32),
}

/// Mouse button types
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// Result of handling a window message
#[derive(Clone, Debug)]
pub enum ControllerResult {
    /// No action needed
    None,
    /// Execute a command
    Command(AppCommand),
    /// Multiple commands to execute
    Commands(Vec<AppCommand>),
    /// Redraw needed
    Redraw,
    /// Exit application
    Exit,
}

/// Controller for window events
pub struct WindowController {
    /// Last mouse position
    mouse_x: f32,
    mouse_y: f32,
    /// Current modifier keys
    modifiers: Modifiers,
    /// Escape action (hide or exit)
    escape_hides: bool,
}

impl WindowController {
    /// Create a new window controller
    pub fn new() -> Self {
        Self {
            mouse_x: 0.0,
            mouse_y: 0.0,
            modifiers: Modifiers::NONE,
            escape_hides: true,
        }
    }

    /// Set whether Escape hides the window (true) or exits (false)
    pub fn set_escape_hides(&mut self, hides: bool) {
        self.escape_hides = hides;
    }

    /// Handle a window message
    pub fn handle_message(&mut self, message: WindowMessage) -> ControllerResult {
        match message {
            WindowMessage::KeyDown(key, mods) => {
                self.modifiers = mods;
                self.handle_key_down(key, mods)
            }
            WindowMessage::KeyUp(_key, mods) => {
                self.modifiers = mods;
                ControllerResult::None
            }
            WindowMessage::CharInput(c) => {
                // Character input is handled separately by search controller
                ControllerResult::None
            }
            WindowMessage::MouseMove(x, y) => {
                self.mouse_x = x;
                self.mouse_y = y;
                ControllerResult::None
            }
            WindowMessage::MouseDown(button, x, y) => {
                self.mouse_x = x;
                self.mouse_y = y;
                self.handle_mouse_down(button, x, y)
            }
            WindowMessage::MouseUp(_button, x, y) => {
                self.mouse_x = x;
                self.mouse_y = y;
                ControllerResult::None
            }
            WindowMessage::MouseWheel(delta) => {
                // Scrolling - could be handled by listview
                ControllerResult::None
            }
            WindowMessage::FocusLost => {
                // Hide on focus loss
                ControllerResult::Command(AppCommand::Hide)
            }
            WindowMessage::FocusGained => ControllerResult::None,
            WindowMessage::Resize(width, height) => ControllerResult::Redraw,
            WindowMessage::Move(_x, _y) => ControllerResult::None,
            WindowMessage::DpiChanged(_dpi) => ControllerResult::Redraw,
            WindowMessage::Timer(_id) => ControllerResult::Redraw,
            WindowMessage::CloseRequested => ControllerResult::Exit,
            WindowMessage::Hotkey(_id) => {
                // Toggle window on hotkey
                ControllerResult::Command(AppCommand::Toggle)
            }
        }
    }

    /// Handle key down event
    fn handle_key_down(&self, key: KeyCode, mods: Modifiers) -> ControllerResult {
        // Special keys with modifiers
        if mods.ctrl {
            match key {
                KeyCode::A => {
                    // Select all (in textbox)
                    return ControllerResult::None;
                }
                KeyCode::C => {
                    // Copy
                    return ControllerResult::None;
                }
                KeyCode::V => {
                    // Paste
                    return ControllerResult::None;
                }
                _ => {}
            }
        }

        // Navigation keys
        match key {
            KeyCode::Escape => {
                if self.escape_hides {
                    ControllerResult::Command(AppCommand::Hide)
                } else {
                    ControllerResult::Exit
                }
            }
            KeyCode::Enter => ControllerResult::Command(AppCommand::LaunchSelected),
            KeyCode::Up => ControllerResult::Command(AppCommand::SelectUp),
            KeyCode::Down => ControllerResult::Command(AppCommand::SelectDown),
            KeyCode::Tab => {
                if mods.shift {
                    ControllerResult::Command(AppCommand::SelectUp)
                } else {
                    ControllerResult::Command(AppCommand::SelectDown)
                }
            }
            KeyCode::PageUp => {
                // Could implement page navigation
                ControllerResult::None
            }
            KeyCode::PageDown => {
                // Could implement page navigation
                ControllerResult::None
            }
            KeyCode::Home => {
                // Could select first item
                ControllerResult::None
            }
            KeyCode::End => {
                // Could select last item
                ControllerResult::None
            }
            _ => ControllerResult::None,
        }
    }

    /// Handle mouse down event
    fn handle_mouse_down(&self, button: MouseButton, _x: f32, _y: f32) -> ControllerResult {
        match button {
            MouseButton::Left => {
                // Click handling would be done by widgets
                ControllerResult::None
            }
            MouseButton::Right => {
                // Context menu
                ControllerResult::None
            }
            MouseButton::Middle => ControllerResult::None,
        }
    }

    /// Get current mouse position
    pub fn mouse_position(&self) -> (f32, f32) {
        (self.mouse_x, self.mouse_y)
    }

    /// Get current modifiers
    pub fn modifiers(&self) -> Modifiers {
        self.modifiers
    }
}

impl Default for WindowController {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_hides() {
        let mut controller = WindowController::new();
        controller.set_escape_hides(true);

        let result = controller.handle_message(WindowMessage::KeyDown(KeyCode::Escape, Modifiers::NONE));

        match result {
            ControllerResult::Command(AppCommand::Hide) => {}
            _ => panic!("Expected Hide command"),
        }
    }

    #[test]
    fn test_enter_launches() {
        let mut controller = WindowController::new();

        let result = controller.handle_message(WindowMessage::KeyDown(KeyCode::Enter, Modifiers::NONE));

        match result {
            ControllerResult::Command(AppCommand::LaunchSelected) => {}
            _ => panic!("Expected LaunchSelected command"),
        }
    }

    #[test]
    fn test_arrow_navigation() {
        let mut controller = WindowController::new();

        let up = controller.handle_message(WindowMessage::KeyDown(KeyCode::Up, Modifiers::NONE));
        let down = controller.handle_message(WindowMessage::KeyDown(KeyCode::Down, Modifiers::NONE));

        match up {
            ControllerResult::Command(AppCommand::SelectUp) => {}
            _ => panic!("Expected SelectUp"),
        }

        match down {
            ControllerResult::Command(AppCommand::SelectDown) => {}
            _ => panic!("Expected SelectDown"),
        }
    }
}
