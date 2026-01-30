//! GridView widget - a scrollable grid of selectable tiles (thumbnails + labels)
//!
//! Supports two layout modes:
//! - Vertical (default): Thumbnail on TOP, label on BOTTOM (for wallpapers)
//! - Horizontal: Thumbnail on LEFT, label on RIGHT (for themes)

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;

use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap;

use crate::platform::win32::{ImageLoader, Renderer};
use crate::platform::Event;
use crate::theme::tree::ThemeTree;
use crate::theme::types::{Color, LayoutContext, Rect};

use super::base::{Constraints, LayoutProps, MeasuredSize};
use super::{EventResult, Widget, WidgetState, WidgetStyle};

/// Layout direction for grid items
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GridLayout {
    /// Thumbnail on top, label on bottom (for wallpapers - portrait orientation)
    #[default]
    Vertical,
    /// Thumbnail on left, label on right (for themes - landscape orientation)
    Horizontal,
}

impl GridLayout {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "horizontal" => GridLayout::Horizontal,
            _ => GridLayout::Vertical,
        }
    }
}

/// Selection style for grid items
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SelectionStyle {
    /// Border around the entire card (default)
    #[default]
    Border,
    /// Highlight background color on the label area only
    LabelBackground,
}

impl SelectionStyle {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "label-background" | "label_background" | "labelbackground" => {
                SelectionStyle::LabelBackground
            }
            _ => SelectionStyle::Border,
        }
    }
}

/// A single grid item (theme, wallpaper, etc.)
#[derive(Clone, Debug)]
pub struct GridItem {
    pub title: String,
    pub subtitle: Option<String>,
    /// Optional path to an image file (thumbnail)
    pub image_path: Option<String>,
    /// Opaque user payload (e.g. theme dir, wallpaper file)
    pub user_data: String,
}

impl GridItem {
    pub fn new(title: impl Into<String>, user_data: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            subtitle: None,
            image_path: None,
            user_data: user_data.into(),
        }
    }

    pub fn with_subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    pub fn with_image(mut self, image_path: impl Into<String>) -> Self {
        self.image_path = Some(image_path.into());
        self
    }
}

/// Style for GridView widget
#[derive(Clone, Debug)]
pub struct GridViewStyle {
    pub background_color: Color,
    pub padding_top: f32,
    pub padding_right: f32,
    pub padding_bottom: f32,
    pub padding_left: f32,
    pub tile_gap: f32,

    /// Layout direction (vertical or horizontal)
    pub layout: GridLayout,

    /// Selection style (border or label-background)
    pub selection_style: SelectionStyle,

    /// Display width for the thumbnail area (used for horizontal layout)
    pub thumb_width: f32,
    /// Display height for the thumbnail area (used for horizontal layout)
    pub thumb_height: f32,
    /// Legacy: Display size (square) for the thumbnail area (used when thumb_width/thumb_height not set)
    pub thumb_size: f32,
    pub thumb_radius: f32,

    /// Label width (used for horizontal layout - label to the right of thumbnail)
    pub label_width: f32,
    pub label_height: f32,
    pub label_color: Color,
    pub label_color_selected: Color,
    /// Background color for label area (used with LabelBackground selection style)
    pub label_background_color: Color,
    /// Selected background color for label area
    pub label_background_color_selected: Color,

    pub selection_color: Color,
    pub selection_width: f32,
    pub max_columns: usize,

    // Typography
    pub font_family: String,
    pub font_size: f32,

    // Message for empty state
    pub message_text: String,
    pub message_color: Color,
    pub message_font_size: f32,
}

impl Default for GridViewStyle {
    fn default() -> Self {
        Self {
            background_color: Color::TRANSPARENT,
            padding_top: 16.0,
            padding_right: 24.0,
            padding_bottom: 16.0,
            padding_left: 24.0,
            tile_gap: 18.0,

            layout: GridLayout::Vertical,
            selection_style: SelectionStyle::Border,

            thumb_width: 0.0,  // 0 means use thumb_size
            thumb_height: 0.0, // 0 means use thumb_size
            thumb_size: 230.0,
            thumb_radius: 14.0,

            label_width: 0.0, // 0 means not applicable (vertical layout)
            label_height: 28.0,
            label_color: Color::from_hex("#d4d4d4").unwrap_or(Color::WHITE),
            label_color_selected: Color::WHITE,
            label_background_color: Color::TRANSPARENT,
            label_background_color_selected: Color::from_hex("#89b4fa").unwrap_or(Color::BLUE),

            selection_color: Color::from_hex("#264f78").unwrap_or(Color::BLUE),
            selection_width: 3.0,
            max_columns: 5,

            font_family: "Segoe UI".to_string(),
            font_size: 12.0,

            message_text: String::new(),
            message_color: Color::from_hex("#6c7086").unwrap_or(Color::WHITE),
            message_font_size: 24.0,
        }
    }
}

impl GridViewStyle {
    pub fn from_theme(theme: &ThemeTree, state: Option<&str>) -> Self {
        let default = Self::default();

        // Parse layout direction
        let layout_str = theme.get_string("gridview", state, "layout", "vertical");
        let layout = GridLayout::from_str(&layout_str);

        // Parse selection style
        let selection_style_str = theme.get_string("gridview", state, "selection-style", "border");
        let selection_style = SelectionStyle::from_str(&selection_style_str);

        // Read thumb dimensions - prefer explicit width/height, fall back to thumb-size
        let thumb_size =
            theme.get_number("gridview", state, "thumb-size", default.thumb_size as f64) as f32;
        let thumb_width = theme.get_number("gridview", state, "thumb-width", 0.0) as f32;
        let thumb_height = theme.get_number("gridview", state, "thumb-height", 0.0) as f32;

        Self {
            background_color: theme.get_color(
                "gridview",
                state,
                "background-color",
                default.background_color,
            ),
            padding_top: theme.get_number(
                "gridview",
                state,
                "padding-top",
                theme.get_number("gridview", state, "padding", default.padding_top as f64),
            ) as f32,
            padding_right: theme.get_number(
                "gridview",
                state,
                "padding-right",
                theme.get_number("gridview", state, "padding", default.padding_right as f64),
            ) as f32,
            padding_bottom: theme.get_number(
                "gridview",
                state,
                "padding-bottom",
                theme.get_number("gridview", state, "padding", default.padding_bottom as f64),
            ) as f32,
            padding_left: theme.get_number(
                "gridview",
                state,
                "padding-left",
                theme.get_number("gridview", state, "padding", default.padding_left as f64),
            ) as f32,
            tile_gap: theme.get_number("gridview", state, "spacing", default.tile_gap as f64)
                as f32,

            layout,
            selection_style,

            thumb_width,
            thumb_height,
            thumb_size,
            thumb_radius: theme.get_number(
                "gridview",
                state,
                "thumb-radius",
                default.thumb_radius as f64,
            ) as f32,

            label_width: theme.get_number(
                "gridview",
                state,
                "label-width",
                default.label_width as f64,
            ) as f32,
            label_height: theme.get_number(
                "gridview",
                state,
                "label-height",
                default.label_height as f64,
            ) as f32,
            label_color: theme.get_color("gridview", state, "label-color", default.label_color),
            label_color_selected: theme.get_color(
                "gridview",
                Some("selected"),
                "label-color",
                default.label_color_selected,
            ),
            label_background_color: theme.get_color(
                "gridview",
                state,
                "label-background-color",
                default.label_background_color,
            ),
            label_background_color_selected: theme.get_color(
                "gridview",
                Some("selected"),
                "label-background-color",
                default.label_background_color_selected,
            ),

            selection_color: theme.get_color(
                "gridview",
                Some("selected"),
                "border-color",
                default.selection_color,
            ),
            selection_width: theme.get_number(
                "gridview",
                Some("selected"),
                "border-width",
                default.selection_width as f64,
            ) as f32,
            max_columns: theme.get_number(
                "gridview",
                state,
                "max-columns",
                default.max_columns as f64,
            ) as usize,

            font_family: theme.get_string("gridview", state, "font-family", &default.font_family),
            font_size: theme.get_number("gridview", state, "font-size", default.font_size as f64)
                as f32,

            message_text: theme.get_string("message", None, "text", &default.message_text),
            message_color: theme.get_color("message", None, "text-color", default.message_color),
            message_font_size: theme.get_number(
                "message",
                None,
                "font-size",
                default.message_font_size as f64,
            ) as f32,
        }
    }

    /// Get effective thumbnail width (uses thumb_width if set, else thumb_size)
    pub fn effective_thumb_width(&self) -> f32 {
        if self.thumb_width > 0.0 {
            self.thumb_width
        } else {
            self.thumb_size
        }
    }

    /// Get effective thumbnail height (uses thumb_height if set, else thumb_size)
    pub fn effective_thumb_height(&self) -> f32 {
        if self.thumb_height > 0.0 {
            self.thumb_height
        } else {
            self.thumb_size
        }
    }

    /// Get total card width based on layout
    pub fn card_width(&self) -> f32 {
        match self.layout {
            GridLayout::Vertical => self.effective_thumb_width(),
            GridLayout::Horizontal => self.effective_thumb_width() + self.label_width,
        }
    }

    /// Get total card height based on layout
    pub fn card_height(&self) -> f32 {
        match self.layout {
            GridLayout::Vertical => self.effective_thumb_height() + self.label_height,
            GridLayout::Horizontal => self.effective_thumb_height().max(self.label_height),
        }
    }
}

#[derive(Clone)]
struct CachedBitmap {
    bitmap: ID2D1Bitmap,
    source_path: String,
}

/// A scrollable grid of tiles with horizontal scrolling.
/// Items are laid out in rows (fixed number of rows based on height),
/// with columns scrolling horizontally.
pub struct GridView {
    items: Vec<GridItem>,
    selected_index: Option<usize>,
    /// Horizontal scroll offset in columns
    scroll_col: usize,
    layout: LayoutProps,
    state: WidgetState,
    style: GridViewStyle,
    /// Cached bounds after arrange (used for ensuring selected visibility)
    bounds: Option<Rect>,
    last_scale_factor: f32,
    /// Thumbnail cache (path -> bitmap)
    bitmap_cache: RefCell<HashMap<String, CachedBitmap>>,
}

impl GridView {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            selected_index: None,
            scroll_col: 0,
            layout: LayoutProps::default(),
            state: WidgetState::Normal,
            style: GridViewStyle::default(),
            bounds: None,
            last_scale_factor: 1.0,
            bitmap_cache: RefCell::new(HashMap::new()),
        }
    }

    pub fn with_style(mut self, style: GridViewStyle) -> Self {
        self.style = style;
        self
    }

    pub fn set_style(&mut self, style: GridViewStyle) {
        self.style = style;
    }

    pub fn set_items(&mut self, items: Vec<GridItem>) {
        self.items = items;

        if self.items.is_empty() {
            self.selected_index = None;
            self.scroll_col = 0;
        } else if self.selected_index.is_none() {
            self.selected_index = Some(0);
        } else if let Some(idx) = self.selected_index {
            if idx >= self.items.len() {
                self.selected_index = Some(0);
                self.scroll_col = 0;
            }
        }

        self.ensure_selected_visible();
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn selected_index(&self) -> Option<usize> {
        self.selected_index
    }

    pub fn selected_item(&self) -> Option<&GridItem> {
        self.selected_index.and_then(|idx| self.items.get(idx))
    }

    pub fn select(&mut self, index: usize) {
        if index < self.items.len() {
            self.selected_index = Some(index);
            self.ensure_selected_visible();
        }
    }

    /// Calculate the number of rows that fit in the visible area
    fn visible_rows(&self, bounds: Rect, scale: f32) -> usize {
        let gap = self.style.tile_gap * scale;
        let tile_h = self.style.card_height() * scale;
        let pad_t = self.style.padding_top * scale;
        let pad_b = self.style.padding_bottom * scale;
        let content_h = (bounds.height - pad_t - pad_b).max(0.0);

        if tile_h <= 0.0 {
            return 1;
        }

        let rows = ((content_h + gap) / (tile_h + gap)).floor() as usize;
        rows.max(1)
    }

    /// Calculate the number of columns that fit in the visible area
    fn visible_columns(&self, bounds: Rect, scale: f32) -> usize {
        let gap = self.style.tile_gap * scale;
        let tile_w = self.style.card_width() * scale;
        let pad_l = self.style.padding_left * scale;
        let pad_r = self.style.padding_right * scale;
        let content_w = (bounds.width - pad_l - pad_r).max(0.0);

        if tile_w <= 0.0 {
            return 1;
        }

        let columns = ((content_w + gap) / (tile_w + gap)).floor() as usize;
        columns.max(1)
    }

    /// Get total number of columns needed for all items (given row count)
    fn total_columns(&self, rows: usize) -> usize {
        if rows == 0 || self.items.is_empty() {
            return 0;
        }
        (self.items.len() + rows - 1) / rows
    }

    /// Convert item index to (col, row) in column-major order
    fn col_row_for_index(&self, index: usize, rows: usize) -> (usize, usize) {
        if rows == 0 {
            return (0, 0);
        }
        let col = index / rows;
        let row = index % rows;
        (col, row)
    }

    /// Convert (col, row) to item index in column-major order
    fn index_for_col_row(&self, col: usize, row: usize, rows: usize) -> usize {
        col * rows + row
    }

    fn ensure_selected_visible(&mut self) {
        let Some(bounds) = self.bounds else {
            return;
        };
        let Some(idx) = self.selected_index else {
            return;
        };

        let scale = self.last_scale_factor.max(0.0001);
        let rows = self.visible_rows(bounds, scale);
        let visible_cols = self.visible_columns(bounds, scale);
        let (selected_col, _) = self.col_row_for_index(idx, rows);

        // Ensure selected column is visible
        if selected_col < self.scroll_col {
            self.scroll_col = selected_col;
        }
        if selected_col >= self.scroll_col + visible_cols {
            self.scroll_col = selected_col.saturating_sub(visible_cols.saturating_sub(1));
        }
    }

    /// Move selection left (previous column, wrap within row or scroll)
    fn move_left(&mut self, rows: usize) {
        if self.items.is_empty() || rows == 0 {
            return;
        }
        let idx = self.selected_index.unwrap_or(0);
        let (col, row) = self.col_row_for_index(idx, rows);
        let total_cols = self.total_columns(rows);

        let new_col = if col > 0 {
            col - 1
        } else {
            total_cols.saturating_sub(1)
        };
        let new_idx = self.index_for_col_row(new_col, row, rows);

        // Clamp to valid index
        let new_idx = new_idx.min(self.items.len().saturating_sub(1));
        self.select(new_idx);
    }

    /// Move selection right (next column, wrap within row or scroll)
    fn move_right(&mut self, rows: usize) {
        if self.items.is_empty() || rows == 0 {
            return;
        }
        let idx = self.selected_index.unwrap_or(0);
        let (col, row) = self.col_row_for_index(idx, rows);
        let total_cols = self.total_columns(rows);

        let new_col = if col + 1 < total_cols { col + 1 } else { 0 };
        let new_idx = self.index_for_col_row(new_col, row, rows);

        // Clamp to valid index (last column may be partial)
        let new_idx = new_idx.min(self.items.len().saturating_sub(1));
        self.select(new_idx);
    }

    /// Move selection up (previous row in same column, wrap to bottom)
    fn move_up(&mut self, rows: usize) {
        if self.items.is_empty() || rows == 0 {
            return;
        }
        let idx = self.selected_index.unwrap_or(0);
        let (col, row) = self.col_row_for_index(idx, rows);

        let new_row = if row > 0 { row - 1 } else { rows - 1 };
        let new_idx = self.index_for_col_row(col, new_row, rows);

        // Clamp to valid index
        let new_idx = new_idx.min(self.items.len().saturating_sub(1));
        self.select(new_idx);
    }

    /// Move selection down (next row in same column, wrap to top)
    fn move_down(&mut self, rows: usize) {
        if self.items.is_empty() || rows == 0 {
            return;
        }
        let idx = self.selected_index.unwrap_or(0);
        let (col, row) = self.col_row_for_index(idx, rows);

        let new_row = if row + 1 < rows { row + 1 } else { 0 };
        let new_idx = self.index_for_col_row(col, new_row, rows);

        // Clamp to valid index
        let new_idx = new_idx.min(self.items.len().saturating_sub(1));
        self.select(new_idx);
    }

    fn load_bitmap(&self, renderer: &Renderer, path: &str) -> Option<ID2D1Bitmap> {
        {
            let cache = self.bitmap_cache.borrow();
            if let Some(cached) = cache.get(path) {
                if cached.source_path == path {
                    return Some(cached.bitmap.clone());
                }
            }
        }

        let loader = match ImageLoader::new() {
            Ok(l) => l,
            Err(e) => {
                crate::log!("GridView: ImageLoader::new failed: {:?}", e);
                return None;
            }
        };

        let loaded = match loader.load_from_file(Path::new(path)) {
            Ok(img) => img,
            Err(e) => {
                crate::log!("GridView: failed to load image '{}': {:?}", path, e);
                return None;
            }
        };

        let bitmap = match renderer.create_bitmap(&loaded) {
            Ok(b) => b,
            Err(e) => {
                crate::log!("GridView: failed to create bitmap '{}': {:?}", path, e);
                return None;
            }
        };

        self.bitmap_cache.borrow_mut().insert(
            path.to_string(),
            CachedBitmap {
                bitmap: bitmap.clone(),
                source_path: path.to_string(),
            },
        );
        Some(bitmap)
    }
}

impl Default for GridView {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for GridView {
    fn handle_event(&mut self, event: &Event, ctx: &LayoutContext) -> EventResult {
        use crate::platform::win32::event::KeyCode;

        match event {
            Event::KeyDown { key, .. } => {
                let Some(bounds) = self.bounds else {
                    return EventResult::none();
                };
                let rows = self.visible_rows(bounds, ctx.scale_factor);
                match *key {
                    KeyCode::Left => {
                        self.move_left(rows);
                        EventResult::repaint()
                    }
                    KeyCode::Right => {
                        self.move_right(rows);
                        EventResult::repaint()
                    }
                    KeyCode::Up => {
                        self.move_up(rows);
                        EventResult::repaint()
                    }
                    KeyCode::Down => {
                        self.move_down(rows);
                        EventResult::repaint()
                    }
                    KeyCode::Home => {
                        if !self.items.is_empty() {
                            self.select(0);
                            return EventResult::repaint();
                        }
                        EventResult::none()
                    }
                    KeyCode::End => {
                        if !self.items.is_empty() {
                            self.select(self.items.len() - 1);
                            return EventResult::repaint();
                        }
                        EventResult::none()
                    }
                    KeyCode::Enter => {
                        if self.selected_index.is_some() {
                            EventResult {
                                needs_repaint: false,
                                consumed: true,
                                text_changed: false,
                                submit: true,
                                cancel: false,
                            }
                        } else {
                            EventResult::none()
                        }
                    }
                    KeyCode::PageDown | KeyCode::PageUp => {
                        // Page through columns (horizontal scroll)
                        let visible_cols = self.visible_columns(bounds, ctx.scale_factor);
                        if let Some(idx) = self.selected_index {
                            let (col, row) = self.col_row_for_index(idx, rows);
                            let total_cols = self.total_columns(rows);

                            let new_col = if *key == KeyCode::PageDown {
                                (col + visible_cols).min(total_cols.saturating_sub(1))
                            } else {
                                col.saturating_sub(visible_cols)
                            };

                            let new_idx = self
                                .index_for_col_row(new_col, row, rows)
                                .min(self.items.len().saturating_sub(1));
                            self.select(new_idx);
                            return EventResult::repaint();
                        }
                        EventResult::none()
                    }
                    _ => EventResult::none(),
                }
            }
            Event::MouseWheel { delta, .. } => {
                // Scroll the grid horizontally - delta > 0 means scroll left, delta < 0 means scroll right
                let Some(bounds) = self.bounds else {
                    return EventResult::none();
                };
                let rows = self.visible_rows(bounds, ctx.scale_factor);
                let total_cols = self.total_columns(rows);
                let visible_cols = self.visible_columns(bounds, ctx.scale_factor);
                let max_scroll = total_cols.saturating_sub(visible_cols);

                if *delta > 0 {
                    // Scroll left
                    if self.scroll_col > 0 {
                        self.scroll_col = self.scroll_col.saturating_sub(1);
                        return EventResult::repaint();
                    }
                } else if *delta < 0 {
                    // Scroll right
                    if self.scroll_col < max_scroll {
                        self.scroll_col = (self.scroll_col + 1).min(max_scroll);
                        return EventResult::repaint();
                    }
                }
                EventResult::none()
            }
            _ => EventResult::none(),
        }
    }

    fn render(
        &self,
        renderer: &mut Renderer,
        rect: Rect,
        ctx: &LayoutContext,
    ) -> Result<(), windows::core::Error> {
        let bounds = D2D_RECT_F {
            left: rect.x,
            top: rect.y,
            right: rect.x + rect.width,
            bottom: rect.y + rect.height,
        };

        if self.style.background_color.a > 0.0 {
            renderer.fill_rect(bounds, self.style.background_color)?;
        }

        // Show message if empty and message_text is set
        if self.items.is_empty() {
            if !self.style.message_text.is_empty() {
                let scale = ctx.scale_factor;
                let msg_format = renderer.create_text_format(
                    &self.style.font_family,
                    (self.style.message_font_size * scale).max(12.0),
                    false,
                    false,
                )?;
                renderer.draw_text_centered(
                    &self.style.message_text,
                    &msg_format,
                    bounds,
                    self.style.message_color,
                )?;
            }
            return Ok(());
        }

        let scale = ctx.scale_factor;
        let rows = self.visible_rows(rect, scale);
        let visible_cols = self.visible_columns(rect, scale);
        let gap = self.style.tile_gap * scale;

        // Get dimensions based on layout
        let thumb_w = self.style.effective_thumb_width() * scale;
        let thumb_h = self.style.effective_thumb_height() * scale;
        let label_h = self.style.label_height * scale;
        let label_w = self.style.label_width * scale;
        let card_w = self.style.card_width() * scale;
        let card_h = self.style.card_height() * scale;

        let pad_l = self.style.padding_left * scale;
        let pad_r = self.style.padding_right * scale;
        let pad_t = self.style.padding_top * scale;
        let pad_b = self.style.padding_bottom * scale;

        let content_x = rect.x + pad_l;
        let content_y = rect.y + pad_t;
        let content_w = (rect.width - pad_l - pad_r).max(0.0);
        let content_h = (rect.height - pad_t - pad_b).max(0.0);

        let total_cols = self.total_columns(rows);
        let start_col = self.scroll_col.min(total_cols.saturating_sub(1));
        let end_col = (start_col + visible_cols + 1).min(total_cols); // +1 for partial column

        // Center grid vertically if it doesn't fill the height
        let grid_h = rows as f32 * card_h + (rows.saturating_sub(1) as f32) * gap;
        let extra_y = ((content_h - grid_h).max(0.0)) / 2.0;
        let origin_y = content_y + extra_y;

        // Center grid horizontally if all columns fit
        let grid_w = visible_cols.min(total_cols) as f32 * card_w
            + (visible_cols.min(total_cols).saturating_sub(1) as f32) * gap;
        let extra_x = if total_cols <= visible_cols {
            ((content_w - grid_w).max(0.0)) / 2.0
        } else {
            0.0
        };
        let origin_x = content_x + extra_x;

        // Shared text format
        let format = match renderer.create_text_format(
            &self.style.font_family,
            (self.style.font_size * scale).max(10.0),
            false,
            false,
        ) {
            Ok(f) => f,
            Err(_) => {
                return Ok(());
            }
        };

        let radius = self.style.thumb_radius * scale;

        // Iterate column-major: for each visible column, draw all rows
        for col in start_col..end_col {
            let x = origin_x + (col - start_col) as f32 * (card_w + gap);
            if x > content_x + content_w {
                break;
            }

            for row in 0..rows {
                let index = self.index_for_col_row(col, row, rows);
                if index >= self.items.len() {
                    continue;
                }

                let y = origin_y + row as f32 * (card_h + gap);
                let item = &self.items[index];
                let selected = self.selected_index == Some(index);

                match self.style.layout {
                    GridLayout::Vertical => {
                        // Vertical layout: thumbnail on top, label on bottom
                        let thumb_rect = D2D_RECT_F {
                            left: x,
                            top: y,
                            right: x + thumb_w,
                            bottom: y + thumb_h,
                        };

                        // Draw thumbnail with rounded corners
                        let _layer = if radius > 0.0 {
                            renderer.push_rounded_clip(thumb_rect, radius, radius)?
                        } else {
                            None
                        };

                        if let Some(ref path) = item.image_path {
                            if let Some(bitmap) = self.load_bitmap(renderer, path) {
                                renderer.draw_bitmap_cover(&bitmap, thumb_rect, 1.0)?;
                            } else {
                                renderer.fill_rect(
                                    thumb_rect,
                                    Color::from_f32(0.12, 0.12, 0.12, 1.0),
                                )?;
                            }
                        } else {
                            renderer
                                .fill_rect(thumb_rect, Color::from_f32(0.12, 0.12, 0.12, 1.0))?;
                        }

                        if _layer.is_some() {
                            renderer.pop_layer();
                        }

                        // Label below thumbnail
                        let label_rect = D2D_RECT_F {
                            left: x,
                            top: y + thumb_h,
                            right: x + thumb_w,
                            bottom: y + thumb_h + label_h,
                        };

                        // Selection rendering based on style
                        match self.style.selection_style {
                            SelectionStyle::Border => {
                                if selected {
                                    // Draw border around entire card (thumbnail)
                                    let inset = (self.style.selection_width * scale) / 2.0;
                                    let ring = D2D_RECT_F {
                                        left: thumb_rect.left + inset,
                                        top: thumb_rect.top + inset,
                                        right: thumb_rect.right - inset,
                                        bottom: thumb_rect.bottom - inset,
                                    };
                                    renderer.draw_rounded_rect(
                                        ring,
                                        radius.max(0.0),
                                        radius.max(0.0),
                                        self.style.selection_color,
                                        (self.style.selection_width * scale).max(1.0),
                                    )?;
                                }
                            }
                            SelectionStyle::LabelBackground => {
                                // Draw label background
                                let bg_color = if selected {
                                    self.style.label_background_color_selected
                                } else {
                                    self.style.label_background_color
                                };
                                if bg_color.a > 0.0 {
                                    renderer.fill_rect(label_rect, bg_color)?;
                                }
                            }
                        }

                        let label_color = if selected {
                            self.style.label_color_selected
                        } else {
                            self.style.label_color
                        };
                        renderer.draw_text_centered(
                            &item.title,
                            &format,
                            label_rect,
                            label_color,
                        )?;
                    }

                    GridLayout::Horizontal => {
                        // Horizontal layout: thumbnail on left, label on right
                        let thumb_rect = D2D_RECT_F {
                            left: x,
                            top: y,
                            right: x + thumb_w,
                            bottom: y + thumb_h,
                        };

                        // Draw thumbnail with rounded corners
                        let _layer = if radius > 0.0 {
                            renderer.push_rounded_clip(thumb_rect, radius, radius)?
                        } else {
                            None
                        };

                        if let Some(ref path) = item.image_path {
                            if let Some(bitmap) = self.load_bitmap(renderer, path) {
                                renderer.draw_bitmap_cover(&bitmap, thumb_rect, 1.0)?;
                            } else {
                                renderer.fill_rect(
                                    thumb_rect,
                                    Color::from_f32(0.12, 0.12, 0.12, 1.0),
                                )?;
                            }
                        } else {
                            renderer
                                .fill_rect(thumb_rect, Color::from_f32(0.12, 0.12, 0.12, 1.0))?;
                        }

                        if _layer.is_some() {
                            renderer.pop_layer();
                        }

                        // Label to the right of thumbnail
                        let label_rect = D2D_RECT_F {
                            left: x + thumb_w,
                            top: y,
                            right: x + thumb_w + label_w,
                            bottom: y + thumb_h,
                        };

                        // Selection rendering - border around entire card for horizontal
                        if selected {
                            let inset = (self.style.selection_width * scale) / 2.0;
                            let card_rect = D2D_RECT_F {
                                left: x + inset,
                                top: y + inset,
                                right: x + card_w - inset,
                                bottom: y + card_h - inset,
                            };
                            renderer.draw_rounded_rect(
                                card_rect,
                                radius.max(0.0),
                                radius.max(0.0),
                                self.style.selection_color,
                                (self.style.selection_width * scale).max(1.0),
                            )?;
                        }

                        let label_color = if selected {
                            self.style.label_color_selected
                        } else {
                            self.style.label_color
                        };
                        renderer.draw_text_centered(
                            &item.title,
                            &format,
                            label_rect,
                            label_color,
                        )?;
                    }
                }
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
        static DEFAULT: std::sync::OnceLock<WidgetStyle> = std::sync::OnceLock::new();
        DEFAULT.get_or_init(WidgetStyle::default)
    }

    fn set_style(&mut self, _style: WidgetStyle) {
        // GridView uses GridViewStyle
    }

    fn measure(&self, constraints: Constraints, _ctx: &LayoutContext) -> MeasuredSize {
        MeasuredSize::new(constraints.max.width, constraints.max.height)
    }

    fn arrange(&mut self, bounds: Rect, ctx: &LayoutContext) {
        self.bounds = Some(bounds);
        self.last_scale_factor = ctx.scale_factor;
    }

    fn layout_props(&self) -> &LayoutProps {
        &self.layout
    }

    fn widget_name(&self) -> &str {
        "gridview"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gridview_column_major_layout() {
        // Test column-major indexing with 2 rows
        // Items laid out as:
        //   col0  col1  col2
        // row0: 0     2     4
        // row1: 1     3     5
        let mut gv = GridView::new();
        gv.set_items(
            (0..6)
                .map(|i| GridItem::new(format!("I{}", i), "x"))
                .collect(),
        );

        // Test col_row_for_index with 2 rows
        assert_eq!(gv.col_row_for_index(0, 2), (0, 0)); // col 0, row 0
        assert_eq!(gv.col_row_for_index(1, 2), (0, 1)); // col 0, row 1
        assert_eq!(gv.col_row_for_index(2, 2), (1, 0)); // col 1, row 0
        assert_eq!(gv.col_row_for_index(3, 2), (1, 1)); // col 1, row 1
        assert_eq!(gv.col_row_for_index(4, 2), (2, 0)); // col 2, row 0
        assert_eq!(gv.col_row_for_index(5, 2), (2, 1)); // col 2, row 1

        // Test total_columns
        assert_eq!(gv.total_columns(2), 3); // 6 items / 2 rows = 3 columns
    }

    #[test]
    fn test_gridview_horizontal_navigation() {
        // With 2 rows, 6 items:
        //   col0  col1  col2
        // row0: 0     2     4
        // row1: 1     3     5
        let mut gv = GridView::new();
        gv.set_items(
            (0..6)
                .map(|i| GridItem::new(format!("I{}", i), "x"))
                .collect(),
        );
        gv.bounds = Some(Rect::new(0.0, 0.0, 1000.0, 800.0));
        gv.select(0); // Start at index 0 (col 0, row 0)

        // Move right from col 0 to col 1 (same row)
        gv.move_right(2); // 2 rows
        assert_eq!(gv.selected_index(), Some(2)); // Index 2 is (col 1, row 0)

        // Move right again to col 2
        gv.move_right(2);
        assert_eq!(gv.selected_index(), Some(4)); // Index 4 is (col 2, row 0)

        // Move right wraps to col 0
        gv.move_right(2);
        assert_eq!(gv.selected_index(), Some(0)); // Back to (col 0, row 0)

        // Move left wraps to last column
        gv.move_left(2);
        assert_eq!(gv.selected_index(), Some(4)); // Index 4 is (col 2, row 0)
    }

    #[test]
    fn test_gridview_vertical_navigation() {
        // With 2 rows, 6 items:
        //   col0  col1  col2
        // row0: 0     2     4
        // row1: 1     3     5
        let mut gv = GridView::new();
        gv.set_items(
            (0..6)
                .map(|i| GridItem::new(format!("I{}", i), "x"))
                .collect(),
        );
        gv.bounds = Some(Rect::new(0.0, 0.0, 1000.0, 800.0));
        gv.select(0); // Start at index 0 (col 0, row 0)

        // Move down within same column
        gv.move_down(2);
        assert_eq!(gv.selected_index(), Some(1)); // Index 1 is (col 0, row 1)

        // Move down wraps to top of same column
        gv.move_down(2);
        assert_eq!(gv.selected_index(), Some(0)); // Back to (col 0, row 0)

        // Move up wraps to bottom of same column
        gv.move_up(2);
        assert_eq!(gv.selected_index(), Some(1)); // Index 1 is (col 0, row 1)
    }
}
