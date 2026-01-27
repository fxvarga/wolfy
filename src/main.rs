//! Wolfy - A Windows application launcher inspired by rofi
//!
//! Press Alt+Space to toggle the launcher window.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[macro_use]
extern crate lalrpop_util;

#[macro_use]
mod log;

mod app;
mod platform;
mod theme;
mod widget;

use std::cell::RefCell;
use std::rc::Rc;

use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, TranslateMessage, MSG, WM_HOTKEY,
};

use platform::win32::{
    self, create_window, enable_dpi_awareness, register_hotkey, register_window_class,
    set_window_callback, unregister_hotkey, unregister_window_class, WindowConfig,
    HOTKEY_ID_TOGGLE,
};

use app::App;
use log::exe_dir;
use theme::tree::ThemeTree;
use widget::WidgetStyle;

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

    // Load theme to determine window dimensions
    let theme_path = exe_dir().join("default.rasi");
    log!("Loading theme from {:?}", theme_path);
    let style = match ThemeTree::load(&theme_path) {
        Ok(theme) => {
            log!("Theme loaded successfully");
            WidgetStyle::from_theme_textbox(&theme, None)
        }
        Err(e) => {
            log!("Failed to load theme: {:?}, using defaults", e);
            WidgetStyle::default()
        }
    };

    // Calculate window height based on font size + padding + border + listview
    // Textbox: font_size + padding_top + padding_bottom + border*2
    // ListView: element_height * max_visible_items + padding
    let textbox_height = style.font_size
        + style.padding_top
        + style.padding_bottom
        + style.border_width * 2.0
        + 16.0; // Extra margin

    // Listview: 10 items * 40px height + padding
    let listview_height = 10.0 * 42.0 + 16.0; // 10 items with spacing + padding

    let window_height = (textbox_height + listview_height + 24.0) as i32; // 24px for spacing
    log!(
        "Calculated window height: {} (textbox={}, listview={})",
        window_height,
        textbox_height,
        listview_height
    );

    // Register window class
    log!("Registering window class...");
    if let Err(e) = register_window_class() {
        log!("FATAL: Failed to register window class: {:?}", e);
        return;
    }
    log!("Window class registered");

    // Create window configuration
    // Split panel: 300px wallpaper + ~500px listbox = ~800px total
    let config = WindowConfig {
        width: 850,
        height: window_height,
        vertical_position: 0.25, // Upper third
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

    // Register global hotkey (Alt+Space)
    log!("Registering hotkey (Alt+Space)...");
    if let Err(e) = register_hotkey(hwnd) {
        log!("FATAL: Failed to register hotkey: {:?}", e);
        win32::destroy_window(hwnd);
        unregister_window_class();
        return;
    }
    log!("Hotkey registered");

    // Create application
    log!("Creating App...");
    let app = match App::new(hwnd, config) {
        Ok(a) => {
            log!("App created successfully");
            Rc::new(RefCell::new(a))
        }
        Err(e) => {
            log!("FATAL: Failed to create App: {:?}", e);
            unregister_hotkey(hwnd);
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

    log!("Wolfy started. Entering message loop. Press Alt+Space to toggle.");

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
    unregister_hotkey(hwnd);
    unregister_window_class();

    log!("Wolfy exited normally.");
}
