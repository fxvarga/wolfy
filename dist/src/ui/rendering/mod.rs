//! UI Rendering - Render scene and primitives

use crate::domain::value_objects::{Color, Rect};

/// A drawable primitive
#[derive(Clone, Debug)]
pub enum Drawable {
    /// Filled rectangle
    Rect {
        rect: Rect,
        color: Color,
    },
    /// Rounded rectangle
    RoundedRect {
        rect: Rect,
        radius: f32,
        color: Color,
    },
    /// Rectangle outline
    RectOutline {
        rect: Rect,
        color: Color,
        stroke_width: f32,
    },
    /// Text
    Text {
        text: String,
        rect: Rect,
        font_family: String,
        font_size: f32,
        color: Color,
    },
    /// Image/texture
    Image {
        texture_id: u64,
        rect: Rect,
        opacity: f32,
    },
}

/// A render scene containing drawables
#[derive(Clone, Debug, Default)]
pub struct RenderScene {
    pub drawables: Vec<Drawable>,
    pub clips: Vec<Rect>,
}

impl RenderScene {
    /// Create a new empty scene
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a drawable
    pub fn add(&mut self, drawable: Drawable) {
        self.drawables.push(drawable);
    }

    /// Add a filled rectangle
    pub fn add_rect(&mut self, rect: Rect, color: Color) {
        self.add(Drawable::Rect { rect, color });
    }

    /// Add a rounded rectangle
    pub fn add_rounded_rect(&mut self, rect: Rect, radius: f32, color: Color) {
        self.add(Drawable::RoundedRect { rect, radius, color });
    }

    /// Add text
    pub fn add_text(
        &mut self,
        text: impl Into<String>,
        rect: Rect,
        font_family: impl Into<String>,
        font_size: f32,
        color: Color,
    ) {
        self.add(Drawable::Text {
            text: text.into(),
            rect,
            font_family: font_family.into(),
            font_size,
            color,
        });
    }

    /// Push a clip rectangle
    pub fn push_clip(&mut self, rect: Rect) {
        self.clips.push(rect);
    }

    /// Pop the current clip
    pub fn pop_clip(&mut self) {
        self.clips.pop();
    }

    /// Clear the scene
    pub fn clear(&mut self) {
        self.drawables.clear();
        self.clips.clear();
    }

    /// Get drawable count
    pub fn drawable_count(&self) -> usize {
        self.drawables.len()
    }
}

/// Scene builder for fluent API
pub struct SceneBuilder {
    scene: RenderScene,
}

impl SceneBuilder {
    pub fn new() -> Self {
        Self {
            scene: RenderScene::new(),
        }
    }

    pub fn rect(mut self, rect: Rect, color: Color) -> Self {
        self.scene.add_rect(rect, color);
        self
    }

    pub fn rounded_rect(mut self, rect: Rect, radius: f32, color: Color) -> Self {
        self.scene.add_rounded_rect(rect, radius, color);
        self
    }

    pub fn text(
        mut self,
        text: impl Into<String>,
        rect: Rect,
        font_family: impl Into<String>,
        font_size: f32,
        color: Color,
    ) -> Self {
        self.scene.add_text(text, rect, font_family, font_size, color);
        self
    }

    pub fn build(self) -> RenderScene {
        self.scene
    }
}

impl Default for SceneBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scene_builder() {
        let scene = SceneBuilder::new()
            .rect(Rect::from_pos_size(0.0, 0.0, 100.0, 100.0), Color::BLACK)
            .text("Hello", Rect::from_pos_size(10.0, 10.0, 80.0, 20.0), "Arial", 14.0, Color::WHITE)
            .build();

        assert_eq!(scene.drawable_count(), 2);
    }
}
