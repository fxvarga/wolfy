//! Direct2D rendering for Wolfy

use std::collections::HashMap;

use windows::core::Error;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::DirectWrite::*;
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_UNKNOWN;

use super::dpi::DpiInfo;
use super::window::get_client_size;
use crate::theme::types::Color;

/// A cached brush key
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct BrushKey {
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

impl From<Color> for BrushKey {
    fn from(c: Color) -> Self {
        Self {
            r: (c.r * 255.0) as u8,
            g: (c.g * 255.0) as u8,
            b: (c.b * 255.0) as u8,
            a: (c.a * 255.0) as u8,
        }
    }
}

/// Direct2D rendering context
pub struct Renderer {
    factory: ID2D1Factory,
    dwrite_factory: IDWriteFactory,
    render_target: Option<ID2D1HwndRenderTarget>,
    brush_cache: HashMap<BrushKey, ID2D1SolidColorBrush>,
    hwnd: HWND,
    dpi: DpiInfo,
}

impl Renderer {
    /// Create a new renderer for a window
    pub fn new(hwnd: HWND) -> Result<Self, Error> {
        log!("Renderer::new() starting, hwnd={:?}", hwnd);

        log!("  Creating D2D1 Factory...");
        let factory: ID2D1Factory =
            unsafe { D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)? };
        log!("  D2D1 Factory created");

        log!("  Creating DWrite Factory...");
        let dwrite_factory: IDWriteFactory =
            unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)? };
        log!("  DWrite Factory created");

        let dpi = DpiInfo::for_window(hwnd);
        log!("  DPI: {}, scale: {}", dpi.dpi, dpi.scale_factor);

        let renderer = Self {
            factory,
            dwrite_factory,
            render_target: None,
            brush_cache: HashMap::new(),
            hwnd,
            dpi,
        };

        // NOTE: Don't create render target here - window may be hidden (size 0,0)
        // Render target will be created lazily on first draw when window is visible
        log!("Renderer::new() completed (render_target=None, will create lazily)");

        Ok(renderer)
    }

    /// Create or recreate the render target
    /// Returns Ok(true) if target was created, Ok(false) if window has zero size
    fn create_render_target(&mut self) -> Result<bool, Error> {
        log!("create_render_target() called");

        let (width, height) = get_client_size(self.hwnd);
        log!("  Client size: {}x{}", width, height);

        // Can't create render target for zero-size window
        if width <= 0 || height <= 0 {
            log!("  Size is zero or negative, cannot create render target");
            return Ok(false);
        }

        let render_props = D2D1_RENDER_TARGET_PROPERTIES {
            r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_UNKNOWN,
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            },
            dpiX: self.dpi.dpi as f32,
            dpiY: self.dpi.dpi as f32,
            usage: D2D1_RENDER_TARGET_USAGE_NONE,
            minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
        };

        let hwnd_props = D2D1_HWND_RENDER_TARGET_PROPERTIES {
            hwnd: self.hwnd,
            pixelSize: D2D_SIZE_U {
                width: width as u32,
                height: height as u32,
            },
            presentOptions: D2D1_PRESENT_OPTIONS_NONE,
        };

        self.brush_cache.clear();

        log!("  Calling CreateHwndRenderTarget...");
        unsafe {
            match self
                .factory
                .CreateHwndRenderTarget(&render_props, &hwnd_props)
            {
                Ok(target) => {
                    log!("  CreateHwndRenderTarget succeeded");
                    self.render_target = Some(target);
                    Ok(true)
                }
                Err(e) => {
                    log!("  CreateHwndRenderTarget FAILED: {:?}", e);
                    Err(e)
                }
            }
        }
    }

    /// Ensure render target exists and is valid, creating if needed
    fn ensure_render_target(&mut self) -> Result<bool, Error> {
        if self.render_target.is_some() {
            log!("ensure_render_target: target already exists");
            return Ok(true);
        }
        log!("ensure_render_target: target is None, creating...");
        self.create_render_target()
    }

    /// Handle DPI change
    pub fn handle_dpi_change(&mut self, new_dpi: u32) -> Result<(), Error> {
        self.dpi = DpiInfo {
            dpi: new_dpi,
            scale_factor: new_dpi as f32 / 96.0,
        };
        // Recreate render target if it exists; ignore if window has zero size
        if self.render_target.is_some() {
            self.render_target = None;
            self.brush_cache.clear();
            let _ = self.create_render_target();
        }
        Ok(())
    }

    /// Handle resize
    pub fn handle_resize(&mut self) -> Result<(), Error> {
        let (width, height) = get_client_size(self.hwnd);
        if let Some(ref target) = self.render_target {
            unsafe {
                let size = D2D_SIZE_U {
                    width: width as u32,
                    height: height as u32,
                };
                target.Resize(&size)?;
            }
        }
        Ok(())
    }

    /// Get or create a solid color brush
    fn get_brush(&mut self, color: Color) -> Result<ID2D1SolidColorBrush, Error> {
        let key = BrushKey::from(color);

        if let Some(brush) = self.brush_cache.get(&key) {
            return Ok(brush.clone());
        }

        let target = self
            .render_target
            .as_ref()
            .ok_or_else(|| Error::from_win32())?;

        let d2d_color = D2D1_COLOR_F {
            r: color.r,
            g: color.g,
            b: color.b,
            a: color.a,
        };

        let brush = unsafe {
            // ID2D1HwndRenderTarget inherits from ID2D1RenderTarget
            target.CreateSolidColorBrush(&d2d_color, None)?
        };

        self.brush_cache.insert(key, brush.clone());
        Ok(brush)
    }

    /// Get the DWrite factory for text formatting
    pub fn dwrite_factory(&self) -> &IDWriteFactory {
        &self.dwrite_factory
    }

    /// Get current DPI info
    pub fn dpi(&self) -> DpiInfo {
        self.dpi
    }

    /// Begin drawing - returns true if drawing can proceed
    pub fn begin_draw(&mut self) -> bool {
        log!("begin_draw() called");

        // Ensure we have a valid render target
        log!("  Calling ensure_render_target()...");
        match self.ensure_render_target() {
            Ok(true) => {
                log!("  ensure_render_target() returned Ok(true)");
            }
            Ok(false) => {
                log!("  ensure_render_target() returned Ok(false) - no valid target");
                return false;
            }
            Err(e) => {
                log!("  ensure_render_target() returned Err: {:?}", e);
                return false;
            }
        }

        if let Some(ref target) = self.render_target {
            log!("  Calling target.BeginDraw()...");
            unsafe {
                target.BeginDraw();
            }
            log!("  BeginDraw() completed");
            return true;
        }

        log!("  render_target is None after ensure_render_target - unexpected!");
        false
    }

    /// End drawing
    pub fn end_draw(&self) -> Result<(), Error> {
        log!("end_draw() called");
        if let Some(ref target) = self.render_target {
            log!("  Calling target.EndDraw()...");
            unsafe {
                let result = target.EndDraw(None, None);
                log!("  EndDraw() result: {:?}", result);
                result?;
            }
        } else {
            log!("  No render target, skipping EndDraw");
        }
        Ok(())
    }

    /// Clear the render target
    pub fn clear(&self, color: Color) {
        if let Some(ref target) = self.render_target {
            let d2d_color = D2D1_COLOR_F {
                r: color.r,
                g: color.g,
                b: color.b,
                a: color.a,
            };
            unsafe {
                target.Clear(Some(&d2d_color));
            }
        }
    }

    /// Fill a rectangle
    pub fn fill_rect(&mut self, rect: D2D_RECT_F, color: Color) -> Result<(), Error> {
        let brush = self.get_brush(color)?;
        if let Some(ref target) = self.render_target {
            unsafe {
                target.FillRectangle(&rect, &brush);
            }
        }
        Ok(())
    }

    /// Draw a rectangle outline
    pub fn draw_rect(
        &mut self,
        rect: D2D_RECT_F,
        color: Color,
        stroke_width: f32,
    ) -> Result<(), Error> {
        let brush = self.get_brush(color)?;
        if let Some(ref target) = self.render_target {
            unsafe {
                target.DrawRectangle(&rect, &brush, stroke_width, None);
            }
        }
        Ok(())
    }

    /// Fill a rounded rectangle
    pub fn fill_rounded_rect(
        &mut self,
        rect: D2D_RECT_F,
        radius_x: f32,
        radius_y: f32,
        color: Color,
    ) -> Result<(), Error> {
        let brush = self.get_brush(color)?;
        let rounded = D2D1_ROUNDED_RECT {
            rect,
            radiusX: radius_x,
            radiusY: radius_y,
        };
        if let Some(ref target) = self.render_target {
            unsafe {
                target.FillRoundedRectangle(&rounded, &brush);
            }
        }
        Ok(())
    }

    /// Draw a rounded rectangle outline
    pub fn draw_rounded_rect(
        &mut self,
        rect: D2D_RECT_F,
        radius_x: f32,
        radius_y: f32,
        color: Color,
        stroke_width: f32,
    ) -> Result<(), Error> {
        let brush = self.get_brush(color)?;
        let rounded = D2D1_ROUNDED_RECT {
            rect,
            radiusX: radius_x,
            radiusY: radius_y,
        };
        if let Some(ref target) = self.render_target {
            unsafe {
                target.DrawRoundedRectangle(&rounded, &brush, stroke_width, None);
            }
        }
        Ok(())
    }

    /// Draw a line
    pub fn draw_line(
        &mut self,
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        color: Color,
        stroke_width: f32,
    ) -> Result<(), Error> {
        let brush = self.get_brush(color)?;
        if let Some(ref target) = self.render_target {
            unsafe {
                target.DrawLine(
                    D2D_POINT_2F { x: x1, y: y1 },
                    D2D_POINT_2F { x: x2, y: y2 },
                    &brush,
                    stroke_width,
                    None,
                );
            }
        }
        Ok(())
    }

    /// Create a text format
    pub fn create_text_format(
        &self,
        font_family: &str,
        font_size: f32,
        bold: bool,
        italic: bool,
    ) -> Result<IDWriteTextFormat, Error> {
        let family: Vec<u16> = font_family
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        let weight = if bold {
            DWRITE_FONT_WEIGHT_BOLD
        } else {
            DWRITE_FONT_WEIGHT_REGULAR
        };

        let style = if italic {
            DWRITE_FONT_STYLE_ITALIC
        } else {
            DWRITE_FONT_STYLE_NORMAL
        };

        unsafe {
            let format = self.dwrite_factory.CreateTextFormat(
                windows::core::PCWSTR(family.as_ptr()),
                None,
                weight,
                style,
                DWRITE_FONT_STRETCH_NORMAL,
                font_size,
                windows::core::w!("en-US"),
            )?;

            // Set text alignment
            format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING)?;
            format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;

            Ok(format)
        }
    }

    /// Draw text
    pub fn draw_text(
        &mut self,
        text: &str,
        format: &IDWriteTextFormat,
        rect: D2D_RECT_F,
        color: Color,
    ) -> Result<(), Error> {
        let brush = self.get_brush(color)?;
        let text_wide: Vec<u16> = text.encode_utf16().collect();

        if let Some(ref target) = self.render_target {
            unsafe {
                target.DrawText(
                    &text_wide,
                    format,
                    &rect,
                    &brush,
                    D2D1_DRAW_TEXT_OPTIONS_NONE,
                    DWRITE_MEASURING_MODE_NATURAL,
                );
            }
        }
        Ok(())
    }

    /// Measure text dimensions
    pub fn measure_text(
        &self,
        text: &str,
        format: &IDWriteTextFormat,
        max_width: f32,
        max_height: f32,
    ) -> Result<(f32, f32), Error> {
        let text_wide: Vec<u16> = text.encode_utf16().collect();

        unsafe {
            let layout = self
                .dwrite_factory
                .CreateTextLayout(&text_wide, format, max_width, max_height)?;

            let mut metrics = DWRITE_TEXT_METRICS::default();
            layout.GetMetrics(&mut metrics)?;

            Ok((metrics.width, metrics.height))
        }
    }

    /// Get caret position for a character index
    pub fn get_caret_position(
        &self,
        text: &str,
        format: &IDWriteTextFormat,
        char_index: usize,
        max_width: f32,
        max_height: f32,
    ) -> Result<f32, Error> {
        let text_wide: Vec<u16> = text.encode_utf16().collect();

        unsafe {
            let layout = self
                .dwrite_factory
                .CreateTextLayout(&text_wide, format, max_width, max_height)?;

            let mut x = 0.0f32;
            let mut y = 0.0f32;
            let mut metrics = DWRITE_HIT_TEST_METRICS::default();

            layout.HitTestTextPosition(char_index as u32, false, &mut x, &mut y, &mut metrics)?;

            Ok(x)
        }
    }

    /// Get character index from a point
    pub fn hit_test_point(
        &self,
        text: &str,
        format: &IDWriteTextFormat,
        x: f32,
        y: f32,
        max_width: f32,
        max_height: f32,
    ) -> Result<usize, Error> {
        let text_wide: Vec<u16> = text.encode_utf16().collect();

        unsafe {
            let layout = self
                .dwrite_factory
                .CreateTextLayout(&text_wide, format, max_width, max_height)?;

            let mut is_trailing = windows::Win32::Foundation::BOOL::default();
            let mut is_inside = windows::Win32::Foundation::BOOL::default();
            let mut metrics = DWRITE_HIT_TEST_METRICS::default();

            layout.HitTestPoint(x, y, &mut is_trailing, &mut is_inside, &mut metrics)?;

            let mut pos = metrics.textPosition as usize;
            if is_trailing.as_bool() {
                pos += 1;
            }

            Ok(pos)
        }
    }
}

/// Helper to create a D2D rect from position and size
pub fn rect(x: f32, y: f32, width: f32, height: f32) -> D2D_RECT_F {
    D2D_RECT_F {
        left: x,
        top: y,
        right: x + width,
        bottom: y + height,
    }
}

/// Helper to inset a rect
pub fn inset_rect(r: D2D_RECT_F, inset: f32) -> D2D_RECT_F {
    D2D_RECT_F {
        left: r.left + inset,
        top: r.top + inset,
        right: r.right - inset,
        bottom: r.bottom - inset,
    }
}
