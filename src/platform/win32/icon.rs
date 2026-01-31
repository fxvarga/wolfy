//! Windows icon extraction and caching
//!
//! Extracts icons from executable files using Windows Shell API
//! and converts them to Direct2D bitmaps for rendering.

use std::collections::HashMap;
use std::sync::Arc;

use windows::core::Error;
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::{ID2D1Bitmap, ID2D1RenderTarget, D2D1_BITMAP_PROPERTIES};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, SelectObject, BITMAPINFO,
    BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HDC,
};
use windows::Win32::UI::Controls::IImageList;
use windows::Win32::UI::Shell::{
    SHGetFileInfoW, SHGetImageList, SHFILEINFOW, SHGFI_SYSICONINDEX, SHIL_JUMBO,
};
use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, HICON, ICONINFO};

/// Size of jumbo icons (256x256)
const ICON_SIZE: u32 = 256;

/// Icon cache entry
pub struct CachedIcon {
    /// The D2D bitmap for rendering
    pub bitmap: ID2D1Bitmap,
    /// Original icon width
    pub width: u32,
    /// Original icon height
    pub height: u32,
}

/// Icon loader and cache
pub struct IconLoader {
    /// Cache of path -> icon bitmap
    cache: HashMap<String, Arc<CachedIcon>>,
}

impl IconLoader {
    /// Create a new icon loader
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    /// Get an icon for a file path, loading and caching if needed
    pub fn get_icon(
        &mut self,
        path: &str,
        render_target: &ID2D1RenderTarget,
    ) -> Option<Arc<CachedIcon>> {
        // Check cache first
        if let Some(cached) = self.cache.get(path) {
            return Some(cached.clone());
        }

        // Try to load the icon
        match self.load_icon(path, render_target) {
            Ok(icon) => {
                let icon = Arc::new(icon);
                self.cache.insert(path.to_string(), icon.clone());
                Some(icon)
            }
            Err(e) => {
                log!("IconLoader: Failed to load icon for '{}': {:?}", path, e);
                None
            }
        }
    }

    /// Load an icon from a file path
    fn load_icon(
        &self,
        path: &str,
        render_target: &ID2D1RenderTarget,
    ) -> Result<CachedIcon, Error> {
        log!("IconLoader::load_icon('{}')", path);

        // Skip problematic file types that can cause crashes
        let path_lower = path.to_lowercase();
        if path_lower.ends_with(".url") {
            log!("  Skipping icon for .url file (known to cause crashes)");
            return Err(Error::from_win32());
        }

        // Convert path to wide string
        let path_wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();

        unsafe {
            // First, get the icon index using SHGetFileInfo
            let mut file_info = SHFILEINFOW::default();
            let result = SHGetFileInfoW(
                windows::core::PCWSTR(path_wide.as_ptr()),
                windows::Win32::Storage::FileSystem::FILE_ATTRIBUTE_NORMAL,
                Some(&mut file_info),
                std::mem::size_of::<SHFILEINFOW>() as u32,
                SHGFI_SYSICONINDEX,
            );

            if result == 0 {
                log!("  SHGetFileInfoW failed");
                return Err(Error::from_win32());
            }

            let icon_index = file_info.iIcon as i32;
            log!("  Icon index: {}", icon_index);

            // Get the jumbo (256x256) image list
            let image_list: IImageList = SHGetImageList(SHIL_JUMBO as i32)?;
            log!("  Got jumbo image list");

            // Get the icon from the image list
            let hicon = image_list.GetIcon(icon_index, 0)?;
            if hicon.is_invalid() {
                log!("  Got invalid HICON from image list");
                return Err(Error::from_win32());
            }

            log!("  Got HICON: {:?}", hicon);

            // Convert HICON to D2D bitmap
            let result = self.hicon_to_bitmap(hicon, render_target);

            // Clean up the icon
            let _ = DestroyIcon(hicon);

            result
        }
    }

    /// Convert an HICON to a D2D bitmap
    fn hicon_to_bitmap(
        &self,
        hicon: HICON,
        render_target: &ID2D1RenderTarget,
    ) -> Result<CachedIcon, Error> {
        unsafe {
            // Get icon info to access the bitmap
            let mut icon_info = ICONINFO::default();
            if GetIconInfo(hicon, &mut icon_info).is_err() {
                log!("  GetIconInfo failed");
                return Err(Error::from_win32());
            }

            // We need to get the icon dimensions and pixel data
            // The icon has a color bitmap (hbmColor) and a mask bitmap (hbmMask)
            let hbm_color = icon_info.hbmColor;
            let hbm_mask = icon_info.hbmMask;

            log!("  hbmColor={:?}, hbmMask={:?}", hbm_color, hbm_mask);

            // Check if we have a valid color bitmap
            // Note: is_invalid() checks for null/0, but some icons may have
            // a non-null but still problematic bitmap handle
            if hbm_color.is_invalid() || hbm_color.0 == std::ptr::null_mut() {
                log!("  hbmColor is invalid, cannot extract icon pixels");
                if !hbm_mask.is_invalid() {
                    let _ = DeleteObject(hbm_mask);
                }
                return Err(Error::from_win32());
            }

            // Create a memory DC
            let hdc = CreateCompatibleDC(HDC::default());
            if hdc.is_invalid() {
                if !hbm_color.is_invalid() {
                    let _ = DeleteObject(hbm_color);
                }
                if !hbm_mask.is_invalid() {
                    let _ = DeleteObject(hbm_mask);
                }
                log!("  CreateCompatibleDC failed");
                return Err(Error::from_win32());
            }

            // Get bitmap info for the color bitmap
            let mut bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: 0,
                    biHeight: 0,
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

            // First call to get dimensions
            let lines = GetDIBits(hdc, hbm_color, 0, 0, None, &mut bmi, DIB_RGB_COLORS);

            if lines == 0 && bmi.bmiHeader.biWidth == 0 {
                // Fallback: use a default size
                bmi.bmiHeader.biWidth = ICON_SIZE as i32;
                bmi.bmiHeader.biHeight = -(ICON_SIZE as i32); // Top-down
            }

            let width = bmi.bmiHeader.biWidth.unsigned_abs();
            let height = bmi.bmiHeader.biHeight.unsigned_abs();

            log!("  Icon dimensions: {}x{}", width, height);

            // Prepare for getting pixel data - request top-down DIB
            bmi.bmiHeader.biHeight = -(height as i32); // Negative for top-down
            bmi.bmiHeader.biBitCount = 32;
            bmi.bmiHeader.biCompression = BI_RGB.0;
            bmi.bmiHeader.biSizeImage = width * height * 4;

            // Allocate buffer for pixel data
            let mut pixels: Vec<u8> = vec![0u8; (width * height * 4) as usize];

            // Select the color bitmap into DC and get its bits
            let old_bmp = SelectObject(hdc, hbm_color);

            let lines = GetDIBits(
                hdc,
                hbm_color,
                0,
                height,
                Some(pixels.as_mut_ptr() as *mut _),
                &mut bmi,
                DIB_RGB_COLORS,
            );

            SelectObject(hdc, old_bmp);

            if lines == 0 {
                log!("  GetDIBits failed to get pixel data");
                DeleteDC(hdc);
                if !hbm_color.is_invalid() {
                    let _ = DeleteObject(hbm_color);
                }
                if !hbm_mask.is_invalid() {
                    let _ = DeleteObject(hbm_mask);
                }
                return Err(Error::from_win32());
            }

            log!("  Got {} scanlines of pixel data", lines);

            // Windows 32-bit icons with alpha are ALREADY premultiplied
            // We only need to handle icons without alpha (using the mask)
            // Some icons have all-zero alpha, in which case we should use the mask
            let has_alpha = pixels.iter().skip(3).step_by(4).any(|&a| a > 0);

            if !has_alpha {
                log!("  Icon has no alpha channel, using mask");
                // Get mask bitmap data
                let mut mask_pixels: Vec<u8> = vec![0u8; (width * height * 4) as usize];

                bmi.bmiHeader.biHeight = -(height as i32);
                let old_bmp = SelectObject(hdc, hbm_mask);
                let mask_lines = GetDIBits(
                    hdc,
                    hbm_mask,
                    0,
                    height,
                    Some(mask_pixels.as_mut_ptr() as *mut _),
                    &mut bmi,
                    DIB_RGB_COLORS,
                );
                SelectObject(hdc, old_bmp);

                if mask_lines > 0 {
                    // Apply mask: where mask is black (0), pixel is opaque
                    for i in (0..pixels.len()).step_by(4) {
                        // Mask is monochrome expanded to 32bpp
                        // Black in mask = visible, White = transparent
                        let mask_val = mask_pixels.get(i).copied().unwrap_or(255);
                        pixels[i + 3] = if mask_val == 0 { 255 } else { 0 };
                    }
                } else {
                    // No valid mask, assume fully opaque
                    for i in (0..pixels.len()).step_by(4) {
                        pixels[i + 3] = 255;
                    }
                }

                // Only premultiply for mask-based icons (non-alpha icons)
                // since we just set the alpha channel ourselves
                for i in (0..pixels.len()).step_by(4) {
                    let a = pixels[i + 3] as f32 / 255.0;
                    pixels[i] = (pixels[i] as f32 * a) as u8; // B
                    pixels[i + 1] = (pixels[i + 1] as f32 * a) as u8; // G
                    pixels[i + 2] = (pixels[i + 2] as f32 * a) as u8; // R
                }
            }
            // Icons with alpha channel are already premultiplied by Windows

            // Clean up GDI objects
            DeleteDC(hdc);
            if !hbm_color.is_invalid() {
                let _ = DeleteObject(hbm_color);
            }
            if !hbm_mask.is_invalid() {
                let _ = DeleteObject(hbm_mask);
            }

            // Create D2D bitmap
            let bitmap_props = D2D1_BITMAP_PROPERTIES {
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                dpiX: 96.0,
                dpiY: 96.0,
            };

            let size = windows::Win32::Graphics::Direct2D::Common::D2D_SIZE_U { width, height };

            let bitmap = render_target.CreateBitmap(
                size,
                Some(pixels.as_ptr() as *const _),
                width * 4, // stride
                &bitmap_props,
            )?;

            log!("  Created D2D bitmap successfully");

            Ok(CachedIcon {
                bitmap,
                width,
                height,
            })
        }
    }

    /// Clear the icon cache (call when render target is recreated)
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }

    /// Get the number of cached icons
    pub fn cache_size(&self) -> usize {
        self.cache.len()
    }
}

impl Default for IconLoader {
    fn default() -> Self {
        Self::new()
    }
}
