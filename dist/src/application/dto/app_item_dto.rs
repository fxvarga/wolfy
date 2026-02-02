//! AppItemDto - Data transfer object for application items

use crate::domain::entities::AppItem;

/// DTO for transferring application item data
#[derive(Clone, Debug)]
pub struct AppItemDto {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub path: String,
    pub icon_path: Option<String>,
    pub category: Option<String>,
    pub launch_count: u32,
}

impl From<&AppItem> for AppItemDto {
    fn from(item: &AppItem) -> Self {
        Self {
            id: item.id.clone(),
            name: item.name.clone(),
            description: item.description.clone(),
            path: item.path.to_string_lossy().to_string(),
            icon_path: item.icon_path.as_ref().map(|p| p.to_string_lossy().to_string()),
            category: item.category.clone(),
            launch_count: item.launch_count,
        }
    }
}

impl From<AppItem> for AppItemDto {
    fn from(item: AppItem) -> Self {
        Self::from(&item)
    }
}
