//! Application state machine for Wolfy

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    KillTimer, SetTimer, WM_DPICHANGED, WM_PAINT, WM_TIMER,
};

use crate::platform::win32::{
    self, invalidate_window, reposition_window, translate_message, Event, Renderer, WindowConfig,
};
use crate::theme::types::{Color, LayoutContext, Rect};
use crate::widget::{EventResult, Textbox, Widget, WidgetState};

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
    layout_ctx: LayoutContext,
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

        let mut textbox = Textbox::new().with_placeholder("Type to search...");
        textbox.set_state(WidgetState::Focused);

        log!("App::new() completed successfully");
        Ok(Self {
            hwnd,
            renderer,
            config,
            textbox,
            layout_ctx,
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
        // Forward to textbox
        let result = self.textbox.handle_event(event, &self.layout_ctx);

        // Handle text changes
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
        let text = self.textbox.text().to_string();
        if !text.is_empty() {
            // TODO: Execute the command/launch the app
            println!("Submit: {}", text);
            self.textbox.clear();
        }
        win32::hide_window(self.hwnd);
        self.stop_cursor_timer();
    }

    /// Handle cancel (Escape pressed)
    fn on_cancel(&mut self) {
        log!("on_cancel() called - hiding window");
        self.textbox.clear();
        win32::hide_window(self.hwnd);
        self.stop_cursor_timer();
        log!("on_cancel() completed");
    }

    /// Handle text changes
    fn on_text_changed(&mut self) {
        let _text = self.textbox.text();
        // TODO: Update search results/suggestions
    }

    /// Handle DPI change
    fn handle_dpi_change(&mut self, new_dpi: u32) -> Result<(), windows::core::Error> {
        self.renderer.handle_dpi_change(new_dpi)?;
        self.layout_ctx.dpi = new_dpi as f32;
        self.layout_ctx.scale_factor = new_dpi as f32 / 96.0;
        reposition_window(self.hwnd, &self.config);
        Ok(())
    }

    /// Paint the window
    fn paint(&mut self) {
        log!("paint() called");

        // begin_draw returns false if render target couldn't be created (e.g., zero-size window)
        log!("  Calling begin_draw()...");
        if !self.renderer.begin_draw() {
            log!("  begin_draw() returned false, skipping paint");
            return;
        }
        log!("  begin_draw() succeeded");

        // Clear with dark background
        log!("  Clearing background...");
        self.renderer
            .clear(Color::from_hex("#1e1e1e").unwrap_or(Color::BLACK));

        // Get client size
        let (width, height) = win32::get_client_size(self.hwnd);
        log!("  Client size: {}x{}", width, height);
        let scale = self.layout_ctx.scale_factor;

        // Calculate textbox rect with padding
        let padding = 8.0 * scale;
        let rect = Rect::new(
            padding,
            padding,
            width as f32 - padding * 2.0,
            height as f32 - padding * 2.0,
        );

        // Render textbox
        log!("  Rendering textbox...");
        let _ = self
            .textbox
            .render(&mut self.renderer, rect, &self.layout_ctx);

        log!("  Calling end_draw()...");
        let result = self.renderer.end_draw();
        log!("  end_draw() result: {:?}", result);
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
