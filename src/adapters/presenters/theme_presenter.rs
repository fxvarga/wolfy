//! ThemePresenter - Formats theme data for UI components

use crate::domain::entities::Theme;
use crate::domain::value_objects::Color;

/// View model for theme colors
#[derive(Clone, Debug)]
pub struct ColorSchemeViewModel {
    pub background: Color,
    pub foreground: Color,
    pub accent: Color,
    pub selection_bg: Color,
    pub selection_fg: Color,
    pub border: Color,
    pub text_primary: Color,
    pub text_secondary: Color,
    pub text_muted: Color,
    pub error: Color,
    pub warning: Color,
    pub success: Color,
}

impl Default for ColorSchemeViewModel {
    fn default() -> Self {
        Self {
            background: Color::rgb(30, 30, 30),
            foreground: Color::WHITE,
            accent: Color::rgb(0, 120, 212),
            selection_bg: Color::rgb(0, 120, 212),
            selection_fg: Color::WHITE,
            border: Color::rgb(60, 60, 60),
            text_primary: Color::WHITE,
            text_secondary: Color::rgb(180, 180, 180),
            text_muted: Color::rgb(120, 120, 120),
            error: Color::rgb(220, 50, 47),
            warning: Color::rgb(203, 75, 22),
            success: Color::rgb(133, 153, 0),
        }
    }
}

/// View model for theme typography
#[derive(Clone, Debug)]
pub struct TypographyViewModel {
    pub font_family: String,
    pub font_size: f32,
    pub font_size_small: f32,
    pub font_size_large: f32,
    pub line_height: f32,
}

impl Default for TypographyViewModel {
    fn default() -> Self {
        Self {
            font_family: "Segoe UI".to_string(),
            font_size: 14.0,
            font_size_small: 12.0,
            font_size_large: 18.0,
            line_height: 1.4,
        }
    }
}

/// View model for theme spacing
#[derive(Clone, Debug)]
pub struct SpacingViewModel {
    pub padding_small: f32,
    pub padding_medium: f32,
    pub padding_large: f32,
    pub margin_small: f32,
    pub margin_medium: f32,
    pub margin_large: f32,
    pub border_radius: f32,
    pub border_width: f32,
}

impl Default for SpacingViewModel {
    fn default() -> Self {
        Self {
            padding_small: 4.0,
            padding_medium: 8.0,
            padding_large: 16.0,
            margin_small: 4.0,
            margin_medium: 8.0,
            margin_large: 16.0,
            border_radius: 4.0,
            border_width: 1.0,
        }
    }
}

/// Complete theme view model
#[derive(Clone, Debug, Default)]
pub struct ThemeViewModel {
    pub name: String,
    pub colors: ColorSchemeViewModel,
    pub typography: TypographyViewModel,
    pub spacing: SpacingViewModel,
}

/// Presenter for theme data
pub struct ThemePresenter {
    current: ThemeViewModel,
}

impl ThemePresenter {
    /// Create a new theme presenter
    pub fn new() -> Self {
        Self {
            current: ThemeViewModel::default(),
        }
    }

    /// Present a theme
    pub fn present_theme(&mut self, theme: &Theme) {
        self.current = ThemeViewModel {
            name: theme.name.clone(),
            colors: self.extract_colors(theme),
            typography: self.extract_typography(theme),
            spacing: self.extract_spacing(theme),
        };
    }

    /// Extract color scheme from theme
    fn extract_colors(&self, theme: &Theme) -> ColorSchemeViewModel {
        ColorSchemeViewModel {
            background: theme.get_color("window", "background-color", Color::rgb(30, 30, 30)),
            foreground: theme.get_color("window", "text-color", Color::WHITE),
            accent: theme.get_color("*", "accent-color", Color::rgb(0, 120, 212)),
            selection_bg: theme.get_color("element selected", "background-color", Color::rgb(0, 120, 212)),
            selection_fg: theme.get_color("element selected", "text-color", Color::WHITE),
            border: theme.get_color("*", "border-color", Color::rgb(60, 60, 60)),
            text_primary: theme.get_color("*", "text-color", Color::WHITE),
            text_secondary: theme.get_color("*", "text-color-secondary", Color::rgb(180, 180, 180)),
            text_muted: theme.get_color("*", "text-color-muted", Color::rgb(120, 120, 120)),
            error: theme.get_color("*", "error-color", Color::rgb(220, 50, 47)),
            warning: theme.get_color("*", "warning-color", Color::rgb(203, 75, 22)),
            success: theme.get_color("*", "success-color", Color::rgb(133, 153, 0)),
        }
    }

    /// Extract typography from theme
    fn extract_typography(&self, theme: &Theme) -> TypographyViewModel {
        TypographyViewModel {
            font_family: theme.get_string("*", "font-family", "Segoe UI"),
            font_size: theme.get_number("*", "font-size", 14.0) as f32,
            font_size_small: theme.get_number("*", "font-size-small", 12.0) as f32,
            font_size_large: theme.get_number("*", "font-size-large", 18.0) as f32,
            line_height: theme.get_number("*", "line-height", 1.4) as f32,
        }
    }

    /// Extract spacing from theme
    fn extract_spacing(&self, theme: &Theme) -> SpacingViewModel {
        SpacingViewModel {
            padding_small: theme.get_number("*", "padding-small", 4.0) as f32,
            padding_medium: theme.get_number("*", "padding", 8.0) as f32,
            padding_large: theme.get_number("*", "padding-large", 16.0) as f32,
            margin_small: theme.get_number("*", "margin-small", 4.0) as f32,
            margin_medium: theme.get_number("*", "margin", 8.0) as f32,
            margin_large: theme.get_number("*", "margin-large", 16.0) as f32,
            border_radius: theme.get_number("*", "border-radius", 4.0) as f32,
            border_width: theme.get_number("*", "border-width", 1.0) as f32,
        }
    }

    /// Get current view model
    pub fn current(&self) -> &ThemeViewModel {
        &self.current
    }

    /// Get colors
    pub fn colors(&self) -> &ColorSchemeViewModel {
        &self.current.colors
    }

    /// Get typography
    pub fn typography(&self) -> &TypographyViewModel {
        &self.current.typography
    }

    /// Get spacing
    pub fn spacing(&self) -> &SpacingViewModel {
        &self.current.spacing
    }
}

impl Default for ThemePresenter {
    fn default() -> Self {
        Self::new()
    }
}
