//! Markdown View widget - Renders markdown content with navigation
//!
//! Displays PR review markdown files with styled text rendering.
//! Features:
//! - Back button to return to launcher
//! - Left/right navigation between reviews
//! - Scrollable content area
//! - Basic markdown rendering (headers, code, bold, lists)

use pulldown_cmark::{Event, HeadingLevel, Parser, Tag};
use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;

use crate::platform::win32::Renderer;
use crate::pr_reviews::PrReview;
use crate::theme::tree::ThemeTree;
use crate::theme::types::{Color, LayoutContext, Rect};

use super::taskpanel::TaskPanelStyle;

/// A styled text segment for rendering
#[derive(Debug, Clone)]
pub struct StyledSegment {
    pub text: String,
    pub style: TextStyle,
}

/// Text styling options
#[derive(Debug, Clone, Copy, Default)]
pub struct TextStyle {
    pub font_size: f32,
    pub bold: bool,
    pub italic: bool,
    pub code: bool,
    pub color: Color,
    pub indent: f32,
}

/// A line of styled text
#[derive(Debug, Clone)]
pub struct StyledLine {
    pub segments: Vec<StyledSegment>,
    pub line_height: f32,
    pub is_code_block: bool,
}

/// Markdown view style from theme
#[derive(Debug, Clone)]
pub struct MarkdownViewStyle {
    pub background_color: Color,
    pub text_color: Color,
    pub heading_color: Color,
    pub code_bg_color: Color,
    pub code_text_color: Color,
    pub link_color: Color,
    pub button_bg: Color,
    pub button_hover_bg: Color,
    pub badge_bg: Color,
    pub badge_text: Color,
    pub font_size: f32,
}

impl Default for MarkdownViewStyle {
    fn default() -> Self {
        Self {
            background_color: Color::from_f32(0.1, 0.1, 0.12, 1.0),
            text_color: Color::from_f32(0.9, 0.9, 0.9, 1.0),
            heading_color: Color::from_f32(0.4, 0.7, 1.0, 1.0),
            code_bg_color: Color::from_f32(0.15, 0.15, 0.18, 1.0),
            code_text_color: Color::from_f32(0.7, 0.9, 0.7, 1.0),
            link_color: Color::from_f32(0.5, 0.7, 1.0, 1.0),
            button_bg: Color::from_f32(0.2, 0.2, 0.25, 1.0),
            button_hover_bg: Color::from_f32(0.3, 0.3, 0.35, 1.0),
            badge_bg: Color::from_f32(0.9, 0.2, 0.2, 1.0),
            badge_text: Color::WHITE,
            font_size: 14.0,
        }
    }
}

impl MarkdownViewStyle {
    pub fn from_theme(theme: &ThemeTree) -> Self {
        let mut style = Self::default();
        style.background_color = theme.get_color("tailview", None, "background-color", style.background_color);
        style.text_color = theme.get_color("tailview", None, "text-color", style.text_color);
        style.heading_color = theme.get_color("textbox", Some("selected"), "text-color", style.heading_color);
        style.code_bg_color = theme.get_color("element", Some("selected"), "background-color", style.code_bg_color);
        style
    }
}

/// Hit test result for markdown view
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkdownViewHit {
    None,
    BackButton,
    PrevButton,
    NextButton,
    LinkButton,
    Content,
}

/// Markdown view state
#[derive(Debug)]
pub struct MarkdownView {
    /// Current style
    style: MarkdownViewStyle,
    /// Task panel style (for button rendering)
    button_style: TaskPanelStyle,
    /// Parsed lines for rendering
    lines: Vec<StyledLine>,
    /// Scroll offset (in pixels)
    scroll_offset: f32,
    /// Total content height
    content_height: f32,
    /// Visible height
    visible_height: f32,
    /// Current review index
    current_index: usize,
    /// Total reviews count
    total_count: usize,
    /// Current review title
    current_title: String,
    /// PR number
    pr_number: u32,
    /// Author extracted from content
    author: String,
    /// PR link URL
    pr_link: Option<String>,
    /// Button rects for hit testing
    back_button_rect: Option<Rect>,
    prev_button_rect: Option<Rect>,
    next_button_rect: Option<Rect>,
    link_button_rect: Option<Rect>,
    /// Bounds
    bounds: Rect,
    /// Content width for word wrapping
    content_width: f32,
}

impl Default for MarkdownView {
    fn default() -> Self {
        Self::new()
    }
}

impl MarkdownView {
    pub fn new() -> Self {
        Self {
            style: MarkdownViewStyle::default(),
            button_style: TaskPanelStyle::default(),
            lines: Vec::new(),
            scroll_offset: 0.0,
            content_height: 0.0,
            visible_height: 0.0,
            current_index: 0,
            total_count: 0,
            current_title: String::new(),
            pr_number: 0,
            author: String::new(),
            back_button_rect: None,
            prev_button_rect: None,
            next_button_rect: None,
            link_button_rect: None,
            pr_link: None,
            bounds: Rect::default(),
            content_width: 800.0,
        }
    }

    pub fn set_style(&mut self, style: MarkdownViewStyle) {
        self.style = style;
    }

    pub fn set_button_style(&mut self, style: TaskPanelStyle) {
        self.button_style = style;
    }

    /// Extract a field value from markdown table format: "| **FieldName** | Value |"
    /// Handles bold field names like "**Title**" and "**Author**"
    fn extract_table_field(content: &str, field_name: &str) -> Option<String> {
        for line in content.lines() {
            let trimmed = line.trim();
            if !trimmed.contains('|') {
                continue;
            }
            // Split by | and look for the field name
            let parts: Vec<&str> = trimmed.split('|').collect();
            for (i, part) in parts.iter().enumerate() {
                let part_trimmed = part.trim();
                // Strip markdown bold markers (**) and check for field name
                let part_clean = part_trimmed
                    .trim_start_matches("**")
                    .trim_end_matches("**")
                    .trim();

                if part_clean.eq_ignore_ascii_case(field_name) {
                    if let Some(value) = parts.get(i + 1) {
                        let value_trimmed = value.trim();
                        if !value_trimmed.is_empty() && !value_trimmed.contains("---") {
                            return Some(value_trimmed.to_string());
                        }
                    }
                }
            }
        }
        None
    }

    /// Extract title from markdown content (table format: "| **Title** | value |")
    fn extract_title(content: &str) -> String {
        Self::extract_table_field(content, "Title").unwrap_or_default()
    }

    /// Extract author from markdown content (table format: "| **Author** | value |")
    fn extract_author(content: &str) -> String {
        Self::extract_table_field(content, "Author").unwrap_or_default()
    }

    /// Extract PR link from markdown content (table format: "| **Link** | [url](url) |")
    fn extract_link(content: &str) -> Option<String> {
        if let Some(link_field) = Self::extract_table_field(content, "Link") {
            // Link is in format: [text](url) - extract the URL
            if let Some(start) = link_field.find('(') {
                if let Some(end) = link_field.rfind(')') {
                    if start < end {
                        return Some(link_field[start + 1..end].to_string());
                    }
                }
            }
            // If no markdown link format, check if it's a plain URL
            if link_field.starts_with("http") {
                return Some(link_field);
            }
        }
        None
    }

    /// Load content from a PR review
    pub fn load_review(&mut self, review: &PrReview, content: &str, index: usize, total: usize) {
        self.current_index = index;
        self.total_count = total;
        self.scroll_offset = 0.0;

        // Extract title from table format, fallback to review.title()
        let title = Self::extract_title(content);
        self.current_title = if title.is_empty() {
            review.title()
        } else {
            title
        };

        self.pr_number = review.pr_number;
        self.author = Self::extract_author(content);
        self.pr_link = Self::extract_link(content);
        self.parse_markdown(content);
    }

    /// Parse markdown content into styled lines
    fn parse_markdown(&mut self, content: &str) {
        self.lines.clear();
        let parser = Parser::new(content);

        let base_size = self.style.font_size;
        let mut current_line = StyledLine {
            segments: Vec::new(),
            line_height: base_size * 1.5,
            is_code_block: false,
        };

        let mut in_code_block = false;
        let mut in_heading = false;
        let mut heading_level: i32 = 1;
        let mut in_bold = false;
        let mut in_italic = false;
        let mut in_code = false;
        let mut list_depth: i32 = 0;

        for event in parser {
            match event {
                Event::Start(tag) => match tag {
                    Tag::Heading(level, _, _) => {
                        in_heading = true;
                        heading_level = match level {
                            HeadingLevel::H1 => 1,
                            HeadingLevel::H2 => 2,
                            HeadingLevel::H3 => 3,
                            HeadingLevel::H4 => 4,
                            HeadingLevel::H5 => 5,
                            HeadingLevel::H6 => 6,
                        };
                        current_line.line_height = base_size * (2.5 - heading_level as f32 * 0.2);
                    }
                    Tag::CodeBlock(_) => {
                        in_code_block = true;
                        current_line.is_code_block = true;
                    }
                    Tag::Strong => in_bold = true,
                    Tag::Emphasis => in_italic = true,
                    Tag::List(_) => list_depth += 1,
                    Tag::Item => {
                        // Add list marker
                        let indent = (list_depth - 1) as f32 * 20.0;
                        current_line.segments.push(StyledSegment {
                            text: "• ".to_string(),
                            style: TextStyle {
                                font_size: base_size,
                                indent,
                                color: self.style.text_color,
                                ..Default::default()
                            },
                        });
                    }
                    _ => {}
                },
                Event::End(tag) => match tag {
                    Tag::Heading(_, _, _) => {
                        in_heading = false;
                        self.lines.push(std::mem::replace(
                            &mut current_line,
                            StyledLine {
                                segments: Vec::new(),
                                line_height: base_size * 1.5,
                                is_code_block: false,
                            },
                        ));
                    }
                    Tag::Paragraph => {
                        self.lines.push(std::mem::replace(
                            &mut current_line,
                            StyledLine {
                                segments: Vec::new(),
                                line_height: base_size * 1.5,
                                is_code_block: false,
                            },
                        ));
                        // Add empty line after paragraph
                        self.lines.push(StyledLine {
                            segments: Vec::new(),
                            line_height: base_size * 0.5,
                            is_code_block: false,
                        });
                    }
                    Tag::CodeBlock(_) => {
                        in_code_block = false;
                        self.lines.push(std::mem::replace(
                            &mut current_line,
                            StyledLine {
                                segments: Vec::new(),
                                line_height: base_size * 1.5,
                                is_code_block: false,
                            },
                        ));
                    }
                    Tag::Strong => in_bold = false,
                    Tag::Emphasis => in_italic = false,
                    Tag::List(_) => list_depth = list_depth.saturating_sub(1),
                    Tag::Item => {
                        self.lines.push(std::mem::replace(
                            &mut current_line,
                            StyledLine {
                                segments: Vec::new(),
                                line_height: base_size * 1.5,
                                is_code_block: false,
                            },
                        ));
                    }
                    _ => {}
                },
                Event::Text(text) => {
                    let style = if in_heading {
                        let size_mult = match heading_level {
                            1 => 1.8,
                            2 => 1.5,
                            3 => 1.3,
                            _ => 1.1,
                        };
                        TextStyle {
                            font_size: base_size * size_mult,
                            bold: true,
                            color: self.style.heading_color,
                            ..Default::default()
                        }
                    } else if in_code_block || in_code {
                        TextStyle {
                            font_size: base_size * 0.9,
                            code: true,
                            color: self.style.code_text_color,
                            ..Default::default()
                        }
                    } else {
                        TextStyle {
                            font_size: base_size,
                            bold: in_bold,
                            italic: in_italic,
                            color: self.style.text_color,
                            indent: (list_depth as f32) * 20.0,
                            ..Default::default()
                        }
                    };

                    // Split text by newlines for code blocks
                    if in_code_block {
                        for (i, line_text) in text.split('\n').enumerate() {
                            if i > 0 {
                                self.lines.push(std::mem::replace(
                                    &mut current_line,
                                    StyledLine {
                                        segments: Vec::new(),
                                        line_height: base_size * 1.3,
                                        is_code_block: true,
                                    },
                                ));
                            }
                            if !line_text.is_empty() {
                                current_line.segments.push(StyledSegment {
                                    text: line_text.to_string(),
                                    style,
                                });
                            }
                        }
                    } else {
                        current_line.segments.push(StyledSegment {
                            text: text.to_string(),
                            style,
                        });
                    }
                }
                Event::Code(code) => {
                    current_line.segments.push(StyledSegment {
                        text: code.to_string(),
                        style: TextStyle {
                            font_size: base_size * 0.9,
                            code: true,
                            color: self.style.code_text_color,
                            ..Default::default()
                        },
                    });
                }
                Event::SoftBreak | Event::HardBreak => {
                    self.lines.push(std::mem::replace(
                        &mut current_line,
                        StyledLine {
                            segments: Vec::new(),
                            line_height: base_size * 1.5,
                            is_code_block: in_code_block,
                        },
                    ));
                }
                Event::Rule => {
                    // Add horizontal rule as empty styled line
                    self.lines.push(StyledLine {
                        segments: vec![StyledSegment {
                            text: "─".repeat(50),
                            style: TextStyle {
                                font_size: base_size,
                                color: Color::from_f32(0.4, 0.4, 0.4, 1.0),
                                ..Default::default()
                            },
                        }],
                        line_height: base_size * 1.5,
                        is_code_block: false,
                    });
                }
                _ => {}
            }
        }

        // Push any remaining content
        if !current_line.segments.is_empty() {
            self.lines.push(current_line);
        }

        // Calculate total content height
        self.content_height = self.lines.iter().map(|l| l.line_height).sum();
    }

    /// Scroll by delta pixels (positive = scroll down, negative = scroll up)
    pub fn scroll(&mut self, delta: f32) {
        self.scroll_offset = (self.scroll_offset + delta)
            .max(0.0)
            .min((self.content_height - self.visible_height).max(0.0));
    }

    /// Scroll to top
    pub fn scroll_to_top(&mut self) {
        self.scroll_offset = 0.0;
    }

    /// Hit test
    pub fn hit_test(&self, x: f32, y: f32) -> MarkdownViewHit {
        if let Some(rect) = self.back_button_rect {
            if rect.contains(x, y) {
                return MarkdownViewHit::BackButton;
            }
        }
        if let Some(rect) = self.prev_button_rect {
            if rect.contains(x, y) {
                return MarkdownViewHit::PrevButton;
            }
        }
        if let Some(rect) = self.next_button_rect {
            if rect.contains(x, y) {
                return MarkdownViewHit::NextButton;
            }
        }
        if let Some(rect) = self.link_button_rect {
            if rect.contains(x, y) {
                return MarkdownViewHit::LinkButton;
            }
        }
        if self.bounds.contains(x, y) {
            return MarkdownViewHit::Content;
        }
        MarkdownViewHit::None
    }

    /// Get PR link URL if available
    pub fn pr_link(&self) -> Option<&str> {
        self.pr_link.as_deref()
    }

    /// Check if can go to previous review
    pub fn can_go_prev(&self) -> bool {
        self.current_index > 0
    }

    /// Check if can go to next review
    pub fn can_go_next(&self) -> bool {
        self.current_index + 1 < self.total_count
    }

    /// Get current index
    pub fn current_index(&self) -> usize {
        self.current_index
    }

    /// Render the markdown view
    pub fn render(
        &mut self,
        renderer: &mut Renderer,
        bounds: Rect,
        ctx: &LayoutContext,
    ) -> Result<(), windows::core::Error> {
        let scale = ctx.scale_factor;
        self.bounds = bounds;
        let button_size = 40.0 * scale;
        let padding = 20.0 * scale;
        let header_height = 72.0 * scale;
        let button_spacing = 12.0 * scale;

        // Button bar on the left (slim vertical strip)
        let button_bar_width = button_size + padding * 1.5;
        let button_bar_rect = D2D_RECT_F {
            left: bounds.x,
            top: bounds.y,
            right: bounds.x + button_bar_width,
            bottom: bounds.y + bounds.height,
        };
        // Darker button bar background
        renderer.fill_rect(button_bar_rect, Color::from_f32(0.08, 0.08, 0.10, 1.0))?;

        // Back button (top)
        let back_y = bounds.y + padding;
        let back_rect = Rect {
            x: bounds.x + (button_bar_width - button_size) / 2.0,
            y: back_y,
            width: button_size,
            height: button_size,
        };
        self.back_button_rect = Some(back_rect);
        self.render_icon_button(renderer, back_rect, "←", scale)?;

        // Navigation buttons centered vertically
        let nav_center_y = bounds.y + bounds.height / 2.0;

        // Prev button
        let prev_rect = Rect {
            x: bounds.x + (button_bar_width - button_size) / 2.0,
            y: nav_center_y - button_size - button_spacing / 2.0,
            width: button_size,
            height: button_size,
        };
        self.prev_button_rect = Some(prev_rect);
        self.render_nav_button(renderer, prev_rect, "▲", self.can_go_prev(), scale)?;

        // Next button
        let next_rect = Rect {
            x: bounds.x + (button_bar_width - button_size) / 2.0,
            y: nav_center_y + button_spacing / 2.0,
            width: button_size,
            height: button_size,
        };
        self.next_button_rect = Some(next_rect);
        self.render_nav_button(renderer, next_rect, "▼", self.can_go_next(), scale)?;

        // Counter display (bottom of button bar)
        let counter_y = bounds.y + bounds.height - 40.0 * scale;
        let counter_text = format!("{}/{}", self.current_index + 1, self.total_count);
        let counter_rect = D2D_RECT_F {
            left: bounds.x,
            top: counter_y,
            right: bounds.x + button_bar_width,
            bottom: counter_y + 24.0 * scale,
        };
        let counter_format = renderer.create_text_format("Segoe UI", 11.0 * scale, false, false)?;
        renderer.draw_text_centered(&counter_text, &counter_format, counter_rect, Color::from_f32(0.6, 0.6, 0.6, 1.0))?;

        // Content area
        let content_x = bounds.x + button_bar_width;
        let content_width = bounds.width - button_bar_width;
        self.content_width = content_width;

        // === Header Section ===
        let header_rect = D2D_RECT_F {
            left: content_x,
            top: bounds.y,
            right: content_x + content_width,
            bottom: bounds.y + header_height,
        };
        // Gradient-like header with subtle color
        renderer.fill_rect(header_rect, Color::from_f32(0.12, 0.13, 0.16, 1.0))?;

        // Header bottom border
        let border_rect = D2D_RECT_F {
            left: content_x,
            top: bounds.y + header_height - 2.0 * scale,
            right: content_x + content_width,
            bottom: bounds.y + header_height,
        };
        renderer.fill_rect(border_rect, Color::from_f32(0.3, 0.5, 0.8, 0.6))?;

        // PR Number badge
        let pr_text = format!("PR #{}", self.pr_number);
        let badge_width = 100.0 * scale;
        let badge_height = 24.0 * scale;
        let badge_rect = D2D_RECT_F {
            left: content_x + padding,
            top: bounds.y + padding / 2.0,
            right: content_x + padding + badge_width,
            bottom: bounds.y + padding / 2.0 + badge_height,
        };
        renderer.fill_rounded_rect(badge_rect, 4.0 * scale, 4.0 * scale, Color::from_f32(0.2, 0.4, 0.7, 1.0))?;
        let badge_format = renderer.create_text_format("Segoe UI", 12.0 * scale, true, false)?;
        renderer.draw_text_centered(&pr_text, &badge_format, badge_rect, Color::WHITE)?;

        // Title (main heading)
        let title_format = renderer.create_text_format("Segoe UI", 18.0 * scale, true, false)?;
        let title_rect = D2D_RECT_F {
            left: content_x + padding,
            top: bounds.y + padding / 2.0 + badge_height + 6.0 * scale,
            right: content_x + content_width - padding,
            bottom: bounds.y + header_height - 4.0 * scale,
        };
        renderer.draw_text(&self.current_title, &title_format, title_rect, self.style.heading_color)?;

        // Author (if available)
        if !self.author.is_empty() {
            let author_text = format!("by {}", self.author);
            let author_format = renderer.create_text_format("Segoe UI", 12.0 * scale, false, true)?;
            let author_rect = D2D_RECT_F {
                left: content_x + padding + badge_width + 12.0 * scale,
                top: bounds.y + padding / 2.0,
                right: content_x + content_width - padding - 120.0 * scale, // Leave room for link button
                bottom: bounds.y + padding / 2.0 + badge_height,
            };
            renderer.draw_text(&author_text, &author_format, author_rect, Color::from_f32(0.7, 0.7, 0.7, 1.0))?;
        }

        // Open PR link button (top right of header)
        if self.pr_link.is_some() {
            let link_btn_width = 100.0 * scale;
            let link_btn_height = 28.0 * scale;
            let link_btn_rect = Rect {
                x: content_x + content_width - padding - link_btn_width,
                y: bounds.y + padding / 2.0,
                width: link_btn_width,
                height: link_btn_height,
            };
            self.link_button_rect = Some(link_btn_rect);

            let link_d2d_rect = D2D_RECT_F {
                left: link_btn_rect.x,
                top: link_btn_rect.y,
                right: link_btn_rect.x + link_btn_rect.width,
                bottom: link_btn_rect.y + link_btn_rect.height,
            };

            // Button background with link color
            renderer.fill_rounded_rect(link_d2d_rect, 4.0 * scale, 4.0 * scale, Color::from_f32(0.15, 0.4, 0.7, 1.0))?;

            // Button text with icon
            let link_format = renderer.create_text_format("Segoe UI", 11.0 * scale, true, false)?;
            renderer.draw_text_centered("🔗 Open PR", &link_format, link_d2d_rect, Color::WHITE)?;
        } else {
            self.link_button_rect = None;
        }

        // === Content Area ===
        let content_top = bounds.y + header_height;
        let content_rect = D2D_RECT_F {
            left: content_x,
            top: content_top,
            right: content_x + content_width,
            bottom: bounds.y + bounds.height,
        };
        renderer.fill_rect(content_rect, self.style.background_color)?;

        self.visible_height = bounds.height - header_height;

        // Render content with clipping (add more padding)
        let content_padding = padding * 1.5;
        let clip_rect = D2D_RECT_F {
            left: content_x,
            top: content_top,
            right: content_x + content_width,
            bottom: bounds.y + bounds.height,
        };

        renderer.push_clip_rect(clip_rect);

        let text_width = content_width - content_padding * 2.0 - 12.0 * scale; // Leave room for scrollbar
        // Use wrapping text formats for proper word wrap
        let normal_format = renderer.create_text_format_wrap("Segoe UI", self.style.font_size * scale, false, false)?;
        let code_format = renderer.create_text_format_wrap("Cascadia Code", self.style.font_size * 0.9 * scale, false, false)?;

        // First pass: calculate actual line heights with word wrapping
        let mut line_heights: Vec<f32> = Vec::with_capacity(self.lines.len());
        let mut total_height = 0.0f32;

        for line in &self.lines {
            // Build line text
            let mut line_text = String::new();
            let mut is_heading = false;
            let mut heading_size = self.style.font_size;

            for segment in &line.segments {
                if !line_text.is_empty() && !segment.text.starts_with(' ') {
                    line_text.push(' ');
                }
                line_text.push_str(&segment.text);
                if segment.style.font_size > self.style.font_size {
                    is_heading = true;
                    heading_size = segment.style.font_size;
                }
            }

            // Measure actual text height with word wrapping
            let format = if is_heading {
                renderer.create_text_format_wrap("Segoe UI", heading_size * scale, true, false)?
            } else if line.is_code_block {
                code_format.clone()
            } else {
                normal_format.clone()
            };

            let line_height = if line_text.is_empty() {
                line.line_height * scale // Use default for empty lines
            } else {
                let (_, measured_height) = renderer.measure_text(&line_text, &format, text_width, 10000.0)?;
                // Add some line spacing
                (measured_height + 4.0 * scale).max(line.line_height * scale)
            };

            line_heights.push(line_height);
            total_height += line_height;
        }

        // Update content height for scrollbar
        self.content_height = total_height;

        // Second pass: render with correct positions
        let mut y = content_top + content_padding - self.scroll_offset;

        for (idx, line) in self.lines.iter().enumerate() {
            let line_height = line_heights[idx];

            // Skip lines above visible area
            if y + line_height < content_top {
                y += line_height;
                continue;
            }
            // Stop if below visible area
            if y > bounds.y + bounds.height {
                break;
            }

            // Code block background with rounded corners and margin
            if line.is_code_block && !line.segments.is_empty() {
                let code_bg_rect = D2D_RECT_F {
                    left: content_x + content_padding,
                    top: y - 2.0 * scale,
                    right: content_x + content_width - content_padding - 12.0 * scale,
                    bottom: y + line_height + 2.0 * scale,
                };
                renderer.fill_rounded_rect(code_bg_rect, 4.0 * scale, 4.0 * scale, self.style.code_bg_color)?;
            }

            let x = content_x + content_padding;

            // Concatenate all segments into a single line text for simpler rendering
            let mut line_text = String::new();
            let mut primary_color = self.style.text_color;
            let mut is_heading = false;
            let mut heading_size = self.style.font_size;

            for segment in &line.segments {
                if !line_text.is_empty() && !segment.text.starts_with(' ') {
                    line_text.push(' ');
                }
                line_text.push_str(&segment.text);
                // Use first segment's styling for the whole line
                if primary_color == self.style.text_color {
                    primary_color = segment.style.color;
                }
                if segment.style.font_size > self.style.font_size {
                    is_heading = true;
                    heading_size = segment.style.font_size;
                }
            }

            let format = if is_heading {
                renderer.create_text_format_wrap("Segoe UI", heading_size * scale, true, false)?
            } else if line.is_code_block {
                code_format.clone()
            } else {
                normal_format.clone()
            };

            // Text rect - sized to fit wrapped text
            let text_rect = D2D_RECT_F {
                left: x,
                top: y,
                right: content_x + content_width - content_padding - 12.0 * scale,
                bottom: y + line_height,
            };

            renderer.draw_text(&line_text, &format, text_rect, primary_color)?;

            y += line_height;
        }

        renderer.pop_clip();

        // Scrollbar (more visible, rounded)
        if self.content_height > self.visible_height {
            let scrollbar_width = 6.0 * scale;
            let scrollbar_height = ((self.visible_height / self.content_height) * self.visible_height).max(30.0 * scale);
            let scrollbar_track_height = self.visible_height - content_padding * 2.0;
            let scroll_ratio = self.scroll_offset / (self.content_height - self.visible_height).max(1.0);
            let scrollbar_y = content_top + content_padding + scroll_ratio * (scrollbar_track_height - scrollbar_height);

            // Scrollbar track (subtle)
            let track_rect = D2D_RECT_F {
                left: content_x + content_width - scrollbar_width - 4.0 * scale,
                top: content_top + content_padding,
                right: content_x + content_width - 4.0 * scale,
                bottom: content_top + self.visible_height - content_padding,
            };
            renderer.fill_rounded_rect(track_rect, scrollbar_width / 2.0, scrollbar_width / 2.0, Color::from_f32(0.2, 0.2, 0.2, 0.3))?;

            // Scrollbar thumb
            let scrollbar_rect = D2D_RECT_F {
                left: content_x + content_width - scrollbar_width - 4.0 * scale,
                top: scrollbar_y,
                right: content_x + content_width - 4.0 * scale,
                bottom: scrollbar_y + scrollbar_height,
            };
            renderer.fill_rounded_rect(scrollbar_rect, scrollbar_width / 2.0, scrollbar_width / 2.0, Color::from_f32(0.5, 0.5, 0.6, 0.7))?;
        }

        Ok(())
    }

    fn render_icon_button(
        &self,
        renderer: &mut Renderer,
        rect: Rect,
        icon: &str,
        scale: f32,
    ) -> Result<(), windows::core::Error> {
        let d2d_rect = D2D_RECT_F {
            left: rect.x,
            top: rect.y,
            right: rect.x + rect.width,
            bottom: rect.y + rect.height,
        };
        let corner = 8.0 * scale;
        renderer.fill_rounded_rect(d2d_rect, corner, corner, self.button_style.item_background_color)?;

        let format = renderer.create_text_format("Segoe UI", 20.0 * scale, false, false)?;
        renderer.draw_text_centered(icon, &format, d2d_rect, self.style.text_color)?;

        Ok(())
    }

    fn render_nav_button(
        &self,
        renderer: &mut Renderer,
        rect: Rect,
        icon: &str,
        enabled: bool,
        scale: f32,
    ) -> Result<(), windows::core::Error> {
        let d2d_rect = D2D_RECT_F {
            left: rect.x,
            top: rect.y,
            right: rect.x + rect.width,
            bottom: rect.y + rect.height,
        };
        let corner = 8.0 * scale;

        let bg_color = if enabled {
            self.button_style.item_background_color
        } else {
            Color::from_f32(0.15, 0.15, 0.15, 0.5)
        };
        let text_color = if enabled {
            self.style.text_color
        } else {
            Color::from_f32(0.4, 0.4, 0.4, 1.0)
        };

        renderer.fill_rounded_rect(d2d_rect, corner, corner, bg_color)?;

        let format = renderer.create_text_format("Segoe UI", 20.0 * scale, false, false)?;
        renderer.draw_text_centered(icon, &format, d2d_rect, text_color)?;

        Ok(())
    }
}
