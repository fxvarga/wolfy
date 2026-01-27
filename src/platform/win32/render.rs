//! Direct2D rendering for Wolfy with per-pixel alpha support

use std::collections::HashMap;

use windows::core::Error;
use windows::Win32::Foundation::{HWND, POINT, SIZE};
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::DirectWrite::*;
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, SelectObject, BITMAPINFO,
    BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HDC, HGDIOBJ,
};
use windows::Win32::UI::WindowsAndMessaging::{UpdateLayeredWindow, ULW_ALPHA};

use super::dpi::DpiInfo;
use super::image::LoadedImage;
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

/// Offscreen buffer for per-pixel alpha rendering
struct OffscreenBuffer {
    dc: HDC,
    bitmap: windows::Win32::Graphics::Gdi::HBITMAP,
    old_bitmap: HGDIOBJ,
    width: i32,
    height: i32,
}

impl OffscreenBuffer {
    fn new(width: i32, height: i32) -> Result<Self, Error> {
        log!("OffscreenBuffer::new({}x{})", width, height);

        if width <= 0 || height <= 0 {
            return Err(Error::from_win32());
        }

        unsafe {
            // Create a memory DC
            let dc = CreateCompatibleDC(HDC::default());
            if dc.is_invalid() {
                log!("  CreateCompatibleDC failed");
                return Err(Error::from_win32());
            }
            log!("  Created memory DC: {:?}", dc);

            // Create a 32-bit DIB section for ARGB
            let bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width,
                    biHeight: -height, // Top-down DIB (negative height)
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    biSizeImage: 0,
                    biXPelsPerMeter: 0,
                    biYPelsPerMeter: 0,
                    biClrUsed: 0,
                    biClrImportant: 0,
                },
                bmiColors: [Default::default()],
            };

            let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
            let bitmap = CreateDIBSection(dc, &bmi, DIB_RGB_COLORS, &mut bits, None, 0)?;

            if bitmap.is_invalid() {
                log!("  CreateDIBSection failed");
                DeleteDC(dc);
                return Err(Error::from_win32());
            }
            log!("  Created DIB section: {:?}", bitmap);

            // Select the bitmap into the DC
            let old_bitmap = SelectObject(dc, bitmap);
            log!("  Selected bitmap into DC");

            Ok(Self {
                dc,
                bitmap,
                old_bitmap,
                width,
                height,
            })
        }
    }

    fn dc(&self) -> HDC {
        self.dc
    }

    fn size(&self) -> (i32, i32) {
        (self.width, self.height)
    }
}

impl Drop for OffscreenBuffer {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.dc, self.old_bitmap);
            let _ = DeleteObject(self.bitmap);
            DeleteDC(self.dc);
        }
    }
}

/// Direct2D rendering context with per-pixel alpha support
pub struct Renderer {
    factory: ID2D1Factory,
    dwrite_factory: IDWriteFactory,
    render_target: Option<ID2D1DCRenderTarget>,
    offscreen: Option<OffscreenBuffer>,
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
            offscreen: None,
            brush_cache: HashMap::new(),
            hwnd,
            dpi,
        };

        log!("Renderer::new() completed (render_target=None, will create lazily)");

        Ok(renderer)
    }

    /// Create or recreate the render target and offscreen buffer
    fn create_render_target(&mut self) -> Result<bool, Error> {
        log!("create_render_target() called");

        let (width, height) = get_client_size(self.hwnd);
        log!("  Client size: {}x{}", width, height);

        // Can't create render target for zero-size window
        if width <= 0 || height <= 0 {
            log!("  Size is zero or negative, cannot create render target");
            return Ok(false);
        }

        // Create offscreen buffer
        log!("  Creating offscreen buffer...");
        let offscreen = OffscreenBuffer::new(width, height)?;
        log!("  Offscreen buffer created");

        // Create DC render target properties for premultiplied alpha
        let render_props = D2D1_RENDER_TARGET_PROPERTIES {
            r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            },
            dpiX: self.dpi.dpi as f32,
            dpiY: self.dpi.dpi as f32,
            usage: D2D1_RENDER_TARGET_USAGE_NONE,
            minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
        };

        self.brush_cache.clear();

        log!("  Calling CreateDCRenderTarget...");
        unsafe {
            match self.factory.CreateDCRenderTarget(&render_props) {
                Ok(target) => {
                    log!("  CreateDCRenderTarget succeeded");

                    // Bind to the offscreen DC
                    let rect = windows::Win32::Foundation::RECT {
                        left: 0,
                        top: 0,
                        right: width,
                        bottom: height,
                    };
                    target.BindDC(offscreen.dc(), &rect)?;
                    log!("  BindDC succeeded");

                    self.render_target = Some(target);
                    self.offscreen = Some(offscreen);
                    Ok(true)
                }
                Err(e) => {
                    log!("  CreateDCRenderTarget FAILED: {:?}", e);
                    Err(e)
                }
            }
        }
    }

    /// Ensure render target exists and is valid, creating if needed
    fn ensure_render_target(&mut self) -> Result<bool, Error> {
        let (width, height) = get_client_size(self.hwnd);

        // Check if we need to recreate due to size change
        if let Some(ref offscreen) = self.offscreen {
            let (buf_w, buf_h) = offscreen.size();
            if buf_w != width || buf_h != height {
                log!(
                    "ensure_render_target: size changed from {}x{} to {}x{}, recreating",
                    buf_w,
                    buf_h,
                    width,
                    height
                );
                self.render_target = None;
                self.offscreen = None;
                self.brush_cache.clear();
            }
        }

        if self.render_target.is_some() && self.offscreen.is_some() {
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
        // Force recreate on next draw
        self.render_target = None;
        self.offscreen = None;
        self.brush_cache.clear();
        Ok(())
    }

    /// Handle resize
    pub fn handle_resize(&mut self) -> Result<(), Error> {
        // Force recreate on next draw - size will be checked in ensure_render_target
        self.render_target = None;
        self.offscreen = None;
        self.brush_cache.clear();
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

        let brush = unsafe { target.CreateSolidColorBrush(&d2d_color, None)? };

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

    /// End drawing and update the layered window
    pub fn end_draw(&self) -> Result<(), Error> {
        log!("end_draw() called");
        if let Some(ref target) = self.render_target {
            log!("  Calling target.EndDraw()...");
            unsafe {
                let result = target.EndDraw(None, None);
                log!("  EndDraw() result: {:?}", result);
                result?;
            }

            // Update the layered window with the offscreen buffer
            if let Some(ref offscreen) = self.offscreen {
                self.update_layered_window(offscreen)?;
            }
        } else {
            log!("  No render target, skipping EndDraw");
        }
        Ok(())
    }

    /// Update the layered window with per-pixel alpha
    fn update_layered_window(&self, offscreen: &OffscreenBuffer) -> Result<(), Error> {
        log!("update_layered_window() called");

        let (width, height) = offscreen.size();

        unsafe {
            let pt_src = POINT { x: 0, y: 0 };
            let size = SIZE {
                cx: width,
                cy: height,
            };

            // BLENDFUNCTION for per-pixel alpha
            let blend = windows::Win32::Graphics::Gdi::BLENDFUNCTION {
                BlendOp: windows::Win32::Graphics::Gdi::AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255, // Use per-pixel alpha, not constant
                AlphaFormat: windows::Win32::Graphics::Gdi::AC_SRC_ALPHA as u8,
            };

            let result = UpdateLayeredWindow(
                self.hwnd,
                HDC::default(), // Use screen DC
                None,           // Keep window position
                Some(&size),
                offscreen.dc(),
                Some(&pt_src),
                None, // No color key
                Some(&blend),
                ULW_ALPHA,
            );

            if result.is_err() {
                log!("  UpdateLayeredWindow failed: {:?}", result);
            } else {
                log!("  UpdateLayeredWindow succeeded");
            }
        }

        Ok(())
    }

    /// Clear the render target with a color (supports alpha for transparent background)
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

    /// Create a Direct2D bitmap from a loaded image
    pub fn create_bitmap(&self, image: &LoadedImage) -> Result<ID2D1Bitmap, Error> {
        let target = self
            .render_target
            .as_ref()
            .ok_or_else(|| Error::from_win32())?;

        image.create_d2d_bitmap(target)
    }

    /// Draw a bitmap at the specified position with opacity
    pub fn draw_bitmap(
        &self,
        bitmap: &ID2D1Bitmap,
        dest_rect: D2D_RECT_F,
        opacity: f32,
    ) -> Result<(), Error> {
        if let Some(ref target) = self.render_target {
            unsafe {
                target.DrawBitmap(
                    bitmap,
                    Some(&dest_rect),
                    opacity,
                    D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
                    None, // Use entire source bitmap
                );
            }
        }
        Ok(())
    }

    /// Draw a bitmap, stretching to fill the destination rectangle
    pub fn draw_bitmap_stretched(
        &self,
        bitmap: &ID2D1Bitmap,
        dest_rect: D2D_RECT_F,
        opacity: f32,
    ) -> Result<(), Error> {
        self.draw_bitmap(bitmap, dest_rect, opacity)
    }

    /// Draw a bitmap centered within the destination rectangle
    /// The bitmap maintains its aspect ratio and is centered
    pub fn draw_bitmap_centered(
        &self,
        bitmap: &ID2D1Bitmap,
        dest_rect: D2D_RECT_F,
        opacity: f32,
    ) -> Result<(), Error> {
        if let Some(ref target) = self.render_target {
            let size = unsafe { bitmap.GetSize() };
            let bitmap_width = size.width;
            let bitmap_height = size.height;

            let dest_width = dest_rect.right - dest_rect.left;
            let dest_height = dest_rect.bottom - dest_rect.top;

            // Calculate centered position
            let x_offset = (dest_width - bitmap_width) / 2.0;
            let y_offset = (dest_height - bitmap_height) / 2.0;

            let centered_rect = D2D_RECT_F {
                left: dest_rect.left + x_offset,
                top: dest_rect.top + y_offset,
                right: dest_rect.left + x_offset + bitmap_width,
                bottom: dest_rect.top + y_offset + bitmap_height,
            };

            unsafe {
                target.DrawBitmap(
                    bitmap,
                    Some(&centered_rect),
                    opacity,
                    D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
                    None,
                );
            }
        }
        Ok(())
    }

    /// Draw a bitmap covering the destination rectangle (may crop)
    /// The bitmap maintains aspect ratio and fills the entire dest_rect
    pub fn draw_bitmap_cover(
        &self,
        bitmap: &ID2D1Bitmap,
        dest_rect: D2D_RECT_F,
        opacity: f32,
    ) -> Result<(), Error> {
        if let Some(ref target) = self.render_target {
            let size = unsafe { bitmap.GetSize() };
            let bitmap_width = size.width;
            let bitmap_height = size.height;

            let dest_width = dest_rect.right - dest_rect.left;
            let dest_height = dest_rect.bottom - dest_rect.top;

            // Calculate scale to cover the destination
            let scale_x = dest_width / bitmap_width;
            let scale_y = dest_height / bitmap_height;
            let scale = scale_x.max(scale_y);

            // Calculate the source rect to crop
            let src_width = dest_width / scale;
            let src_height = dest_height / scale;

            // Center the crop
            let src_x = (bitmap_width - src_width) / 2.0;
            let src_y = (bitmap_height - src_height) / 2.0;

            let src_rect = D2D_RECT_F {
                left: src_x,
                top: src_y,
                right: src_x + src_width,
                bottom: src_y + src_height,
            };

            unsafe {
                target.DrawBitmap(
                    bitmap,
                    Some(&dest_rect),
                    opacity,
                    D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
                    Some(&src_rect),
                );
            }
        }
        Ok(())
    }

    /// Get render target (for advanced bitmap operations)
    pub fn render_target(&self) -> Option<&ID2D1DCRenderTarget> {
        self.render_target.as_ref()
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
