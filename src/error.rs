//! Error types for the Python sandbox.

use thiserror::Error;

/// Errors that can occur during sandbox execution.
#[derive(Error, Debug)]
pub enum SandboxError {
    /// The execution exceeded the configured timeout.
    #[error("execution timed out after {0:?}")]
    Timeout(std::time::Duration),

    /// The execution exceeded memory limits.
    #[error("memory limit exceeded: {0}")]
    MemoryLimitExceeded(String),

    /// Failed to initialize the Wasm runtime.
    #[error("failed to initialize runtime: {0}")]
    RuntimeInit(#[source] anyhow::Error),

    /// Failed to load or instantiate the Python interpreter module.
    #[error("failed to load Python interpreter: {0}")]
    ModuleLoad(#[source] anyhow::Error),

    /// The Python code execution failed.
    #[error("execution failed: {0}")]
    ExecutionFailed(String),

    /// A Python exception was raised during execution.
    #[error("Python {exception_type}: {message}")]
    PythonException {
        /// The type of Python exception (e.g., "ValueError", "TypeError").
        exception_type: String,
        /// The exception message.
        message: String,
        /// The full Python traceback, if available.
        traceback: Option<String>,
    },

    /// I/O error during execution.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Configuration error.
    #[error("configuration error: {0}")]
    Config(String),

    /// The interpreter wasm file was not found.
    #[error("Python interpreter wasm not found at: {0}")]
    InterpreterNotFound(String),

    /// Execution ran out of fuel (instruction limit).
    #[error("execution ran out of fuel after {consumed:?} instructions")]
    OutOfFuel {
        /// Number of instructions consumed before running out.
        consumed: Option<u64>,
    },
}

impl SandboxError {
    /// Create a Python exception error from stderr output.
    ///
    /// Attempts to parse the stderr to extract exception type, message, and traceback.
    pub fn from_python_stderr(stderr: &str) -> Option<Self> {
        parse_python_exception(stderr)
    }

    /// Check if this error represents a timeout.
    pub fn is_timeout(&self) -> bool {
        matches!(self, SandboxError::Timeout(_))
    }

    /// Check if this error represents a memory limit exceeded.
    pub fn is_memory_limit(&self) -> bool {
        matches!(self, SandboxError::MemoryLimitExceeded(_))
    }

    /// Check if this error represents a Python exception.
    pub fn is_python_exception(&self) -> bool {
        matches!(self, SandboxError::PythonException { .. })
    }

    /// Check if this error represents an out-of-fuel condition.
    pub fn is_out_of_fuel(&self) -> bool {
        matches!(self, SandboxError::OutOfFuel { .. })
    }
}

/// Result type alias for sandbox operations.
pub type Result<T> = std::result::Result<T, SandboxError>;

/// Parse a Python exception from stderr output.
///
/// This attempts to extract the exception type, message, and traceback
/// from Python's standard error output format.
pub fn parse_python_exception(stderr: &str) -> Option<SandboxError> {
    if stderr.trim().is_empty() {
        return None;
    }

    let lines: Vec<&str> = stderr.lines().collect();
    if lines.is_empty() {
        return None;
    }

    // Look for the exception line (typically the last line with an exception)
    // Format: "ExceptionType: message" or "ExceptionType"
    let mut exception_line = None;
    let mut traceback_start = None;

    for (i, line) in lines.iter().enumerate() {
        if line.starts_with("Traceback (most recent call last):") {
            traceback_start = Some(i);
        }
        // Python exception lines typically start with a capital letter
        // and contain the exception type followed by optional message
        if !line.starts_with(' ') && !line.is_empty() && !line.starts_with("Traceback") {
            // Check if it looks like an exception (has format "SomethingError: message" or just "SomethingError")
            if looks_like_exception(line) {
                exception_line = Some((i, *line));
            }
        }
    }

    // Parse the exception line
    if let Some((line_idx, exception_str)) = exception_line {
        let (exception_type, message) = if let Some(colon_pos) = exception_str.find(':') {
            let exc_type = exception_str[..colon_pos].trim().to_string();
            let msg = exception_str[colon_pos + 1..].trim().to_string();
            (exc_type, msg)
        } else {
            (exception_str.trim().to_string(), String::new())
        };

        // Extract traceback if present
        let traceback = if let Some(start) = traceback_start {
            Some(lines[start..=line_idx].join("\n"))
        } else {
            None
        };

        return Some(SandboxError::PythonException {
            exception_type,
            message,
            traceback,
        });
    }

    None
}

/// Check if a line looks like a Python exception.
fn looks_like_exception(line: &str) -> bool {
    // Common Python exception suffixes/patterns
    let exception_suffixes = ["Error", "Exception", "Warning"];
    let standalone_exceptions = [
        "KeyboardInterrupt",
        "SystemExit",
        "StopIteration",
        "GeneratorExit",
    ];

    // First check if line starts with a capital letter (common for exception names)
    let first_char = line.chars().next();
    if !first_char.map(|c| c.is_ascii_uppercase()).unwrap_or(false) {
        return false;
    }

    // Check for exception suffixes (like ValueError, TypeError, etc.)
    for suffix in exception_suffixes.iter() {
        if let Some(idx) = line.find(suffix) {
            // Check if the suffix is followed by a colon, space, or end of string
            let after_idx = idx + suffix.len();
            let after_ok = after_idx >= line.len()
                || line.as_bytes()[after_idx] == b':'
                || line.as_bytes()[after_idx] == b' '
                || line.as_bytes()[after_idx] == b'\n';
            if after_ok {
                return true;
            }
        }
    }

    // Check for standalone exceptions
    for exc in standalone_exceptions.iter() {
        if line.starts_with(exc) {
            let after_idx = exc.len();
            let after_ok = after_idx >= line.len()
                || line.as_bytes()[after_idx] == b':'
                || line.as_bytes()[after_idx] == b' '
                || line.as_bytes()[after_idx] == b'\n';
            if after_ok {
                return true;
            }
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_exception() {
        let stderr = "ValueError: invalid literal for int() with base 10: 'abc'";
        let result = parse_python_exception(stderr);

        assert!(result.is_some());
        if let Some(SandboxError::PythonException {
            exception_type,
            message,
            traceback,
        }) = result
        {
            assert_eq!(exception_type, "ValueError");
            assert_eq!(message, "invalid literal for int() with base 10: 'abc'");
            assert!(traceback.is_none());
        } else {
            panic!("Expected PythonException");
        }
    }

    #[test]
    fn test_parse_exception_with_traceback() {
        let stderr = r#"Traceback (most recent call last):
  File "<string>", line 1, in <module>
ValueError: invalid value"#;

        let result = parse_python_exception(stderr);

        assert!(result.is_some());
        if let Some(SandboxError::PythonException {
            exception_type,
            message,
            traceback,
        }) = result
        {
            assert_eq!(exception_type, "ValueError");
            assert_eq!(message, "invalid value");
            assert!(traceback.is_some());
            assert!(traceback.unwrap().contains("Traceback"));
        } else {
            panic!("Expected PythonException");
        }
    }

    #[test]
    fn test_parse_exception_no_message() {
        let stderr = "StopIteration";
        let result = parse_python_exception(stderr);

        assert!(result.is_some());
        if let Some(SandboxError::PythonException {
            exception_type,
            message,
            ..
        }) = result
        {
            assert_eq!(exception_type, "StopIteration");
            assert!(message.is_empty());
        } else {
            panic!("Expected PythonException");
        }
    }

    #[test]
    fn test_parse_empty_stderr() {
        assert!(parse_python_exception("").is_none());
        assert!(parse_python_exception("   ").is_none());
    }

    #[test]
    fn test_error_helpers() {
        let timeout = SandboxError::Timeout(std::time::Duration::from_secs(5));
        assert!(timeout.is_timeout());
        assert!(!timeout.is_memory_limit());
        assert!(!timeout.is_python_exception());

        let memory = SandboxError::MemoryLimitExceeded("test".to_string());
        assert!(!memory.is_timeout());
        assert!(memory.is_memory_limit());

        let python_exc = SandboxError::PythonException {
            exception_type: "ValueError".to_string(),
            message: "test".to_string(),
            traceback: None,
        };
        assert!(python_exc.is_python_exception());
    }
}
