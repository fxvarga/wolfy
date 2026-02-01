//! Tail View widget - Embedded terminal output viewer
//!
//! Displays task output in a themed terminal-like view with scrolling.
//! Features a button bar on the left (compact icon style like task panel)
//! with Back and Open Terminal buttons.

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::time::Instant;

use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;

use crate::platform::win32::Renderer;
use crate::platform::Event;
use crate::theme::tree::ThemeTree;
use crate::theme::types::{Color, LayoutContext, Rect};

use super::base::CornerRadii;
use super::taskpanel::TaskPanelStyle;

/// Maximum number of lines to keep in buffer
const MAX_LINES: usize = 10000;

/// Result of hit testing the tail view
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TailViewHit {
    /// Back button was clicked
    BackButton,
    /// Open Terminal button was clicked
    OpenTerminalButton,
    /// Rerun button was clicked
    RerunButton,
    /// Kill process button was clicked
    KillButton,
    /// Content area was clicked (for focus)
    Content,
    /// Nothing was hit
    None,
}

/// Style for the tail view content area
#[derive(Clone, Debug)]
pub struct TailViewStyle {
    /// Background color for the entire view
    pub background_color: Color,
    /// Text color for output lines
    pub text_color: Color,
    /// Font family (should be monospace)
    pub font_family: String,
    /// Font size in pixels
    pub font_size: f32,
    /// Padding around content area
    pub padding: f32,
    /// Line spacing
    pub line_spacing: f32,
    /// Border radius for the view
    pub border_radius: f32,
}

impl Default for TailViewStyle {
    fn default() -> Self {
        Self {
            background_color: Color::from_hex("#1a1625f0").unwrap_or(Color::BLACK),
            text_color: Color::from_hex("#e8e0f0").unwrap_or(Color::WHITE),
            font_family: "Cascadia Code".to_string(),
            font_size: 12.0,
            padding: 16.0,
            line_spacing: 4.0,
            border_radius: 24.0,
        }
    }
}

impl TailViewStyle {
    /// Load style from theme
    pub fn from_theme(theme: &ThemeTree, state: Option<&str>) -> Self {
        let default = Self::default();
        Self {
            background_color: theme.get_color(
                "tailview",
                state,
                "background-color",
                default.background_color,
            ),
            text_color: theme.get_color("tailview", state, "text-color", default.text_color),
            font_family: theme.get_string("tailview", state, "font-family", &default.font_family),
            font_size: theme.get_number("tailview", state, "font-size", default.font_size as f64)
                as f32,
            padding: theme.get_number("tailview", state, "padding", default.padding as f64) as f32,
            line_spacing: theme.get_number(
                "tailview",
                state,
                "line-spacing",
                default.line_spacing as f64,
            ) as f32,
            border_radius: theme.get_number(
                "tailview",
                state,
                "border-radius",
                default.border_radius as f64,
            ) as f32,
        }
    }
}

/// Tail view widget for displaying task output
pub struct TailView {
    /// Lines of output
    lines: Vec<String>,
    /// Scroll offset (line index at top of view)
    scroll_offset: usize,
    /// Auto-scroll to bottom on new content
    auto_scroll: bool,
    /// Task being tailed (group:name)
    task_key: Option<String>,
    /// Path to the log file
    log_path: Option<PathBuf>,
    /// Cached bounds
    bounds: Option<Rect>,
    /// Cached scale factor
    cached_scale: f32,
    /// Style for content area
    style: TailViewStyle,
    /// Style for button bar (from task panel)
    button_style: TaskPanelStyle,
    /// Back button rect (for hit testing)
    back_button_rect: Option<Rect>,
    /// Terminal button rect (for hit testing)
    terminal_button_rect: Option<Rect>,
    /// Rerun button rect (for hit testing)
    rerun_button_rect: Option<Rect>,
    /// Kill button rect (for hit testing)
    kill_button_rect: Option<Rect>,
    /// Number of visible lines (calculated during render)
    visible_lines: usize,
    /// Currently selected button (0=Back, 1=Terminal, 2=Rerun)
    selected_button: usize,
    /// Last time we refreshed the file
    last_refresh: Instant,
}

impl TailView {
    /// Create a new tail view
    pub fn new() -> Self {
        Self {
            lines: Vec::new(),
            scroll_offset: 0,
            auto_scroll: true,
            task_key: None,
            log_path: None,
            bounds: None,
            cached_scale: 1.0,
            style: TailViewStyle::default(),
            button_style: TaskPanelStyle::default(),
            back_button_rect: None,
            terminal_button_rect: None,
            rerun_button_rect: None,
            kill_button_rect: None,
            visible_lines: 0,
            selected_button: 0,
            last_refresh: Instant::now(),
        }
    }

    /// Set the style from theme
    pub fn set_style(&mut self, style: TailViewStyle) {
        self.style = style;
    }

    /// Set the button bar style (from task panel)
    pub fn set_button_style(&mut self, style: TaskPanelStyle) {
        self.button_style = style;
    }

    /// Start tailing a task
    pub fn start_tail(&mut self, task_key: String, log_path: PathBuf) {
        self.task_key = Some(task_key);
        self.log_path = Some(log_path);
        self.lines.clear();
        self.scroll_offset = 0;
        self.auto_scroll = true;
        self.refresh();
    }

    /// Stop tailing and clear buffer
    pub fn stop_tail(&mut self) {
        self.task_key = None;
        self.log_path = None;
        self.lines.clear();
        self.scroll_offset = 0;
        self.selected_button = 0;
        self.last_refresh = Instant::now();
    }

    /// Get the task key being tailed
    pub fn task_key(&self) -> Option<&str> {
        self.task_key.as_deref()
    }

    /// Refresh lines from log file
    pub fn refresh(&mut self) {
        self.last_refresh = Instant::now();

        let Some(path) = &self.log_path else {
            return;
        };

        let Ok(file) = File::open(path) else {
            crate::log!("TailView::refresh: Failed to open {:?}", path);
            return;
        };

        let reader = BufReader::new(file);
        let mut new_lines: Vec<String> = reader.lines().flatten().collect();

        // Cap at MAX_LINES
        if new_lines.len() > MAX_LINES {
            new_lines = new_lines.split_off(new_lines.len() - MAX_LINES);
        }

        let old_len = self.lines.len();
        let had_new_content = new_lines.len() > old_len;
        self.lines = new_lines;

        if had_new_content {
            crate::log!("TailView::refresh: {} -> {} lines", old_len, self.lines.len());
        }

        // Auto-scroll to bottom if enabled and new content arrived
        if self.auto_scroll && had_new_content {
            self.scroll_to_bottom();
        }
    }

    /// Check if we should refresh based on time elapsed (200ms)
    pub fn maybe_refresh(&mut self) {
        if self.log_path.is_some() && self.last_refresh.elapsed().as_millis() >= 200 {
            self.refresh();
        }
    }

    /// Scroll by delta lines (positive = down, negative = up)
    pub fn scroll_by(&mut self, delta: i32) {
        if delta < 0 {
            self.scroll_offset = self.scroll_offset.saturating_sub((-delta) as usize);
        } else {
            let max_scroll = self.lines.len().saturating_sub(self.visible_lines);
            self.scroll_offset = (self.scroll_offset + delta as usize).min(max_scroll);
        }

        // Disable auto-scroll when manually scrolling up
        if delta < 0 {
            self.auto_scroll = false;
        }
    }

    /// Scroll to bottom and re-enable auto-scroll
    pub fn scroll_to_bottom(&mut self) {
        let max_scroll = self.lines.len().saturating_sub(self.visible_lines.max(1));
        self.scroll_offset = max_scroll;
        self.auto_scroll = true;
    }

    /// Scroll to top
    pub fn scroll_to_top(&mut self) {
        self.scroll_offset = 0;
        self.auto_scroll = false;
    }

    /// Page down
    pub fn page_down(&mut self) {
        self.scroll_by(self.visible_lines.max(1) as i32);
    }

    /// Page up
    pub fn page_up(&mut self) {
        self.scroll_by(-(self.visible_lines.max(1) as i32));
    }

    /// Move selection to next button (down)
    pub fn select_next_button(&mut self) {
        self.selected_button = (self.selected_button + 1) % 4;
    }

    /// Move selection to previous button (up)
    pub fn select_prev_button(&mut self) {
        if self.selected_button == 0 {
            self.selected_button = 3;
        } else {
            self.selected_button -= 1;
        }
    }

    /// Get the currently selected button as a hit result
    pub fn get_selected_button(&self) -> TailViewHit {
        match self.selected_button {
            0 => TailViewHit::BackButton,
            1 => TailViewHit::OpenTerminalButton,
            2 => TailViewHit::RerunButton,
            3 => TailViewHit::KillButton,
            _ => TailViewHit::BackButton,
        }
    }

    /// Hit test to find what was clicked
    pub fn hit_test(&self, x: f32, y: f32) -> TailViewHit {
        // Check back button
        if let Some(rect) = &self.back_button_rect {
            if x >= rect.x && x <= rect.x + rect.width && y >= rect.y && y <= rect.y + rect.height {
                return TailViewHit::BackButton;
            }
        }

        // Check terminal button
        if let Some(rect) = &self.terminal_button_rect {
            if x >= rect.x && x <= rect.x + rect.width && y >= rect.y && y <= rect.y + rect.height {
                return TailViewHit::OpenTerminalButton;
            }
        }

        // Check rerun button
        if let Some(rect) = &self.rerun_button_rect {
            if x >= rect.x && x <= rect.x + rect.width && y >= rect.y && y <= rect.y + rect.height {
                return TailViewHit::RerunButton;
            }
        }

        // Check kill button
        if let Some(rect) = &self.kill_button_rect {
            if x >= rect.x && x <= rect.x + rect.width && y >= rect.y && y <= rect.y + rect.height {
                return TailViewHit::KillButton;
            }
        }

        // Check content area
        if let Some(bounds) = &self.bounds {
            if x >= bounds.x
                && x <= bounds.x + bounds.width
                && y >= bounds.y
                && y <= bounds.y + bounds.height
            {
                return TailViewHit::Content;
            }
        }

        TailViewHit::None
    }

    /// Render the tail view
    pub fn render(
        &mut self,
        renderer: &mut Renderer,
        rect: Rect,
        ctx: &LayoutContext,
    ) -> Result<(), windows::core::Error> {
        // Auto-refresh from file if enough time has passed
        self.maybe_refresh();

        let scale = ctx.scale_factor;
        self.cached_scale = scale;
        self.bounds = Some(rect);

        // Scale dimensions
        let border_radius = self.style.border_radius * scale;
        let padding = self.style.padding * scale;
        let font_size = self.style.font_size * scale;
        let line_spacing = self.style.line_spacing * scale;
        let button_width = self.button_style.compact_width * scale;
        let button_height = self.button_style.item_height * scale;
        let button_radius = self.button_style.item_corner_radius * scale;
        let button_padding = self.button_style.padding * scale;
        let icon_size = self.button_style.icon_size * scale;

        // Draw background
        let bounds = D2D_RECT_F {
            left: rect.x,
            top: rect.y,
            right: rect.x + rect.width,
            bottom: rect.y + rect.height,
        };

        renderer.fill_rounded_rect(
            bounds,
            border_radius,
            border_radius,
            self.style.background_color,
        )?;

        // === Button Bar (left side) ===
        let bar_x = rect.x + button_padding;
        let bar_y = rect.y + button_padding;

        // Selection border color (cyan accent like task panel)
        let selection_color = Color::from_hex("#03edf9").unwrap_or(Color::WHITE);
        let border_width = 2.0 * scale;

        // Create icon format
        let icon_format = renderer
            .create_text_format(&self.button_style.icon_font_family, icon_size, false, false)
            .ok();

        // Back button
        let back_rect = Rect::new(bar_x, bar_y, button_width, button_height);
        self.back_button_rect = Some(back_rect);

        let back_bounds = D2D_RECT_F {
            left: back_rect.x,
            top: back_rect.y,
            right: back_rect.x + back_rect.width,
            bottom: back_rect.y + back_rect.height,
        };

        renderer.fill_rounded_rect(
            back_bounds,
            button_radius,
            button_radius,
            self.button_style.item_background_color,
        )?;

        // Draw selection border if this button is selected
        if self.selected_button == 0 {
            renderer.draw_rounded_rect(
                back_bounds,
                button_radius,
                button_radius,
                selection_color,
                border_width,
            )?;
        }

        // Draw back icon (nerd font left arrow)
        if let Some(ref fmt) = icon_format {
            let icon = "󰁍"; // nf-md-arrow_left
            renderer.draw_text_centered(icon, fmt, back_bounds, self.button_style.icon_color)?;
        }

        // Terminal button (below back)
        let term_rect = Rect::new(
            bar_x,
            bar_y + button_height + self.button_style.item_spacing * scale,
            button_width,
            button_height,
        );
        self.terminal_button_rect = Some(term_rect);

        let term_bounds = D2D_RECT_F {
            left: term_rect.x,
            top: term_rect.y,
            right: term_rect.x + term_rect.width,
            bottom: term_rect.y + term_rect.height,
        };

        renderer.fill_rounded_rect(
            term_bounds,
            button_radius,
            button_radius,
            self.button_style.item_background_color,
        )?;

        // Draw selection border if this button is selected
        if self.selected_button == 1 {
            renderer.draw_rounded_rect(
                term_bounds,
                button_radius,
                button_radius,
                selection_color,
                border_width,
            )?;
        }

        // Draw terminal icon (using > character as fallback)
        if let Some(ref fmt) = icon_format {
            let icon = ">"; // Simple terminal prompt character
            renderer.draw_text_centered(icon, fmt, term_bounds, self.button_style.icon_color)?;
        }

        // Rerun button (below terminal)
        let rerun_rect = Rect::new(
            bar_x,
            bar_y + (button_height + self.button_style.item_spacing * scale) * 2.0,
            button_width,
            button_height,
        );
        self.rerun_button_rect = Some(rerun_rect);

        let rerun_bounds = D2D_RECT_F {
            left: rerun_rect.x,
            top: rerun_rect.y,
            right: rerun_rect.x + rerun_rect.width,
            bottom: rerun_rect.y + rerun_rect.height,
        };

        renderer.fill_rounded_rect(
            rerun_bounds,
            button_radius,
            button_radius,
            self.button_style.item_background_color,
        )?;

        // Draw selection border if this button is selected
        if self.selected_button == 2 {
            renderer.draw_rounded_rect(
                rerun_bounds,
                button_radius,
                button_radius,
                selection_color,
                border_width,
            )?;
        }

        // Draw rerun icon (nf-md-refresh)
        if let Some(ref fmt) = icon_format {
            let icon = "󰑓"; // nf-md-refresh
            renderer.draw_text_centered(icon, fmt, rerun_bounds, self.button_style.icon_color)?;
        }

        // Kill button (below rerun)
        let kill_rect = Rect::new(
            bar_x,
            bar_y + (button_height + self.button_style.item_spacing * scale) * 3.0,
            button_width,
            button_height,
        );
        self.kill_button_rect = Some(kill_rect);

        let kill_bounds = D2D_RECT_F {
            left: kill_rect.x,
            top: kill_rect.y,
            right: kill_rect.x + kill_rect.width,
            bottom: kill_rect.y + kill_rect.height,
        };

        // Use red-ish background for kill button to indicate danger
        let kill_bg_color = Color::from_hex("#f97e7230").unwrap_or(self.button_style.item_background_color);
        renderer.fill_rounded_rect(
            kill_bounds,
            button_radius,
            button_radius,
            kill_bg_color,
        )?;

        // Draw selection border if this button is selected
        if self.selected_button == 3 {
            renderer.draw_rounded_rect(
                kill_bounds,
                button_radius,
                button_radius,
                selection_color,
                border_width,
            )?;
        }

        // Draw kill icon (X or stop symbol)
        if let Some(ref fmt) = icon_format {
            let icon = "󰅖"; // nf-md-close
            let kill_icon_color = Color::from_hex("#f97e72").unwrap_or(self.button_style.icon_color);
            renderer.draw_text_centered(icon, fmt, kill_bounds, kill_icon_color)?;
        }

        // === Content Area ===
        let content_x = rect.x + button_width + button_padding * 2.0 + padding;
        let content_y = rect.y + padding;
        let content_width = rect.width - button_width - button_padding * 2.0 - padding * 2.0 - 12.0 * scale; // Reserve space for scrollbar
        let content_height = rect.height - padding * 2.0;

        // Create text format WITHOUT word wrapping (terminal-style, clip long lines)
        let text_format = match renderer.create_text_format(
            &self.style.font_family,
            font_size,
            false,
            false,
        ) {
            Ok(f) => f,
            Err(_) => return Ok(()),
        };

        // Calculate line height
        let line_height = font_size + line_spacing;

        // Calculate visible lines
        self.visible_lines = ((content_height / line_height) as usize).max(1);

        // Calculate scroll range
        let max_scroll = self.lines.len().saturating_sub(self.visible_lines);
        if self.scroll_offset > max_scroll {
            self.scroll_offset = max_scroll;
        }

        // Render visible lines
        let start_line = self.scroll_offset;
        let end_line = (start_line + self.visible_lines + 1).min(self.lines.len());

        for (i, line_idx) in (start_line..end_line).enumerate() {
            let y = content_y + (i as f32) * line_height;

            if y + line_height > rect.y + rect.height - padding {
                break;
            }

            let line = &self.lines[line_idx];
            let text_rect = D2D_RECT_F {
                left: content_x,
                top: y,
                right: content_x + content_width,
                bottom: y + line_height,
            };

            renderer.draw_text(line, &text_format, text_rect, self.style.text_color)?;
        }

        // Draw scrollbar if needed
        if self.lines.len() > self.visible_lines {
            let scrollbar_width = 6.0 * scale;
            let scrollbar_x = rect.x + rect.width - padding - scrollbar_width;
            let scrollbar_y = content_y;
            let scrollbar_height = content_height;

            // Track
            let track_rect = D2D_RECT_F {
                left: scrollbar_x,
                top: scrollbar_y,
                right: scrollbar_x + scrollbar_width,
                bottom: scrollbar_y + scrollbar_height,
            };
            let track_color = Color::from_f32(1.0, 1.0, 1.0, 0.1);
            renderer.fill_rounded_rect(
                track_rect,
                scrollbar_width / 2.0,
                scrollbar_width / 2.0,
                track_color,
            )?;

            // Thumb
            let thumb_ratio = self.visible_lines as f32 / self.lines.len() as f32;
            let thumb_height = (scrollbar_height * thumb_ratio).max(20.0 * scale);
            let scroll_ratio = if max_scroll > 0 {
                self.scroll_offset as f32 / max_scroll as f32
            } else {
                0.0
            };
            let thumb_y = scrollbar_y + scroll_ratio * (scrollbar_height - thumb_height);

            let thumb_rect = D2D_RECT_F {
                left: scrollbar_x,
                top: thumb_y,
                right: scrollbar_x + scrollbar_width,
                bottom: thumb_y + thumb_height,
            };
            let thumb_color = Color::from_f32(1.0, 1.0, 1.0, 0.3);
            renderer.fill_rounded_rect(
                thumb_rect,
                scrollbar_width / 2.0,
                scrollbar_width / 2.0,
                thumb_color,
            )?;
        }

        Ok(())
    }
}

impl Default for TailView {
    fn default() -> Self {
        Self::new()
    }
}
