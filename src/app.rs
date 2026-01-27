//! Application state machine for Wolfy

use std::path::Path;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap;
use windows::Win32::UI::WindowsAndMessaging::{
    KillTimer, SetTimer, WM_DPICHANGED, WM_PAINT, WM_TIMER,
};

use crate::log::exe_dir;
use crate::platform::win32::{
    self, get_wallpaper_path, invalidate_window, reposition_window, translate_message, Event,
    ImageLoader, Renderer, WindowConfig,
};
use crate::theme::tree::ThemeTree;
use crate::theme::types::{ImageScale, LayoutContext, Rect};
use crate::widget::{
    ElementData, ElementStyle, EventResult, ListView, ListViewStyle, Textbox, Widget, WidgetState,
    WidgetStyle,
};

/// Cursor blink timer ID
const TIMER_CURSOR_BLINK: usize = 1;
/// Cursor blink interval in milliseconds
const CURSOR_BLINK_MS: u32 = 530;

/// Application state
pub struct App {
    hwnd: HWND,
    renderer: Renderer,
    config: WindowConfig,
    textbox: Textbox,
    listview: ListView,
    /// All available items (unfiltered)
    all_items: Vec<ElementData>,
    layout_ctx: LayoutContext,
    style: WidgetStyle,
    /// Cached background bitmap
    background_bitmap: Option<ID2D1Bitmap>,
    /// Path that was used to load the background bitmap (for cache invalidation)
    background_bitmap_path: Option<String>,
}

impl App {
    /// Create new application
    pub fn new(hwnd: HWND, config: WindowConfig) -> Result<Self, windows::core::Error> {
        log!("App::new() starting, hwnd={:?}", hwnd);

        log!("  Creating Renderer...");
        let renderer = Renderer::new(hwnd)?;
        log!("  Renderer created");

        let dpi_info = renderer.dpi();
        log!(
            "  DPI info: dpi={}, scale={}",
            dpi_info.dpi,
            dpi_info.scale_factor
        );

        let layout_ctx = LayoutContext {
            dpi: dpi_info.dpi as f32,
            scale_factor: dpi_info.scale_factor,
            base_font_size: 16.0,
            parent_size: config.width as f32,
        };

        // Load theme from exe directory
        let theme_path = exe_dir().join("default.rasi");
        log!("  Loading theme from {:?}", theme_path);
        let theme = match ThemeTree::load(&theme_path) {
            Ok(t) => {
                log!("  Theme loaded successfully");
                Some(t)
            }
            Err(e) => {
                log!("  Failed to load theme: {:?}, using defaults", e);
                None
            }
        };

        // Create textbox with theme style
        let style = theme
            .as_ref()
            .map(|t| WidgetStyle::from_theme_textbox(t, None))
            .unwrap_or_default();
        log!(
            "  Textbox style: font_size={}, font_family={}",
            style.font_size,
            style.font_family
        );

        let mut textbox = Textbox::new()
            .with_placeholder("Type to search...")
            .with_style(style.clone());
        textbox.set_state(WidgetState::Focused);

        // Load listview and element styles from theme
        let listview_style = theme
            .as_ref()
            .map(|t| ListViewStyle::from_theme(t, None))
            .unwrap_or_default();
        let element_style = theme
            .as_ref()
            .map(|t| ElementStyle::from_theme(t, None))
            .unwrap_or_default();

        let listview = ListView::new()
            .with_style(listview_style)
            .with_element_style(element_style);

        // Sample items for development - will be replaced with app discovery
        let all_items = vec![
            ElementData::new("Calculator", "calc.exe").with_subtext("Windows Calculator"),
            ElementData::new("Notepad", "notepad.exe").with_subtext("Text Editor"),
            ElementData::new("Paint", "mspaint.exe").with_subtext("Image Editor"),
            ElementData::new("Command Prompt", "cmd.exe").with_subtext("Windows Terminal"),
            ElementData::new("PowerShell", "powershell.exe").with_subtext("Windows PowerShell"),
            ElementData::new("File Explorer", "explorer.exe").with_subtext("File Manager"),
            ElementData::new("Task Manager", "taskmgr.exe").with_subtext("System Monitor"),
            ElementData::new("Control Panel", "control.exe").with_subtext("System Settings"),
            ElementData::new("Registry Editor", "regedit.exe").with_subtext("Windows Registry"),
            ElementData::new("Device Manager", "devmgmt.msc").with_subtext("Hardware Manager"),
            ElementData::new("Disk Management", "diskmgmt.msc").with_subtext("Disk Utility"),
            ElementData::new("Services", "services.msc").with_subtext("Windows Services"),
        ];

        log!("App::new() completed successfully");
        Ok(Self {
            hwnd,
            renderer,
            config,
            textbox,
            listview,
            all_items,
            layout_ctx,
            style,
            background_bitmap: None,
            background_bitmap_path: None,
        })
    }

    /// Start cursor blink timer
    pub fn start_cursor_timer(&self) {
        unsafe {
            SetTimer(self.hwnd, TIMER_CURSOR_BLINK, CURSOR_BLINK_MS, None);
        }
    }

    /// Stop cursor blink timer
    pub fn stop_cursor_timer(&self) {
        unsafe {
            let _ = KillTimer(self.hwnd, TIMER_CURSOR_BLINK);
        }
    }

    /// Handle window procedure messages
    pub fn handle_message(
        &mut self,
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> Option<LRESULT> {
        // Log interesting messages
        if msg == 0x0100 || msg == 0x0101 || msg == 0x0102 || msg == 0x0010 || msg == 0x0112 {
            // WM_KEYDOWN, WM_KEYUP, WM_CHAR, WM_CLOSE, WM_SYSCOMMAND
            log!("handle_message: msg=0x{:04X}, wparam=0x{:X}", msg, wparam.0);
        }

        // Translate to our Event type
        if let Some(event) = translate_message(hwnd, msg, wparam, lparam) {
            log!("  Translated to event: {:?}", event);
            let result = self.handle_event(&event);
            log!(
                "  Result: consumed={}, submit={}, cancel={}",
                result.consumed,
                result.submit,
                result.cancel
            );

            if result.needs_repaint {
                invalidate_window(self.hwnd);
            }

            if result.submit {
                log!("  Calling on_submit()");
                self.on_submit();
            }

            if result.cancel {
                log!("  Calling on_cancel()");
                self.on_cancel();
            }

            if result.consumed {
                log!("  Returning consumed (LRESULT(0))");
                return Some(LRESULT(0));
            }
        }

        // Handle specific messages that need special treatment
        match msg {
            WM_PAINT => {
                self.paint();
                return Some(LRESULT(0));
            }
            WM_TIMER if wparam.0 == TIMER_CURSOR_BLINK => {
                self.textbox.toggle_cursor_blink();
                invalidate_window(self.hwnd);
                return Some(LRESULT(0));
            }
            WM_DPICHANGED => {
                let new_dpi = (wparam.0 & 0xFFFF) as u32;
                let _ = self.handle_dpi_change(new_dpi);
                return Some(LRESULT(0));
            }
            _ => {}
        }

        None
    }

    /// Handle an event
    fn handle_event(&mut self, event: &Event) -> EventResult {
        use crate::platform::win32::event::KeyCode;

        // Check for navigation keys that should go to listview
        if let Event::KeyDown { key, .. } = event {
            match *key {
                // Arrow keys navigate the list
                KeyCode::Down | KeyCode::Up | KeyCode::PageDown | KeyCode::PageUp => {
                    let result = self.listview.handle_event(event, &self.layout_ctx);
                    if result.consumed {
                        return result;
                    }
                }
                // Enter activates selected item
                KeyCode::Enter => {
                    if self.listview.selected_data().is_some() {
                        return EventResult {
                            needs_repaint: false,
                            consumed: true,
                            text_changed: false,
                            submit: true,
                            cancel: false,
                        };
                    }
                }
                _ => {}
            }
        }

        // Forward other events to textbox
        let result = self.textbox.handle_event(event, &self.layout_ctx);

        // Handle text changes - filter the list
        if result.text_changed {
            self.on_text_changed();
        }

        // Reset cursor blink on any key event
        if matches!(event, Event::KeyDown { .. } | Event::Char(_)) {
            self.textbox.show_cursor();
        }

        result
    }

    /// Handle submit (Enter pressed)
    fn on_submit(&mut self) {
        // Get selected item from listview
        if let Some(data) = self.listview.selected_data() {
            let command = data.user_data.clone();
            let name = data.text.clone();
            log!("Launching: {} ({})", name, command);

            // Try to launch the application
            if let Err(e) = self.launch_app(&command) {
                log!("Failed to launch {}: {:?}", command, e);
            }
        }

        self.textbox.clear();
        win32::hide_window(self.hwnd);
        self.stop_cursor_timer();
    }

    /// Launch an application
    fn launch_app(&self, command: &str) -> Result<(), windows::core::Error> {
        use std::os::windows::process::CommandExt;
        use std::process::Command;

        const CREATE_NO_WINDOW: u32 = 0x08000000;
        const DETACHED_PROCESS: u32 = 0x00000008;

        // Try to run the command
        let result = Command::new("cmd")
            .args(["/C", "start", "", command])
            .creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS)
            .spawn();

        match result {
            Ok(_) => {
                log!("Successfully spawned: {}", command);
                Ok(())
            }
            Err(e) => {
                log!("Failed to spawn {}: {}", command, e);
                Err(windows::core::Error::from_win32())
            }
        }
    }

    /// Handle cancel (Escape pressed)
    fn on_cancel(&mut self) {
        log!("on_cancel() called - hiding window");
        self.textbox.clear();
        win32::hide_window(self.hwnd);
        self.stop_cursor_timer();
        log!("on_cancel() completed");
    }

    /// Handle text changes - filter the list
    fn on_text_changed(&mut self) {
        let query = self.textbox.text().to_lowercase();

        if query.is_empty() {
            // Show all items when no search query
            self.listview.set_items(self.all_items.clone());
        } else {
            // Filter items by name (case-insensitive)
            let filtered: Vec<ElementData> = self
                .all_items
                .iter()
                .filter(|item| {
                    item.text.to_lowercase().contains(&query)
                        || item
                            .subtext
                            .as_ref()
                            .map(|s| s.to_lowercase().contains(&query))
                            .unwrap_or(false)
                })
                .cloned()
                .collect();

            self.listview.set_items(filtered);
        }
    }

    /// Handle DPI change
    fn handle_dpi_change(&mut self, new_dpi: u32) -> Result<(), windows::core::Error> {
        self.renderer.handle_dpi_change(new_dpi)?;
        self.layout_ctx.dpi = new_dpi as f32;
        self.layout_ctx.scale_factor = new_dpi as f32 / 96.0;
        reposition_window(self.hwnd, &self.config);
        Ok(())
    }

    /// Paint the window - Split panel layout (HyDE style)
    fn paint(&mut self) {
        log!("paint() called");

        // begin_draw returns false if render target couldn't be created (e.g., zero-size window)
        log!("  Calling begin_draw()...");
        if !self.renderer.begin_draw() {
            log!("  begin_draw() returned false, skipping paint");
            return;
        }
        log!("  begin_draw() succeeded");

        // Clear with transparent first (for layered window)
        self.renderer.clear(crate::theme::types::Color::TRANSPARENT);

        // Get client size
        let (width, height) = win32::get_client_size(self.hwnd);
        log!("  Client size: {}x{}", width, height);
        let scale = self.layout_ctx.scale_factor;

        // Split panel layout dimensions
        let _corner_radius = 12.0 * scale;
        let wallpaper_width = 300.0 * scale;
        let right_panel_x = wallpaper_width;
        let right_panel_width = width as f32 - wallpaper_width;

        // =====================
        // LEFT PANEL: Wallpaper
        // =====================
        self.draw_wallpaper_panel(0.0, 0.0, wallpaper_width, height as f32);

        // =====================
        // RIGHT PANEL: Listbox (search + results)
        // =====================
        // Draw dark semi-transparent background
        let listbox_color = crate::theme::types::Color::from_f32(0.118, 0.118, 0.118, 0.9); // rgba(30, 30, 30, 230)
        self.draw_right_panel(
            right_panel_x,
            0.0,
            right_panel_width,
            height as f32,
            listbox_color,
        );

        // Calculate textbox rect within right panel
        let padding = 12.0 * scale;
        let textbox_height = 56.0 * scale;
        let spacing = 8.0 * scale;

        let textbox_rect = Rect::new(
            right_panel_x + padding,
            padding,
            right_panel_width - padding * 2.0,
            textbox_height,
        );

        // Render textbox
        log!("  Rendering textbox...");
        let _ = self
            .textbox
            .render(&mut self.renderer, textbox_rect, &self.layout_ctx);

        // Calculate listview rect (below textbox, within right panel)
        let listview_top = padding + textbox_height + spacing;
        let listview_height = height as f32 - listview_top - padding;

        if listview_height > 0.0 && !self.listview.is_empty() {
            let listview_rect = Rect::new(
                right_panel_x + padding,
                listview_top,
                right_panel_width - padding * 2.0,
                listview_height,
            );

            log!("  Rendering listview ({} items)...", self.listview.len());
            let _ = self
                .listview
                .render(&mut self.renderer, listview_rect, &self.layout_ctx);
        }

        log!("  Calling end_draw()...");
        let result = self.renderer.end_draw();
        log!("  end_draw() result: {:?}", result);
    }

    /// Draw the left wallpaper panel
    fn draw_wallpaper_panel(&mut self, x: f32, y: f32, width: f32, height: f32) {
        use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;

        log!(
            "  draw_wallpaper_panel: x={}, y={}, w={}, h={}",
            x,
            y,
            width,
            height
        );

        // First, draw a solid dark background so we have something visible even without wallpaper
        let fallback_bg = crate::theme::types::Color::from_f32(0.1, 0.1, 0.1, 1.0);
        let bounds = D2D_RECT_F {
            left: x,
            top: y,
            right: x + width,
            bottom: y + height,
        };
        log!("  draw_wallpaper_panel: calling fill_rect for fallback bg");
        let _ = self.renderer.fill_rect(bounds, fallback_bg);
        log!("  draw_wallpaper_panel: fill_rect done, calling draw_background_image_in_rect");

        // Try to draw the wallpaper (auto-detect) - this will overlay the fallback
        self.draw_background_image_in_rect(
            "auto".to_string(),
            ImageScale::Height,
            x,
            y,
            width,
            height,
        );
        log!("  draw_wallpaper_panel: draw_background_image_in_rect done");

        // Draw dark overlay on the wallpaper/background
        let overlay_color = crate::theme::types::Color::from_f32(0.0, 0.0, 0.0, 0.24); // rgba(0, 0, 0, 60)
        let _ = self.renderer.fill_rect(bounds, overlay_color);
        log!("  draw_wallpaper_panel: overlay done");
    }

    /// Draw the right panel with solid color
    fn draw_right_panel(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        color: crate::theme::types::Color,
    ) {
        use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;

        log!(
            "  draw_right_panel: x={}, y={}, w={}, h={}, color=({},{},{},{})",
            x,
            y,
            width,
            height,
            color.r,
            color.g,
            color.b,
            color.a
        );

        // Draw the main rectangle
        let bounds = D2D_RECT_F {
            left: x,
            top: y,
            right: x + width,
            bottom: y + height,
        };
        let _ = self.renderer.fill_rect(bounds, color);
    }

    /// Draw the background image in a specific rectangle, loading and caching as needed
    fn draw_background_image_in_rect(
        &mut self,
        path: String,
        scale: ImageScale,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    ) {
        log!(
            "  draw_background_image_in_rect: path={}, x={}, y={}, w={}, h={}",
            path,
            x,
            y,
            width,
            height
        );

        // Resolve "auto" to wallpaper path
        log!("  calling get_wallpaper_path...");
        let resolved_path = if path.eq_ignore_ascii_case("auto") {
            match get_wallpaper_path() {
                Some(p) => {
                    let path_str = p.to_string_lossy().into_owned();
                    log!("  got wallpaper path: {}", path_str);
                    path_str
                }
                None => {
                    log!("  No wallpaper found for 'auto' path");
                    return;
                }
            }
        } else {
            path.clone()
        };

        // Check if we need to reload the bitmap
        let need_reload = match &self.background_bitmap_path {
            Some(cached_path) => cached_path != &resolved_path,
            None => true,
        };

        if need_reload {
            log!("  Loading background image: {}", resolved_path);

            // Load the image
            let loader = match ImageLoader::new() {
                Ok(l) => l,
                Err(e) => {
                    log!("  Failed to create ImageLoader: {:?}", e);
                    return;
                }
            };

            // For the wallpaper panel, load with height scaling to ensure vertical coverage
            let loaded = match scale {
                ImageScale::None => loader.load_from_file(Path::new(&resolved_path)),
                ImageScale::Width => loader.load_scaled(
                    Path::new(&resolved_path),
                    width as u32,
                    0,
                    ImageScale::Width,
                ),
                ImageScale::Height => loader.load_scaled(
                    Path::new(&resolved_path),
                    0,
                    height as u32,
                    ImageScale::Height,
                ),
                ImageScale::Both => {
                    loader.load_cover(Path::new(&resolved_path), width as u32, height as u32)
                }
            };

            let loaded = match loaded {
                Ok(l) => l,
                Err(e) => {
                    log!("  Failed to load image: {:?}", e);
                    return;
                }
            };

            log!("  Loaded image: {}x{}", loaded.width(), loaded.height());

            // Create D2D bitmap
            let bitmap = match self.renderer.create_bitmap(&loaded) {
                Ok(b) => b,
                Err(e) => {
                    log!("  Failed to create D2D bitmap: {:?}", e);
                    return;
                }
            };

            self.background_bitmap = Some(bitmap);
            self.background_bitmap_path = Some(resolved_path);
        }

        // Draw the cached bitmap in the specified rect
        if let Some(ref bitmap) = self.background_bitmap {
            let bounds = windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F {
                left: x,
                top: y,
                right: x + width,
                bottom: y + height,
            };
            let _ = self.renderer.draw_bitmap_cover(bitmap, bounds, 1.0);
        }
    }

    /// Get the window handle
    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }

    /// Get current input text
    pub fn input_text(&self) -> &str {
        self.textbox.text()
    }

    /// Toggle window visibility (called from hotkey)
    pub fn toggle_visibility(&mut self) {
        log!("toggle_visibility() called");

        log!("  Calling win32::toggle_window()...");
        let visible = win32::toggle_window(self.hwnd);
        log!("  toggle_window() returned visible={}", visible);

        if visible {
            log!("  Window now visible, setting up...");
            self.textbox.set_state(WidgetState::Focused);
            self.textbox.show_cursor();
            self.start_cursor_timer();

            // Initialize listview with all items
            self.listview.set_items(self.all_items.clone());

            // NOTE: Don't call invalidate_window here - it's called by the main loop
            // after we return. This avoids re-entrancy issues since ShowWindow
            // may synchronously send WM_PAINT while we hold the RefCell borrow.
            log!("  Setup complete (repaint will be triggered by caller)");
        } else {
            log!("  Window now hidden");
            self.stop_cursor_timer();
        }
        log!("toggle_visibility() completed");
    }
}
