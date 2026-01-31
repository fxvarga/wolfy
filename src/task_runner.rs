//! Background task runner with output capture
//!
//! Runs PowerShell tasks in the background, captures output to files,
//! and provides status tracking for visual indicators.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Instant;

use crate::log;

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
