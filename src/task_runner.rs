//! Background task runner with output capture
//!
//! Runs PowerShell tasks in the background, captures output to files,
//! and provides status tracking for visual indicators.
//!
//! Supports two modes:
//! - File-based: Output captured to log files (default)
//! - Interactive: PTY-based terminal for shell access

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Instant;

use crate::log;
use crate::pty::Pty;
use crate::terminal::{Terminal, TerminalColors, TerminalConfig};
use crate::theme::ThemeTree;

/// Status of a running task
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TaskStatus {
    Running,
    Completed,
    Failed,
}

/// A task that is or was running
pub struct RunningTask {
    /// Task name
    pub name: String,
    /// Group name
    pub group: String,
    /// Script that was run
    pub script: String,
    /// Child process handle (None if completed)
    child: Option<Child>,
    /// Output file path
    pub output_file: PathBuf,
    /// When the task started
    pub started_at: Instant,
    /// Current status
    pub status: TaskStatus,
}

/// Manages background task execution
pub struct TaskRunner {
    /// Running/completed tasks, keyed by "group:name"
    tasks: HashMap<String, RunningTask>,
    /// Active interactive terminals, keyed by "group:name"
    interactive_terminals: HashMap<String, Terminal>,
    /// Directory for output files
    output_dir: PathBuf,
}

impl TaskRunner {
    /// Create a new task runner
    pub fn new() -> Self {
        // Use %TEMP%\wolfy_tasks for output files
        let output_dir = std::env::temp_dir().join("wolfy_tasks");

        // Create directory if it doesn't exist
        let _ = fs::create_dir_all(&output_dir);

        log!("TaskRunner created, output dir: {:?}", output_dir);

        Self {
            tasks: HashMap::new(),
            interactive_terminals: HashMap::new(),
            output_dir,
        }
    }

    /// Clean up old output files from previous runs
    pub fn cleanup_old_files(&self) {
        log!("Cleaning up old task output files...");
        if let Ok(entries) = fs::read_dir(&self.output_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map_or(false, |ext| ext == "log") {
                    log!("  Removing: {:?}", path);
                    let _ = fs::remove_file(path);
                }
            }
        }
    }

    /// Get the task key from group and name
    fn task_key(group: &str, name: &str) -> String {
        format!("{}:{}", group, name)
    }

    /// Get output file path for a task
    pub fn get_output_file(&self, group: &str, name: &str) -> PathBuf {
        // Sanitize names for filesystem
        let safe_group = group.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
        let safe_name = name.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
        self.output_dir
            .join(format!("{}_{}.log", safe_group, safe_name))
    }

    /// Start a task in the background
    pub fn start_task(&mut self, group: &str, name: &str, script: &str) -> Result<(), String> {
        let key = Self::task_key(group, name);
        log!("Starting task: {} ({})", key, script);

        // If task is already running, don't restart
        if self.is_running(group, name) {
            log!("Task {} is already running", key);
            return Err("Task is already running".to_string());
        }

        let output_file = self.get_output_file(group, name);
        log!("Output file: {:?}", output_file);

        // Clear old output file
        let _ = fs::remove_file(&output_file);

        // Create the output file
        let _ = File::create(&output_file);

        const CREATE_NO_WINDOW: u32 = 0x08000000;

        // PowerShell command that writes output to file
        // Using Out-File with -Append for real-time writing
        let ps_script = format!(
            r#"& {{ {} }} 2>&1 | ForEach-Object {{ $_ | Out-File -FilePath '{}' -Append -Encoding UTF8; $_ }}"#,
            script,
            output_file.display()
        );

        log!("PowerShell script: {}", ps_script);

        // Try pwsh (PowerShell 7) first, fall back to powershell (5.1)
        let child = Command::new("pwsh")
            .args(["-ExecutionPolicy", "Bypass", "-Command", &ps_script])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .or_else(|_| {
                log!("pwsh not found, falling back to powershell");
                Command::new("powershell")
                    .args(["-ExecutionPolicy", "Bypass", "-Command", &ps_script])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .creation_flags(CREATE_NO_WINDOW)
                    .spawn()
            })
            .map_err(|e| format!("Failed to spawn task: {}", e))?;

        log!("Task spawned with PID: {:?}", child.id());

        let task = RunningTask {
            name: name.to_string(),
            group: group.to_string(),
            script: script.to_string(),
            child: Some(child),
            output_file,
            started_at: Instant::now(),
            status: TaskStatus::Running,
        };

        self.tasks.insert(key, task);
        Ok(())
    }

    /// Check if an interactive terminal exists for a task
    pub fn has_interactive_terminal(&self, group: &str, name: &str) -> bool {
        let key = Self::task_key(group, name);
        self.interactive_terminals.contains_key(&key)
    }

    /// Take an existing interactive terminal (removes it from storage)
    /// Returns None if no terminal exists for this task
    pub fn take_interactive_terminal(&mut self, group: &str, name: &str) -> Option<Terminal> {
        let key = Self::task_key(group, name);
        log!("Taking interactive terminal for: {}", key);
        self.interactive_terminals.remove(&key)
    }

    /// Store an interactive terminal for later retrieval
    pub fn store_interactive_terminal(&mut self, group: &str, name: &str, terminal: Terminal) {
        let key = Self::task_key(group, name);
        log!("Storing interactive terminal for: {}", key);
        self.interactive_terminals.insert(key, terminal);
    }

    /// Kill an interactive terminal session
    pub fn kill_interactive_terminal(&mut self, group: &str, name: &str) {
        let key = Self::task_key(group, name);
        log!("Killing interactive terminal: {}", key);
        self.interactive_terminals.remove(&key);
        self.tasks.remove(&key);
    }

    /// Start an interactive task using PTY
    ///
    /// Returns a Terminal that can be used for rendering and input.
    /// If a terminal already exists for this task, returns an error -
    /// use take_interactive_terminal() to get the existing one.
    pub fn start_interactive_task(
        &mut self,
        group: &str,
        name: &str,
        script: &str,
        theme: Option<&ThemeTree>,
        cwd: Option<&str>,
    ) -> Result<Terminal, String> {
        let key = Self::task_key(group, name);
        log!("Starting interactive task: {} ({}) in {:?}", key, script, cwd);

        // Check if we already have an interactive terminal for this task
        if self.interactive_terminals.contains_key(&key) {
            return Err(format!("Interactive terminal already exists for {}", key));
        }

        // Kill any existing non-interactive task with this key
        if self.is_running(group, name) {
            log!("Killing existing task {} before starting interactive", key);
            self.kill_task(group, name);
        }
        // Also remove stale task entries that aren't running
        self.tasks.remove(&key);

        // Build terminal configuration from theme
        let config = if let Some(t) = theme {
            TerminalConfig {
                cols: 120,
                rows: 30,
                scrollback_lines: 1000,
                font_family: t.get_string("terminal", None, "font-family", "Cascadia Code"),
                font_size: t.get_number("terminal", None, "font-size", 14.0) as f32,
            }
        } else {
            TerminalConfig::default()
        };

        // Load colors from theme
        let colors = theme
            .map(|t| TerminalColors::from_theme(t))
            .unwrap_or_default();

        // Build the command: use pwsh directly (faster than checking version)
        // If pwsh isn't available, PTY spawn will fail and we handle it
        let command = if script.trim().is_empty() {
            "pwsh".to_string()
        } else {
            format!("pwsh -ExecutionPolicy Bypass -Command \"{}\"", script)
        };

        log!("Interactive command: {}", command);

        // Spawn PTY with the command and working directory
        let pty = Pty::spawn_command_in_dir(&command, config.cols, config.rows, cwd)
            .map_err(|e| format!("Failed to spawn PTY: {:?}", e))?;

        // Create terminal and attach PTY
        let mut terminal = Terminal::new(config, colors);
        terminal.attach_pty(pty);

        // Track as running (we store a dummy task without output file)
        let output_file = self.get_output_file(group, name);
        let task = RunningTask {
            name: name.to_string(),
            group: group.to_string(),
            script: script.to_string(),
            child: None, // No child process to track; PTY handles it
            output_file,
            started_at: Instant::now(),
            status: TaskStatus::Running,
        };
        self.tasks.insert(key, task);

        Ok(terminal)
    }

    /// Check if a task is currently running
    pub fn is_running(&self, group: &str, name: &str) -> bool {
        let key = Self::task_key(group, name);
        self.tasks
            .get(&key)
            .map_or(false, |t| t.status == TaskStatus::Running)
    }

    /// Get the status of a task
    pub fn get_status(&self, group: &str, name: &str) -> Option<TaskStatus> {
        let key = Self::task_key(group, name);
        self.tasks.get(&key).map(|t| t.status)
    }

    /// Get a task by group and name
    pub fn get_task(&self, group: &str, name: &str) -> Option<&RunningTask> {
        let key = Self::task_key(group, name);
        self.tasks.get(&key)
    }

    /// Poll all running tasks to update their status
    /// Returns true if any status changed
    pub fn poll(&mut self) -> bool {
        let mut changed = false;

        for task in self.tasks.values_mut() {
            if task.status != TaskStatus::Running {
                continue;
            }

            if let Some(ref mut child) = task.child {
                match child.try_wait() {
                    Ok(Some(status)) => {
                        let new_status = if status.success() {
                            TaskStatus::Completed
                        } else {
                            TaskStatus::Failed
                        };
                        log!(
                            "Task {}:{} finished with status: {:?} (exit code: {:?})",
                            task.group,
                            task.name,
                            new_status,
                            status.code()
                        );
                        task.status = new_status;
                        task.child = None;
                        changed = true;
                    }
                    Ok(None) => {
                        // Still running
                    }
                    Err(e) => {
                        log!("Error checking task {}:{}: {}", task.group, task.name, e);
                        task.status = TaskStatus::Failed;
                        task.child = None;
                        changed = true;
                    }
                }
            }
        }

        changed
    }

    /// Get the last N lines from a task's output file
    pub fn get_output_tail(&self, group: &str, name: &str, lines: usize) -> Vec<String> {
        let output_file = self.get_output_file(group, name);

        if let Ok(file) = File::open(&output_file) {
            let reader = BufReader::new(file);
            let all_lines: Vec<String> = reader.lines().flatten().collect();
            let start = all_lines.len().saturating_sub(lines);
            all_lines[start..].to_vec()
        } else {
            Vec::new()
        }
    }

    /// Kill all running tasks (for shutdown)
    pub fn kill_all(&mut self) {
        log!("Killing all running tasks...");
        for (key, task) in self.tasks.iter_mut() {
            if let Some(ref mut child) = task.child {
                log!("  Killing task: {}", key);
                let _ = child.kill();
                task.status = TaskStatus::Failed;
                task.child = None;
            }
        }
    }

    /// Kill a specific running task
    pub fn kill_task(&mut self, group: &str, name: &str) -> bool {
        let key = Self::task_key(group, name);
        if let Some(task) = self.tasks.get_mut(&key) {
            if let Some(ref mut child) = task.child {
                log!("Killing task: {}", key);
                let _ = child.kill();
                task.status = TaskStatus::Failed;
                task.child = None;
                return true;
            }
        }
        false
    }

    /// Get all tasks with their current status (for UI)
    pub fn get_all_statuses(&self) -> HashMap<String, TaskStatus> {
        self.tasks
            .iter()
            .map(|(k, v)| (k.clone(), v.status))
            .collect()
    }
}

impl Default for TaskRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for TaskRunner {
    fn drop(&mut self) {
        self.kill_all();
    }
}
