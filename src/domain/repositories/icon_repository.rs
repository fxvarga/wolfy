//! IconRepository - interface for icon loading
//!
//! This trait defines how to load and cache application icons.

use std::path::Path;

use crate::domain::errors::DomainError;

/// A loaded icon (platform-independent representation)
#[derive(Clone, Debug)]
pub struct IconData {
    /// Icon width in pixels
    pub width: u32,
    /// Icon height in pixels
    pub height: u32,
    /// RGBA pixel data (4 bytes per pixel)
    pub pixels: Vec<u8>,
}

impl IconData {
    /// Create a new icon from RGBA data
    pub fn new(width: u32, height: u32, pixels: Vec<u8>) -> Self {
        debug_assert_eq!(pixels.len(), (width * height * 4) as usize);
        Self {
            width,
            height,
            pixels,
        }
    }

    /// Create a placeholder icon (solid color)
    pub fn placeholder(width: u32, height: u32, r: u8, g: u8, b: u8) -> Self {
        let pixels = (0..width * height)
            .flat_map(|_| [r, g, b, 255])
            .collect();
        Self::new(width, height, pixels)
    }
}

/// Repository interface for icon data
pub trait IconRepository: Send + Sync {
    /// Load an icon from a file path
    fn load(&self, path: &Path) -> Result<IconData, DomainError>;

    /// Load an icon for an executable
    fn load_for_executable(&self, exe_path: &Path) -> Result<IconData, DomainError>;

    /// Get a default/placeholder icon
    fn default_icon(&self) -> IconData;

    /// Clear the icon cache
    fn clear_cache(&mut self);
}

/// A null implementation for testing
pub struct NullIconRepository;

impl IconRepository for NullIconRepository {
    fn load(&self, _path: &Path) -> Result<IconData, DomainError> {
        Ok(self.default_icon())
    }

    fn load_for_executable(&self, _exe_path: &Path) -> Result<IconData, DomainError> {
        Ok(self.default_icon())
    }

    fn default_icon(&self) -> IconData {
        IconData::placeholder(32, 32, 128, 128, 128)
    }

    fn clear_cache(&mut self) {}
}
