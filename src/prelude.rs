//! Prelude module for convenient imports.

pub use crate::error::{Result, SandboxError};
pub use crate::sandbox::{
    cache::{global_cache, ModuleCache, SharedEngine},
    config::{SandboxConfig, SandboxConfigBuilder},
    executor::{ExecutionMetadata, ExecutionResult, PythonSandbox, SandboxOptions},
};
