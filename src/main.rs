//! Wolfy - A Windows application launcher inspired by rofi
//!
//! Press the configured hotkey (default: Alt+Space) to toggle the launcher window.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[macro_use]
extern crate lalrpop_util;

#[macro_use]
mod log;

mod animation;
mod app;
mod history;
mod platform;
mod tasks;
mod theme;
mod widget;

use std::cell::RefCell;
use std::rc::Rc;

use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, TranslateMessage, MSG, WM_HOTKEY,
};

use platform::win32::{
    self, create_window, enable_dpi_awareness, parse_hotkey_string, register_window_class,
    set_window_callback, unregister_window_class, HotkeyConfig, WindowConfig,
    DEFAULT_TOGGLE_HOTKEY, HOTKEY_ID_TOGGLE,
};

use app::App;
use log::exe_dir;
use theme::tree::ThemeTree;

fn main() {
    // Initialize logging first
    log::init();
    log!("main() starting");

    // Enable DPI awareness early
    log!("Enabling DPI awareness...");
    if let Err(e) = enable_dpi_awareness() {
        log!("Warning: Failed to enable DPI awareness: {:?}", e);
    } else {
        log!("DPI awareness enabled");
    }

    // Load theme to determine window dimensions and hotkey
    let theme_path = exe_dir().join("default.rasi");
    log!("Loading theme from {:?}", theme_path);
    let (window_width, window_height, hotkey_config) = match ThemeTree::load(&theme_path) {
        Ok(theme) => {
            log!("Theme loaded successfully");
            // Read window dimensions from theme
            let width = theme.get_number("window", None, "width", 928.0) as i32;
            let height = theme.get_number("window", None, "height", 480.0) as i32;
            log!("Theme window size: {}x{}", width, height);

            // Read hotkey configuration from theme
            let hotkey_str = theme.get_hotkey_string("alt+space");
            log!("Theme hotkey string: {}", hotkey_str);
            let hotkey = parse_hotkey_string(&hotkey_str).unwrap_or_else(|| {
                log!("Invalid hotkey '{}', using default Alt+Space", hotkey_str);
                DEFAULT_TOGGLE_HOTKEY
            });
            log!("Using hotkey: {}", hotkey.display_name());

            (width, height, hotkey)
        }
        Err(e) => {
            log!("Failed to load theme: {:?}, using defaults", e);
            (928, 480, DEFAULT_TOGGLE_HOTKEY)
        }
    };
    log!("Window size: {}x{}", window_width, window_height);

    // Register window class
    log!("Registering window class...");
    if let Err(e) = register_window_class() {
        log!("FATAL: Failed to register window class: {:?}", e);
        return;
    }
    log!("Window class registered");

    // Create window configuration from theme dimensions
    let config = WindowConfig {
        width: window_width,
        height: window_height,
        vertical_position: 0.5, // True center of screen
    };
    log!("Config: {}x{}", config.width, config.height);

    // Create the window (hidden initially)
    log!("Creating window...");
    let hwnd = match create_window(&config) {
        Ok(h) => {
            log!("Window created: HWND={:?}", h);
            h
        }
        Err(e) => {
            log!("FATAL: Failed to create window: {:?}", e);
            unregister_window_class();
            return;
        }
    };

    // Register global hotkey (from theme or default Alt+Space)
    log!("Registering hotkey ({})...", hotkey_config.display_name());
    if let Err(e) = hotkey_config.register(hwnd) {
        log!("FATAL: Failed to register hotkey: {:?}", e);
        win32::destroy_window(hwnd);
        unregister_window_class();
        return;
    }
    log!("Hotkey registered: {}", hotkey_config.display_name());

    // Create application
    log!("Creating App...");
    let app = match App::new(hwnd, config) {
        Ok(a) => {
            log!("App created successfully");
            Rc::new(RefCell::new(a))
        }
        Err(e) => {
            log!("FATAL: Failed to create App: {:?}", e);
            hotkey_config.unregister(hwnd);
            win32::destroy_window(hwnd);
            unregister_window_class();
            return;
        }
    };

    // Set up window procedure callback
    log!("Setting window callback...");
    let app_clone = app.clone();
    set_window_callback(move |hwnd, msg, wparam, lparam| {
        // Use try_borrow_mut to avoid panic on re-entrancy
        // (ShowWindow can synchronously send WM_PAINT while we hold a borrow)
        match app_clone.try_borrow_mut() {
            Ok(mut app) => app.handle_message(hwnd, msg, wparam, lparam),
            Err(_) => {
                // Already borrowed - we're in a re-entrant call
                // Return None to let DefWindowProc handle it
                log!("WARNING: Re-entrant callback for msg={}, skipping", msg);
                None
            }
        }
    });

    // Start file watch timer for theme hot-reload
    log!("Starting file watch timer for theme hot-reload...");
    app.borrow().start_file_watch_timer();

    log!(
        "Wolfy started. Entering message loop. Press {} to toggle, F5 to reload theme.",
        hotkey_config.display_name()
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

            // Handle global hotkey in the message loop (before dispatch)
            if msg.message == WM_HOTKEY && msg.wParam.0 as i32 == HOTKEY_ID_TOGGLE {
                log!(
                    ">>> WM_HOTKEY received (msg #{}) - calling toggle_visibility()",
                    msg_count
                );
                let hwnd = app.borrow().hwnd();
                app.borrow_mut().toggle_visibility();
                log!("<<< toggle_visibility() returned");
                // Force a repaint after toggle (in case WM_PAINT was skipped due to re-entrancy)
                win32::invalidate_window(hwnd);
                continue;
            }

            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    // Cleanup
    log!("Cleaning up...");
    win32::clear_window_callback();
    hotkey_config.unregister(hwnd);
    unregister_window_class();

    log!("Wolfy exited normally.");
}
