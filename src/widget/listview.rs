//! ListView widget - a scrollable list of elements

use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;

use crate::platform::win32::Renderer;
use crate::platform::Event;
use crate::theme::tree::ThemeTree;
use crate::theme::types::{Color, LayoutContext, Rect};

use super::base::{Constraints, LayoutProps, MeasuredSize};
use super::element::{Element, ElementData, ElementStyle};
use super::{EventResult, Widget, WidgetState, WidgetStyle};

/// Style for ListView widget
#[derive(Clone, Debug)]
pub struct ListViewStyle {
    pub background_color: Color,
    pub border_color: Color,
    pub border_width: f32,
    pub border_radius: f32,
    pub padding_top: f32,
    pub padding_right: f32,
    pub padding_bottom: f32,
    pub padding_left: f32,
    pub element_spacing: f32,
    pub max_visible_items: usize,
    pub scrollbar_width: f32,
    pub scrollbar_color: Color,
    pub scrollbar_track_color: Color,
}

impl Default for ListViewStyle {
    fn default() -> Self {
        Self {
            background_color: Color::TRANSPARENT,
            border_color: Color::TRANSPARENT,
            border_width: 0.0,
            border_radius: 0.0,
            padding_top: 4.0,
            padding_right: 4.0,
            padding_bottom: 4.0,
            padding_left: 4.0,
            element_spacing: 2.0,
            max_visible_items: 10,
            scrollbar_width: 6.0,
            scrollbar_color: Color::from_hex("#606060").unwrap_or(Color::WHITE),
            scrollbar_track_color: Color::from_hex("#303030").unwrap_or(Color::BLACK),
        }
    }
}

impl ListViewStyle {
    /// Load style from theme
    pub fn from_theme(theme: &ThemeTree, state: Option<&str>) -> Self {
        let default = Self::default();
        Self {
            background_color: theme.get_color(
                "listview",
                state,
                "background-color",
                default.background_color,
            ),
            border_color: theme.get_color("listview", state, "border-color", default.border_color),
            border_width: theme.get_number(
                "listview",
                state,
                "border-width",
                default.border_width as f64,
            ) as f32,
            border_radius: theme.get_number(
                "listview",
                state,
                "border-radius",
                default.border_radius as f64,
            ) as f32,
            padding_top: theme.get_number(
                "listview",
                state,
                "padding-top",
                default.padding_top as f64,
            ) as f32,
            padding_right: theme.get_number(
                "listview",
                state,
                "padding-right",
                default.padding_right as f64,
            ) as f32,
            padding_bottom: theme.get_number(
                "listview",
                state,
                "padding-bottom",
                default.padding_bottom as f64,
            ) as f32,
            padding_left: theme.get_number(
                "listview",
                state,
                "padding-left",
                default.padding_left as f64,
            ) as f32,
            element_spacing: theme.get_number(
                "listview",
                state,
                "spacing",
                default.element_spacing as f64,
            ) as f32,
            max_visible_items: theme.get_number(
                "listview",
                state,
                "lines",
                default.max_visible_items as f64,
            ) as usize,
            scrollbar_width: theme.get_number(
                "listview",
                state,
                "scrollbar-width",
                default.scrollbar_width as f64,
            ) as f32,
            scrollbar_color: theme.get_color(
                "listview",
                state,
                "scrollbar-color",
                default.scrollbar_color,
            ),
            scrollbar_track_color: theme.get_color(
                "listview",
                state,
                "scrollbar-track-color",
                default.scrollbar_track_color,
            ),
        }
    }
}

/// A scrollable list of elements
pub struct ListView {
    /// Elements in the list
    elements: Vec<Element>,
    /// Currently selected index
    selected_index: Option<usize>,
    /// Scroll offset (in items, not pixels)
    scroll_offset: usize,
    /// Layout properties
    layout: LayoutProps,
    /// Widget state
    state: WidgetState,
    /// Visual style
    style: ListViewStyle,
    /// Element style (shared by all elements)
    element_style: ElementStyle,
    /// Cached bounds after arrange
    bounds: Option<Rect>,
}

impl ListView {
    /// Create a new empty ListView
    pub fn new() -> Self {
        Self {
            elements: Vec::new(),
            selected_index: None,
            scroll_offset: 0,
            layout: LayoutProps::default(),
            state: WidgetState::Normal,
            style: ListViewStyle::default(),
            element_style: ElementStyle::default(),
            bounds: None,
        }
    }

    /// Set the ListView style
    pub fn with_style(mut self, style: ListViewStyle) -> Self {
        self.style = style;
        self
    }

    /// Set the element style
    pub fn with_element_style(mut self, style: ElementStyle) -> Self {
        self.element_style = style;
        self
    }

    /// Set items from element data
    pub fn set_items(&mut self, items: Vec<ElementData>) {
        self.elements = items
            .into_iter()
            .map(|data| Element::new(data).with_style(self.element_style.clone()))
            .collect();

        // Reset selection if out of bounds
        if let Some(idx) = self.selected_index {
            if idx >= self.elements.len() {
                self.selected_index = if self.elements.is_empty() {
                    None
                } else {
                    Some(0)
                };
            }
        } else if !self.elements.is_empty() {
            self.selected_index = Some(0);
        }

        self.update_selection_state();
        self.ensure_selected_visible();
    }

    /// Get the number of elements
    pub fn len(&self) -> usize {
        self.elements.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    /// Get the selected index
    pub fn selected_index(&self) -> Option<usize> {
        self.selected_index
    }

    /// Get the selected element data
    pub fn selected_data(&self) -> Option<&ElementData> {
        self.selected_index
            .and_then(|idx| self.elements.get(idx))
            .map(|e| e.data())
    }

    /// Select an item by index
    pub fn select(&mut self, index: usize) {
        if index < self.elements.len() {
            self.selected_index = Some(index);
            self.update_selection_state();
            self.ensure_selected_visible();
        }
    }

    /// Select next item
    pub fn select_next(&mut self) {
        if self.elements.is_empty() {
            return;
        }

        let new_index = match self.selected_index {
            Some(idx) => (idx + 1).min(self.elements.len() - 1),
            None => 0,
        };

        self.selected_index = Some(new_index);
        self.update_selection_state();
        self.ensure_selected_visible();
    }

    /// Select previous item
    pub fn select_previous(&mut self) {
        if self.elements.is_empty() {
            return;
        }

        let new_index = match self.selected_index {
            Some(idx) => idx.saturating_sub(1),
            None => 0,
        };

        self.selected_index = Some(new_index);
        self.update_selection_state();
        self.ensure_selected_visible();
    }

    /// Page down (move selection by visible count)
    pub fn page_down(&mut self) {
        if self.elements.is_empty() {
            return;
        }

        let page_size = self.style.max_visible_items.max(1);
        let new_index = match self.selected_index {
            Some(idx) => (idx + page_size).min(self.elements.len() - 1),
            None => page_size.min(self.elements.len() - 1),
        };

        self.selected_index = Some(new_index);
        self.update_selection_state();
        self.ensure_selected_visible();
    }

    /// Page up (move selection by visible count)
    pub fn page_up(&mut self) {
        if self.elements.is_empty() {
            return;
        }

        let page_size = self.style.max_visible_items.max(1);
        let new_index = match self.selected_index {
            Some(idx) => idx.saturating_sub(page_size),
            None => 0,
        };

        self.selected_index = Some(new_index);
        self.update_selection_state();
        self.ensure_selected_visible();
    }

    /// Select first item
    pub fn select_first(&mut self) {
        if !self.elements.is_empty() {
            self.selected_index = Some(0);
            self.update_selection_state();
            self.ensure_selected_visible();
        }
    }

    /// Select last item
    pub fn select_last(&mut self) {
        if !self.elements.is_empty() {
            self.selected_index = Some(self.elements.len() - 1);
            self.update_selection_state();
            self.ensure_selected_visible();
        }
    }

    /// Update selected state on all elements
    fn update_selection_state(&mut self) {
        for (i, elem) in self.elements.iter_mut().enumerate() {
            elem.set_selected(self.selected_index == Some(i));
        }
    }

    /// Ensure the selected item is visible (adjust scroll)
    fn ensure_selected_visible(&mut self) {
        let Some(idx) = self.selected_index else {
            return;
        };

        let max_visible = self.style.max_visible_items;

        // If selected is before visible area, scroll up
        if idx < self.scroll_offset {
            self.scroll_offset = idx;
        }

        // If selected is after visible area, scroll down
        if idx >= self.scroll_offset + max_visible {
            self.scroll_offset = idx - max_visible + 1;
        }
    }

    /// Get the number of visible items
    fn visible_count(&self) -> usize {
        self.style
            .max_visible_items
            .min(self.elements.len().saturating_sub(self.scroll_offset))
    }

    /// Check if scrollbar should be shown
    fn needs_scrollbar(&self) -> bool {
        self.elements.len() > self.style.max_visible_items
    }

    /// Calculate element height including spacing
    fn element_total_height(&self) -> f32 {
        self.element_style.height + self.style.element_spacing
    }

    /// Calculate the total content height
    fn content_height(&self) -> f32 {
        let element_height = self.element_total_height();
        let visible = self.visible_count();
        (visible as f32 * element_height - self.style.element_spacing).max(0.0)
    }
}

impl Default for ListView {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for ListView {
    fn handle_event(&mut self, event: &Event, _ctx: &LayoutContext) -> EventResult {
        use crate::platform::win32::event::KeyCode;

        match event {
            Event::KeyDown { key, .. } => {
                match *key {
                    // Down arrow
                    KeyCode::Down => {
                        self.select_next();
                        EventResult::repaint()
                    }
                    // Up arrow
                    KeyCode::Up => {
                        self.select_previous();
                        EventResult::repaint()
                    }
                    // Page Down
                    KeyCode::PageDown => {
                        self.page_down();
                        EventResult::repaint()
                    }
                    // Page Up
                    KeyCode::PageUp => {
                        self.page_up();
                        EventResult::repaint()
                    }
                    // Home
                    KeyCode::Home => {
                        self.select_first();
                        EventResult::repaint()
                    }
                    // End
                    KeyCode::End => {
                        self.select_last();
                        EventResult::repaint()
                    }
                    // Enter - submit
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
                    _ => EventResult::none(),
                }
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

        // Draw background
        if self.style.background_color.a > 0.0 {
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
        }

        // Draw border
        if self.style.border_width > 0.0 && self.style.border_color.a > 0.0 {
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

        // Calculate content area
        let content_x = rect.x + self.style.padding_left;
        let content_y = rect.y + self.style.padding_top;
        let scrollbar_space = if self.needs_scrollbar() {
            self.style.scrollbar_width + 4.0
        } else {
            0.0
        };
        let content_width =
            rect.width - self.style.padding_left - self.style.padding_right - scrollbar_space;

        // Render visible elements
        let start = self.scroll_offset;
        let end = (start + self.style.max_visible_items).min(self.elements.len());
        let element_height = self.element_total_height();

        log!(
            "ListView::render - rendering {} elements (start={}, end={}), element_height={}",
            end - start,
            start,
            end,
            element_height
        );
        for (i, elem) in self.elements[start..end].iter().enumerate() {
            let elem_rect = Rect {
                x: content_x,
                y: content_y + (i as f32 * element_height),
                width: content_width,
                height: self.element_style.height,
            };
            log!(
                "  Rendering element {} at rect ({},{},{},{})",
                i,
                elem_rect.x,
                elem_rect.y,
                elem_rect.width,
                elem_rect.height
            );
            elem.render(renderer, elem_rect, ctx)?;
        }

        // Draw scrollbar if needed
        if self.needs_scrollbar() {
            let scrollbar_x =
                rect.x + rect.width - self.style.padding_right - self.style.scrollbar_width;
            let scrollbar_y = rect.y + self.style.padding_top;
            let scrollbar_height = self.content_height();

            // Track
            let track_rect = D2D_RECT_F {
                left: scrollbar_x,
                top: scrollbar_y,
                right: scrollbar_x + self.style.scrollbar_width,
                bottom: scrollbar_y + scrollbar_height,
            };
            let scrollbar_radius = self.style.scrollbar_width / 2.0;
            renderer.fill_rounded_rect(
                track_rect,
                scrollbar_radius,
                scrollbar_radius,
                self.style.scrollbar_track_color,
            )?;

            // Thumb
            let total_items = self.elements.len() as f32;
            let visible_items = self.style.max_visible_items as f32;
            let thumb_ratio = visible_items / total_items;
            let thumb_height = (scrollbar_height * thumb_ratio).max(20.0);
            let scroll_ratio = self.scroll_offset as f32 / (total_items - visible_items).max(1.0);
            let thumb_y = scrollbar_y + scroll_ratio * (scrollbar_height - thumb_height);

            let thumb_rect = D2D_RECT_F {
                left: scrollbar_x,
                top: thumb_y,
                right: scrollbar_x + self.style.scrollbar_width,
                bottom: thumb_y + thumb_height,
            };
            renderer.fill_rounded_rect(
                thumb_rect,
                scrollbar_radius,
                scrollbar_radius,
                self.style.scrollbar_color,
            )?;
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
        // ListView uses ListViewStyle
    }

    fn measure(&self, constraints: Constraints, _ctx: &LayoutContext) -> MeasuredSize {
        // Calculate height based on visible items
        let visible_count = self.style.max_visible_items.min(self.elements.len()).max(1);
        let element_height = self.element_total_height();
        let content_height = visible_count as f32 * element_height - self.style.element_spacing;
        let total_height = content_height + self.style.padding_top + self.style.padding_bottom;

        MeasuredSize::new(
            constraints.max.width,
            total_height.clamp(constraints.min.height, constraints.max.height),
        )
    }

    fn arrange(&mut self, bounds: Rect, _ctx: &LayoutContext) {
        self.bounds = Some(bounds);
    }

    fn layout_props(&self) -> &LayoutProps {
        &self.layout
    }

    fn widget_name(&self) -> &str {
        "listview"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_listview_selection() {
        let mut lv = ListView::new();

        // Empty list
        assert_eq!(lv.selected_index(), None);
        lv.select_next();
        assert_eq!(lv.selected_index(), None);

        // Add items
        let items = vec![
            ElementData::new("App 1", "app1.exe"),
            ElementData::new("App 2", "app2.exe"),
            ElementData::new("App 3", "app3.exe"),
        ];
        lv.set_items(items);

        // Should auto-select first
        assert_eq!(lv.selected_index(), Some(0));

        // Navigate down
        lv.select_next();
        assert_eq!(lv.selected_index(), Some(1));
        lv.select_next();
        assert_eq!(lv.selected_index(), Some(2));
        // At end, should stay at last
        lv.select_next();
        assert_eq!(lv.selected_index(), Some(2));

        // Navigate up
        lv.select_previous();
        assert_eq!(lv.selected_index(), Some(1));
        lv.select_previous();
        assert_eq!(lv.selected_index(), Some(0));
        // At start, should stay at first
        lv.select_previous();
        assert_eq!(lv.selected_index(), Some(0));
    }

    #[test]
    fn test_listview_scroll() {
        let mut lv = ListView::new();
        lv.style.max_visible_items = 3;

        let items: Vec<ElementData> = (0..10)
            .map(|i| ElementData::new(format!("App {}", i), format!("app{}.exe", i)))
            .collect();
        lv.set_items(items);

        // Initial state
        assert_eq!(lv.scroll_offset, 0);
        assert_eq!(lv.selected_index(), Some(0));

        // Navigate to item 3 (should scroll)
        lv.select(3);
        assert_eq!(lv.selected_index(), Some(3));
        // scroll_offset should be 1 (items 1,2,3 visible)
        assert_eq!(lv.scroll_offset, 1);

        // Navigate to last
        lv.select_last();
        assert_eq!(lv.selected_index(), Some(9));
        // scroll_offset should be 7 (items 7,8,9 visible)
        assert_eq!(lv.scroll_offset, 7);

        // Navigate to first
        lv.select_first();
        assert_eq!(lv.selected_index(), Some(0));
        assert_eq!(lv.scroll_offset, 0);
    }

    #[test]
    fn test_listview_page_navigation() {
        let mut lv = ListView::new();
        lv.style.max_visible_items = 5;

        let items: Vec<ElementData> = (0..20)
            .map(|i| ElementData::new(format!("App {}", i), format!("app{}.exe", i)))
            .collect();
        lv.set_items(items);

        // Page down
        lv.page_down();
        assert_eq!(lv.selected_index(), Some(5));

        lv.page_down();
        assert_eq!(lv.selected_index(), Some(10));

        // Page up
        lv.page_up();
        assert_eq!(lv.selected_index(), Some(5));

        lv.page_up();
        assert_eq!(lv.selected_index(), Some(0));
    }
}
