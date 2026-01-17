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

    /// I/O error during execution.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Configuration error.
    #[error("configuration error: {0}")]
    Config(String),

    /// The interpreter wasm file was not found.
    #[error("Python interpreter wasm not found at: {0}")]
    InterpreterNotFound(String),
}

/// Result type alias for sandbox operations.
pub type Result<T> = std::result::Result<T, SandboxError>;
