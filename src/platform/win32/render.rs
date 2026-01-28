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
use crate::widget::CornerRadii;

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
        self.end_draw_with_opacity(1.0)
    }

    /// End drawing and present to screen with specified opacity (0.0 to 1.0)
    pub fn end_draw_with_opacity(&self, opacity: f32) -> Result<(), Error> {
        log!("end_draw_with_opacity({}) called", opacity);
        if let Some(ref target) = self.render_target {
            log!("  Calling target.EndDraw()...");
            unsafe {
                let result = target.EndDraw(None, None);
                log!("  EndDraw() result: {:?}", result);
                result?;
            }

            // Update the layered window with the offscreen buffer
            if let Some(ref offscreen) = self.offscreen {
                self.update_layered_window_with_opacity(offscreen, opacity)?;
            }
        } else {
            log!("  No render target, skipping EndDraw");
        }
        Ok(())
    }

    /// Update the layered window with per-pixel alpha
    fn update_layered_window(&self, offscreen: &OffscreenBuffer) -> Result<(), Error> {
        self.update_layered_window_with_opacity(offscreen, 1.0)
    }

    /// Update the layered window with per-pixel alpha and global opacity
    fn update_layered_window_with_opacity(
        &self,
        offscreen: &OffscreenBuffer,
        opacity: f32,
    ) -> Result<(), Error> {
        log!("update_layered_window_with_opacity({}) called", opacity);

        let (width, height) = offscreen.size();
        let alpha = (opacity.clamp(0.0, 1.0) * 255.0) as u8;

        unsafe {
            let pt_src = POINT { x: 0, y: 0 };
            let size = SIZE {
                cx: width,
                cy: height,
            };

            // BLENDFUNCTION for per-pixel alpha with global opacity
            let blend = windows::Win32::Graphics::Gdi::BLENDFUNCTION {
                BlendOp: windows::Win32::Graphics::Gdi::AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: alpha, // Global opacity (255 = fully opaque)
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
                log!("  UpdateLayeredWindow succeeded (alpha={})", alpha);
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

    /// Push an axis-aligned clip rectangle
    pub fn push_clip_rect(&mut self, rect: D2D_RECT_F) {
        if let Some(ref target) = self.render_target {
            unsafe {
                target.PushAxisAlignedClip(&rect, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
            }
        }
    }

    /// Pop the current clip
    pub fn pop_clip(&mut self) {
        if let Some(ref target) = self.render_target {
            unsafe {
                target.PopAxisAlignedClip();
            }
        }
    }

    /// Push a rounded rectangle clip using a geometry layer
    /// Returns a layer that must be popped with pop_layer()
    pub fn push_rounded_clip(
        &mut self,
        rect: D2D_RECT_F,
        radius_x: f32,
        radius_y: f32,
    ) -> Result<Option<ID2D1Layer>, Error> {
        if let Some(ref target) = self.render_target {
            unsafe {
                // Create a rounded rectangle geometry
                let rounded = D2D1_ROUNDED_RECT {
                    rect,
                    radiusX: radius_x,
                    radiusY: radius_y,
                };
                let geometry = self.factory.CreateRoundedRectangleGeometry(&rounded)?;

                // Create a layer
                let layer = target.CreateLayer(None)?;

                // Push the layer with the geometry as a mask
                let layer_params = D2D1_LAYER_PARAMETERS {
                    contentBounds: rect,
                    geometricMask: std::mem::transmute_copy(&geometry),
                    maskAntialiasMode: D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
                    maskTransform: windows::Foundation::Numerics::Matrix3x2::identity(),
                    opacity: 1.0,
                    opacityBrush: std::mem::ManuallyDrop::new(None),
                    layerOptions: D2D1_LAYER_OPTIONS_NONE,
                };
                target.PushLayer(&layer_params, &layer);

                return Ok(Some(layer));
            }
        }
        Ok(None)
    }

    /// Pop a layer pushed with push_rounded_clip()
    pub fn pop_layer(&mut self) {
        if let Some(ref target) = self.render_target {
            unsafe {
                target.PopLayer();
            }
        }
    }

    /// Create a path geometry for a rounded rectangle with per-corner radii
    fn create_rounded_rect_path(
        &self,
        rect: D2D_RECT_F,
        radii: CornerRadii,
    ) -> Result<ID2D1PathGeometry, Error> {
        unsafe {
            let path = self.factory.CreatePathGeometry()?;
            let sink = path.Open()?;

            let left = rect.left;
            let top = rect.top;
            let right = rect.right;
            let bottom = rect.bottom;

            // Start at top-left, after the top-left corner arc
            sink.BeginFigure(
                D2D_POINT_2F {
                    x: left + radii.top_left,
                    y: top,
                },
                D2D1_FIGURE_BEGIN_FILLED,
            );

            // Top edge to top-right corner
            sink.AddLine(D2D_POINT_2F {
                x: right - radii.top_right,
                y: top,
            });

            // Top-right corner
            if radii.top_right > 0.0 {
                sink.AddArc(&D2D1_ARC_SEGMENT {
                    point: D2D_POINT_2F {
                        x: right,
                        y: top + radii.top_right,
                    },
                    size: D2D_SIZE_F {
                        width: radii.top_right,
                        height: radii.top_right,
                    },
                    rotationAngle: 0.0,
                    sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
                    arcSize: D2D1_ARC_SIZE_SMALL,
                });
            } else {
                sink.AddLine(D2D_POINT_2F { x: right, y: top });
            }

            // Right edge to bottom-right corner
            sink.AddLine(D2D_POINT_2F {
                x: right,
                y: bottom - radii.bottom_right,
            });

            // Bottom-right corner
            if radii.bottom_right > 0.0 {
                sink.AddArc(&D2D1_ARC_SEGMENT {
                    point: D2D_POINT_2F {
                        x: right - radii.bottom_right,
                        y: bottom,
                    },
                    size: D2D_SIZE_F {
                        width: radii.bottom_right,
                        height: radii.bottom_right,
                    },
                    rotationAngle: 0.0,
                    sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
                    arcSize: D2D1_ARC_SIZE_SMALL,
                });
            } else {
                sink.AddLine(D2D_POINT_2F {
                    x: right,
                    y: bottom,
                });
            }

            // Bottom edge to bottom-left corner
            sink.AddLine(D2D_POINT_2F {
                x: left + radii.bottom_left,
                y: bottom,
            });

            // Bottom-left corner
            if radii.bottom_left > 0.0 {
                sink.AddArc(&D2D1_ARC_SEGMENT {
                    point: D2D_POINT_2F {
                        x: left,
                        y: bottom - radii.bottom_left,
                    },
                    size: D2D_SIZE_F {
                        width: radii.bottom_left,
                        height: radii.bottom_left,
                    },
                    rotationAngle: 0.0,
                    sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
                    arcSize: D2D1_ARC_SIZE_SMALL,
                });
            } else {
                sink.AddLine(D2D_POINT_2F { x: left, y: bottom });
            }

            // Left edge to top-left corner
            sink.AddLine(D2D_POINT_2F {
                x: left,
                y: top + radii.top_left,
            });

            // Top-left corner
            if radii.top_left > 0.0 {
                sink.AddArc(&D2D1_ARC_SEGMENT {
                    point: D2D_POINT_2F {
                        x: left + radii.top_left,
                        y: top,
                    },
                    size: D2D_SIZE_F {
                        width: radii.top_left,
                        height: radii.top_left,
                    },
                    rotationAngle: 0.0,
                    sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
                    arcSize: D2D1_ARC_SIZE_SMALL,
                });
            }

            sink.EndFigure(D2D1_FIGURE_END_CLOSED);
            sink.Close()?;

            Ok(path)
        }
    }

    /// Create a path geometry for a rounded rectangle with a diagonal edge on the right side
    /// The diagonal goes from (right, top) to (right - diagonal_offset, bottom)
    /// This creates the slanted wallpaper panel effect like HyDE's rofi style_11
    fn create_diagonal_rect_path(
        &self,
        rect: D2D_RECT_F,
        radii: CornerRadii,
        diagonal_offset: f32,
    ) -> Result<ID2D1PathGeometry, Error> {
        unsafe {
            let path = self.factory.CreatePathGeometry()?;
            let sink = path.Open()?;

            let left = rect.left;
            let top = rect.top;
            let right = rect.right;
            let bottom = rect.bottom;

            // The diagonal edge: top-right corner stays at (right, top)
            // bottom-right corner is at (right - diagonal_offset, bottom)
            let bottom_right_x = right - diagonal_offset;

            // Start at top-left, after the top-left corner arc
            sink.BeginFigure(
                D2D_POINT_2F {
                    x: left + radii.top_left,
                    y: top,
                },
                D2D1_FIGURE_BEGIN_FILLED,
            );

            // Top edge to top-right corner (full width at top)
            sink.AddLine(D2D_POINT_2F { x: right, y: top });

            // Diagonal edge from top-right down to bottom-right (narrower at bottom)
            sink.AddLine(D2D_POINT_2F {
                x: bottom_right_x,
                y: bottom,
            });

            // Bottom edge to bottom-left corner
            sink.AddLine(D2D_POINT_2F {
                x: left + radii.bottom_left,
                y: bottom,
            });

            // Bottom-left corner
            if radii.bottom_left > 0.0 {
                sink.AddArc(&D2D1_ARC_SEGMENT {
                    point: D2D_POINT_2F {
                        x: left,
                        y: bottom - radii.bottom_left,
                    },
                    size: D2D_SIZE_F {
                        width: radii.bottom_left,
                        height: radii.bottom_left,
                    },
                    rotationAngle: 0.0,
                    sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
                    arcSize: D2D1_ARC_SIZE_SMALL,
                });
            } else {
                sink.AddLine(D2D_POINT_2F { x: left, y: bottom });
            }

            // Left edge to top-left corner
            sink.AddLine(D2D_POINT_2F {
                x: left,
                y: top + radii.top_left,
            });

            // Top-left corner
            if radii.top_left > 0.0 {
                sink.AddArc(&D2D1_ARC_SEGMENT {
                    point: D2D_POINT_2F {
                        x: left + radii.top_left,
                        y: top,
                    },
                    size: D2D_SIZE_F {
                        width: radii.top_left,
                        height: radii.top_left,
                    },
                    rotationAngle: 0.0,
                    sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
                    arcSize: D2D1_ARC_SIZE_SMALL,
                });
            }

            sink.EndFigure(D2D1_FIGURE_END_CLOSED);
            sink.Close()?;

            Ok(path)
        }
    }

    /// Fill a rounded rectangle with per-corner radii
    pub fn fill_rounded_rect_corners(
        &mut self,
        rect: D2D_RECT_F,
        radii: CornerRadii,
        color: Color,
    ) -> Result<(), Error> {
        // If all corners are the same, use the simpler method
        if radii.is_uniform() {
            return self.fill_rounded_rect(rect, radii.top_left, radii.top_left, color);
        }

        let brush = self.get_brush(color)?;
        let path = self.create_rounded_rect_path(rect, radii)?;

        if let Some(ref target) = self.render_target {
            unsafe {
                target.FillGeometry(&path, &brush, None);
            }
        }
        Ok(())
    }

    /// Draw (stroke) a rounded rectangle border with per-corner radii
    pub fn draw_rounded_rect_corners(
        &mut self,
        rect: D2D_RECT_F,
        radii: CornerRadii,
        color: Color,
        stroke_width: f32,
    ) -> Result<(), Error> {
        // If all corners are the same, use the simpler method
        if radii.is_uniform() {
            return self.draw_rounded_rect(
                rect,
                radii.top_left,
                radii.top_left,
                color,
                stroke_width,
            );
        }

        let brush = self.get_brush(color)?;
        let path = self.create_rounded_rect_path(rect, radii)?;

        if let Some(ref target) = self.render_target {
            unsafe {
                target.DrawGeometry(&path, &brush, stroke_width, None);
            }
        }
        Ok(())
    }

    /// Push a rounded rectangle clip with per-corner radii using a geometry layer
    pub fn push_rounded_clip_corners(
        &mut self,
        rect: D2D_RECT_F,
        radii: CornerRadii,
    ) -> Result<Option<ID2D1Layer>, Error> {
        // If all corners are the same, use the simpler method
        if radii.is_uniform() {
            return self.push_rounded_clip(rect, radii.top_left, radii.top_left);
        }

        if let Some(ref target) = self.render_target {
            unsafe {
                let geometry = self.create_rounded_rect_path(rect, radii)?;
                let layer = target.CreateLayer(None)?;

                let layer_params = D2D1_LAYER_PARAMETERS {
                    contentBounds: rect,
                    geometricMask: std::mem::transmute_copy(&geometry),
                    maskAntialiasMode: D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
                    maskTransform: windows::Foundation::Numerics::Matrix3x2::identity(),
                    opacity: 1.0,
                    opacityBrush: std::mem::ManuallyDrop::new(None),
                    layerOptions: D2D1_LAYER_OPTIONS_NONE,
                };
                target.PushLayer(&layer_params, &layer);

                return Ok(Some(layer));
            }
        }
        Ok(None)
    }

    /// Push a diagonal clip for the wallpaper panel
    /// The diagonal goes from (right - diagonal_offset, top) to (right, bottom)
    pub fn push_diagonal_clip(
        &mut self,
        rect: D2D_RECT_F,
        radii: CornerRadii,
        diagonal_offset: f32,
    ) -> Result<Option<ID2D1Layer>, Error> {
        if diagonal_offset <= 0.0 {
            // No diagonal, use regular rounded clip
            return self.push_rounded_clip_corners(rect, radii);
        }

        if let Some(ref target) = self.render_target {
            unsafe {
                let geometry = self.create_diagonal_rect_path(rect, radii, diagonal_offset)?;
                let layer = target.CreateLayer(None)?;

                let layer_params = D2D1_LAYER_PARAMETERS {
                    contentBounds: rect,
                    geometricMask: std::mem::transmute_copy(&geometry),
                    maskAntialiasMode: D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
                    maskTransform: windows::Foundation::Numerics::Matrix3x2::identity(),
                    opacity: 1.0,
                    opacityBrush: std::mem::ManuallyDrop::new(None),
                    layerOptions: D2D1_LAYER_OPTIONS_NONE,
                };
                target.PushLayer(&layer_params, &layer);

                return Ok(Some(layer));
            }
        }
        Ok(None)
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
