//! Task panel widget for quick-launch PowerShell scripts
//!
//! Displays a collapsible sidebar with task icons and labels.
//! - Compact mode (unfocused): Icons only in a narrow strip
//! - Expanded mode (focused): Full sidebar with icons, labels, and expandable groups

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
    /// Whether the panel is in expanded mode (shows labels and sub-items)
    pub expanded: bool,
    /// Calculated item bounds for hit-testing (populated during render)
    pub item_states: Vec<TaskItemState>,
    /// Panel bounds (calculated during layout)
    pub panel_bounds: Rect,
    /// Pending selection: group index to select first task from after next render
    pub pending_select_first_task_in_group: Option<usize>,
    /// Notification badge count (PR reviews, etc.)
    pub notification_count: usize,
    /// Notification icon bounds for hit testing
    pub notification_bounds: Option<Rect>,
}

impl TaskPanelState {
    /// Create a new task panel state from configuration
    pub fn new(config: TasksConfig) -> Self {
        // Start with all groups collapsed - they expand when panel is focused
        let expanded_groups = vec![false; config.groups.len()];

        Self {
            config,
            expanded_groups,
            hovered_item: None,
            selected_item: None,
            focused: false,
            expanded: false,
            item_states: Vec::new(),
            panel_bounds: Rect::default(),
            pending_select_first_task_in_group: None,
            notification_count: 0,
            notification_bounds: None,
        }
    }

    /// Check if the panel has any tasks
    pub fn has_tasks(&self) -> bool {
        !self.config.groups.is_empty()
    }

    /// Check if panel is in expanded (full sidebar) mode
    pub fn is_expanded(&self) -> bool {
        self.expanded
    }

    /// Toggle a group's expanded state (accordion: only one open at a time)
    /// When opening a group, selects the first task in that group
    pub fn toggle_group(&mut self, group_index: usize) {
        if group_index < self.expanded_groups.len() {
            let is_currently_expanded = self.expanded_groups[group_index];

            // Close all groups first (accordion behavior)
            for expanded in &mut self.expanded_groups {
                *expanded = false;
            }

            // If the clicked group was closed, open it
            // If it was already open, it stays closed (we just closed everything)
            if !is_currently_expanded {
                self.expanded_groups[group_index] = true;
                // Mark that we want to select the first task after render
                self.pending_select_first_task_in_group = Some(group_index);
            }
        }
    }

    /// Apply pending selection (call after item_states is populated)
    pub fn apply_pending_selection(&mut self) {
        if let Some(group_index) = self.pending_select_first_task_in_group.take() {
            // Find the first task item for this group (not the header)
            for (idx, item_state) in self.item_states.iter().enumerate() {
                if item_state.group_index == group_index && !item_state.is_group_header {
                    self.selected_item = Some(idx);
                    break;
                }
            }
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
        self.expanded = false;
        for expanded in &mut self.expanded_groups {
            *expanded = false;
        }
    }

    /// Set focus and manage expanded state
    pub fn set_focus(&mut self, focused: bool) {
        self.focused = focused;

        if focused {
            // Expand the panel when focused
            self.expanded = true;
            // Always reset selection to first item when gaining focus
            if !self.item_states.is_empty() {
                self.selected_item = Some(0);
            }
        } else {
            // Collapse the panel when losing focus
            self.expanded = false;
            // Clear selection so next focus starts fresh
            self.selected_item = None;
            // Also collapse all groups
            for expanded in &mut self.expanded_groups {
                *expanded = false;
            }
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

    // === Colors ===
    /// Background color for the panel
    pub background_color: Color,
    /// Icon color (normal)
    pub icon_color: Color,
    /// Icon color (hovered/selected)
    pub icon_color_hover: Color,
    /// Group header icon color
    pub group_icon_color: Color,
    /// Text color for labels (normal)
    pub text_color: Color,
    /// Text color for labels (hovered/selected)
    pub text_color_hover: Color,
    /// Background color for selected/hovered items
    pub item_background_color: Color,
    /// Background color for selected item (accent color)
    pub selected_background_color: Color,
    /// Color for tree line connectors (├─ └─)
    pub tree_line_color: Color,
    /// Color for expand/collapse chevron
    pub chevron_color: Color,

    // === Typography ===
    /// Font family for icons (should be a NerdFont)
    pub icon_font_family: String,
    /// Font family for text labels
    pub text_font_family: String,
    /// Icon size in pixels
    pub icon_size: f32,
    /// Text size for labels
    pub text_size: f32,

    // === Dimensions ===
    /// Panel width when collapsed (compact mode)
    pub compact_width: f32,
    /// Panel width when expanded (full sidebar)
    pub expanded_width: f32,
    /// Height of each item row
    pub item_height: f32,
    /// Corner radius for item backgrounds
    pub item_corner_radius: f32,
    /// Padding around panel edges
    pub padding: f32,
    /// Spacing between groups
    pub group_spacing: f32,
    /// Spacing between items within a group
    pub item_spacing: f32,
    /// Border radius for the panel background
    pub border_radius: f32,
    /// Indent for sub-items (tree items)
    pub sub_item_indent: f32,

    // === Icons ===
    /// Chevron icon when group is collapsed (pointing down/right)
    pub chevron_collapsed: String,
    /// Chevron icon when group is expanded (pointing up)
    pub chevron_expanded: String,
    /// Tree branch character for middle items
    pub tree_branch: String,
    /// Tree branch character for last item
    pub tree_corner: String,
}

impl Default for TaskPanelStyle {
    fn default() -> Self {
        Self {
            enabled: true,
            position: TaskPanelPosition::Left,

            // Colors - dark theme inspired by screenshot
            background_color: Color::from_f32(0.1, 0.11, 0.14, 0.95), // Dark blue-gray
            icon_color: Color::from_f32(0.6, 0.63, 0.69, 1.0),        // Muted gray
            icon_color_hover: Color::from_f32(0.54, 0.71, 0.98, 1.0), // Blue accent #89b4fa
            group_icon_color: Color::from_f32(0.8, 0.84, 0.96, 1.0),  // Light gray
            text_color: Color::from_f32(0.8, 0.84, 0.96, 1.0),        // Light gray #cdd6f4
            text_color_hover: Color::from_f32(1.0, 1.0, 1.0, 1.0),    // White
            item_background_color: Color::from_f32(1.0, 1.0, 1.0, 0.08), // Subtle hover bg
            selected_background_color: Color::from_f32(0.22, 0.35, 0.55, 1.0), // Blue selection
            tree_line_color: Color::from_f32(0.4, 0.42, 0.48, 1.0),   // Muted gray for lines
            chevron_color: Color::from_f32(0.6, 0.63, 0.69, 1.0),     // Muted gray

            // Typography
            icon_font_family: "JetBrainsMono NF".to_string(),
            text_font_family: "Segoe UI".to_string(),
            icon_size: 20.0,
            text_size: 14.0,

            // Dimensions
            compact_width: 48.0,
            expanded_width: 220.0,
            item_height: 40.0,
            item_corner_radius: 8.0,
            padding: 8.0,
            group_spacing: 8.0,
            item_spacing: 2.0,
            border_radius: 12.0,
            sub_item_indent: 32.0,

            // Icons (using box-drawing and Nerd Font chevrons)
            chevron_collapsed: "\u{f078}".to_string(), // fa-chevron-down
            chevron_expanded: "\u{f077}".to_string(),  // fa-chevron-up
            tree_branch: "\u{251C}\u{2500}".to_string(), // ├─
            tree_corner: "\u{2514}\u{2500}".to_string(), // └─
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
                    expanded: true, // Note: this is ignored now, groups start collapsed
                    tasks: vec![
                        Task {
                            name: "Docker".to_string(),
                            icon: "\u{f308}".to_string(),
                            script: "docker.ps1".to_string(),
                            interactive: false,
                        },
                        Task {
                            name: "Server".to_string(),
                            icon: "\u{f6ff}".to_string(),
                            script: "server.ps1".to_string(),
                            interactive: false,
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
                        interactive: false,
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
        // All groups start collapsed now
        assert!(!state.expanded_groups[0]);
        assert!(!state.expanded_groups[1]);
        // Panel starts collapsed
        assert!(!state.expanded);
        assert!(!state.focused);
    }

    #[test]
    fn test_toggle_group() {
        let config = create_test_config();
        let mut state = TaskPanelState::new(config);

        // Initially all groups are collapsed
        assert!(!state.expanded_groups[0]);
        assert!(!state.expanded_groups[1]);

        // Toggle Dev open
        state.toggle_group(0);
        assert!(state.expanded_groups[0]);
        assert!(!state.expanded_groups[1]);

        // Toggle Dev closed
        state.toggle_group(0);
        assert!(!state.expanded_groups[0]);
        assert!(!state.expanded_groups[1]);

        // Toggle Dev open again
        state.toggle_group(0);
        assert!(state.expanded_groups[0]);
        assert!(!state.expanded_groups[1]);

        // Toggle System open - should close Dev (accordion behavior)
        state.toggle_group(1);
        assert!(!state.expanded_groups[0]); // Dev should now be closed
        assert!(state.expanded_groups[1]); // System should be open
    }

    #[test]
    fn test_focus_expands_panel() {
        let config = create_test_config();
        let mut state = TaskPanelState::new(config);

        // Panel starts collapsed
        assert!(!state.expanded);
        assert!(!state.focused);

        // Focus expands the panel
        state.set_focus(true);
        assert!(state.focused);
        assert!(state.expanded);

        // Unfocus collapses the panel and all groups
        state.toggle_group(0); // Open a group first
        assert!(state.expanded_groups[0]);

        state.set_focus(false);
        assert!(!state.focused);
        assert!(!state.expanded);
        assert!(!state.expanded_groups[0]); // Groups also collapsed
    }

    #[test]
    fn test_task_panel_style_default() {
        let style = TaskPanelStyle::default();
        assert!(style.enabled);
        assert_eq!(style.icon_size, 20.0);
        assert_eq!(style.compact_width, 48.0);
        assert_eq!(style.expanded_width, 220.0);
        assert_eq!(style.item_height, 40.0);
    }
}
