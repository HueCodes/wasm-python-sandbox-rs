//! Prelude module for convenient imports.

pub use crate::error::{Result, SandboxError};
pub use crate::sandbox::{
    config::SandboxConfig,
    executor::{ExecutionResult, PythonSandbox},
};
