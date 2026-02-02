//! RenderPort - interface for rendering operations
//!
//! This port defines the rendering capabilities required by the application.

use crate::domain::value_objects::{Color, Rect};

/// Unique identifier for a texture
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TextureId(pub u64);

/// Text alignment options
#[derive(Clone, Copy, Debug, Default)]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}

/// Vertical alignment options
#[derive(Clone, Copy, Debug, Default)]
pub enum VerticalAlign {
    Top,
    #[default]
    Center,
    Bottom,
}

/// Text style for rendering
#[derive(Clone, Debug)]
pub struct TextStyle {
    pub font_family: String,
    pub font_size: f32,
    pub color: Color,
    pub align: TextAlign,
    pub vertical_align: VerticalAlign,
    pub bold: bool,
    pub italic: bool,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            font_family: "Segoe UI".to_string(),
            font_size: 14.0,
            color: Color::WHITE,
            align: TextAlign::Left,
            vertical_align: VerticalAlign::Center,
            bold: false,
            italic: false,
        }
    }
}

/// Render operation error
#[derive(Debug, Clone)]
pub enum RenderError {
    /// Failed to initialize renderer
    InitError(String),
    /// Failed to create texture
    TextureError(String),
    /// Failed to render frame
    FrameError(String),
    /// Resource not found
    ResourceNotFound(String),
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RenderError::InitError(s) => write!(f, "Render init error: {}", s),
            RenderError::TextureError(s) => write!(f, "Texture error: {}", s),
            RenderError::FrameError(s) => write!(f, "Frame error: {}", s),
            RenderError::ResourceNotFound(s) => write!(f, "Resource not found: {}", s),
        }
    }
}

impl std::error::Error for RenderError {}

/// Port interface for rendering operations
pub trait RenderPort: Send + Sync {
    /// Begin a new frame
    fn begin_frame(&mut self) -> Result<(), RenderError>;

    /// End the current frame and present
    fn end_frame(&mut self) -> Result<(), RenderError>;

    /// Draw a filled rectangle
    fn draw_rect(&mut self, rect: Rect, color: Color);

    /// Draw a rectangle outline
    fn draw_rect_outline(&mut self, rect: Rect, color: Color, stroke_width: f32);

    /// Draw a rounded rectangle
    fn draw_rounded_rect(&mut self, rect: Rect, radius: f32, color: Color);

    /// Draw a rounded rectangle outline
    fn draw_rounded_rect_outline(
        &mut self,
        rect: Rect,
        radius: f32,
        color: Color,
        stroke_width: f32,
    );

    /// Draw text
    fn draw_text(&mut self, text: &str, rect: Rect, style: &TextStyle);

    /// Draw a texture
    fn draw_texture(&mut self, texture_id: TextureId, rect: Rect, opacity: f32);

    /// Create a texture from RGBA data
    fn create_texture(
        &mut self,
        width: u32,
        height: u32,
        data: &[u8],
    ) -> Result<TextureId, RenderError>;

    /// Delete a texture
    fn delete_texture(&mut self, texture_id: TextureId);

    /// Push a clip rectangle
    fn push_clip(&mut self, rect: Rect);

    /// Pop the current clip rectangle
    fn pop_clip(&mut self);

    /// Set the opacity for subsequent draw operations
    fn set_opacity(&mut self, opacity: f32);

    /// Measure text dimensions
    fn measure_text(&self, text: &str, style: &TextStyle, max_width: f32) -> (f32, f32);
}

/// A null render port for testing
pub struct NullRenderPort;

impl RenderPort for NullRenderPort {
    fn begin_frame(&mut self) -> Result<(), RenderError> {
        Ok(())
    }

    fn end_frame(&mut self) -> Result<(), RenderError> {
        Ok(())
    }

    fn draw_rect(&mut self, _rect: Rect, _color: Color) {}

    fn draw_rect_outline(&mut self, _rect: Rect, _color: Color, _stroke_width: f32) {}

    fn draw_rounded_rect(&mut self, _rect: Rect, _radius: f32, _color: Color) {}

    fn draw_rounded_rect_outline(
        &mut self,
        _rect: Rect,
        _radius: f32,
        _color: Color,
        _stroke_width: f32,
    ) {
    }

    fn draw_text(&mut self, _text: &str, _rect: Rect, _style: &TextStyle) {}

    fn draw_texture(&mut self, _texture_id: TextureId, _rect: Rect, _opacity: f32) {}

    fn create_texture(
        &mut self,
        _width: u32,
        _height: u32,
        _data: &[u8],
    ) -> Result<TextureId, RenderError> {
        Ok(TextureId(0))
    }

    fn delete_texture(&mut self, _texture_id: TextureId) {}

    fn push_clip(&mut self, _rect: Rect) {}

    fn pop_clip(&mut self) {}

    fn set_opacity(&mut self, _opacity: f32) {}

    fn measure_text(&self, _text: &str, style: &TextStyle, _max_width: f32) -> (f32, f32) {
        (0.0, style.font_size)
    }
}
