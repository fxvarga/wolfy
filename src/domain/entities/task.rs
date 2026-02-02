//! Task entity - represents a runnable task/command
//!
//! Tasks are predefined commands that can be executed,
//! similar to VS Code tasks or npm scripts.

use std::collections::HashMap;

/// Unique identifier for a task
pub type TaskId = String;

/// A runnable task/command
#[derive(Clone, Debug, PartialEq)]
pub struct Task {
    /// Unique identifier
    pub id: TaskId,
    /// Display name
    pub name: String,
    /// Command to execute
    pub command: String,
    /// Optional arguments
    pub args: Vec<String>,
    /// Environment variables
    pub env: HashMap<String, String>,
    /// Working directory
    pub cwd: Option<String>,
    /// Whether to show terminal output
    pub show_output: bool,
    /// Whether task runs in background
    pub background: bool,
    /// Task group/category
    pub group: Option<String>,
    /// Optional icon name
    pub icon: Option<String>,
    /// Keyboard shortcut
    pub shortcut: Option<String>,
}

impl Task {
    /// Create a new task with minimal fields
    pub fn new(name: impl Into<String>, command: impl Into<String>) -> Self {
        let name = name.into();
        let id = name
            .to_lowercase()
            .replace(' ', "_")
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '_')
            .collect();

        Self {
            id,
            name,
            command: command.into(),
            args: Vec::new(),
            env: HashMap::new(),
            cwd: None,
            show_output: true,
            background: false,
            group: None,
            icon: None,
            shortcut: None,
        }
    }

    /// Builder: set arguments
    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }

    /// Builder: add environment variable
    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    /// Builder: set working directory
    pub fn with_cwd(mut self, cwd: impl Into<String>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }

    /// Builder: set show output flag
    pub fn with_show_output(mut self, show: bool) -> Self {
        self.show_output = show;
        self
    }

    /// Builder: set background flag
    pub fn with_background(mut self, background: bool) -> Self {
        self.background = background;
        self
    }

    /// Builder: set group
    pub fn with_group(mut self, group: impl Into<String>) -> Self {
        self.group = Some(group.into());
        self
    }

    /// Builder: set icon
    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Builder: set shortcut
    pub fn with_shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }

    /// Get the full command line to execute
    pub fn command_line(&self) -> String {
        if self.args.is_empty() {
            self.command.clone()
        } else {
            format!("{} {}", self.command, self.args.join(" "))
        }
    }
}

impl Default for Task {
    fn default() -> Self {
        Self::new("default", "echo hello")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_creation() {
        let task = Task::new("Build Project", "cargo build");

        assert_eq!(task.name, "Build Project");
        assert_eq!(task.command, "cargo build");
        assert_eq!(task.id, "build_project");
    }

    #[test]
    fn test_task_builder() {
        let task = Task::new("Test", "cargo test")
            .with_args(vec!["--release".to_string()])
            .with_cwd("/project")
            .with_group("build");

        assert_eq!(task.args, vec!["--release"]);
        assert_eq!(task.cwd, Some("/project".to_string()));
        assert_eq!(task.group, Some("build".to_string()));
    }

    #[test]
    fn test_command_line() {
        let task = Task::new("Build", "cargo")
            .with_args(vec!["build".to_string(), "--release".to_string()]);

        assert_eq!(task.command_line(), "cargo build --release");
    }
}
