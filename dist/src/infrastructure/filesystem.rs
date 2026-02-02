//! FileSystem infrastructure - File system operations

use std::fs;
use std::path::{Path, PathBuf};

use crate::application::ports::filesystem_port::{FileEvent, FileSystemError, FileSystemPort, WatchHandle};

/// Standard file system implementation
pub struct StdFileSystem {
    app_data_dir: PathBuf,
    exe_dir: PathBuf,
}

impl StdFileSystem {
    /// Create a new standard file system
    pub fn new() -> Self {
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("."));

        let app_data_dir = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("wolfy");

        Self {
            app_data_dir,
            exe_dir,
        }
    }

    /// Create with custom directories
    pub fn with_dirs(app_data_dir: PathBuf, exe_dir: PathBuf) -> Self {
        Self {
            app_data_dir,
            exe_dir,
        }
    }
}

impl Default for StdFileSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl FileSystemPort for StdFileSystem {
    fn read_bytes(&self, path: &Path) -> Result<Vec<u8>, FileSystemError> {
        fs::read(path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                FileSystemError::NotFound(path.to_path_buf())
            } else if e.kind() == std::io::ErrorKind::PermissionDenied {
                FileSystemError::PermissionDenied(path.to_path_buf())
            } else {
                FileSystemError::IoError(e.to_string())
            }
        })
    }

    fn read_string(&self, path: &Path) -> Result<String, FileSystemError> {
        fs::read_to_string(path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                FileSystemError::NotFound(path.to_path_buf())
            } else if e.kind() == std::io::ErrorKind::PermissionDenied {
                FileSystemError::PermissionDenied(path.to_path_buf())
            } else {
                FileSystemError::IoError(e.to_string())
            }
        })
    }

    fn write_bytes(&self, path: &Path, data: &[u8]) -> Result<(), FileSystemError> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(path, data)?;
        Ok(())
    }

    fn write_string(&self, path: &Path, content: &str) -> Result<(), FileSystemError> {
        self.write_bytes(path, content.as_bytes())
    }

    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }

    fn is_dir(&self, path: &Path) -> bool {
        path.is_dir()
    }

    fn list_dir(&self, path: &Path) -> Result<Vec<PathBuf>, FileSystemError> {
        let entries: Result<Vec<PathBuf>, _> = fs::read_dir(path)?
            .map(|entry| entry.map(|e| e.path()))
            .collect();

        entries.map_err(|e| FileSystemError::IoError(e.to_string()))
    }

    fn create_dir(&self, path: &Path) -> Result<(), FileSystemError> {
        fs::create_dir_all(path)?;
        Ok(())
    }

    fn app_data_dir(&self) -> PathBuf {
        self.app_data_dir.clone()
    }

    fn exe_dir(&self) -> PathBuf {
        self.exe_dir.clone()
    }

    fn watch(&mut self, _path: &Path) -> Result<WatchHandle, FileSystemError> {
        // File watching would need a proper implementation with notify crate
        // For now, return a placeholder
        Ok(WatchHandle(0))
    }

    fn unwatch(&mut self, _handle: WatchHandle) {
        // No-op for now
    }

    fn poll_events(&mut self) -> Vec<FileEvent> {
        // No-op for now
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;

    #[test]
    fn test_read_write_string() {
        let fs = StdFileSystem::new();
        let path = temp_dir().join(format!("wolfy_test_{}.txt", std::process::id()));

        fs.write_string(&path, "hello world").unwrap();

        let content = fs.read_string(&path).unwrap();
        assert_eq!(content, "hello world");

        // Cleanup
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_exists() {
        let fs = StdFileSystem::new();
        let path = temp_dir().join(format!("wolfy_test_exists_{}.txt", std::process::id()));

        assert!(!fs.exists(&path));

        fs.write_string(&path, "test").unwrap();
        assert!(fs.exists(&path));

        // Cleanup
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_not_found() {
        let fs = StdFileSystem::new();
        let path = PathBuf::from("/nonexistent/path/to/file.txt");

        let result = fs.read_string(&path);

        match result {
            Err(FileSystemError::NotFound(_)) => {}
            _ => panic!("Expected NotFound error"),
        }
    }
}
