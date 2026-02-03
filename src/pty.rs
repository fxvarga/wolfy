//! PTY module - Windows ConPTY wrapper for interactive terminal sessions
//!
//! Provides a cross-platform-ish abstraction over Windows Pseudo Console (ConPTY)
//! for spawning interactive shell processes with full terminal emulation.

use std::io::{Read, Write};
use std::process::Command;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use conpty::io::{PipeReader, PipeWriter};
use conpty::Process;

use crate::log;

/// Default terminal size
const DEFAULT_COLS: u16 = 120;
const DEFAULT_ROWS: u16 = 30;

/// A PTY (pseudo-terminal) session wrapping a ConPTY process
pub struct Pty {
    /// The ConPTY process
    process: Arc<Mutex<Process>>,
    /// Input pipe for writing to the process
    input: Arc<Mutex<PipeWriter>>,
    /// Channel to receive output from the PTY read thread
    output_rx: Receiver<Vec<u8>>,
    /// Thread handle for the reader
    _reader_thread: JoinHandle<()>,
    /// Current terminal size
    cols: u16,
    rows: u16,
    /// Flag to signal shutdown
    shutdown: Arc<Mutex<bool>>,
}

impl Pty {
    /// Spawn a new PTY with the default shell (pwsh or powershell)
    pub fn spawn_shell() -> Result<Self, String> {
        Self::spawn_command("pwsh.exe", DEFAULT_COLS, DEFAULT_ROWS)
    }

    /// Spawn a new PTY with a specific command
    pub fn spawn_command(command: &str, cols: u16, rows: u16) -> Result<Self, String> {
        Self::spawn_command_in_dir(command, cols, rows, None)
    }

    /// Spawn a new PTY with a specific command and working directory
    pub fn spawn_command_in_dir(command: &str, cols: u16, rows: u16, cwd: Option<&str>) -> Result<Self, String> {
        log!(
            "PTY: Spawning command '{}' with size {}x{}, cwd={:?}",
            command,
            cols,
            rows,
            cwd
        );

        // Create command
        let mut cmd = Command::new(command);

        // Set working directory if provided
        if let Some(dir) = cwd {
            cmd.current_dir(dir);
        }

        // Create the ConPTY process
        let mut process =
            Process::spawn(cmd).map_err(|e| format!("Failed to spawn PTY process: {}", e))?;

        // Get input/output pipes
        let input = process
            .input()
            .map_err(|e| format!("Failed to get PTY input pipe: {}", e))?;
        let output = process
            .output()
            .map_err(|e| format!("Failed to get PTY output pipe: {}", e))?;

        let process = Arc::new(Mutex::new(process));
        let input = Arc::new(Mutex::new(input));
        let shutdown = Arc::new(Mutex::new(false));

        // Create channel for output
        let (output_tx, output_rx) = mpsc::channel();

        // Spawn reader thread
        let reader_shutdown = Arc::clone(&shutdown);
        let reader_thread = thread::spawn(move || {
            Self::reader_loop(output, output_tx, reader_shutdown);
        });

        log!("PTY: Successfully spawned process");

        Ok(Self {
            process,
            input,
            output_rx,
            _reader_thread: reader_thread,
            cols,
            rows,
            shutdown,
        })
    }

    /// Reader loop running in a separate thread
    fn reader_loop(mut output: PipeReader, output_tx: Sender<Vec<u8>>, shutdown: Arc<Mutex<bool>>) {
        let mut buffer = [0u8; 4096];

        loop {
            // Check shutdown flag
            if *shutdown.lock().unwrap() {
                break;
            }

            // Try to read from the output pipe
            match output.read(&mut buffer) {
                Ok(0) => {
                    // EOF - process exited
                    log!("PTY reader: EOF, process exited");
                    break;
                }
                Ok(n) => {
                    // Send the data
                    if output_tx.send(buffer[..n].to_vec()).is_err() {
                        // Receiver dropped
                        break;
                    }
                }
                Err(e) => {
                    // Check if it's a would-block error (non-blocking read)
                    if e.kind() == std::io::ErrorKind::WouldBlock {
                        // Sleep briefly and try again
                        thread::sleep(std::time::Duration::from_millis(10));
                        continue;
                    }
                    log!("PTY reader: Error reading: {:?}", e);
                    break;
                }
            }
        }

        log!("PTY reader thread exiting");
    }

    /// Read any available output from the PTY (non-blocking)
    pub fn read(&self) -> Option<Vec<u8>> {
        match self.output_rx.try_recv() {
            Ok(data) => Some(data),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => None,
        }
    }

    /// Read all available output (drains the channel)
    pub fn read_all(&self) -> Vec<u8> {
        let mut result = Vec::new();
        while let Some(data) = self.read() {
            result.extend(data);
        }
        result
    }

    /// Write input to the PTY
    pub fn write(&self, data: &[u8]) -> Result<(), String> {
        let mut input = self
            .input
            .lock()
            .map_err(|_| "Failed to lock input pipe".to_string())?;

        input
            .write_all(data)
            .map_err(|e| format!("Failed to write to PTY: {}", e))?;

        input
            .flush()
            .map_err(|e| format!("Failed to flush PTY: {}", e))?;

        Ok(())
    }

    /// Write a string to the PTY
    pub fn write_str(&self, s: &str) -> Result<(), String> {
        self.write(s.as_bytes())
    }

    /// Resize the PTY
    pub fn resize(&mut self, cols: u16, rows: u16) -> Result<(), String> {
        if cols == self.cols && rows == self.rows {
            return Ok(());
        }

        log!("PTY: Resizing to {}x{}", cols, rows);

        let mut process = self
            .process
            .lock()
            .map_err(|_| "Failed to lock process".to_string())?;

        process
            .resize(cols as i16, rows as i16)
            .map_err(|e| format!("Failed to resize PTY: {}", e))?;

        self.cols = cols;
        self.rows = rows;

        Ok(())
    }

    /// Check if the process is still running
    pub fn is_alive(&self) -> bool {
        let process = match self.process.lock() {
            Ok(p) => p,
            Err(_) => return false,
        };

        process.is_alive()
    }

    /// Get current terminal size
    pub fn size(&self) -> (u16, u16) {
        (self.cols, self.rows)
    }

    /// Kill the process
    pub fn kill(&self) -> Result<(), String> {
        log!("PTY: Killing process");

        // Signal shutdown
        if let Ok(mut shutdown) = self.shutdown.lock() {
            *shutdown = true;
        }

        // Exit the process
        if let Ok(mut process) = self.process.lock() {
            let _ = process.exit(1);
        }

        Ok(())
    }
}

impl Drop for Pty {
    fn drop(&mut self) {
        log!("PTY: Dropping, signaling shutdown");
        if let Ok(mut shutdown) = self.shutdown.lock() {
            *shutdown = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pty_constants() {
        assert_eq!(DEFAULT_COLS, 120);
        assert_eq!(DEFAULT_ROWS, 30);
    }
}
