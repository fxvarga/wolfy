//! Task panel widget for quick-launch PowerShell scripts
//!
//! Displays collapsible groups of task icons overlaid on the wallpaper panel.

use crate::tasks::{TaskGroup, TaskItemState, TaskPanelPosition, TasksConfig};
use crate::theme::types::{Color, Rect};

/// Task panel runtime state
#[derive(Debug, Clone)]
pub struct TaskPanelState {
    /// The loaded configuration
    pub config: TasksConfig,
    /// Which groups are currently expanded (by group index)
    pub expanded_groups: Vec<bool>,
    /// Currently hovered item (for tooltip display)
    pub hovered_item: Option<usize>,
    /// Currently selected item (for keyboard navigation)
    pub selected_item: Option<usize>,
    /// Whether the task panel has keyboard focus
    pub focused: bool,
    /// Calculated item bounds for hit-testing (populated during render)
    pub item_states: Vec<TaskItemState>,
    /// Panel bounds (calculated during layout)
    pub panel_bounds: Rect,
}

impl TaskPanelState {
    /// Create a new task panel state from configuration
    pub fn new(config: TasksConfig) -> Self {
        let expanded_groups = config.groups.iter().map(|g| g.expanded).collect();

        Self {
            config,
            expanded_groups,
            hovered_item: None,
            selected_item: None,
            focused: false,
            item_states: Vec::new(),
            panel_bounds: Rect::default(),
        }
    }

    /// Check if the panel has any tasks
    pub fn has_tasks(&self) -> bool {
        !self.config.groups.is_empty()
    }

    /// Toggle a group's expanded state
    pub fn toggle_group(&mut self, group_index: usize) {
        if group_index < self.expanded_groups.len() {
            self.expanded_groups[group_index] = !self.expanded_groups[group_index];
        }
    }

    /// Get the task at a given item index
    pub fn get_task_at_index(
        &self,
        item_index: usize,
    ) -> Option<(&TaskGroup, Option<&crate::tasks::Task>)> {
        if item_index >= self.item_states.len() {
            return None;
        }

        let state = &self.item_states[item_index];
        let group = self.config.groups.get(state.group_index)?;

        if state.is_group_header {
            Some((group, None))
        } else {
            let task = state.task_index.and_then(|idx| group.tasks.get(idx));
            Some((group, task))
        }
    }

    /// Hit test a point and return the item index if hit
    pub fn hit_test(&self, x: f32, y: f32) -> Option<usize> {
        // First check if we're in the panel at all
        if !self.panel_bounds.contains(x, y) {
            return None;
        }

        // Check each item
        for (index, item) in self.item_states.iter().enumerate() {
            if item.bounds.contains(x, y) {
                return Some(index);
            }
        }

        None
    }

    /// Move selection up
    pub fn select_prev(&mut self) {
        if self.item_states.is_empty() {
            return;
        }
        match self.selected_item {
            Some(idx) if idx > 0 => self.selected_item = Some(idx - 1),
            Some(_) => self.selected_item = Some(self.item_states.len() - 1), // wrap to end
            None => self.selected_item = Some(self.item_states.len() - 1),
        }
    }

    /// Move selection down
    pub fn select_next(&mut self) {
        if self.item_states.is_empty() {
            return;
        }
        match self.selected_item {
            Some(idx) if idx < self.item_states.len() - 1 => self.selected_item = Some(idx + 1),
            Some(_) => self.selected_item = Some(0), // wrap to start
            None => self.selected_item = Some(0),
        }
    }

    /// Clear selection (when losing focus)
    pub fn clear_selection(&mut self) {
        self.selected_item = None;
        self.focused = false;
    }

    /// Set focus and select first item if nothing selected
    pub fn set_focus(&mut self, focused: bool) {
        self.focused = focused;
        if focused && self.selected_item.is_none() && !self.item_states.is_empty() {
            self.selected_item = Some(0);
        }
        if !focused {
            // Keep selection visible but mark as unfocused
        }
    }

    /// Get the tooltip text for the hovered item
    pub fn get_tooltip_text(&self) -> Option<String> {
        let hovered = self.hovered_item?;
        let (group, task) = self.get_task_at_index(hovered)?;

        if let Some(task) = task {
            Some(task.name.clone())
        } else {
            Some(format!(
                "{} (click to {})",
                group.name,
                if self
                    .expanded_groups
                    .get(self.item_states[hovered].group_index)
                    .copied()
                    .unwrap_or(false)
                {
                    "collapse"
                } else {
                    "expand"
                }
            ))
        }
    }
}

/// Task panel style configuration (from theme)
#[derive(Clone, Debug)]
pub struct TaskPanelStyle {
    /// Whether the task panel is enabled
    pub enabled: bool,
    /// Position within wallpaper panel
    pub position: TaskPanelPosition,
    /// Background color for the panel
    pub background_color: Color,
    /// Icon color (normal)
    pub icon_color: Color,
    /// Icon color (hovered)
    pub icon_color_hover: Color,
    /// Group header icon color
    pub group_icon_color: Color,
    /// Font family for icons (should be a NerdFont)
    pub font_family: String,
    /// Icon size
    pub icon_size: f32,
    /// Panel width (overrides config if set)
    pub width: f32,
    /// Padding around panel
    pub padding: f32,
    /// Spacing between groups
    pub group_spacing: f32,
    /// Spacing between tasks
    pub task_spacing: f32,
    /// Border radius for the panel
    pub border_radius: f32,
}

impl Default for TaskPanelStyle {
    fn default() -> Self {
        Self {
            enabled: true,
            position: TaskPanelPosition::Left,
            background_color: Color::from_f32(0.0, 0.0, 0.0, 0.3),
            icon_color: Color::from_f32(0.8, 0.84, 0.96, 1.0), // #cdd6f4
            icon_color_hover: Color::from_f32(0.54, 0.71, 0.98, 1.0), // #89b4fa
            group_icon_color: Color::from_f32(0.98, 0.49, 0.45, 1.0), // #f97e72
            font_family: "JetBrainsMono Nerd Font".to_string(),
            icon_size: 24.0,
            width: 48.0,
            padding: 8.0,
            group_spacing: 12.0,
            task_spacing: 4.0,
            border_radius: 8.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tasks::{Task, TaskGroup, TaskPanelSettings, TasksConfig};

    fn create_test_config() -> TasksConfig {
        TasksConfig {
            settings: TaskPanelSettings::default(),
            groups: vec![
                TaskGroup {
                    name: "Dev".to_string(),
                    icon: "\u{f121}".to_string(),
                    expanded: true,
                    tasks: vec![
                        Task {
                            name: "Docker".to_string(),
                            icon: "\u{f308}".to_string(),
                            script: "docker.ps1".to_string(),
                        },
                        Task {
                            name: "Server".to_string(),
                            icon: "\u{f6ff}".to_string(),
                            script: "server.ps1".to_string(),
                        },
                    ],
                },
                TaskGroup {
                    name: "System".to_string(),
                    icon: "\u{f013}".to_string(),
                    expanded: false,
                    tasks: vec![Task {
                        name: "Cleanup".to_string(),
                        icon: "\u{f1f8}".to_string(),
                        script: "cleanup.ps1".to_string(),
                    }],
                },
            ],
        }
    }

    #[test]
    fn test_task_panel_state_new() {
        let config = create_test_config();
        let state = TaskPanelState::new(config);

        assert!(state.has_tasks());
        assert_eq!(state.expanded_groups.len(), 2);
        assert!(state.expanded_groups[0]); // Dev is expanded
        assert!(!state.expanded_groups[1]); // System is collapsed
    }

    #[test]
    fn test_toggle_group() {
        let config = create_test_config();
        let mut state = TaskPanelState::new(config);

        assert!(state.expanded_groups[0]);
        state.toggle_group(0);
        assert!(!state.expanded_groups[0]);
        state.toggle_group(0);
        assert!(state.expanded_groups[0]);
    }

    #[test]
    fn test_task_panel_style_default() {
        let style = TaskPanelStyle::default();
        assert!(style.enabled);
        assert_eq!(style.icon_size, 24.0);
        assert_eq!(style.width, 48.0);
    }
}
