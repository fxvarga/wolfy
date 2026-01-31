//! Grid Window - a simple window for displaying grid pickers
//!
//! This module provides a standalone window for ThemePicker and WallpaperPicker modes.
//! Each grid window has its own HWND, Renderer, and loads its own .rasi theme file.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{KillTimer, SetTimer};

use crate::animation::{Easing, WindowAnimator};
use crate::app::VERSION;
use crate::log::find_config_file;
use crate::mode::Mode;
use crate::platform::win32::{
    get_monitor_width, invalidate_window, resize_window, translate_message, Renderer, WindowConfig,
};
use crate::state::{scan_hyde_themes, scan_theme_wallpapers, AppState, HydeTheme};
use crate::theme::tree::ThemeTree;
use crate::theme::types::{Color, LayoutContext, Rect};
use crate::widget::{EventResult, GridItem, GridView, GridViewStyle, Widget, WidgetState};

/// Animation timer ID
const TIMER_ANIMATION: usize = 3;
/// Animation frame interval in milliseconds (~60fps)
const ANIMATION_FRAME_MS: u32 = 16;

/// Grid window style loaded from theme
#[derive(Clone, Debug)]
pub struct GridWindowStyle {
    pub background_color: Color,
    pub border_radius: f32,
    pub animation_duration_ms: u32,
    pub animation_easing: String,
}

impl Default for GridWindowStyle {
    fn default() -> Self {
        Self {
            background_color: Color::from_hex("#1e1e2ecc").unwrap_or(Color::BLACK),
            border_radius: 0.0,
            animation_duration_ms: 150,
            animation_easing: "ease-out-expo".to_string(),
        }
    }
}

impl GridWindowStyle {
    pub fn from_theme(theme: &ThemeTree) -> Self {
        let default = Self::default();
        Self {
            background_color: theme.get_color(
                "window",
                None,
                "background-color",
                default.background_color,
            ),
            border_radius: theme.get_number(
                "window",
                None,
                "border-radius",
                default.border_radius as f64,
            ) as f32,
            animation_duration_ms: theme.get_number(
                "window",
                None,
                "animation-duration",
                default.animation_duration_ms as f64,
            ) as u32,
            animation_easing: theme.get_string(
                "window",
                None,
                "animation-easing",
                &default.animation_easing,
            ),
        }
    }
}

/// A simple window for displaying a grid picker
pub struct GridWindow {
    hwnd: HWND,
    renderer: Renderer,
    mode: Mode,
    gridview: GridView,
    layout_ctx: LayoutContext,
    style: GridWindowStyle,
    animator: WindowAnimator,
    is_visible: bool,
    /// Shared application state
    app_state: Rc<RefCell<AppState>>,
    /// Theme file path for this window
    theme_path: PathBuf,
    /// Window dimensions from theme
    window_width: i32,
    window_height: i32,
}

impl GridWindow {
    /// Create a new grid window
    pub fn new(
        hwnd: HWND,
        mode: Mode,
        app_state: Rc<RefCell<AppState>>,
    ) -> Result<Self, windows::core::Error> {
        log!("GridWindow::new() for {:?}", mode);

        let renderer = Renderer::new(hwnd)?;
        let dpi_info = renderer.dpi();

        let layout_ctx = LayoutContext {
            dpi: dpi_info.dpi as f32,
            scale_factor: dpi_info.scale_factor,
            base_font_size: 16.0,
            parent_size: 1920.0, // Will be updated
        };

        // Determine theme file based on mode
        let theme_filename = match mode {
            Mode::ThemePicker => "theme_picker.rasi",
            Mode::WallpaperPicker => "wallpaper_picker.rasi",
            Mode::Launcher | Mode::TailView => "launcher.rasi", // Shouldn't happen for grid, but handle it
        };
        let theme_path = find_config_file(theme_filename);
        log!("  Loading theme from {:?}", theme_path);

        let theme = ThemeTree::load(&theme_path).ok();

        // Load styles from theme
        let style = theme
            .as_ref()
            .map(GridWindowStyle::from_theme)
            .unwrap_or_default();

        let gridview_style = theme
            .as_ref()
            .map(|t| GridViewStyle::from_theme(t, None))
            .unwrap_or_default();

        let gridview = GridView::new().with_style(gridview_style);

        // Get window dimensions from theme
        let window_width = theme
            .as_ref()
            .map(|t| t.get_number("window", None, "width", 1920.0) as i32)
            .unwrap_or(1920);
        let window_height = theme
            .as_ref()
            .map(|t| t.get_number("window", None, "height", 520.0) as i32)
            .unwrap_or(520);

        // Create animator
        let easing = Easing::from_name(&style.animation_easing);
        let animator = WindowAnimator::new(style.animation_duration_ms, easing);

        log!("GridWindow::new() completed for {:?}", mode);
        Ok(Self {
            hwnd,
            renderer,
            mode,
            gridview,
            layout_ctx,
            style,
            animator,
            is_visible: false,
            app_state,
            theme_path,
            window_width,
            window_height,
        })
    }

    /// Get the window handle
    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }

    /// Get the mode this window is for
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// Start animation timer
    fn start_animation_timer(&self) {
        unsafe {
            SetTimer(self.hwnd, TIMER_ANIMATION, ANIMATION_FRAME_MS, None);
        }
    }

    /// Stop animation timer
    fn stop_animation_timer(&self) {
        unsafe {
            let _ = KillTimer(self.hwnd, TIMER_ANIMATION);
        }
    }

    /// Show the window
    pub fn show(&mut self) {
        if self.is_visible {
            return;
        }

        log!("GridWindow::show() for {:?}", self.mode);

        // Resize to full monitor width, centered vertically
        let monitor_width = get_monitor_width();
        resize_window(self.hwnd, monitor_width, self.window_height, 0.5);

        // Load content based on mode
        self.load_content();

        // Start fade-in animation
        self.animator.start_fade_in();
        self.start_animation_timer();

        // Show the window
        crate::platform::win32::show_window(self.hwnd);
        self.is_visible = true;
    }

    /// Hide the window
    pub fn hide(&mut self) {
        if !self.is_visible {
            return;
        }

        log!("GridWindow::hide() for {:?}", self.mode);

        // Hide immediately (skip animation for now - animation has issues)
        crate::platform::win32::hide_window(self.hwnd);
        self.is_visible = false;
        self.stop_animation_timer();
        self.animator.clear();
    }

    /// Load content for the grid based on mode
    fn load_content(&mut self) {
        match self.mode {
            Mode::ThemePicker => {
                self.load_themes();
            }
            Mode::WallpaperPicker => {
                self.load_wallpapers();
            }
            Mode::Launcher | Mode::TailView => {
                // Launcher and TailView don't use GridWindow
            }
        }
    }

    /// Load HyDE themes into the grid
    fn load_themes(&mut self) {
        let mut state = self.app_state.borrow_mut();
        state.ensure_themes_loaded();

        let items: Vec<GridItem> = state
            .hyde_themes
            .iter()
            .map(|theme| {
                let mut item = GridItem::new(&theme.name, theme.path.to_string_lossy().to_string());
                if let Some(ref thumb) = theme.thumbnail {
                    item = item.with_image(thumb.to_string_lossy().to_string());
                }
                item
            })
            .collect();

        log!("Loaded {} themes into grid", items.len());
        drop(state); // Release borrow before calling set_items
        self.gridview.set_items(items);
    }

    /// Load wallpapers for the currently selected theme
    fn load_wallpapers(&mut self) {
        let state = self.app_state.borrow();
        let current_theme = state.current_theme.clone();
        drop(state);

        if let Some(theme_name) = current_theme {
            let wallpapers = scan_theme_wallpapers(&theme_name);
            let items: Vec<GridItem> = wallpapers
                .iter()
                .map(|path| {
                    let filename = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("Unknown");
                    GridItem::new(filename, path.to_string_lossy().to_string())
                        .with_image(path.to_string_lossy().to_string())
                })
                .collect();

            log!(
                "Loaded {} wallpapers for theme: {}",
                items.len(),
                theme_name
            );
            self.gridview.set_items(items);
        } else {
            // No theme selected - show empty grid with message
            log!("No theme selected, showing empty wallpaper grid");
            self.gridview.set_items(Vec::new());
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
        // Translate to our Event type
        if let Some(event) = translate_message(hwnd, msg, wparam, lparam) {
            let result = self.handle_event(&event);

            if result.needs_repaint {
                self.renderer.mark_dirty();
                invalidate_window(self.hwnd);
            }

            if result.submit {
                self.on_submit();
            }

            if result.cancel {
                self.on_cancel();
            }

            if result.consumed {
                return Some(LRESULT(0));
            }
        }

        // Handle paint
        if msg == 0x000F {
            // WM_PAINT
            if let Err(e) = self.render() {
                log!("GridWindow render error: {:?}", e);
            }
            return Some(LRESULT(0));
        }

        // Handle timer
        if msg == 0x0113 {
            // WM_TIMER
            let timer_id = wparam.0;
            if timer_id == TIMER_ANIMATION {
                self.on_animation_tick();
                return Some(LRESULT(0));
            }
        }

        None
    }

    /// Handle events
    fn handle_event(&mut self, event: &crate::platform::Event) -> EventResult {
        use crate::platform::win32::event::KeyCode;
        use crate::platform::Event;

        // Handle ESC and FocusLost at the window level
        match event {
            Event::KeyDown {
                key: KeyCode::Escape,
                ..
            } => {
                log!("GridWindow: ESC pressed, canceling");
                return EventResult {
                    needs_repaint: false,
                    consumed: true,
                    text_changed: false,
                    submit: false,
                    cancel: true,
                };
            }
            Event::FocusLost => {
                log!("GridWindow: Focus lost, canceling");
                return EventResult {
                    needs_repaint: false,
                    consumed: true,
                    text_changed: false,
                    submit: false,
                    cancel: true,
                };
            }
            _ => {}
        }

        // Forward other events to gridview
        self.gridview.handle_event(event, &self.layout_ctx)
    }

    /// Handle submit (Enter pressed)
    fn on_submit(&mut self) {
        if let Some(item) = self.gridview.selected_item() {
            match self.mode {
                Mode::ThemePicker => {
                    // Set current theme and hide
                    let theme_name = item.title.clone();
                    log!("Theme selected: {}", theme_name);
                    self.app_state
                        .borrow_mut()
                        .set_current_theme(Some(theme_name));
                    self.hide();
                }
                Mode::WallpaperPicker => {
                    // Set wallpaper and hide
                    let wallpaper_path = item.user_data.clone();
                    log!("Wallpaper selected: {}", wallpaper_path);
                    crate::platform::win32::set_wallpaper(&wallpaper_path);
                    self.hide();
                }
                Mode::Launcher | Mode::TailView => {
                    // Shouldn't happen - grid is not used in these modes
                }
            }
        }
    }

    /// Handle cancel (Escape pressed)
    fn on_cancel(&mut self) {
        self.hide();
    }

    /// Handle animation tick
    fn on_animation_tick(&mut self) {
        if !self.animator.update() {
            // Animation complete
            self.stop_animation_timer();

            // Check if we were fading out (opacity went to 0)
            if self.animator.get_opacity() < 0.01 {
                // Fade out complete - actually hide
                crate::platform::win32::hide_window(self.hwnd);
                self.is_visible = false;
            }
        }

        // Update window opacity and repaint
        self.renderer.mark_dirty();
        invalidate_window(self.hwnd);
    }

    /// Render the window
    fn render(&mut self) -> Result<(), windows::core::Error> {
        let size = self.renderer.get_size();
        let _scale = self.layout_ctx.scale_factor;

        // Begin render
        if !self.renderer.begin_draw() {
            return Ok(()); // Can't draw right now
        }

        // Apply animation opacity
        let opacity = self.animator.get_opacity();

        // Draw background
        let mut bg = self.style.background_color;
        bg.a *= opacity;
        self.renderer.clear(bg);

        // Calculate gridview bounds (full window minus any padding we want)
        let bounds = Rect::new(0.0, 0.0, size.0 as f32, size.1 as f32);

        // Arrange and render gridview
        self.gridview.arrange(bounds, &self.layout_ctx);
        self.gridview
            .render(&mut self.renderer, bounds, &self.layout_ctx)?;

        // Draw version watermark in bottom right corner
        self.draw_version_watermark(size.0, size.1);

        // End render
        self.renderer.end_draw()?;

        Ok(())
    }

    /// Reload theme
    pub fn reload_theme(&mut self) {
        log!("GridWindow::reload_theme() for {:?}", self.mode);

        if let Ok(theme) = ThemeTree::load(&self.theme_path) {
            self.style = GridWindowStyle::from_theme(&theme);
            let gridview_style = GridViewStyle::from_theme(&theme, None);
            self.gridview.set_style(gridview_style);

            self.window_width = theme.get_number("window", None, "width", 1920.0) as i32;
            self.window_height = theme.get_number("window", None, "height", 520.0) as i32;

            let easing = Easing::from_name(&self.style.animation_easing);
            self.animator = WindowAnimator::new(self.style.animation_duration_ms, easing);
        }
    }

    /// Draw version watermark in bottom right corner
    fn draw_version_watermark(&mut self, width: i32, height: i32) {
        use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;

        let version_text = format!("v{}", VERSION);
        let font_size = 10.0 * self.layout_ctx.scale_factor;

        // Create a small text format for the version
        let text_format = match self
            .renderer
            .create_text_format("Segoe UI", font_size, false, false)
        {
            Ok(fmt) => fmt,
            Err(_) => return,
        };

        // Position in bottom right with some margin
        let text_width = 60.0 * self.layout_ctx.scale_factor;
        let text_height = 14.0 * self.layout_ctx.scale_factor;
        let margin = 8.0 * self.layout_ctx.scale_factor;

        let rect = D2D_RECT_F {
            left: width as f32 - text_width - margin,
            top: height as f32 - text_height - margin,
            right: width as f32 - margin,
            bottom: height as f32 - margin,
        };

        // Draw with low opacity as a subtle watermark
        let watermark_color = Color::from_f32(1.0, 1.0, 1.0, 0.3);
        let _ = self.renderer.draw_text_right_aligned(
            &version_text,
            &text_format,
            rect,
            watermark_color,
        );
    }
}
