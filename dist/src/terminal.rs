//! Terminal module - Wrapper around alacritty_terminal for terminal state management
//!
//! Provides terminal emulation state (grid, cursor, scrollback) using alacritty_terminal,
//! with an adapter for our theming system.

use std::sync::Arc;

use alacritty_terminal::event::{Event, EventListener, WindowSize};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::term::cell::Cell;
use alacritty_terminal::term::{Config, Term, TermMode};
use alacritty_terminal::vte::ansi::{Color as AnsiColor, CursorShape, NamedColor, Rgb};
use parking_lot::Mutex;

use crate::pty::Pty;
use crate::theme::tree::ThemeTree;
use crate::theme::types::Color;

use crate::log;

/// Default scrollback lines
const DEFAULT_SCROLLBACK: usize = 1000;

/// Terminal color palette loaded from theme
#[derive(Clone, Debug)]
pub struct TerminalColors {
    pub foreground: Color,
    pub background: Color,
    pub cursor: Color,
    pub black: Color,
    pub red: Color,
    pub green: Color,
    pub yellow: Color,
    pub blue: Color,
    pub magenta: Color,
    pub cyan: Color,
    pub white: Color,
    pub bright_black: Color,
    pub bright_red: Color,
    pub bright_green: Color,
    pub bright_yellow: Color,
    pub bright_blue: Color,
    pub bright_magenta: Color,
    pub bright_cyan: Color,
    pub bright_white: Color,
}

impl Default for TerminalColors {
    fn default() -> Self {
        // Default to a dark theme (Tokyo Night-ish)
        Self {
            foreground: Color::from_hex("#c0caf5").unwrap_or(Color::WHITE),
            background: Color::from_hex("#1a1b26").unwrap_or(Color::BLACK),
            cursor: Color::from_hex("#c0caf5").unwrap_or(Color::WHITE),
            black: Color::from_hex("#414868").unwrap_or(Color::BLACK),
            red: Color::from_hex("#f7768e").unwrap_or(Color::from_f32(1.0, 0.0, 0.0, 1.0)),
            green: Color::from_hex("#9ece6a").unwrap_or(Color::from_f32(0.0, 1.0, 0.0, 1.0)),
            yellow: Color::from_hex("#e0af68").unwrap_or(Color::from_f32(1.0, 1.0, 0.0, 1.0)),
            blue: Color::from_hex("#7aa2f7").unwrap_or(Color::from_f32(0.0, 0.0, 1.0, 1.0)),
            magenta: Color::from_hex("#bb9af7").unwrap_or(Color::from_f32(1.0, 0.0, 1.0, 1.0)),
            cyan: Color::from_hex("#7dcfff").unwrap_or(Color::from_f32(0.0, 1.0, 1.0, 1.0)),
            white: Color::from_hex("#c0caf5").unwrap_or(Color::WHITE),
            bright_black: Color::from_hex("#565f89").unwrap_or(Color::BLACK),
            bright_red: Color::from_hex("#f7768e").unwrap_or(Color::from_f32(1.0, 0.0, 0.0, 1.0)),
            bright_green: Color::from_hex("#9ece6a").unwrap_or(Color::from_f32(0.0, 1.0, 0.0, 1.0)),
            bright_yellow: Color::from_hex("#e0af68")
                .unwrap_or(Color::from_f32(1.0, 1.0, 0.0, 1.0)),
            bright_blue: Color::from_hex("#7aa2f7").unwrap_or(Color::from_f32(0.0, 0.0, 1.0, 1.0)),
            bright_magenta: Color::from_hex("#bb9af7")
                .unwrap_or(Color::from_f32(1.0, 0.0, 1.0, 1.0)),
            bright_cyan: Color::from_hex("#7dcfff").unwrap_or(Color::from_f32(0.0, 1.0, 1.0, 1.0)),
            bright_white: Color::from_hex("#c0caf5").unwrap_or(Color::WHITE),
        }
    }
}

impl TerminalColors {
    /// Load terminal colors from theme
    pub fn from_theme(theme: &ThemeTree) -> Self {
        let default = Self::default();

        Self {
            foreground: theme.get_color("*", None, "term-foreground", default.foreground),
            background: theme.get_color("*", None, "term-background", default.background),
            cursor: theme.get_color("*", None, "term-cursor", default.cursor),
            black: theme.get_color("*", None, "term-black", default.black),
            red: theme.get_color("*", None, "term-red", default.red),
            green: theme.get_color("*", None, "term-green", default.green),
            yellow: theme.get_color("*", None, "term-yellow", default.yellow),
            blue: theme.get_color("*", None, "term-blue", default.blue),
            magenta: theme.get_color("*", None, "term-magenta", default.magenta),
            cyan: theme.get_color("*", None, "term-cyan", default.cyan),
            white: theme.get_color("*", None, "term-white", default.white),
            bright_black: theme.get_color("*", None, "term-bright-black", default.bright_black),
            bright_red: theme.get_color("*", None, "term-bright-red", default.bright_red),
            bright_green: theme.get_color("*", None, "term-bright-green", default.bright_green),
            bright_yellow: theme.get_color("*", None, "term-bright-yellow", default.bright_yellow),
            bright_blue: theme.get_color("*", None, "term-bright-blue", default.bright_blue),
            bright_magenta: theme.get_color(
                "*",
                None,
                "term-bright-magenta",
                default.bright_magenta,
            ),
            bright_cyan: theme.get_color("*", None, "term-bright-cyan", default.bright_cyan),
            bright_white: theme.get_color("*", None, "term-bright-white", default.bright_white),
        }
    }

    /// Convert ANSI color to our Color type
    pub fn resolve_color(&self, ansi_color: AnsiColor) -> Color {
        match ansi_color {
            AnsiColor::Named(named) => self.resolve_named_color(named),
            AnsiColor::Spec(Rgb { r, g, b }) => {
                Color::from_f32(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0)
            }
            AnsiColor::Indexed(idx) => self.resolve_indexed_color(idx),
        }
    }

    fn resolve_named_color(&self, named: NamedColor) -> Color {
        match named {
            NamedColor::Black => self.black,
            NamedColor::Red => self.red,
            NamedColor::Green => self.green,
            NamedColor::Yellow => self.yellow,
            NamedColor::Blue => self.blue,
            NamedColor::Magenta => self.magenta,
            NamedColor::Cyan => self.cyan,
            NamedColor::White => self.white,
            NamedColor::BrightBlack => self.bright_black,
            NamedColor::BrightRed => self.bright_red,
            NamedColor::BrightGreen => self.bright_green,
            NamedColor::BrightYellow => self.bright_yellow,
            NamedColor::BrightBlue => self.bright_blue,
            NamedColor::BrightMagenta => self.bright_magenta,
            NamedColor::BrightCyan => self.bright_cyan,
            NamedColor::BrightWhite => self.bright_white,
            NamedColor::Foreground => self.foreground,
            NamedColor::Background => self.background,
            NamedColor::Cursor => self.cursor,
            _ => self.foreground, // Default for other special colors
        }
    }

    fn resolve_indexed_color(&self, idx: u8) -> Color {
        match idx {
            0 => self.black,
            1 => self.red,
            2 => self.green,
            3 => self.yellow,
            4 => self.blue,
            5 => self.magenta,
            6 => self.cyan,
            7 => self.white,
            8 => self.bright_black,
            9 => self.bright_red,
            10 => self.bright_green,
            11 => self.bright_yellow,
            12 => self.bright_blue,
            13 => self.bright_magenta,
            14 => self.bright_cyan,
            15 => self.bright_white,
            // 16-231: 6x6x6 color cube
            16..=231 => {
                let idx = idx - 16;
                let r = (idx / 36) % 6;
                let g = (idx / 6) % 6;
                let b = idx % 6;
                let r = if r == 0 { 0 } else { r * 40 + 55 };
                let g = if g == 0 { 0 } else { g * 40 + 55 };
                let b = if b == 0 { 0 } else { b * 40 + 55 };
                Color::from_f32(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0)
            }
            // 232-255: grayscale ramp
            232..=255 => {
                let gray = (idx - 232) * 10 + 8;
                Color::from_f32(
                    gray as f32 / 255.0,
                    gray as f32 / 255.0,
                    gray as f32 / 255.0,
                    1.0,
                )
            }
        }
    }
}

/// Event listener that captures terminal events
#[derive(Clone)]
pub struct TerminalEventListener {
    /// Terminal title (set by escape sequences)
    pub title: Arc<Mutex<String>>,
    /// Bell triggered
    pub bell: Arc<Mutex<bool>>,
}

impl TerminalEventListener {
    pub fn new() -> Self {
        Self {
            title: Arc::new(Mutex::new(String::new())),
            bell: Arc::new(Mutex::new(false)),
        }
    }
}

impl EventListener for TerminalEventListener {
    fn send_event(&self, event: Event) {
        match event {
            Event::Title(title) => {
                *self.title.lock() = title;
            }
            Event::Bell => {
                *self.bell.lock() = true;
            }
            _ => {}
        }
    }
}

/// Terminal size that implements Dimensions
#[derive(Clone, Copy, Debug)]
pub struct TermSize {
    pub cols: usize,
    pub rows: usize,
}

impl Dimensions for TermSize {
    fn total_lines(&self) -> usize {
        self.rows
    }

    fn screen_lines(&self) -> usize {
        self.rows
    }

    fn columns(&self) -> usize {
        self.cols
    }

    fn last_column(&self) -> alacritty_terminal::index::Column {
        alacritty_terminal::index::Column(self.cols.saturating_sub(1))
    }

    fn bottommost_line(&self) -> alacritty_terminal::index::Line {
        alacritty_terminal::index::Line((self.rows as i32) - 1)
    }

    fn topmost_line(&self) -> alacritty_terminal::index::Line {
        alacritty_terminal::index::Line(0)
    }
}

/// Terminal configuration
#[derive(Clone, Debug)]
pub struct TerminalConfig {
    pub cols: u16,
    pub rows: u16,
    pub scrollback_lines: usize,
    pub font_family: String,
    pub font_size: f32,
}

impl Default for TerminalConfig {
    fn default() -> Self {
        Self {
            cols: 120,
            rows: 30,
            scrollback_lines: DEFAULT_SCROLLBACK,
            font_family: "Cascadia Code".to_string(),
            font_size: 13.0,
        }
    }
}

impl TerminalConfig {
    /// Load from theme
    pub fn from_theme(theme: &ThemeTree) -> Self {
        let default = Self::default();

        Self {
            cols: default.cols,
            rows: default.rows,
            scrollback_lines: theme
                .get_number("terminal", None, "scrollback-lines", default.scrollback_lines as f64)
                as usize,
            font_family: theme.get_string("terminal", None, "font-family", &default.font_family),
            font_size: theme.get_number("terminal", None, "font-size", default.font_size as f64)
                as f32,
        }
    }
}

/// Terminal emulator wrapping alacritty_terminal
pub struct Terminal {
    /// The terminal state
    term: Term<TerminalEventListener>,
    /// Event listener
    listener: TerminalEventListener,
    /// Color palette
    pub colors: TerminalColors,
    /// Configuration
    pub config: TerminalConfig,
    /// Connected PTY
    pty: Option<Pty>,
}

impl Terminal {
    /// Create a new terminal with the given configuration
    pub fn new(config: TerminalConfig, colors: TerminalColors) -> Self {
        log!(
            "Terminal: Creating with size {}x{}, scrollback {}",
            config.cols,
            config.rows,
            config.scrollback_lines
        );

        let listener = TerminalEventListener::new();

        // Create alacritty terminal config
        let term_config = Config::default();

        // Create the terminal size
        let size = TermSize {
            cols: config.cols as usize,
            rows: config.rows as usize,
        };

        let term = Term::new(term_config, &size, listener.clone());

        Self {
            term,
            listener,
            colors,
            config,
            pty: None,
        }
    }

    /// Create a new terminal from theme settings
    pub fn from_theme(theme: &ThemeTree) -> Self {
        let config = TerminalConfig::from_theme(theme);
        let colors = TerminalColors::from_theme(theme);
        Self::new(config, colors)
    }

    /// Attach a PTY to this terminal
    pub fn attach_pty(&mut self, pty: Pty) {
        self.pty = Some(pty);
    }

    /// Get the attached PTY
    pub fn pty(&self) -> Option<&Pty> {
        self.pty.as_ref()
    }

    /// Get mutable access to the attached PTY
    pub fn pty_mut(&mut self) -> Option<&mut Pty> {
        self.pty.as_mut()
    }

    /// Process bytes from PTY output (feed to terminal)
    pub fn process_bytes(&mut self, bytes: &[u8]) {
        use alacritty_terminal::vte::ansi::{Processor, StdSyncHandler};

        let mut processor: Processor<StdSyncHandler> = Processor::new();
        processor.advance(&mut self.term, bytes);
    }

    /// Poll and process any available PTY output
    pub fn poll_pty(&mut self) -> bool {
        if let Some(ref pty) = self.pty {
            let data = pty.read_all();
            if !data.is_empty() {
                self.process_bytes(&data);
                return true;
            }
        }
        false
    }

    /// Write input to the PTY
    pub fn write_to_pty(&self, data: &[u8]) -> Result<(), String> {
        if let Some(ref pty) = self.pty {
            pty.write(data)
        } else {
            Err("No PTY attached".to_string())
        }
    }

    /// Write a string to the PTY
    pub fn write_str_to_pty(&self, s: &str) -> Result<(), String> {
        self.write_to_pty(s.as_bytes())
    }

    /// Resize the terminal
    pub fn resize(&mut self, cols: u16, rows: u16) {
        if cols == self.config.cols && rows == self.config.rows {
            return;
        }

        log!("Terminal: Resizing to {}x{}", cols, rows);

        self.config.cols = cols;
        self.config.rows = rows;

        let size = TermSize {
            cols: cols as usize,
            rows: rows as usize,
        };

        self.term.resize(size);

        // Also resize PTY
        if let Some(ref mut pty) = self.pty {
            let _ = pty.resize(cols, rows);
        }
    }

    /// Get terminal dimensions
    pub fn size(&self) -> (u16, u16) {
        (self.config.cols, self.config.rows)
    }

    /// Get the number of columns
    pub fn cols(&self) -> usize {
        self.term.columns()
    }

    /// Get the number of visible rows
    pub fn rows(&self) -> usize {
        self.term.screen_lines()
    }

    /// Get a cell at the given position (0-indexed from top of visible screen)
    pub fn cell(&self, col: usize, row: usize) -> Option<&Cell> {
        use alacritty_terminal::index::{Column, Line};

        // Bounds check
        if col >= self.cols() || row >= self.rows() {
            return None;
        }

        let point = alacritty_terminal::index::Point::new(Line(row as i32), Column(col));
        Some(&self.term.grid()[point])
    }

    /// Get cursor position (col, row)
    pub fn cursor_position(&self) -> (usize, usize) {
        let cursor = self.term.grid().cursor.point;
        (cursor.column.0, cursor.line.0 as usize)
    }

    /// Get cursor shape
    pub fn cursor_shape(&self) -> CursorShape {
        self.term.cursor_style().shape
    }

    /// Check if cursor should be visible
    pub fn cursor_visible(&self) -> bool {
        // SHOW_CURSOR mode means cursor is visible (not hidden by escape sequence)
        self.term.mode().contains(TermMode::SHOW_CURSOR)
    }

    /// Get the terminal title (if set by escape sequence)
    pub fn title(&self) -> String {
        self.listener.title.lock().clone()
    }

    /// Check if bell was triggered (and clear flag)
    pub fn bell_triggered(&self) -> bool {
        let mut bell = self.listener.bell.lock();
        let triggered = *bell;
        *bell = false;
        triggered
    }

    /// Check if the terminal is in alternate screen mode
    pub fn is_alternate_screen(&self) -> bool {
        self.term.mode().contains(TermMode::ALT_SCREEN)
    }

    /// Get scrollback offset
    pub fn scroll_offset(&self) -> usize {
        self.term.grid().display_offset()
    }

    /// Scroll up by delta lines
    pub fn scroll_up(&mut self, delta: usize) {
        use alacritty_terminal::grid::Scroll;
        self.term.scroll_display(Scroll::Delta(delta as i32));
    }

    /// Scroll down by delta lines
    pub fn scroll_down(&mut self, delta: usize) {
        use alacritty_terminal::grid::Scroll;
        self.term.scroll_display(Scroll::Delta(-(delta as i32)));
    }

    /// Scroll to bottom
    pub fn scroll_to_bottom(&mut self) {
        use alacritty_terminal::grid::Scroll;
        self.term.scroll_display(Scroll::Bottom);
    }

    /// Check if PTY is still alive
    pub fn is_alive(&self) -> bool {
        self.pty.as_ref().map(|p| p.is_alive()).unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terminal_colors_default() {
        let colors = TerminalColors::default();
        assert!(colors.foreground.r > 0.0);
    }

    #[test]
    fn test_terminal_config_default() {
        let config = TerminalConfig::default();
        assert_eq!(config.cols, 120);
        assert_eq!(config.rows, 30);
        assert_eq!(config.scrollback_lines, 1000);
    }
}
