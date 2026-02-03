//! Tail View widget - Embedded terminal output viewer
//!
//! Displays task output in a themed terminal-like view with scrolling.
//! Supports two modes:
//! - Log mode: File-based output from non-interactive tasks
//! - Interactive mode: PTY-based terminal for interactive tasks (uses alacritty_terminal)
//!
//! Features a button bar on the left (compact icon style like task panel)
//! with Back and Open Terminal buttons.

use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::Instant;

use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;

use crate::platform::win32::event::KeyCode;
use crate::platform::win32::Renderer;
use crate::platform::Event;
use crate::terminal::{Terminal, TerminalColors, TerminalConfig};
use crate::theme::tree::ThemeTree;
use crate::theme::types::{Color, LayoutContext, Rect};

use super::base::CornerRadii;
use super::taskpanel::TaskPanelStyle;

/// Maximum number of lines to keep in buffer
const MAX_LINES: usize = 10000;

/// Mode of the tail view
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TailViewMode {
    /// File-based log tailing (default for non-interactive tasks)
    #[default]
    LogFile,
    /// Interactive PTY terminal (for interactive tasks)
    Interactive,
    /// Directory picker before launching interactive terminal
    DirectoryPicker,
}

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
    /// Current mode (log file or interactive)
    mode: TailViewMode,
    /// Lines of output (used in LogFile mode)
    lines: Vec<String>,
    /// Scroll offset (line index at top of view)
    scroll_offset: usize,
    /// Auto-scroll to bottom on new content
    auto_scroll: bool,
    /// Task being tailed (group:name)
    task_key: Option<String>,
    /// Path to the log file (used in LogFile mode)
    log_path: Option<PathBuf>,
    /// Interactive terminal (used in Interactive mode)
    terminal: Option<Terminal>,
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

    // Directory picker state
    /// List of directories for the picker
    picker_dirs: Vec<String>,
    /// Currently selected directory index in picker
    picker_selected: usize,
    /// Text input for new directory path
    picker_input: String,
    /// Whether the text input is focused (vs the list)
    picker_input_focused: bool,
    /// Cursor position in the text input
    picker_cursor: usize,
    /// Autocomplete matches for cycling
    autocomplete_matches: Vec<String>,
    /// Current index in autocomplete matches
    autocomplete_index: usize,
    /// The original input before autocomplete started
    autocomplete_original: String,
}

impl TailView {
    /// Create a new tail view
    pub fn new() -> Self {
        Self {
            mode: TailViewMode::LogFile,
            lines: Vec::new(),
            scroll_offset: 0,
            auto_scroll: true,
            task_key: None,
            log_path: None,
            terminal: None,
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
            picker_dirs: Vec::new(),
            picker_selected: 0,
            picker_input: String::new(),
            picker_input_focused: true,
            picker_cursor: 0,
            autocomplete_matches: Vec::new(),
            autocomplete_index: 0,
            autocomplete_original: String::new(),
        }
    }

    /// Get current mode
    pub fn mode(&self) -> TailViewMode {
        self.mode
    }

    /// Check if in interactive mode
    pub fn is_interactive(&self) -> bool {
        self.mode == TailViewMode::Interactive
    }

    /// Set the style from theme
    pub fn set_style(&mut self, style: TailViewStyle) {
        self.style = style;
    }

    /// Set the button bar style (from task panel)
    pub fn set_button_style(&mut self, style: TaskPanelStyle) {
        self.button_style = style;
    }

    /// Start tailing a task (log file mode)
    pub fn start_tail(&mut self, task_key: String, log_path: PathBuf) {
        self.mode = TailViewMode::LogFile;
        self.task_key = Some(task_key);
        self.log_path = Some(log_path);
        self.terminal = None;
        self.lines.clear();
        self.scroll_offset = 0;
        self.auto_scroll = true;
        self.refresh();
    }

    /// Start an interactive terminal session
    pub fn start_interactive(&mut self, task_key: String, terminal: Terminal) {
        crate::log!("TailView: Starting interactive mode for {}", task_key);
        self.mode = TailViewMode::Interactive;
        self.task_key = Some(task_key);
        self.log_path = None;
        self.lines.clear();
        self.terminal = Some(terminal);
        self.scroll_offset = 0;
        self.auto_scroll = true;
    }

    /// Start directory picker mode
    pub fn start_directory_picker(&mut self, task_key: String, directories: Vec<String>) {
        crate::log!("TailView: Starting directory picker for {}", task_key);
        self.mode = TailViewMode::DirectoryPicker;
        self.task_key = Some(task_key);
        self.log_path = None;
        self.terminal = None;
        self.lines.clear();
        self.picker_dirs = directories;
        self.picker_selected = 0;
        // Pre-populate input with the first (most recent) directory
        self.picker_input = self.picker_dirs.first().cloned().unwrap_or_default();
        self.picker_cursor = self.picker_input.chars().count();
        self.picker_input_focused = true;
    }

    /// Get the selected directory from the picker
    pub fn get_selected_directory(&self) -> String {
        if !self.picker_input.is_empty() {
            self.picker_input.clone()
        } else if let Some(dir) = self.picker_dirs.get(self.picker_selected) {
            dir.clone()
        } else {
            dirs::home_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| "C:\\".to_string())
        }
    }

    /// Check if in directory picker mode
    pub fn is_directory_picker(&self) -> bool {
        self.mode == TailViewMode::DirectoryPicker
    }

    /// Get mutable access to the terminal (for input handling)
    pub fn terminal_mut(&mut self) -> Option<&mut Terminal> {
        self.terminal.as_mut()
    }

    /// Take the terminal out of the TailView without destroying it
    /// Returns the terminal if we're in interactive mode, None otherwise
    pub fn take_terminal(&mut self) -> Option<Terminal> {
        self.terminal.take()
    }

    /// Stop tailing and clear buffer
    pub fn stop_tail(&mut self) {
        self.mode = TailViewMode::LogFile;
        self.task_key = None;
        self.log_path = None;
        self.terminal = None;
        self.lines.clear();
        self.scroll_offset = 0;
        self.selected_button = 0;
        self.last_refresh = Instant::now();
    }

    /// Get the task key being tailed
    pub fn task_key(&self) -> Option<&str> {
        self.task_key.as_deref()
    }

    /// Refresh content (log file or terminal)
    pub fn refresh(&mut self) {
        self.last_refresh = Instant::now();

        // In interactive mode, poll the PTY instead
        if self.mode == TailViewMode::Interactive {
            if let Some(ref mut terminal) = self.terminal {
                terminal.poll_pty();
            }
            return;
        }

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

    /// Check if we should refresh based on time elapsed
    pub fn maybe_refresh(&mut self) {
        let interval = if self.mode == TailViewMode::Interactive {
            50 // 50ms for interactive (faster updates)
        } else {
            200 // 200ms for log file
        };

        if self.last_refresh.elapsed().as_millis() >= interval as u128 {
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

    // ========================================================================
    // Directory Picker Methods
    // ========================================================================

    /// Handle key input for directory picker mode
    /// Returns true if the key was handled
    pub fn picker_handle_key(&mut self, key: KeyCode) -> bool {
        if self.mode != TailViewMode::DirectoryPicker {
            return false;
        }

        match key {
            KeyCode::Up => {
                if self.picker_input_focused {
                    // Switch focus to list if there are items
                    if !self.picker_dirs.is_empty() {
                        self.picker_input_focused = false;
                        self.picker_selected = 0;
                    }
                } else if self.picker_selected > 0 {
                    self.picker_selected -= 1;
                } else {
                    // Wrap to input
                    self.picker_input_focused = true;
                }
                true
            }
            KeyCode::Down => {
                if self.picker_input_focused {
                    // Switch focus to list if there are items
                    if !self.picker_dirs.is_empty() {
                        self.picker_input_focused = false;
                        self.picker_selected = 0;
                    }
                } else if self.picker_selected + 1 < self.picker_dirs.len() {
                    self.picker_selected += 1;
                } else {
                    // Wrap to input
                    self.picker_input_focused = true;
                }
                true
            }
            KeyCode::Left => {
                if self.picker_input_focused && self.picker_cursor > 0 {
                    // Move cursor left by one character
                    let chars: Vec<char> = self.picker_input.chars().collect();
                    if self.picker_cursor > 0 {
                        self.picker_cursor = self.picker_cursor.saturating_sub(1).min(chars.len());
                    }
                }
                true
            }
            KeyCode::Right => {
                if self.picker_input_focused {
                    // Move cursor right by one character
                    let char_count = self.picker_input.chars().count();
                    if self.picker_cursor < char_count {
                        self.picker_cursor += 1;
                    }
                }
                true
            }
            KeyCode::Home => {
                if self.picker_input_focused {
                    self.picker_cursor = 0;
                }
                true
            }
            KeyCode::End => {
                if self.picker_input_focused {
                    self.picker_cursor = self.picker_input.chars().count();
                }
                true
            }
            KeyCode::Backspace => {
                if self.picker_input_focused && self.picker_cursor > 0 {
                    // Clear autocomplete state
                    self.autocomplete_matches.clear();
                    self.autocomplete_index = 0;

                    // Remove character before cursor
                    let mut chars: Vec<char> = self.picker_input.chars().collect();
                    if self.picker_cursor > 0 && self.picker_cursor <= chars.len() {
                        chars.remove(self.picker_cursor - 1);
                        self.picker_input = chars.into_iter().collect();
                        self.picker_cursor -= 1;
                    }
                }
                true
            }
            KeyCode::Delete => {
                if self.picker_input_focused {
                    // Clear autocomplete state
                    self.autocomplete_matches.clear();
                    self.autocomplete_index = 0;

                    // Remove character at cursor
                    let mut chars: Vec<char> = self.picker_input.chars().collect();
                    if self.picker_cursor < chars.len() {
                        chars.remove(self.picker_cursor);
                        self.picker_input = chars.into_iter().collect();
                    }
                }
                true
            }
            KeyCode::Tab => {
                if self.picker_input_focused && !self.picker_input.is_empty() {
                    // If we already have matches from a previous Tab, cycle through them
                    if !self.autocomplete_matches.is_empty() {
                        self.autocomplete_index = (self.autocomplete_index + 1) % self.autocomplete_matches.len();
                        self.picker_input = self.autocomplete_matches[self.autocomplete_index].clone();
                        self.picker_cursor = self.picker_input.chars().count();
                    } else {
                        // First Tab press - get matches
                        self.get_autocomplete_matches();
                        if !self.autocomplete_matches.is_empty() {
                            self.autocomplete_index = 0;
                            self.picker_input = self.autocomplete_matches[0].clone();
                            self.picker_cursor = self.picker_input.chars().count();
                        }
                    }
                }
                true
            }
            _ => false,
        }
    }

    /// Handle character input for directory picker
    pub fn picker_handle_char(&mut self, c: char) {
        if self.mode != TailViewMode::DirectoryPicker || !self.picker_input_focused {
            return;
        }

        // Clear autocomplete state when user types
        self.autocomplete_matches.clear();
        self.autocomplete_index = 0;

        // Insert character at cursor position (character-based)
        let mut chars: Vec<char> = self.picker_input.chars().collect();
        let insert_pos = self.picker_cursor.min(chars.len());
        chars.insert(insert_pos, c);
        self.picker_input = chars.into_iter().collect();
        self.picker_cursor = insert_pos + 1;
    }

    /// Select a directory from the list (puts it in the input box)
    pub fn picker_select_from_list(&mut self) {
        if !self.picker_input_focused && self.picker_selected < self.picker_dirs.len() {
            self.picker_input = self.picker_dirs[self.picker_selected].clone();
            self.picker_cursor = self.picker_input.chars().count();
            self.picker_input_focused = true;
        }
    }

    /// Get autocomplete matches and populate the matches list for Tab cycling
    fn get_autocomplete_matches(&mut self) {
        self.autocomplete_matches.clear();
        self.autocomplete_original = self.picker_input.clone();

        let input = self.picker_input.trim(); // Trim any whitespace
        if input.is_empty() {
            return;
        }

        let path = Path::new(input);

        // Determine parent directory and prefix to match
        let (parent, prefix) = if input.ends_with('\\') || input.ends_with('/') {
            // Input ends with separator - list contents of that directory
            (path.to_path_buf(), String::new())
        } else if path.is_dir() {
            // Input is an existing directory without trailing slash - list its contents
            (path.to_path_buf(), String::new())
        } else {
            // Get parent directory and the partial name we're trying to complete
            let parent = path.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| PathBuf::from("."));
            let prefix = path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_lowercase();
            (parent, prefix)
        };

        // Read the parent directory
        let entries = match fs::read_dir(&parent) {
            Ok(entries) => entries,
            Err(_) => return,
        };

        // Collect matching directories
        let mut matches: Vec<String> = entries
            .filter_map(|e| e.ok())
            .filter(|e| {
                // Only directories
                e.file_type().map(|ft| ft.is_dir()).unwrap_or(false)
            })
            .filter(|e| {
                // Matches our prefix (case-insensitive)
                if prefix.is_empty() {
                    true
                } else {
                    e.file_name()
                        .to_str()
                        .map(|name| name.to_lowercase().starts_with(&prefix))
                        .unwrap_or(false)
                }
            })
            .map(|e| {
                let mut path_str = e.path().to_string_lossy().to_string();
                if !path_str.ends_with('\\') {
                    path_str.push('\\');
                }
                path_str
            })
            .collect();

        // Sort alphabetically
        matches.sort();

        self.autocomplete_matches = matches;
    }

    /// Send a character to the terminal (interactive mode only)
    pub fn send_char(&mut self, c: char) {
        if self.mode != TailViewMode::Interactive {
            return;
        }
        if let Some(ref mut terminal) = self.terminal {
            let mut buf = [0u8; 4];
            let s = c.encode_utf8(&mut buf);
            terminal.write_to_pty(s.as_bytes());
        }
    }

    /// Send a key to the terminal (interactive mode only)
    /// Converts KeyCode to the appropriate escape sequence for PTY input.
    /// Returns true if the key was consumed by the terminal.
    pub fn send_key(&mut self, key: KeyCode, ctrl: bool, shift: bool) -> bool {
        if self.mode != TailViewMode::Interactive {
            return false;
        }
        let Some(ref mut terminal) = self.terminal else {
            return false;
        };

        // Build escape sequence for the key
        let seq: Option<&[u8]> = match key {
            KeyCode::Enter => Some(b"\r"),
            KeyCode::Backspace => {
                if ctrl {
                    Some(b"\x17") // Ctrl+Backspace = delete word
                } else {
                    Some(b"\x7f") // ASCII DEL
                }
            }
            KeyCode::Tab => {
                if shift {
                    Some(b"\x1b[Z") // Shift+Tab = backtab
                } else {
                    Some(b"\t")
                }
            }
            KeyCode::Space => Some(b" "),
            KeyCode::Delete => Some(b"\x1b[3~"),
            KeyCode::Insert => Some(b"\x1b[2~"),
            KeyCode::Home => Some(b"\x1b[H"),
            KeyCode::End => Some(b"\x1b[F"),
            KeyCode::PageUp => Some(b"\x1b[5~"),
            KeyCode::PageDown => Some(b"\x1b[6~"),
            KeyCode::Up => Some(b"\x1b[A"),
            KeyCode::Down => Some(b"\x1b[B"),
            KeyCode::Right => Some(b"\x1b[C"),
            KeyCode::Left => Some(b"\x1b[D"),
            // Ctrl+letter combinations
            KeyCode::A if ctrl => Some(b"\x01"),
            KeyCode::B if ctrl => Some(b"\x02"),
            KeyCode::C if ctrl => Some(b"\x03"),
            KeyCode::D if ctrl => Some(b"\x04"),
            KeyCode::E if ctrl => Some(b"\x05"),
            KeyCode::F if ctrl => Some(b"\x06"),
            KeyCode::G if ctrl => Some(b"\x07"),
            KeyCode::H if ctrl => Some(b"\x08"),
            KeyCode::I if ctrl => Some(b"\x09"),
            KeyCode::J if ctrl => Some(b"\x0a"),
            KeyCode::K if ctrl => Some(b"\x0b"),
            KeyCode::L if ctrl => Some(b"\x0c"),
            KeyCode::M if ctrl => Some(b"\x0d"),
            KeyCode::N if ctrl => Some(b"\x0e"),
            KeyCode::O if ctrl => Some(b"\x0f"),
            KeyCode::P if ctrl => Some(b"\x10"),
            KeyCode::Q if ctrl => Some(b"\x11"),
            KeyCode::R if ctrl => Some(b"\x12"),
            KeyCode::S if ctrl => Some(b"\x13"),
            KeyCode::T if ctrl => Some(b"\x14"),
            KeyCode::U if ctrl => Some(b"\x15"),
            KeyCode::V if ctrl => Some(b"\x16"),
            KeyCode::W if ctrl => Some(b"\x17"),
            KeyCode::X if ctrl => Some(b"\x18"),
            KeyCode::Y if ctrl => Some(b"\x19"),
            KeyCode::Z if ctrl => Some(b"\x1a"),
            // Function keys
            KeyCode::F1 => Some(b"\x1bOP"),
            KeyCode::F2 => Some(b"\x1bOQ"),
            KeyCode::F3 => Some(b"\x1bOR"),
            KeyCode::F4 => Some(b"\x1bOS"),
            KeyCode::F5 => Some(b"\x1b[15~"),
            KeyCode::F6 => Some(b"\x1b[17~"),
            KeyCode::F7 => Some(b"\x1b[18~"),
            KeyCode::F8 => Some(b"\x1b[19~"),
            KeyCode::F9 => Some(b"\x1b[20~"),
            KeyCode::F10 => Some(b"\x1b[21~"),
            KeyCode::F11 => Some(b"\x1b[23~"),
            KeyCode::F12 => Some(b"\x1b[24~"),
            // Letter and number keys (non-ctrl) - handled via WM_CHAR
            KeyCode::A | KeyCode::B | KeyCode::C | KeyCode::D | KeyCode::E |
            KeyCode::F | KeyCode::G | KeyCode::H | KeyCode::I | KeyCode::J |
            KeyCode::K | KeyCode::L | KeyCode::M | KeyCode::N | KeyCode::O |
            KeyCode::P | KeyCode::Q | KeyCode::R | KeyCode::S | KeyCode::T |
            KeyCode::U | KeyCode::V | KeyCode::W | KeyCode::X | KeyCode::Y |
            KeyCode::Z => None,
            KeyCode::Num0 | KeyCode::Num1 | KeyCode::Num2 | KeyCode::Num3 |
            KeyCode::Num4 | KeyCode::Num5 | KeyCode::Num6 | KeyCode::Num7 |
            KeyCode::Num8 | KeyCode::Num9 => None,
            _ => None,
        };

        if let Some(data) = seq {
            terminal.write_to_pty(data);
            true
        } else {
            false
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

        // Branch rendering based on mode
        match self.mode {
            TailViewMode::Interactive => {
                self.render_terminal_content(
                    renderer,
                    content_x,
                    content_y,
                    content_width,
                    content_height,
                    scale,
                )?;
            }
            TailViewMode::LogFile => {
                self.render_log_content(
                    renderer,
                    content_x,
                    content_y,
                    content_width,
                    content_height,
                    font_size,
                    line_spacing,
                    padding,
                    rect,
                )?;
            }
            TailViewMode::DirectoryPicker => {
                self.render_directory_picker(
                    renderer,
                    content_x,
                    content_y,
                    content_width,
                    content_height,
                    font_size,
                    line_spacing,
                    scale,
                )?;
            }
        }

        Ok(())
    }

    /// Render log file content (text lines)
    fn render_log_content(
        &mut self,
        renderer: &mut Renderer,
        content_x: f32,
        content_y: f32,
        content_width: f32,
        content_height: f32,
        font_size: f32,
        line_spacing: f32,
        padding: f32,
        rect: Rect,
    ) -> Result<(), windows::core::Error> {
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
            let scrollbar_width = 6.0 * self.cached_scale;
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
            let max_scroll = self.lines.len().saturating_sub(self.visible_lines);
            let thumb_ratio = self.visible_lines as f32 / self.lines.len() as f32;
            let thumb_height = (scrollbar_height * thumb_ratio).max(20.0 * self.cached_scale);
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

    /// Render interactive terminal content (using alacritty_terminal grid)
    fn render_terminal_content(
        &mut self,
        renderer: &mut Renderer,
        content_x: f32,
        content_y: f32,
        content_width: f32,
        content_height: f32,
        scale: f32,
    ) -> Result<(), windows::core::Error> {
        let Some(ref mut terminal) = self.terminal else {
            // No terminal attached, show placeholder
            let text = "Connecting...";
            let font_size = self.style.font_size * scale;
            if let Ok(fmt) = renderer.create_text_format(&self.style.font_family, font_size, false, false) {
                let text_rect = D2D_RECT_F {
                    left: content_x,
                    top: content_y,
                    right: content_x + content_width,
                    bottom: content_y + content_height,
                };
                renderer.draw_text(text, &fmt, text_rect, self.style.text_color)?;
            }
            return Ok(());
        };

        // Calculate cell dimensions based on font
        let font_size = terminal.config.font_size * scale;
        let cell_width = font_size * 0.6; // Approximate monospace width
        let cell_height = font_size * 1.2; // Line height

        // Calculate how many rows/cols fit in visible area
        let visible_cols = ((content_width / cell_width) as u16).max(1);
        let visible_rows = ((content_height / cell_height) as u16).max(1);

        // Resize terminal/PTY if size changed
        let (current_cols, current_rows) = terminal.size();
        if current_cols != visible_cols || current_rows != visible_rows {
            terminal.resize(visible_cols, visible_rows);
        }

        // Get updated dimensions after potential resize
        let term_cols = terminal.cols();
        let term_rows = terminal.rows();
        let colors = &terminal.colors;

        // Create monospace text format
        let text_format = match renderer.create_text_format(
            &terminal.config.font_family,
            font_size,
            false,
            false,
        ) {
            Ok(f) => f,
            Err(_) => return Ok(()),
        };

        // Draw terminal background
        let term_bg_rect = D2D_RECT_F {
            left: content_x,
            top: content_y,
            right: content_x + content_width,
            bottom: content_y + content_height,
        };
        renderer.fill_rect(term_bg_rect, colors.background)?;

        // Get cursor position for highlighting
        let (cursor_col, cursor_row) = terminal.cursor_position();
        let cursor_visible = terminal.cursor_visible();

        // Render each cell in the grid (terminal is already sized to fit)
        for row in 0..term_rows {
            let y = content_y + (row as f32) * cell_height;

            for col in 0..term_cols {
                if let Some(cell) = terminal.cell(col, row) {
                    let x = content_x + (col as f32) * cell_width;

                    let cell_rect = D2D_RECT_F {
                        left: x,
                        top: y,
                        right: x + cell_width,
                        bottom: y + cell_height,
                    };

                    // Draw cell background if not default
                    let bg_color = colors.resolve_color(cell.bg);
                    if bg_color != colors.background {
                        renderer.fill_rect(cell_rect, bg_color)?;
                    }

                    // Draw cursor
                    if cursor_visible && col == cursor_col && row == cursor_row {
                        let cursor_color = colors.cursor;
                        renderer.fill_rect(cell_rect, cursor_color)?;
                    }

                    // Draw character
                    let c = cell.c;
                    if c != ' ' && c != '\0' {
                        let fg_color = if cursor_visible && col == cursor_col && row == cursor_row {
                            colors.background // Inverted for cursor
                        } else {
                            colors.resolve_color(cell.fg)
                        };

                        let char_str = c.to_string();
                        renderer.draw_text(&char_str, &text_format, cell_rect, fg_color)?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Render directory picker content
    fn render_directory_picker(
        &self,
        renderer: &mut Renderer,
        content_x: f32,
        content_y: f32,
        content_width: f32,
        _content_height: f32,
        font_size: f32,
        line_spacing: f32,
        scale: f32,
    ) -> Result<(), windows::core::Error> {
        let padding = 8.0 * scale;
        let line_height = font_size + line_spacing;
        let input_height = font_size + padding * 2.0;
        let corner_radius = 4.0 * scale;

        // Colors
        let input_bg = Color::from_hex("#2a273f").unwrap_or(Color::BLACK);
        let input_border = Color::from_hex("#03edf9").unwrap_or(Color::WHITE);
        let input_border_unfocused = Color::from_hex("#444444").unwrap_or(Color::from_f32(0.5, 0.5, 0.5, 1.0));
        let list_item_bg = Color::from_hex("#1e1b2e").unwrap_or(Color::BLACK);
        let list_item_selected_bg = Color::from_hex("#03edf930").unwrap_or(Color::from_f32(0.5, 0.5, 0.5, 0.3));
        let text_color = self.style.text_color;
        let placeholder_color = Color::from_f32(text_color.r, text_color.g, text_color.b, 0.5);

        // Create text format
        let text_format = renderer.create_text_format(&self.style.font_family, font_size, false, false)
            .map_err(|_| windows::core::Error::from_win32())?;

        // === Title ===
        let title = "Select Working Directory";
        let title_rect = D2D_RECT_F {
            left: content_x,
            top: content_y,
            right: content_x + content_width,
            bottom: content_y + line_height,
        };
        renderer.draw_text(title, &text_format, title_rect, text_color)?;

        // === Input Box ===
        let input_y = content_y + line_height + padding;
        let input_rect = D2D_RECT_F {
            left: content_x,
            top: input_y,
            right: content_x + content_width,
            bottom: input_y + input_height,
        };

        // Input background
        renderer.fill_rounded_rect(input_rect, corner_radius, corner_radius, input_bg)?;

        // Input border (highlighted if focused)
        let border_color = if self.picker_input_focused { input_border } else { input_border_unfocused };
        renderer.draw_rounded_rect(input_rect, corner_radius, corner_radius, border_color, 1.5 * scale)?;

        // Input text or placeholder
        let text_x = content_x + padding;
        let text_y = input_y + padding;
        let text_rect = D2D_RECT_F {
            left: text_x,
            top: text_y,
            right: content_x + content_width - padding,
            bottom: input_y + input_height - padding,
        };

        if self.picker_input.is_empty() {
            renderer.draw_text("Type path or select below...", &text_format, text_rect, placeholder_color)?;

            // Draw cursor at start if focused
            if self.picker_input_focused {
                let cursor_rect = D2D_RECT_F {
                    left: text_x,
                    top: text_y,
                    right: text_x + 2.0 * scale,
                    bottom: text_y + font_size,
                };
                renderer.fill_rect(cursor_rect, input_border)?;
            }
        } else {
            renderer.draw_text(&self.picker_input, &text_format, text_rect, text_color)?;

            // Draw cursor if focused
            if self.picker_input_focused {
                // Get the text before cursor (character-based)
                let text_before_cursor: String = self.picker_input.chars().take(self.picker_cursor).collect();
                // Use monospace character width approximation (0.6 of font size is common for monospace fonts)
                let char_width = font_size * 0.6;
                let cursor_offset = text_before_cursor.chars().count() as f32 * char_width;
                let cursor_x = text_x + cursor_offset;
                let cursor_rect = D2D_RECT_F {
                    left: cursor_x,
                    top: text_y,
                    right: cursor_x + 2.0 * scale,
                    bottom: text_y + font_size,
                };
                renderer.fill_rect(cursor_rect, input_border)?;
            }
        }

        // === Directory List ===
        let list_y = input_y + input_height + padding * 2.0;
        let item_height = line_height + padding;

        for (i, dir) in self.picker_dirs.iter().enumerate() {
            let item_y = list_y + (i as f32) * item_height;
            let item_rect = D2D_RECT_F {
                left: content_x,
                top: item_y,
                right: content_x + content_width,
                bottom: item_y + item_height,
            };

            // Background (selected highlight)
            let is_selected = !self.picker_input_focused && i == self.picker_selected;
            let bg_color = if is_selected { list_item_selected_bg } else { list_item_bg };
            renderer.fill_rounded_rect(item_rect, corner_radius, corner_radius, bg_color)?;

            // Selection border
            if is_selected {
                renderer.draw_rounded_rect(item_rect, corner_radius, corner_radius, input_border, 1.5 * scale)?;
            }

            // Directory text
            let text_rect = D2D_RECT_F {
                left: content_x + padding,
                top: item_y + padding / 2.0,
                right: content_x + content_width - padding,
                bottom: item_y + item_height - padding / 2.0,
            };
            renderer.draw_text(dir, &text_format, text_rect, text_color)?;
        }

        // === Hint text at bottom ===
        let hint_y = list_y + (self.picker_dirs.len() as f32) * item_height + padding * 2.0;
        let hint_rect = D2D_RECT_F {
            left: content_x,
            top: hint_y,
            right: content_x + content_width,
            bottom: hint_y + line_height,
        };
        let hint_text = "Enter: Launch  |  ↑↓: Navigate  |  Esc: Back";
        renderer.draw_text(hint_text, &text_format, hint_rect, placeholder_color)?;

        Ok(())
    }
}

impl Default for TailView {
    fn default() -> Self {
        Self::new()
    }
}
