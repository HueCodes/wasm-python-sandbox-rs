//! I/O capture for sandbox stdin/stdout/stderr.

use std::io::{Read, Write};
use std::sync::{Arc, Mutex};

/// A writer that captures output to a buffer.
#[derive(Clone, Debug)]
pub struct CapturedOutput {
    buffer: Arc<Mutex<Vec<u8>>>,
}

impl CapturedOutput {
    /// Create a new captured output buffer.
    pub fn new() -> Self {
        Self {
            buffer: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Get the captured output as a string.
    pub fn to_string_lossy(&self) -> String {
        let buffer = self.buffer.lock().unwrap();
        String::from_utf8_lossy(&buffer).to_string()
    }

    /// Get the captured output as bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let buffer = self.buffer.lock().unwrap();
        buffer.clone()
    }

    /// Clear the buffer.
    pub fn clear(&self) {
        let mut buffer = self.buffer.lock().unwrap();
        buffer.clear();
    }

    /// Get the length of captured data.
    pub fn len(&self) -> usize {
        let buffer = self.buffer.lock().unwrap();
        buffer.len()
    }

    /// Check if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for CapturedOutput {
    fn default() -> Self {
        Self::new()
    }
}

impl Write for CapturedOutput {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut buffer = self.buffer.lock().unwrap();
        buffer.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// A reader that provides input from a buffer.
#[derive(Clone, Debug)]
pub struct ProvidedInput {
    buffer: Arc<Mutex<std::io::Cursor<Vec<u8>>>>,
}

impl ProvidedInput {
    /// Create a new input provider with the given data.
    pub fn new(data: Vec<u8>) -> Self {
        Self {
            buffer: Arc::new(Mutex::new(std::io::Cursor::new(data))),
        }
    }

    /// Create an empty input provider.
    pub fn empty() -> Self {
        Self::new(Vec::new())
    }

    /// Create from a string.
    pub fn from_str(s: &str) -> Self {
        Self::new(s.as_bytes().to_vec())
    }
}

impl Default for ProvidedInput {
    fn default() -> Self {
        Self::empty()
    }
}

impl Read for ProvidedInput {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let mut cursor = self.buffer.lock().unwrap();
        cursor.read(buf)
    }
}

/// I/O configuration for a sandbox execution.
#[derive(Clone)]
pub struct SandboxIo {
    /// Input to provide to the sandbox.
    pub stdin: ProvidedInput,
    /// Captured stdout.
    pub stdout: CapturedOutput,
    /// Captured stderr.
    pub stderr: CapturedOutput,
}

impl SandboxIo {
    /// Create a new I/O configuration with optional input.
    pub fn new(input: Option<&str>) -> Self {
        Self {
            stdin: input.map(ProvidedInput::from_str).unwrap_or_default(),
            stdout: CapturedOutput::new(),
            stderr: CapturedOutput::new(),
        }
    }

    /// Get the captured stdout as a string.
    pub fn stdout_str(&self) -> String {
        self.stdout.to_string_lossy()
    }

    /// Get the captured stderr as a string.
    pub fn stderr_str(&self) -> String {
        self.stderr.to_string_lossy()
    }
}

impl Default for SandboxIo {
    fn default() -> Self {
        Self::new(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_captured_output() {
        let mut output = CapturedOutput::new();
        output.write_all(b"hello ").unwrap();
        output.write_all(b"world").unwrap();
        assert_eq!(output.to_string_lossy(), "hello world");
    }

    #[test]
    fn test_provided_input() {
        let mut input = ProvidedInput::from_str("test input");
        let mut buf = [0u8; 4];
        let n = input.read(&mut buf).unwrap();
        assert_eq!(n, 4);
        assert_eq!(&buf, b"test");
    }

    #[test]
    fn test_sandbox_io() {
        let io = SandboxIo::new(Some("input data"));
        assert!(io.stdout_str().is_empty());
        assert!(io.stderr_str().is_empty());
    }
}
