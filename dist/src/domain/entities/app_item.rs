//! AppItem entity - represents a launchable application
//!
//! This is a core domain entity representing an application entry
//! that can be launched by the user.

use std::path::PathBuf;

/// Unique identifier for an application
pub type AppId = String;

/// An application entry that can be launched
#[derive(Clone, Debug, PartialEq)]
pub struct AppItem {
    /// Unique identifier
    pub id: AppId,
    /// Display name of the application
    pub name: String,
    /// Optional description/subtitle
    pub description: Option<String>,
    /// Path to the executable
    pub path: PathBuf,
    /// Optional path to icon file
    pub icon_path: Option<PathBuf>,
    /// Working directory for launch
    pub working_dir: Option<PathBuf>,
    /// Command-line arguments
    pub arguments: Option<String>,
    /// Application category (for grouping)
    pub category: Option<String>,
    /// Keywords for search matching
    pub keywords: Vec<String>,
    /// Launch count (for history boosting)
    pub launch_count: u32,
    /// Last launch timestamp (Unix epoch)
    pub last_launched: Option<u64>,
}

impl AppItem {
    /// Create a new AppItem with minimal required fields
    pub fn new(name: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        let name = name.into();
        let path = path.into();
        let id = Self::generate_id(&name, &path);

        Self {
            id,
            name,
            description: None,
            path,
            icon_path: None,
            working_dir: None,
            arguments: None,
            category: None,
            keywords: Vec::new(),
            launch_count: 0,
            last_launched: None,
        }
    }

    /// Generate a unique ID from name and path
    fn generate_id(name: &str, path: &PathBuf) -> AppId {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        name.hash(&mut hasher);
        path.hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }

    /// Builder pattern: set description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Builder pattern: set icon path
    pub fn with_icon(mut self, icon_path: impl Into<PathBuf>) -> Self {
        self.icon_path = Some(icon_path.into());
        self
    }

    /// Builder pattern: set working directory
    pub fn with_working_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.working_dir = Some(dir.into());
        self
    }

    /// Builder pattern: set arguments
    pub fn with_arguments(mut self, args: impl Into<String>) -> Self {
        self.arguments = Some(args.into());
        self
    }

    /// Builder pattern: set category
    pub fn with_category(mut self, category: impl Into<String>) -> Self {
        self.category = Some(category.into());
        self
    }

    /// Builder pattern: add keywords
    pub fn with_keywords(mut self, keywords: Vec<String>) -> Self {
        self.keywords = keywords;
        self
    }

    /// Record a launch event
    pub fn record_launch(&mut self) {
        self.launch_count += 1;
        self.last_launched = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        );
    }

    /// Get all searchable text for this app
    pub fn searchable_text(&self) -> String {
        let mut parts = vec![self.name.clone()];
        if let Some(ref desc) = self.description {
            parts.push(desc.clone());
        }
        parts.extend(self.keywords.clone());
        parts.join(" ")
    }
}

impl Default for AppItem {
    fn default() -> Self {
        Self::new("Unknown", PathBuf::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_item_creation() {
        let app = AppItem::new("Chrome", "/path/to/chrome.exe");

        assert_eq!(app.name, "Chrome");
        assert_eq!(app.path, PathBuf::from("/path/to/chrome.exe"));
        assert!(!app.id.is_empty());
        assert_eq!(app.launch_count, 0);
    }

    #[test]
    fn test_app_item_builder() {
        let app = AppItem::new("VSCode", "/path/to/code.exe")
            .with_description("Visual Studio Code")
            .with_category("Development")
            .with_keywords(vec!["editor".to_string(), "ide".to_string()]);

        assert_eq!(app.description, Some("Visual Studio Code".to_string()));
        assert_eq!(app.category, Some("Development".to_string()));
        assert_eq!(app.keywords.len(), 2);
    }

    #[test]
    fn test_record_launch() {
        let mut app = AppItem::new("Test", "/test");

        assert_eq!(app.launch_count, 0);
        app.record_launch();
        assert_eq!(app.launch_count, 1);
        assert!(app.last_launched.is_some());
    }

    #[test]
    fn test_searchable_text() {
        let app = AppItem::new("Firefox", "/firefox")
            .with_description("Web Browser")
            .with_keywords(vec!["internet".to_string(), "web".to_string()]);

        let text = app.searchable_text();
        assert!(text.contains("Firefox"));
        assert!(text.contains("Web Browser"));
        assert!(text.contains("internet"));
    }
}
