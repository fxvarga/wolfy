//! ThemeDto - Data transfer object for theme data

use crate::domain::entities::Theme;

/// DTO for transferring theme metadata
#[derive(Clone, Debug)]
pub struct ThemeDto {
    pub name: String,
    pub selector_count: usize,
}

impl From<&Theme> for ThemeDto {
    fn from(theme: &Theme) -> Self {
        Self {
            name: theme.name.clone(),
            selector_count: theme.selectors().count(),
        }
    }
}

impl From<Theme> for ThemeDto {
    fn from(theme: Theme) -> Self {
        Self::from(&theme)
    }
}
