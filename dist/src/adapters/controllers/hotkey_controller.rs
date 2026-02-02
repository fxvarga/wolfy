//! HotkeyController - Manages hotkey registration and handling

use crate::application::services::command_handler::AppCommand;
use crate::domain::value_objects::{Hotkey, KeyCode, Modifiers};
use std::collections::HashMap;

/// Hotkey action
#[derive(Clone, Debug)]
pub enum HotkeyAction {
    /// Toggle window visibility
    ToggleWindow,
    /// Show window
    ShowWindow,
    /// Hide window
    HideWindow,
    /// Launch selected
    LaunchSelected,
    /// Custom command
    Custom(AppCommand),
}

/// Registered hotkey
#[derive(Clone, Debug)]
pub struct RegisteredHotkey {
    pub id: u32,
    pub hotkey: Hotkey,
    pub action: HotkeyAction,
}

/// Controller for hotkey management
pub struct HotkeyController {
    /// Registered hotkeys by ID
    hotkeys: HashMap<u32, RegisteredHotkey>,
    /// Next hotkey ID
    next_id: u32,
}

impl HotkeyController {
    /// Create a new hotkey controller
    pub fn new() -> Self {
        Self {
            hotkeys: HashMap::new(),
            next_id: 1,
        }
    }

    /// Register a hotkey
    pub fn register(&mut self, hotkey: Hotkey, action: HotkeyAction) -> u32 {
        let id = self.next_id;
        self.next_id += 1;

        self.hotkeys.insert(
            id,
            RegisteredHotkey {
                id,
                hotkey,
                action,
            },
        );

        id
    }

    /// Unregister a hotkey
    pub fn unregister(&mut self, id: u32) -> Option<RegisteredHotkey> {
        self.hotkeys.remove(&id)
    }

    /// Handle a hotkey trigger
    pub fn handle_hotkey(&self, id: u32) -> Option<AppCommand> {
        self.hotkeys.get(&id).map(|hk| match &hk.action {
            HotkeyAction::ToggleWindow => AppCommand::Toggle,
            HotkeyAction::ShowWindow => AppCommand::Show,
            HotkeyAction::HideWindow => AppCommand::Hide,
            HotkeyAction::LaunchSelected => AppCommand::LaunchSelected,
            HotkeyAction::Custom(cmd) => cmd.clone(),
        })
    }

    /// Get all registered hotkeys
    pub fn hotkeys(&self) -> impl Iterator<Item = &RegisteredHotkey> {
        self.hotkeys.values()
    }

    /// Find hotkey by key combination
    pub fn find_by_hotkey(&self, hotkey: &Hotkey) -> Option<&RegisteredHotkey> {
        self.hotkeys.values().find(|hk| &hk.hotkey == hotkey)
    }

    /// Clear all hotkeys
    pub fn clear(&mut self) {
        self.hotkeys.clear();
    }

    /// Register the default toggle hotkey (Alt+Space)
    pub fn register_default_toggle(&mut self) -> u32 {
        let hotkey = Hotkey::alt(KeyCode::Space);
        self.register(hotkey, HotkeyAction::ToggleWindow)
    }
}

impl Default for HotkeyController {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_hotkey() {
        let mut controller = HotkeyController::new();

        let id = controller.register(Hotkey::alt(KeyCode::Space), HotkeyAction::ToggleWindow);

        assert!(controller.hotkeys.contains_key(&id));
    }

    #[test]
    fn test_handle_hotkey() {
        let mut controller = HotkeyController::new();
        let id = controller.register(Hotkey::alt(KeyCode::Space), HotkeyAction::ToggleWindow);

        let command = controller.handle_hotkey(id);

        match command {
            Some(AppCommand::Toggle) => {}
            _ => panic!("Expected Toggle command"),
        }
    }

    #[test]
    fn test_unregister() {
        let mut controller = HotkeyController::new();
        let id = controller.register(Hotkey::alt(KeyCode::Space), HotkeyAction::ToggleWindow);

        let removed = controller.unregister(id);
        assert!(removed.is_some());
        assert!(controller.handle_hotkey(id).is_none());
    }
}
