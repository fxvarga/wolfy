//! Wolfy - A Windows application launcher inspired by rofi
//!
//! Multi-window architecture:
//! - Ctrl+0: App Launcher - main search window with task panel
//! - Ctrl+1: Theme Picker - full-width grid of HyDE themes
//! - Ctrl+2: Wallpaper Picker - full-width grid of wallpapers
//!
//! Each mode has its own window with independent rendering and theme.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[macro_use]
extern crate lalrpop_util;

#[macro_use]
mod log;

mod animation;
mod app;
mod cwd_history;
mod grid_window;
mod history;
mod mode;
mod platform;
mod pr_reviews;
mod pty;
mod state;
mod task_runner;
mod tasks;
mod terminal;
mod theme;
mod widget;

use std::cell::RefCell;
use std::rc::Rc;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, TranslateMessage, MSG, WM_HOTKEY,
};

use app::App;
use grid_window::GridWindow;
use log::find_config_file;
use mode::Mode;
use platform::win32::{
    self, create_window, default_mode_hotkeys, enable_dpi_awareness, get_monitor_width,
    register_window_class, set_window_callback, unregister_hotkeys, unregister_window_class,
    WindowConfig,
};
use state::AppState;
use theme::tree::ThemeTree;

/// Manages all application windows
struct WindowManager {
    /// Launcher window (Ctrl+0)
    launcher: Rc<RefCell<App>>,
    /// Theme picker window (Ctrl+1)
    theme_picker: Rc<RefCell<GridWindow>>,
    /// Wallpaper picker window (Ctrl+2)
    wallpaper_picker: Rc<RefCell<GridWindow>>,
    /// Shared application state
    #[allow(dead_code)]
    app_state: Rc<RefCell<AppState>>,
}

impl WindowManager {
    /// Handle a window message, routing to the appropriate window
    fn handle_message(
        &mut self,
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> Option<LRESULT> {
        // Route message based on HWND
        let launcher_hwnd = self.launcher.borrow().hwnd();
        let theme_hwnd = self.theme_picker.borrow().hwnd();
        let wallpaper_hwnd = self.wallpaper_picker.borrow().hwnd();

        if hwnd == launcher_hwnd {
            match self.launcher.try_borrow_mut() {
                Ok(mut app) => app.handle_message(hwnd, msg, wparam, lparam),
                Err(_) => {
                    log!("WARNING: Re-entrant callback for launcher, msg={}", msg);
                    None
                }
            }
        } else if hwnd == theme_hwnd {
            match self.theme_picker.try_borrow_mut() {
                Ok(mut window) => window.handle_message(hwnd, msg, wparam, lparam),
                Err(_) => {
                    log!("WARNING: Re-entrant callback for theme_picker, msg={}", msg);
                    None
                }
            }
        } else if hwnd == wallpaper_hwnd {
            match self.wallpaper_picker.try_borrow_mut() {
                Ok(mut window) => window.handle_message(hwnd, msg, wparam, lparam),
                Err(_) => {
                    log!(
                        "WARNING: Re-entrant callback for wallpaper_picker, msg={}",
                        msg
                    );
                    None
                }
            }
        } else {
            log!("WARNING: Unknown HWND {:?} in callback", hwnd);
            None
        }
    }

    /// Show a mode's window, hiding others
    fn show_mode(&mut self, mode: Mode) {
        // Check if theme needs to be reloaded (set by GridWindow when theme is selected)
        {
            let mut state = self.app_state.borrow_mut();
            if state.theme_needs_reload {
                state.theme_needs_reload = false;
                // Get the theme name before dropping the borrow
                let theme_name = state.current_theme.clone();
                drop(state); // Release borrow before calling reload_theme

                // Update launcher's current_theme and reload its styling
                let mut launcher = self.launcher.borrow_mut();
                launcher.set_current_theme(theme_name);
                launcher.reload_theme();
                log!("Theme reloaded after theme picker selection");
            }
        }

        match mode {
            Mode::Launcher => {
                // Hide grid windows, show launcher
                self.theme_picker.borrow_mut().hide();
                self.wallpaper_picker.borrow_mut().hide();
                self.launcher.borrow_mut().show();
            }
            Mode::ThemePicker => {
                // Hide other windows, show theme picker
                self.launcher.borrow_mut().hide();
                self.wallpaper_picker.borrow_mut().hide();
                self.theme_picker.borrow_mut().show();
            }
            Mode::WallpaperPicker => {
                // Hide other windows, show wallpaper picker
                self.launcher.borrow_mut().hide();
                self.theme_picker.borrow_mut().hide();
                self.wallpaper_picker.borrow_mut().show();
            }
            Mode::TailView => {
                // TailView is a mode within the launcher, not a separate window
                // Just show the launcher (it will be in TailView mode)
                self.theme_picker.borrow_mut().hide();
                self.wallpaper_picker.borrow_mut().hide();
                self.launcher.borrow_mut().show();
            }
        }
    }
}

fn main() {
    // Initialize logging first
    log::init();
    log!("main() starting - multi-window architecture");

    // Enable DPI awareness early
    log!("Enabling DPI awareness...");
    if let Err(e) = enable_dpi_awareness() {
        log!("Warning: Failed to enable DPI awareness: {:?}", e);
    } else {
        log!("DPI awareness enabled");
    }

    // Register window class (shared by all windows)
    log!("Registering window class...");
    if let Err(e) = register_window_class() {
        log!("FATAL: Failed to register window class: {:?}", e);
        return;
    }
    log!("Window class registered");

    // Create shared application state
    let app_state = Rc::new(RefCell::new(AppState::new()));
    log!("Created shared AppState");

    // Load launcher theme to determine window dimensions
    let theme_path = find_config_file("default.rasi");
    log!("Loading launcher theme from {:?}", theme_path);
    let (launcher_width, launcher_height) = match ThemeTree::load(&theme_path) {
        Ok(theme) => {
            log!("Theme loaded successfully");
            let width = theme.get_number("window", None, "width", 928.0) as i32;
            let height = theme.get_number("window", None, "height", 480.0) as i32;
            log!("Launcher window size: {}x{}", width, height);
            (width, height)
        }
        Err(e) => {
            log!("Failed to load theme: {:?}, using defaults", e);
            (928, 480)
        }
    };

    // Get monitor width for grid windows
    let monitor_width = get_monitor_width();
    log!("Monitor width: {}", monitor_width);

    // Load theme picker dimensions
    let theme_picker_path = find_config_file("theme_picker.rasi");
    let theme_picker_height = match ThemeTree::load(&theme_picker_path) {
        Ok(theme) => theme.get_number("window", None, "height", 520.0) as i32,
        Err(_) => 520,
    };

    // Load wallpaper picker dimensions
    let wallpaper_picker_path = find_config_file("wallpaper_picker.rasi");
    let wallpaper_picker_height = match ThemeTree::load(&wallpaper_picker_path) {
        Ok(theme) => theme.get_number("window", None, "height", 650.0) as i32,
        Err(_) => 650,
    };

    // --- Create Launcher Window ---
    let launcher_config = WindowConfig {
        width: launcher_width,
        height: launcher_height,
        vertical_position: 0.5, // True center of screen
    };
    log!(
        "Creating launcher window: {}x{}",
        launcher_config.width,
        launcher_config.height
    );

    let launcher_hwnd = match create_window(&launcher_config) {
        Ok(h) => {
            log!("Launcher window created: HWND={:?}", h);
            h
        }
        Err(e) => {
            log!("FATAL: Failed to create launcher window: {:?}", e);
            unregister_window_class();
            return;
        }
    };

    // --- Create Theme Picker Window ---
    let theme_picker_config = WindowConfig {
        width: monitor_width,
        height: theme_picker_height,
        vertical_position: 0.5, // Centered vertically
    };
    log!(
        "Creating theme picker window: {}x{}",
        theme_picker_config.width,
        theme_picker_config.height
    );

    let theme_picker_hwnd = match create_window(&theme_picker_config) {
        Ok(h) => {
            log!("Theme picker window created: HWND={:?}", h);
            h
        }
        Err(e) => {
            log!("FATAL: Failed to create theme picker window: {:?}", e);
            win32::destroy_window(launcher_hwnd);
            unregister_window_class();
            return;
        }
    };

    // --- Create Wallpaper Picker Window ---
    let wallpaper_picker_config = WindowConfig {
        width: monitor_width,
        height: wallpaper_picker_height,
        vertical_position: 0.5, // Centered vertically
    };
    log!(
        "Creating wallpaper picker window: {}x{}",
        wallpaper_picker_config.width,
        wallpaper_picker_config.height
    );

    let wallpaper_picker_hwnd = match create_window(&wallpaper_picker_config) {
        Ok(h) => {
            log!("Wallpaper picker window created: HWND={:?}", h);
            h
        }
        Err(e) => {
            log!("FATAL: Failed to create wallpaper picker window: {:?}", e);
            win32::destroy_window(theme_picker_hwnd);
            win32::destroy_window(launcher_hwnd);
            unregister_window_class();
            return;
        }
    };

    // Register all mode hotkeys on the launcher window (main message receiver)
    let hotkeys = default_mode_hotkeys();
    log!("Registering {} mode hotkeys...", hotkeys.len());
    for hotkey in &hotkeys {
        match hotkey.register(launcher_hwnd) {
            Ok(()) => log!("  Registered: {}", hotkey.display_name()),
            Err(e) => log!("  Failed to register {}: {:?}", hotkey.display_name(), e),
        }
    }

    // --- Create App objects ---
    log!("Creating App (launcher)...");
    let launcher = match App::new(launcher_hwnd, launcher_config) {
        Ok(a) => {
            log!("Launcher App created successfully");
            Rc::new(RefCell::new(a))
        }
        Err(e) => {
            log!("FATAL: Failed to create launcher App: {:?}", e);
            unregister_hotkeys(launcher_hwnd, &hotkeys);
            win32::destroy_window(wallpaper_picker_hwnd);
            win32::destroy_window(theme_picker_hwnd);
            win32::destroy_window(launcher_hwnd);
            unregister_window_class();
            return;
        }
    };

    log!("Creating GridWindow (theme picker)...");
    let theme_picker =
        match GridWindow::new(theme_picker_hwnd, Mode::ThemePicker, app_state.clone()) {
            Ok(w) => {
                log!("Theme picker GridWindow created successfully");
                Rc::new(RefCell::new(w))
            }
            Err(e) => {
                log!("FATAL: Failed to create theme picker: {:?}", e);
                unregister_hotkeys(launcher_hwnd, &hotkeys);
                win32::destroy_window(wallpaper_picker_hwnd);
                win32::destroy_window(theme_picker_hwnd);
                win32::destroy_window(launcher_hwnd);
                unregister_window_class();
                return;
            }
        };

    log!("Creating GridWindow (wallpaper picker)...");
    let wallpaper_picker = match GridWindow::new(
        wallpaper_picker_hwnd,
        Mode::WallpaperPicker,
        app_state.clone(),
    ) {
        Ok(w) => {
            log!("Wallpaper picker GridWindow created successfully");
            Rc::new(RefCell::new(w))
        }
        Err(e) => {
            log!("FATAL: Failed to create wallpaper picker: {:?}", e);
            unregister_hotkeys(launcher_hwnd, &hotkeys);
            win32::destroy_window(wallpaper_picker_hwnd);
            win32::destroy_window(theme_picker_hwnd);
            win32::destroy_window(launcher_hwnd);
            unregister_window_class();
            return;
        }
    };

    // Create window manager
    let window_manager = Rc::new(RefCell::new(WindowManager {
        launcher: launcher.clone(),
        theme_picker,
        wallpaper_picker,
        app_state,
    }));

    // Set up window procedure callback
    log!("Setting window callback...");
    let wm_clone = window_manager.clone();
    set_window_callback(
        move |hwnd, msg, wparam, lparam| match wm_clone.try_borrow_mut() {
            Ok(mut wm) => wm.handle_message(hwnd, msg, wparam, lparam),
            Err(_) => {
                log!("WARNING: Re-entrant callback, msg={}", msg);
                None
            }
        },
    );

    // Start file watch timer for theme hot-reload on launcher
    log!("Starting file watch timer for theme hot-reload...");
    launcher.borrow().start_file_watch_timer();

    log!(
        "Wolfy started (multi-window). Hotkeys: Ctrl+0 (launcher), Ctrl+1 (theme), Ctrl+2 (wallpaper). F5=reload theme, F6=restart app."
    );

    // Run message loop with hotkey handling
    unsafe {
        let mut msg = MSG::default();
        let mut msg_count = 0u64;
        loop {
            let ret = GetMessageW(&mut msg, None, 0, 0);
            if ret.0 <= 0 {
                log!("GetMessageW returned {}, exiting loop", ret.0);
                break;
            }

            msg_count += 1;

            // Handle global hotkeys in the message loop (before dispatch)
            if msg.message == WM_HOTKEY {
                let hotkey_id = msg.wParam.0 as i32;
                if let Some(mode) = Mode::from_hotkey_id(hotkey_id) {
                    log!(
                        ">>> WM_HOTKEY received (msg #{}, id={}) - showing {} mode",
                        msg_count,
                        hotkey_id,
                        mode.display_name()
                    );
                    window_manager.borrow_mut().show_mode(mode);
                    log!("<<< show_mode() returned");
                    continue;
                }
            }

            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    // Cleanup
    log!("Cleaning up...");
    win32::clear_window_callback();
    unregister_hotkeys(launcher_hwnd, &hotkeys);
    win32::destroy_window(wallpaper_picker_hwnd);
    win32::destroy_window(theme_picker_hwnd);
    win32::destroy_window(launcher_hwnd);
    unregister_window_class();

    log!("Wolfy exited normally.");
}
