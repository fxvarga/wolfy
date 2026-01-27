//! Panel widget - background container with color or image

use std::cell::RefCell;
use std::path::Path;

use windows::Win32::Graphics::Direct2D::ID2D1Bitmap;

use crate::platform::win32::{get_wallpaper_path, ImageLoader, Renderer};
use crate::platform::Event;
use crate::theme::tree::ThemeTree;
use crate::theme::types::{Color, ImageScale, ImageSource, LayoutContext, Rect};

use super::base::{Constraints, LayoutProps, MeasuredSize};
use super::{EventResult, Widget, WidgetState, WidgetStyle};

/// A panel that displays a background color or image
pub struct Panel {
    /// Widget name (for theme lookups)
    name: String,
    /// Layout properties
    layout: LayoutProps,
    /// Visual style
    style: PanelStyle,
    /// Widget state
    state: WidgetState,
    /// Cached bitmap for background image (RefCell for interior mutability in render)
    cached_bitmap: RefCell<Option<CachedBitmap>>,
}

/// Cached bitmap with its source info for invalidation
struct CachedBitmap {
    bitmap: ID2D1Bitmap,
    source_path: String,
    width: u32,
    height: u32,
}

/// Style for panel widget
#[derive(Clone, Debug)]
pub struct PanelStyle {
    pub background_color: Color,
    pub background_image: Option<ImageSource>,
    pub border_color: Color,
    pub border_width: f32,
    pub border_radius: f32,
}

impl Default for PanelStyle {
    fn default() -> Self {
        Self {
            background_color: Color::TRANSPARENT,
            background_image: None,
            border_color: Color::TRANSPARENT,
            border_width: 0.0,
            border_radius: 0.0,
        }
    }
}

impl PanelStyle {
    /// Load style from theme for a named widget
    pub fn from_theme(theme: &ThemeTree, name: &str, state: Option<&str>) -> Self {
        let default = Self::default();
        Self {
            background_color: theme.get_color(
                name,
                state,
                "background-color",
                default.background_color,
            ),
            background_image: theme.get_image(name, state, "background-image"),
            border_color: theme.get_color(name, state, "border-color", default.border_color),
            border_width: theme.get_number(name, state, "border-width", default.border_width as f64)
                as f32,
            border_radius: theme.get_number(
                name,
                state,
                "border-radius",
                default.border_radius as f64,
            ) as f32,
        }
    }
}

impl Panel {
    /// Create a new panel with a name
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            layout: LayoutProps::default(),
            style: PanelStyle::default(),
            state: WidgetState::Normal,
            cached_bitmap: RefCell::new(None),
        }
    }

    /// Set background color
    pub fn with_background_color(mut self, color: Color) -> Self {
        self.style.background_color = color;
        self
    }

    /// Set background image
    pub fn with_background_image(mut self, image: ImageSource) -> Self {
        self.style.background_image = Some(image);
        // Clear cached bitmap when image changes
        *self.cached_bitmap.borrow_mut() = None;
        self
    }

    /// Set whether this panel expands to fill available space
    pub fn with_expand(mut self, expand: bool) -> Self {
        self.layout.expand = expand;
        self
    }

    /// Set fixed width
    pub fn with_fixed_width(mut self, width: f32) -> Self {
        self.layout.fixed_width = Some(width);
        self
    }

    /// Set fixed height
    pub fn with_fixed_height(mut self, height: f32) -> Self {
        self.layout.fixed_height = Some(height);
        self
    }

    /// Set style
    pub fn with_style(mut self, style: PanelStyle) -> Self {
        self.style = style;
        // Clear cached bitmap when style changes
        *self.cached_bitmap.borrow_mut() = None;
        self
    }

    /// Get the widget name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Load layout properties from theme
    pub fn load_from_theme(&mut self, theme: &ThemeTree) {
        self.layout.expand = theme.get_expand(&self.name, false);

        // Load fixed dimensions if specified
        if let Some(val) = theme.get_value(&self.name, None, "width") {
            if let Some(w) = val.as_number() {
                self.layout.fixed_width = Some(w as f32);
            }
        }
        if let Some(val) = theme.get_value(&self.name, None, "height") {
            if let Some(h) = val.as_number() {
                self.layout.fixed_height = Some(h as f32);
            }
        }

        // Load style
        self.style = PanelStyle::from_theme(theme, &self.name, None);
        // Clear cached bitmap when loading from theme
        *self.cached_bitmap.borrow_mut() = None;
    }

    /// Get the background image source (for loading)
    pub fn background_image(&self) -> Option<&ImageSource> {
        self.style.background_image.as_ref()
    }

    /// Invalidate the cached bitmap (call when window resizes)
    pub fn invalidate_bitmap(&self) {
        *self.cached_bitmap.borrow_mut() = None;
    }

    /// Resolve the image path, handling "auto" for wallpaper
    fn resolve_image_path(path: &str) -> Option<String> {
        if path.eq_ignore_ascii_case("auto") {
            // Get system wallpaper
            get_wallpaper_path().map(|p| p.to_string_lossy().into_owned())
        } else {
            Some(path.to_string())
        }
    }

    /// Load and cache the background bitmap
    fn ensure_bitmap(
        &self,
        renderer: &Renderer,
        rect_width: u32,
        rect_height: u32,
    ) -> Option<ID2D1Bitmap> {
        let image_source = self.style.background_image.as_ref()?;

        // Check if we have a valid cached bitmap
        {
            let cache = self.cached_bitmap.borrow();
            if let Some(ref cached) = *cache {
                // Check if cache is still valid (same path and reasonable size)
                if cached.source_path == image_source.path {
                    return Some(cached.bitmap.clone());
                }
            }
        }

        // Need to load the image
        let resolved_path = Self::resolve_image_path(&image_source.path)?;
        crate::log!(
            "Panel '{}' loading image: {} (resolved from {})",
            self.name,
            resolved_path,
            image_source.path
        );

        // Create image loader
        let loader = match ImageLoader::new() {
            Ok(l) => l,
            Err(e) => {
                crate::log!("Failed to create ImageLoader: {:?}", e);
                return None;
            }
        };

        // Load with appropriate scaling
        let loaded = match image_source.scale {
            ImageScale::None => loader.load_from_file(Path::new(&resolved_path)),
            ImageScale::Width => {
                loader.load_scaled(Path::new(&resolved_path), rect_width, 0, ImageScale::Width)
            }
            ImageScale::Height => loader.load_scaled(
                Path::new(&resolved_path),
                0,
                rect_height,
                ImageScale::Height,
            ),
            ImageScale::Both => {
                loader.load_cover(Path::new(&resolved_path), rect_width, rect_height)
            }
        };

        let loaded = match loaded {
            Ok(l) => l,
            Err(e) => {
                crate::log!("Failed to load image '{}': {:?}", resolved_path, e);
                return None;
            }
        };

        crate::log!("  Loaded image: {}x{}", loaded.width(), loaded.height());

        // Create D2D bitmap
        let bitmap = match renderer.create_bitmap(&loaded) {
            Ok(b) => b,
            Err(e) => {
                crate::log!("Failed to create D2D bitmap: {:?}", e);
                return None;
            }
        };

        // Cache it
        let cached = CachedBitmap {
            bitmap: bitmap.clone(),
            source_path: image_source.path.clone(),
            width: loaded.width(),
            height: loaded.height(),
        };
        *self.cached_bitmap.borrow_mut() = Some(cached);

        Some(bitmap)
    }
}

impl Widget for Panel {
    fn handle_event(&mut self, _event: &Event, _ctx: &LayoutContext) -> EventResult {
        // Panels don't handle events
        EventResult::none()
    }

    fn render(
        &self,
        renderer: &mut Renderer,
        rect: Rect,
        _ctx: &LayoutContext,
    ) -> Result<(), windows::core::Error> {
        use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;

        let bounds = D2D_RECT_F {
            left: rect.x,
            top: rect.y,
            right: rect.x + rect.width,
            bottom: rect.y + rect.height,
        };

        // Draw background image if present
        if self.style.background_image.is_some() {
            if let Some(bitmap) =
                self.ensure_bitmap(renderer, rect.width as u32, rect.height as u32)
            {
                // Draw the bitmap covering the entire panel
                renderer.draw_bitmap_cover(&bitmap, bounds, 1.0)?;
            }
        }

        // Draw background color (can overlay on top of image for tinting)
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

        // Draw border if present
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

        Ok(())
    }

    fn state(&self) -> WidgetState {
        self.state
    }

    fn set_state(&mut self, state: WidgetState) {
        self.state = state;
    }

    fn style(&self) -> &WidgetStyle {
        // Panels don't use WidgetStyle directly, return default
        static DEFAULT: std::sync::OnceLock<WidgetStyle> = std::sync::OnceLock::new();
        DEFAULT.get_or_init(WidgetStyle::default)
    }

    fn set_style(&mut self, _style: WidgetStyle) {
        // Panels use PanelStyle instead
    }

    fn measure(&self, constraints: Constraints, _ctx: &LayoutContext) -> MeasuredSize {
        // Use fixed dimensions if specified, otherwise use constraints
        let width = self.layout.fixed_width.unwrap_or(constraints.max.width);
        let height = self.layout.fixed_height.unwrap_or(constraints.max.height);

        MeasuredSize::new(
            width.min(constraints.max.width).max(constraints.min.width),
            height
                .min(constraints.max.height)
                .max(constraints.min.height),
        )
    }

    fn arrange(&mut self, _bounds: Rect, _ctx: &LayoutContext) {
        // Panels have no children to arrange
    }

    fn layout_props(&self) -> &LayoutProps {
        &self.layout
    }

    fn widget_name(&self) -> &str {
        &self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_panel_creation() {
        let panel = Panel::new("wallpaper-panel")
            .with_background_color(Color::rgb(30, 30, 30))
            .with_expand(true);

        assert_eq!(panel.name(), "wallpaper-panel");
        assert!(panel.layout.expand);
    }

    #[test]
    fn test_resolve_image_path() {
        // Non-auto paths should be returned as-is
        let path = Panel::resolve_image_path("/path/to/image.png");
        assert_eq!(path, Some("/path/to/image.png".to_string()));

        // "auto" resolves to None in test environment (no wallpaper available)
        // In real Windows environment it would return the wallpaper path
    }
}
