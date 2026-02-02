//! Textbox widget - single-line text input with cursor and selection

use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;
use windows::Win32::Graphics::DirectWrite::IDWriteTextFormat;

use crate::platform::win32::{render::rect as d2d_rect, Renderer};
use crate::platform::{Event, KeyCode};
use crate::theme::types::{LayoutContext, Rect};

use super::base::{Constraints, LayoutProps, MeasuredSize};
use super::{EventResult, Widget, WidgetState, WidgetStyle};

/// A single-line text input widget
pub struct Textbox {
    /// Current text content
    text: String,
    /// Cursor position (character index)
    cursor: usize,
    /// Selection anchor (None if no selection)
    selection_anchor: Option<usize>,
    /// Placeholder text shown when empty
    placeholder: String,
    /// Widget state
    state: WidgetState,
    /// Visual style
    style: WidgetStyle,
    /// Layout properties
    layout: LayoutProps,
    /// Cached text format
    text_format: Option<IDWriteTextFormat>,
    /// Cursor blink state
    cursor_visible: bool,
    /// Scroll offset for long text
    scroll_offset: f32,
}

impl Default for Textbox {
    fn default() -> Self {
        Self::new()
    }
}

impl Textbox {
    pub fn new() -> Self {
        Self {
            text: String::new(),
            cursor: 0,
            selection_anchor: None,
            placeholder: String::new(),
            state: WidgetState::Normal,
            style: WidgetStyle::default(),
            layout: LayoutProps::default(),
            text_format: None,
            cursor_visible: true,
            scroll_offset: 0.0,
        }
    }

    pub fn with_placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    pub fn with_style(mut self, style: WidgetStyle) -> Self {
        self.style = style;
        self
    }

    /// Get current text
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Set text and reset cursor
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.cursor = self.text.len();
        self.selection_anchor = None;
        self.scroll_offset = 0.0;
    }

    /// Clear the text
    pub fn clear(&mut self) {
        self.text.clear();
        self.cursor = 0;
        self.selection_anchor = None;
        self.scroll_offset = 0.0;
    }

    /// Get placeholder text
    pub fn placeholder(&self) -> &str {
        &self.placeholder
    }

    /// Set placeholder text
    pub fn set_placeholder(&mut self, placeholder: impl Into<String>) {
        self.placeholder = placeholder.into();
    }

    /// Toggle cursor blink
    pub fn toggle_cursor_blink(&mut self) {
        self.cursor_visible = !self.cursor_visible;
    }

    /// Reset cursor to visible
    pub fn show_cursor(&mut self) {
        self.cursor_visible = true;
    }

    /// Get selection range (start, end) if any
    pub fn selection(&self) -> Option<(usize, usize)> {
        self.selection_anchor.map(|anchor| {
            if anchor < self.cursor {
                (anchor, self.cursor)
            } else {
                (self.cursor, anchor)
            }
        })
    }

    /// Get selected text if any
    pub fn selected_text(&self) -> Option<&str> {
        self.selection().map(|(start, end)| &self.text[start..end])
    }

    /// Delete selected text
    fn delete_selection(&mut self) -> bool {
        if let Some((start, end)) = self.selection() {
            self.text.drain(start..end);
            self.cursor = start;
            self.selection_anchor = None;
            true
        } else {
            false
        }
    }

    /// Insert text at cursor position
    fn insert_text(&mut self, text: &str) {
        self.delete_selection();
        self.text.insert_str(self.cursor, text);
        self.cursor += text.len();
    }

    /// Move cursor left
    fn move_left(&mut self, select: bool) {
        if !select {
            // If there was a selection, move to start of selection
            if let Some((start, _)) = self.selection() {
                self.cursor = start;
                self.selection_anchor = None;
                return;
            }
        } else if self.selection_anchor.is_none() {
            self.selection_anchor = Some(self.cursor);
        }

        if self.cursor > 0 {
            // Move back by one character (handle UTF-8)
            let mut new_pos = self.cursor - 1;
            while new_pos > 0 && !self.text.is_char_boundary(new_pos) {
                new_pos -= 1;
            }
            self.cursor = new_pos;
        }

        if !select {
            self.selection_anchor = None;
        }
    }

    /// Move cursor right
    fn move_right(&mut self, select: bool) {
        if !select {
            // If there was a selection, move to end of selection
            if let Some((_, end)) = self.selection() {
                self.cursor = end;
                self.selection_anchor = None;
                return;
            }
        } else if self.selection_anchor.is_none() {
            self.selection_anchor = Some(self.cursor);
        }

        if self.cursor < self.text.len() {
            // Move forward by one character (handle UTF-8)
            let mut new_pos = self.cursor + 1;
            while new_pos < self.text.len() && !self.text.is_char_boundary(new_pos) {
                new_pos += 1;
            }
            self.cursor = new_pos;
        }

        if !select {
            self.selection_anchor = None;
        }
    }

    /// Move cursor to start
    fn move_home(&mut self, select: bool) {
        if select && self.selection_anchor.is_none() {
            self.selection_anchor = Some(self.cursor);
        }
        self.cursor = 0;
        if !select {
            self.selection_anchor = None;
        }
    }

    /// Move cursor to end
    fn move_end(&mut self, select: bool) {
        if select && self.selection_anchor.is_none() {
            self.selection_anchor = Some(self.cursor);
        }
        self.cursor = self.text.len();
        if !select {
            self.selection_anchor = None;
        }
    }

    /// Delete character before cursor (backspace)
    fn backspace(&mut self) -> bool {
        if self.delete_selection() {
            return true;
        }
        if self.cursor > 0 {
            let mut del_pos = self.cursor - 1;
            while del_pos > 0 && !self.text.is_char_boundary(del_pos) {
                del_pos -= 1;
            }
            self.text.drain(del_pos..self.cursor);
            self.cursor = del_pos;
            return true;
        }
        false
    }

    /// Delete character after cursor (delete)
    fn delete(&mut self) -> bool {
        if self.delete_selection() {
            return true;
        }
        if self.cursor < self.text.len() {
            let mut del_end = self.cursor + 1;
            while del_end < self.text.len() && !self.text.is_char_boundary(del_end) {
                del_end += 1;
            }
            self.text.drain(self.cursor..del_end);
            return true;
        }
        false
    }

    /// Select all text
    fn select_all(&mut self) {
        self.selection_anchor = Some(0);
        self.cursor = self.text.len();
    }

    /// Copy selected text (returns text to copy)
    pub fn copy(&self) -> Option<String> {
        self.selected_text().map(|s| s.to_string())
    }

    /// Cut selected text (returns text to cut)
    pub fn cut(&mut self) -> Option<String> {
        let text = self.copy();
        if text.is_some() {
            self.delete_selection();
        }
        text
    }

    /// Paste text
    pub fn paste(&mut self, text: &str) {
        self.insert_text(text);
    }

    /// Ensure text format is created/cached
    fn ensure_text_format(&mut self, renderer: &Renderer) {
        if self.text_format.is_none() {
            self.text_format = renderer
                .create_text_format(&self.style.font_family, self.style.font_size, false, false)
                .ok();
        }
    }

    /// Get inner content rect (after padding)
    fn content_rect(&self, rect: Rect) -> Rect {
        Rect {
            x: rect.x + self.style.padding_left,
            y: rect.y + self.style.padding_top,
            width: rect.width - self.style.padding_left - self.style.padding_right,
            height: rect.height - self.style.padding_top - self.style.padding_bottom,
        }
    }
}

impl Widget for Textbox {
    fn handle_event(&mut self, event: &Event, _ctx: &LayoutContext) -> EventResult {
        match event {
            Event::KeyDown { key, modifiers } => {
                self.show_cursor();

                match key {
                    KeyCode::Left => {
                        self.move_left(modifiers.shift);
                        EventResult::repaint()
                    }
                    KeyCode::Right => {
                        self.move_right(modifiers.shift);
                        EventResult::repaint()
                    }
                    KeyCode::Home => {
                        self.move_home(modifiers.shift);
                        EventResult::repaint()
                    }
                    KeyCode::End => {
                        self.move_end(modifiers.shift);
                        EventResult::repaint()
                    }
                    KeyCode::Backspace => {
                        if self.backspace() {
                            EventResult {
                                needs_repaint: true,
                                consumed: true,
                                text_changed: true,
                                ..Default::default()
                            }
                        } else {
                            EventResult::consumed()
                        }
                    }
                    KeyCode::Delete => {
                        if self.delete() {
                            EventResult {
                                needs_repaint: true,
                                consumed: true,
                                text_changed: true,
                                ..Default::default()
                            }
                        } else {
                            EventResult::consumed()
                        }
                    }
                    KeyCode::Enter => EventResult {
                        consumed: true,
                        submit: true,
                        ..Default::default()
                    },
                    KeyCode::Escape => EventResult {
                        consumed: true,
                        cancel: true,
                        ..Default::default()
                    },
                    KeyCode::A if modifiers.ctrl => {
                        self.select_all();
                        EventResult::repaint()
                    }
                    // C, X, V handled by app (clipboard access needed)
                    _ => EventResult::none(),
                }
            }
            Event::Char(ch) => {
                // Filter out control characters
                if *ch >= ' ' {
                    self.show_cursor();
                    self.insert_text(&ch.to_string());
                    EventResult {
                        needs_repaint: true,
                        consumed: true,
                        text_changed: true,
                        ..Default::default()
                    }
                } else {
                    EventResult::none()
                }
            }
            Event::FocusGained => {
                self.state = WidgetState::Focused;
                self.show_cursor();
                EventResult::repaint()
            }
            Event::FocusLost => {
                self.state = WidgetState::Normal;
                self.selection_anchor = None;
                EventResult::repaint()
            }
            _ => EventResult::none(),
        }
    }

    fn render(
        &self,
        renderer: &mut Renderer,
        rect: Rect,
        _ctx: &LayoutContext,
    ) -> Result<(), windows::core::Error> {
        let bounds = D2D_RECT_F {
            left: rect.x,
            top: rect.y,
            right: rect.x + rect.width,
            bottom: rect.y + rect.height,
        };

        // Draw background
        if self.style.border_radius > 0.0 {
            renderer.fill_rounded_rect(
                bounds,
                self.style.border_radius,
                self.style.border_radius,
                self.style.background_color,
            )?;
        } else {
            renderer.fill_rect(bounds, self.style.background_color)?;
        }

        // Draw border
        if self.style.border_width > 0.0 {
            if self.style.border_radius > 0.0 {
                renderer.draw_rounded_rect(
                    bounds,
                    self.style.border_radius,
                    self.style.border_radius,
                    self.style.border_color,
                    self.style.border_width,
                )?;
            } else {
                renderer.draw_rect(bounds, self.style.border_color, self.style.border_width)?;
            }
        }

        // Get text format - need to create if not exists
        let format = match renderer.create_text_format(
            &self.style.font_family,
            self.style.font_size,
            false,
            false,
        ) {
            Ok(f) => f,
            Err(_) => return Ok(()),
        };

        let content = self.content_rect(rect);
        let text_rect = d2d_rect(content.x, content.y, content.width, content.height);

        // Draw text or placeholder
        let (display_text, text_color) = if self.text.is_empty() {
            (&self.placeholder, self.style.placeholder_color)
        } else {
            (&self.text, self.style.text_color)
        };

        // Draw selection highlight if any
        if self.state == WidgetState::Focused {
            if let Some((start, end)) = self.selection() {
                if let (Ok(start_x), Ok(end_x)) = (
                    renderer.get_caret_position(
                        &self.text,
                        &format,
                        start,
                        content.width,
                        content.height,
                    ),
                    renderer.get_caret_position(
                        &self.text,
                        &format,
                        end,
                        content.width,
                        content.height,
                    ),
                ) {
                    let sel_rect = d2d_rect(
                        content.x + start_x,
                        content.y,
                        end_x - start_x,
                        content.height,
                    );
                    renderer.fill_rect(sel_rect, self.style.selection_color)?;
                }
            }
        }

        // Draw text
        if !display_text.is_empty() {
            renderer.draw_text(display_text, &format, text_rect, text_color)?;
        }

        // Draw cursor
        if self.state == WidgetState::Focused && self.cursor_visible {
            if let Ok(cursor_x) = renderer.get_caret_position(
                &self.text,
                &format,
                self.cursor,
                content.width,
                content.height,
            ) {
                let cursor_x = content.x + cursor_x;
                // Scale cursor width with font size (min 2px)
                let cursor_width = (self.style.font_size / 12.0).max(2.0);
                renderer.draw_line(
                    cursor_x,
                    content.y + 2.0,
                    cursor_x,
                    content.y + content.height - 2.0,
                    self.style.cursor_color,
                    cursor_width,
                )?;
            }
        }

        Ok(())
    }

    fn state(&self) -> WidgetState {
        self.state
    }

    fn set_state(&mut self, state: WidgetState) {
        self.state = state;
    }

    fn style(&self) -> &WidgetStyle {
        &self.style
    }

    fn set_style(&mut self, style: WidgetStyle) {
        self.style = style;
        self.text_format = None; // Invalidate cached format
    }

    fn measure(&self, constraints: Constraints, _ctx: &LayoutContext) -> MeasuredSize {
        // Calculate desired height based on font size + padding
        let height = self.style.font_size
            + self.style.padding_top
            + self.style.padding_bottom
            + self.style.border_width * 2.0
            + 8.0; // Extra padding for cursor/descenders

        // Width: use available width or fixed if specified
        let width = self
            .layout
            .fixed_width
            .unwrap_or(constraints.max.width)
            .min(constraints.max.width)
            .max(constraints.min.width);

        MeasuredSize::new(width, height)
    }

    fn layout_props(&self) -> &LayoutProps {
        &self.layout
    }

    fn widget_name(&self) -> &str {
        "textbox"
    }
}
