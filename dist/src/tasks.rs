//! Task runner configuration
//!
//! Loads task definitions from tasks.toml for quick-launch shortcuts

use serde::Deserialize;
use std::fs;
use std::path::Path;

use crate::log;
use crate::theme::types::Rect;

/// Position of the task panel within the wallpaper panel
#[derive(Debug, Clone, Copy, PartialEq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskPanelPosition {
    #[default]
    Left,
    Right,
}

/// Root configuration structure
#[derive(Debug, Clone, Deserialize)]
pub struct TasksConfig {
    #[serde(default)]
    pub settings: TaskPanelSettings,
    #[serde(default)]
    pub groups: Vec<TaskGroup>,
}

impl Default for TasksConfig {
    fn default() -> Self {
        Self {
            settings: TaskPanelSettings::default(),
            groups: Vec::new(),
        }
    }
}

impl TasksConfig {
    /// Find a task by group name and task name
    pub fn find_task(&self, group_name: &str, task_name: &str) -> Option<&Task> {
        for group in &self.groups {
            if group.name == group_name {
                for task in &group.tasks {
                    if task.name == task_name {
                        return Some(task);
                    }
                }
            }
        }
        None
    }
}

/// Panel-level settings
#[derive(Debug, Clone, Deserialize)]
pub struct TaskPanelSettings {
    /// Position within wallpaper panel (left or right)
    #[serde(default)]
    pub position: TaskPanelPosition,
    /// Panel width in pixels
    #[serde(default = "default_panel_width")]
    pub width: f32,
    /// Icon size in pixels
    #[serde(default = "default_icon_size")]
    pub icon_size: f32,
    /// Spacing between groups
    #[serde(default = "default_group_spacing")]
    pub group_spacing: f32,
    /// Spacing between tasks within a group
    #[serde(default = "default_task_spacing")]
    pub task_spacing: f32,
    /// Padding around the panel
    #[serde(default = "default_padding")]
    pub padding: f32,
}

fn default_panel_width() -> f32 {
    48.0
}
fn default_icon_size() -> f32 {
    24.0
}
fn default_group_spacing() -> f32 {
    12.0
}
fn default_task_spacing() -> f32 {
    4.0
}
fn default_padding() -> f32 {
    8.0
}

impl Default for TaskPanelSettings {
    fn default() -> Self {
        Self {
            position: TaskPanelPosition::Left,
            width: default_panel_width(),
            icon_size: default_icon_size(),
            group_spacing: default_group_spacing(),
            task_spacing: default_task_spacing(),
            padding: default_padding(),
        }
    }
}

/// A group of related tasks
#[derive(Debug, Clone, Deserialize)]
pub struct TaskGroup {
    /// Group name (shown in tooltip)
    pub name: String,
    /// NerdFont icon for the group header
    #[serde(default = "default_group_icon")]
    pub icon: String,
    /// Whether group starts expanded
    #[serde(default = "default_expanded")]
    pub expanded: bool,
    /// Tasks in this group
    #[serde(default)]
    pub tasks: Vec<Task>,
}

fn default_group_icon() -> String {
    "\u{f07b}".to_string()
} // folder icon
fn default_expanded() -> bool {
    true
}

/// A single task/shortcut
#[derive(Debug, Clone, Deserialize)]
pub struct Task {
    /// Task name (shown in tooltip)
    pub name: String,
    /// NerdFont icon character
    #[serde(default = "default_task_icon")]
    pub icon: String,
    /// PowerShell script path or command
    pub script: String,
    /// Whether this task is interactive (uses PTY terminal instead of log file)
    #[serde(default)]
    pub interactive: bool,
}

fn default_task_icon() -> String {
    "\u{f489}".to_string()
} // terminal icon

/// Runtime state for a task item (includes calculated bounds for hit testing)
#[derive(Debug, Clone)]
pub struct TaskItemState {
    /// Reference to the task definition
    pub group_index: usize,
    pub task_index: Option<usize>, // None = group header
    /// Calculated bounds for hit testing
    pub bounds: Rect,
    /// Whether this is a group header
    pub is_group_header: bool,
}

/// Load tasks configuration from file
pub fn load_tasks_config(path: &Path) -> TasksConfig {
    match fs::read_to_string(path) {
        Ok(content) => match toml::from_str(&content) {
            Ok(config) => {
                log!("Loaded tasks config from {:?}", path);
                config
            }
            Err(e) => {
                log!("Failed to parse tasks.toml: {}", e);
                TasksConfig::default()
            }
        },
        Err(e) => {
            log!("Failed to read tasks.toml: {} (using empty config)", e);
            TasksConfig::default()
        }
    }
}

/// Find tasks.toml in standard locations (user config first, then exe dir)
pub fn find_tasks_config() -> Option<std::path::PathBuf> {
    // Check user config directory first
    if let Some(config_dir) = dirs::config_dir() {
        let config_path = config_dir.join("wolfy").join("tasks.toml");
        log!(
            "find_tasks_config: checking config {:?} exists={}",
            config_path,
            config_path.exists()
        );
        if config_path.exists() {
            return Some(config_path);
        }
    }

    // Check next to executable
    if let Ok(exe_path) = std::env::current_exe() {
        log!("find_tasks_config: exe_path={:?}", exe_path);
        if let Some(exe_dir) = exe_path.parent() {
            let config_path = exe_dir.join("tasks.toml");
            log!(
                "find_tasks_config: checking {:?} exists={}",
                config_path,
                config_path.exists()
            );
            if config_path.exists() {
                return Some(config_path);
            }
        }
    }

    // Check current directory
    let cwd_path = Path::new("tasks.toml");
    log!(
        "find_tasks_config: checking cwd {:?} exists={}",
        cwd_path,
        cwd_path.exists()
    );
    if cwd_path.exists() {
        return Some(cwd_path.to_path_buf());
    }

    log!("find_tasks_config: no tasks.toml found");
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_tasks_config() {
        // Note: TOML uses \uXXXX (no braces), not Rust's \u{XXXX}
        let toml_content = r#"
[settings]
position = "left"
width = 56
icon_size = 28

[[groups]]
name = "Dev"
icon = "\uf121"
expanded = true

[[groups.tasks]]
name = "Start Docker"
icon = "\uf308"
script = "C:/scripts/docker.ps1"

[[groups.tasks]]
name = "Dev Server"
icon = "\uf6ff"
script = "npm run dev"

[[groups]]
name = "System"
icon = "\uf013"
expanded = false

[[groups.tasks]]
name = "Clean Temp"
icon = "\uf1f8"
script = "C:/scripts/cleanup.ps1"
"#;

        let config: TasksConfig = toml::from_str(toml_content).unwrap();

        assert_eq!(config.settings.width, 56.0);
        assert_eq!(config.settings.icon_size, 28.0);
        assert_eq!(config.groups.len(), 2);
        assert_eq!(config.groups[0].name, "Dev");
        assert!(config.groups[0].expanded);
        assert_eq!(config.groups[0].tasks.len(), 2);
        assert_eq!(config.groups[0].tasks[0].name, "Start Docker");
        assert_eq!(config.groups[1].name, "System");
        assert!(!config.groups[1].expanded);
    }

    #[test]
    fn test_default_config() {
        let config = TasksConfig::default();
        assert_eq!(config.settings.width, 48.0);
        assert_eq!(config.groups.len(), 0);
    }
}
