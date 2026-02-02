//! FileSystemPort - interface for file system operations
//!
//! This port defines file system capabilities.

use std::path::{Path, PathBuf};

/// File system operation error
#[derive(Debug, Clone)]
pub enum FileSystemError {
    /// File not found
    NotFound(PathBuf),
    /// Permission denied
    PermissionDenied(PathBuf),
    /// IO error
    IoError(String),
    /// Watch error
    WatchError(String),
}

impl std::fmt::Display for FileSystemError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FileSystemError::NotFound(p) => write!(f, "File not found: {}", p.display()),
            FileSystemError::PermissionDenied(p) => {
                write!(f, "Permission denied: {}", p.display())
            }
            FileSystemError::IoError(s) => write!(f, "IO error: {}", s),
            FileSystemError::WatchError(s) => write!(f, "Watch error: {}", s),
        }
    }
}

impl std::error::Error for FileSystemError {}

impl From<std::io::Error> for FileSystemError {
    fn from(err: std::io::Error) -> Self {
        FileSystemError::IoError(err.to_string())
    }
}

/// Handle to a file watch
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WatchHandle(pub u64);

/// File change event
#[derive(Clone, Debug)]
pub enum FileEvent {
    /// File was created
    Created(PathBuf),
    /// File was modified
    Modified(PathBuf),
    /// File was deleted
    Deleted(PathBuf),
    /// File was renamed
    Renamed(PathBuf, PathBuf),
}

/// Port interface for file system operations
pub trait FileSystemPort: Send + Sync {
    /// Read a file as bytes
    fn read_bytes(&self, path: &Path) -> Result<Vec<u8>, FileSystemError>;

    /// Read a file as string
    fn read_string(&self, path: &Path) -> Result<String, FileSystemError>;

    /// Write bytes to a file
    fn write_bytes(&self, path: &Path, data: &[u8]) -> Result<(), FileSystemError>;

    /// Write string to a file
    fn write_string(&self, path: &Path, content: &str) -> Result<(), FileSystemError>;

    /// Check if a file exists
    fn exists(&self, path: &Path) -> bool;

    /// Check if path is a directory
    fn is_dir(&self, path: &Path) -> bool;

    /// List directory contents
    fn list_dir(&self, path: &Path) -> Result<Vec<PathBuf>, FileSystemError>;

    /// Create a directory (and parents)
    fn create_dir(&self, path: &Path) -> Result<(), FileSystemError>;

    /// Get the application data directory
    fn app_data_dir(&self) -> PathBuf;

    /// Get the executable directory
    fn exe_dir(&self) -> PathBuf;

    /// Watch a file for changes
    fn watch(&mut self, path: &Path) -> Result<WatchHandle, FileSystemError>;

    /// Stop watching a file
    fn unwatch(&mut self, handle: WatchHandle);

    /// Poll for file events
    fn poll_events(&mut self) -> Vec<FileEvent>;
}

/// A null file system port for testing
pub struct NullFileSystemPort;

impl FileSystemPort for NullFileSystemPort {
    fn read_bytes(&self, path: &Path) -> Result<Vec<u8>, FileSystemError> {
        Err(FileSystemError::NotFound(path.to_path_buf()))
    }

    fn read_string(&self, path: &Path) -> Result<String, FileSystemError> {
        Err(FileSystemError::NotFound(path.to_path_buf()))
    }

    fn write_bytes(&self, _path: &Path, _data: &[u8]) -> Result<(), FileSystemError> {
        Ok(())
    }

    fn write_string(&self, _path: &Path, _content: &str) -> Result<(), FileSystemError> {
        Ok(())
    }

    fn exists(&self, _path: &Path) -> bool {
        false
    }

    fn is_dir(&self, _path: &Path) -> bool {
        false
    }

    fn list_dir(&self, _path: &Path) -> Result<Vec<PathBuf>, FileSystemError> {
        Ok(Vec::new())
    }

    fn create_dir(&self, _path: &Path) -> Result<(), FileSystemError> {
        Ok(())
    }

    fn app_data_dir(&self) -> PathBuf {
        PathBuf::from(".")
    }

    fn exe_dir(&self) -> PathBuf {
        PathBuf::from(".")
    }

    fn watch(&mut self, _path: &Path) -> Result<WatchHandle, FileSystemError> {
        Ok(WatchHandle(0))
    }

    fn unwatch(&mut self, _handle: WatchHandle) {}

    fn poll_events(&mut self) -> Vec<FileEvent> {
        Vec::new()
    }
}
