//! RunTaskUseCase - Run a task/command
//!
//! Handles the business logic for running tasks.

use std::sync::Arc;

use crate::application::ports::runtime_port::{ProcessHandle, RuntimePort};
use crate::domain::entities::Task;

/// Error for task execution
#[derive(Debug)]
pub enum TaskError {
    /// Task not found
    NotFound(String),
    /// Failed to execute task
    ExecutionFailed(String),
}

impl std::fmt::Display for TaskError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskError::NotFound(id) => write!(f, "Task not found: {}", id),
            TaskError::ExecutionFailed(msg) => write!(f, "Execution failed: {}", msg),
        }
    }
}

impl std::error::Error for TaskError {}

/// Use case for running tasks
pub struct RunTaskUseCase<RT>
where
    RT: RuntimePort,
{
    runtime: Arc<RT>,
}

impl<RT> RunTaskUseCase<RT>
where
    RT: RuntimePort,
{
    /// Create a new run task use case
    pub fn new(runtime: Arc<RT>) -> Self {
        Self { runtime }
    }

    /// Execute a task
    pub fn execute(&self, task: &Task) -> Result<Option<ProcessHandle>, TaskError> {
        // Build argument list
        let args: Vec<&str> = task.args.iter().map(|s| s.as_str()).collect();

        if task.background {
            // Spawn background process
            let handle = self
                .runtime
                .spawn(std::path::Path::new(&task.command), &args)
                .map_err(|e| TaskError::ExecutionFailed(e.to_string()))?;
            Ok(Some(handle))
        } else {
            // Run and wait (simplified - in reality would need async)
            self.runtime
                .execute_with_args(std::path::Path::new(&task.command), &args)
                .map_err(|e| TaskError::ExecutionFailed(e.to_string()))?;
            Ok(None)
        }
    }

    /// Execute a task in a terminal
    pub fn execute_in_terminal(&self, task: &Task) -> Result<ProcessHandle, TaskError> {
        let command_line = task.command_line();
        self.runtime
            .spawn_terminal(&command_line)
            .map_err(|e| TaskError::ExecutionFailed(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::ports::runtime_port::NullRuntimePort;

    #[test]
    fn test_run_task() {
        let runtime = Arc::new(NullRuntimePort);
        let use_case = RunTaskUseCase::new(runtime);

        let task = Task::new("Echo", "echo").with_args(vec!["hello".to_string()]);

        let result = use_case.execute(&task);
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_background_task() {
        let runtime = Arc::new(NullRuntimePort);
        let use_case = RunTaskUseCase::new(runtime);

        let task = Task::new("Server", "server")
            .with_background(true);

        let result = use_case.execute(&task);
        assert!(result.is_ok());
        assert!(result.unwrap().is_some());
    }
}
