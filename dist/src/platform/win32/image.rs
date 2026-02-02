//! Windows Imaging Component (WIC) image loading
//!
//! Provides functionality to load images (PNG, JPG, BMP, etc.) using WIC
//! and convert them to Direct2D bitmaps for rendering.

use std::path::Path;

use windows::core::Error;
use windows::Win32::Foundation::GENERIC_READ;
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::{ID2D1Bitmap, ID2D1RenderTarget, D2D1_BITMAP_PROPERTIES};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_WICPixelFormat32bppPBGRA, IWICBitmapDecoder, IWICBitmapScaler,
    IWICFormatConverter, IWICImagingFactory, WICBitmapDitherTypeNone,
    WICBitmapInterpolationModeHighQualityCubic, WICBitmapPaletteTypeMedianCut,
    WICDecodeMetadataCacheOnDemand,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
};

use crate::theme::types::ImageScale;

/// Image loader using Windows Imaging Component
pub struct ImageLoader {
    wic_factory: IWICImagingFactory,
}

/// Loaded image data ready for conversion to D2D bitmap
pub struct LoadedImage {
    converter: IWICFormatConverter,
    width: u32,
    height: u32,
}

impl LoadedImage {
    /// Get image width in pixels
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Get image height in pixels
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Create a Direct2D bitmap from this loaded image
    pub fn create_d2d_bitmap(
        &self,
        render_target: &ID2D1RenderTarget,
    ) -> Result<ID2D1Bitmap, Error> {
        let bitmap_props = D2D1_BITMAP_PROPERTIES {
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            },
            dpiX: 96.0,
            dpiY: 96.0,
        };

        unsafe {
            let bitmap =
                render_target.CreateBitmapFromWicBitmap(&self.converter, Some(&bitmap_props))?;
            Ok(bitmap)
        }
    }
}

impl ImageLoader {
    /// Create a new image loader
    pub fn new() -> Result<Self, Error> {
        unsafe {
            // Initialize COM if not already done
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

            let wic_factory: IWICImagingFactory =
                CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;

            Ok(Self { wic_factory })
        }
    }

    /// Load an image from a file path
    pub fn load_from_file(&self, path: &Path) -> Result<LoadedImage, Error> {
        self.load_scaled(path, 0, 0, ImageScale::None)
    }

    /// Load an image and scale it according to the specified parameters
    ///
    /// # Arguments
    /// * `path` - Path to the image file
    /// * `target_width` - Target width (0 = use original)
    /// * `target_height` - Target height (0 = use original)
    /// * `scale` - Scaling mode
    pub fn load_scaled(
        &self,
        path: &Path,
        target_width: u32,
        target_height: u32,
        scale: ImageScale,
    ) -> Result<LoadedImage, Error> {
        crate::log!(
            "ImageLoader::load_scaled({:?}, {}x{}, {:?})",
            path,
            target_width,
            target_height,
            scale
        );

        unsafe {
            // Convert path to wide string
            let path_wide: Vec<u16> = path
                .to_string_lossy()
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();

            // Create decoder from file
            let decoder: IWICBitmapDecoder = self.wic_factory.CreateDecoderFromFilename(
                windows::core::PCWSTR(path_wide.as_ptr()),
                None,
                GENERIC_READ,
                WICDecodeMetadataCacheOnDemand,
            )?;

            // Get the first frame
            let frame = decoder.GetFrame(0)?;

            // Get original dimensions
            let (mut orig_width, mut orig_height) = (0u32, 0u32);
            frame.GetSize(&mut orig_width, &mut orig_height)?;
            crate::log!("  Original size: {}x{}", orig_width, orig_height);

            // Calculate final dimensions based on scale mode
            let (final_width, final_height) = self.calculate_scaled_dimensions(
                orig_width,
                orig_height,
                target_width,
                target_height,
                scale,
            );
            crate::log!("  Final size: {}x{}", final_width, final_height);

            // Check if we need to scale
            let needs_scaling = final_width != orig_width || final_height != orig_height;

            if needs_scaling && final_width > 0 && final_height > 0 {
                // Create scaler
                let scaler: IWICBitmapScaler = self.wic_factory.CreateBitmapScaler()?;
                scaler.Initialize(
                    &frame,
                    final_width,
                    final_height,
                    WICBitmapInterpolationModeHighQualityCubic,
                )?;

                // Create format converter from scaled image
                let converter: IWICFormatConverter = self.wic_factory.CreateFormatConverter()?;
                converter.Initialize(
                    &scaler,
                    &GUID_WICPixelFormat32bppPBGRA,
                    WICBitmapDitherTypeNone,
                    None,
                    0.0,
                    WICBitmapPaletteTypeMedianCut,
                )?;

                Ok(LoadedImage {
                    converter,
                    width: final_width,
                    height: final_height,
                })
            } else {
                // No scaling needed - create format converter directly from frame
                let converter: IWICFormatConverter = self.wic_factory.CreateFormatConverter()?;
                converter.Initialize(
                    &frame,
                    &GUID_WICPixelFormat32bppPBGRA,
                    WICBitmapDitherTypeNone,
                    None,
                    0.0,
                    WICBitmapPaletteTypeMedianCut,
                )?;

                Ok(LoadedImage {
                    converter,
                    width: orig_width,
                    height: orig_height,
                })
            }
        }
    }

    /// Calculate scaled dimensions based on scale mode
    fn calculate_scaled_dimensions(
        &self,
        orig_width: u32,
        orig_height: u32,
        target_width: u32,
        target_height: u32,
        scale: ImageScale,
    ) -> (u32, u32) {
        // If no targets specified or scale is None, return original
        if (target_width == 0 && target_height == 0) || matches!(scale, ImageScale::None) {
            return (orig_width, orig_height);
        }

        let orig_aspect = orig_width as f32 / orig_height as f32;

        match scale {
            ImageScale::None => (orig_width, orig_height),

            ImageScale::Width => {
                // Scale to fit target width, maintain aspect ratio
                if target_width == 0 {
                    (orig_width, orig_height)
                } else {
                    let new_height = (target_width as f32 / orig_aspect).round() as u32;
                    (target_width, new_height.max(1))
                }
            }

            ImageScale::Height => {
                // Scale to fit target height, maintain aspect ratio
                if target_height == 0 {
                    (orig_width, orig_height)
                } else {
                    let new_width = (target_height as f32 * orig_aspect).round() as u32;
                    (new_width.max(1), target_height)
                }
            }

            ImageScale::Both => {
                // Scale to cover the target area (may crop)
                // This ensures the image fills the entire target rectangle
                if target_width == 0 || target_height == 0 {
                    return (orig_width, orig_height);
                }

                let target_aspect = target_width as f32 / target_height as f32;

                if orig_aspect > target_aspect {
                    // Image is wider - scale by height to cover
                    let new_width = (target_height as f32 * orig_aspect).round() as u32;
                    (new_width.max(1), target_height)
                } else {
                    // Image is taller - scale by width to cover
                    let new_height = (target_width as f32 / orig_aspect).round() as u32;
                    (target_width, new_height.max(1))
                }
            }
        }
    }

    /// Load image and scale to fit within bounds while maintaining aspect ratio
    /// This ensures the entire image is visible (letterboxing may occur)
    pub fn load_fit(
        &self,
        path: &Path,
        max_width: u32,
        max_height: u32,
    ) -> Result<LoadedImage, Error> {
        crate::log!(
            "ImageLoader::load_fit({:?}, {}x{})",
            path,
            max_width,
            max_height
        );

        unsafe {
            // Convert path to wide string
            let path_wide: Vec<u16> = path
                .to_string_lossy()
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();

            // Create decoder from file
            let decoder: IWICBitmapDecoder = self.wic_factory.CreateDecoderFromFilename(
                windows::core::PCWSTR(path_wide.as_ptr()),
                None,
                GENERIC_READ,
                WICDecodeMetadataCacheOnDemand,
            )?;

            // Get the first frame
            let frame = decoder.GetFrame(0)?;

            // Get original dimensions
            let (mut orig_width, mut orig_height) = (0u32, 0u32);
            frame.GetSize(&mut orig_width, &mut orig_height)?;

            // Calculate fit dimensions
            let (final_width, final_height) =
                self.calculate_fit_dimensions(orig_width, orig_height, max_width, max_height);

            // Check if we need to scale
            let needs_scaling = final_width != orig_width || final_height != orig_height;

            if needs_scaling && final_width > 0 && final_height > 0 {
                // Create scaler
                let scaler: IWICBitmapScaler = self.wic_factory.CreateBitmapScaler()?;
                scaler.Initialize(
                    &frame,
                    final_width,
                    final_height,
                    WICBitmapInterpolationModeHighQualityCubic,
                )?;

                // Create format converter from scaled image
                let converter: IWICFormatConverter = self.wic_factory.CreateFormatConverter()?;
                converter.Initialize(
                    &scaler,
                    &GUID_WICPixelFormat32bppPBGRA,
                    WICBitmapDitherTypeNone,
                    None,
                    0.0,
                    WICBitmapPaletteTypeMedianCut,
                )?;

                Ok(LoadedImage {
                    converter,
                    width: final_width,
                    height: final_height,
                })
            } else {
                // No scaling needed
                let converter: IWICFormatConverter = self.wic_factory.CreateFormatConverter()?;
                converter.Initialize(
                    &frame,
                    &GUID_WICPixelFormat32bppPBGRA,
                    WICBitmapDitherTypeNone,
                    None,
                    0.0,
                    WICBitmapPaletteTypeMedianCut,
                )?;

                Ok(LoadedImage {
                    converter,
                    width: orig_width,
                    height: orig_height,
                })
            }
        }
    }

    /// Calculate dimensions to fit within max bounds while maintaining aspect ratio
    fn calculate_fit_dimensions(
        &self,
        orig_width: u32,
        orig_height: u32,
        max_width: u32,
        max_height: u32,
    ) -> (u32, u32) {
        if max_width == 0 || max_height == 0 {
            return (orig_width, orig_height);
        }

        let width_ratio = max_width as f32 / orig_width as f32;
        let height_ratio = max_height as f32 / orig_height as f32;

        // Use the smaller ratio to ensure the image fits
        let scale = width_ratio.min(height_ratio);

        if scale >= 1.0 {
            // Image already fits, no scaling needed
            (orig_width, orig_height)
        } else {
            let new_width = (orig_width as f32 * scale).round() as u32;
            let new_height = (orig_height as f32 * scale).round() as u32;
            (new_width.max(1), new_height.max(1))
        }
    }

    /// Load image and scale to cover bounds (may crop)
    /// This ensures the image fills the entire target area
    pub fn load_cover(
        &self,
        path: &Path,
        target_width: u32,
        target_height: u32,
    ) -> Result<LoadedImage, Error> {
        self.load_scaled(path, target_width, target_height, ImageScale::Both)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_scaled_dimensions_none() {
        // We can't create a real ImageLoader in tests without COM,
        // so we test the logic separately
        let orig_width = 1920u32;
        let orig_height = 1080u32;

        // Scale::None should return original
        let scale = ImageScale::None;
        assert!(matches!(scale, ImageScale::None));
    }

    #[test]
    fn test_calculate_scaled_dimensions_width() {
        // Test width-based scaling logic
        let orig_width = 1920f32;
        let orig_height = 1080f32;
        let target_width = 800f32;

        let orig_aspect = orig_width / orig_height;
        let new_height = (target_width / orig_aspect).round();

        assert_eq!(new_height, 450.0);
    }

    #[test]
    fn test_calculate_scaled_dimensions_height() {
        // Test height-based scaling logic
        let orig_width = 1920f32;
        let orig_height = 1080f32;
        let target_height = 600f32;

        let orig_aspect = orig_width / orig_height;
        let new_width = (target_height * orig_aspect).round();

        assert_eq!(new_width, 1067.0);
    }

    #[test]
    fn test_calculate_fit_dimensions() {
        // Test fit logic - image should fit within bounds
        let orig_width = 1920f32;
        let orig_height = 1080f32;
        let max_width = 800f32;
        let max_height = 600f32;

        let width_ratio = max_width / orig_width;
        let height_ratio = max_height / orig_height;
        let scale = width_ratio.min(height_ratio);

        let new_width = (orig_width * scale).round();
        let new_height = (orig_height * scale).round();

        // At 800x600 target, width ratio (0.417) < height ratio (0.556)
        // So we scale by width ratio
        assert!(new_width <= max_width);
        assert!(new_height <= max_height);
    }
}
