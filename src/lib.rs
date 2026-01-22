//! # Python Sandbox
//!
//! A secure Python execution environment using WebAssembly isolation.
//!
//! This crate provides a sandboxed Python interpreter using RustPython compiled
//! to WebAssembly, running in the Wasmtime runtime. It enforces strict security
//! boundaries including:
//!
//! - **Memory limits**: Configurable maximum memory allocation
//! - **Timeout protection**: Epoch-based interruption for infinite loop protection
//! - **Filesystem isolation**: No access to the host filesystem
//! - **Network isolation**: No network access (WASI Preview 1)
//! - **Process isolation**: Cannot spawn subprocesses
//!
//! ## Example
//!
//! ```rust,ignore
//! use wasm_python_sandbox_rs::prelude::*;
//! use std::time::Duration;
//!
//! #[tokio::main]
//! async fn main() -> Result<()> {
//!     let config = SandboxConfig::builder()
//!         .timeout(Duration::from_secs(5))
//!         .max_memory(32 * 1024 * 1024)  // 32MB
//!         .build();
//!
//!     let sandbox = PythonSandbox::new(config)?;
//!     let result = sandbox.execute("print(1 + 1)", None).await?;
//!
//!     assert_eq!(result.stdout.trim(), "2");
//!     assert!(result.is_success());
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Security Model
//!
//! The sandbox provides defense-in-depth through multiple isolation layers:
//!
//! 1. **WebAssembly sandboxing**: Code runs in Wasm with no direct host access
//! 2. **WASI restrictions**: No preopened directories or network capabilities
//! 3. **Resource limits**: Memory and execution time are bounded
//! 4. **Epoch interruption**: Cooperative timeout even for tight loops

pub mod error;
pub mod prelude;
pub mod sandbox;

// Re-export main types at crate root for convenience
pub use error::{Result, SandboxError};
pub use sandbox::cache::{global_cache, ModuleCache, SharedEngine};
pub use sandbox::config::{SandboxConfig, SandboxConfigBuilder};
pub use sandbox::executor::{ExecutionMetadata, ExecutionResult, PythonSandbox, SandboxOptions};
