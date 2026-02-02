//! RuntimePort - interface for process execution
//!
//! This port defines how to launch applications and run commands.

use std::path::Path;

/// Runtime operation error
#[derive(Debug, Clone)]
pub enum RuntimeError {
    /// Failed to launch application
    LaunchError(String),
    /// Failed to spawn process
    SpawnError(String),
    /// Process not found
    NotFound(String),
    /// Permission denied
    PermissionDenied(String),
}

impl std::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RuntimeError::LaunchError(s) => write!(f, "Launch error: {}", s),
            RuntimeError::SpawnError(s) => write!(f, "Spawn error: {}", s),
            RuntimeError::NotFound(s) => write!(f, "Not found: {}", s),
            RuntimeError::PermissionDenied(s) => write!(f, "Permission denied: {}", s),
        }
    }
}

impl std::error::Error for RuntimeError {}

/// Handle to a running process
#[derive(Clone, Debug)]
pub struct ProcessHandle {
    /// Process ID
    pub pid: u32,
    /// Process name
    pub name: String,
}

/// Port interface for process execution
pub trait RuntimePort: Send + Sync {
    /// Execute/launch an application by path
    fn execute(&self, path: &Path) -> Result<(), RuntimeError>;

    /// Execute with arguments
    fn execute_with_args(&self, path: &Path, args: &[&str]) -> Result<(), RuntimeError>;

    /// Execute with working directory
    fn execute_with_cwd(&self, path: &Path, cwd: &Path) -> Result<(), RuntimeError>;

    /// Spawn a process and return a handle
    fn spawn(&self, path: &Path, args: &[&str]) -> Result<ProcessHandle, RuntimeError>;

    /// Spawn a terminal process with a command
    fn spawn_terminal(&self, command: &str) -> Result<ProcessHandle, RuntimeError>;

    /// Open a file with the default application
    fn open_file(&self, path: &Path) -> Result<(), RuntimeError>;

    /// Open a URL in the default browser
    fn open_url(&self, url: &str) -> Result<(), RuntimeError>;
}

/// A null runtime port for testing
pub struct NullRuntimePort;

impl RuntimePort for NullRuntimePort {
    fn execute(&self, _path: &Path) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn execute_with_args(&self, _path: &Path, _args: &[&str]) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn execute_with_cwd(&self, _path: &Path, _cwd: &Path) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn spawn(&self, _path: &Path, _args: &[&str]) -> Result<ProcessHandle, RuntimeError> {
        Ok(ProcessHandle {
            pid: 0,
            name: String::new(),
        })
    }

    fn spawn_terminal(&self, _command: &str) -> Result<ProcessHandle, RuntimeError> {
        Ok(ProcessHandle {
            pid: 0,
            name: String::new(),
        })
    }

    fn open_file(&self, _path: &Path) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn open_url(&self, _url: &str) -> Result<(), RuntimeError> {
        Ok(())
    }
}
