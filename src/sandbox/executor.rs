//! Core execution engine for the Python sandbox.

use wasmtime::{Engine, Linker, Module, Store};
use wasmtime_wasi::preview1;
use wasmtime_wasi::{I32Exit, WasiCtxBuilder};

use crate::error::{Result, SandboxError};
use crate::sandbox::config::SandboxConfig;
use crate::sandbox::io::SandboxIo;
use crate::sandbox::limits::{StoreData, StoreLimiterExt};

/// Result of a Python execution.
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    /// Captured stdout output.
    pub stdout: String,
    /// Captured stderr output.
    pub stderr: String,
    /// Exit code (0 for success).
    pub exit_code: i32,
}

impl ExecutionResult {
    /// Check if the execution was successful (exit code 0).
    pub fn is_success(&self) -> bool {
        self.exit_code == 0
    }
}

/// A sandboxed Python execution environment.
pub struct PythonSandbox {
    config: SandboxConfig,
    engine: Engine,
    module: Module,
}

impl PythonSandbox {
    /// Create a new Python sandbox with the given configuration.
    pub fn new(config: SandboxConfig) -> Result<Self> {
        // Configure the engine with epoch interruption for timeout support
        let mut engine_config = wasmtime::Config::new();
        engine_config.epoch_interruption(true);
        engine_config.consume_fuel(config.max_fuel.is_some());

        let engine = Engine::new(&engine_config).map_err(|e| {
            SandboxError::RuntimeInit(anyhow::anyhow!("failed to create engine: {}", e))
        })?;

        // Load the Python interpreter module
        let wasm_bytes = std::fs::read(&config.interpreter_path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                SandboxError::InterpreterNotFound(
                    config.interpreter_path.display().to_string(),
                )
            } else {
                SandboxError::Io(e)
            }
        })?;

        let module = Module::new(&engine, &wasm_bytes)
            .map_err(|e| SandboxError::ModuleLoad(anyhow::anyhow!("failed to compile module: {}", e)))?;

        Ok(Self {
            config,
            engine,
            module,
        })
    }

    /// Execute Python code in the sandbox.
    ///
    /// # Arguments
    /// * `code` - The Python code to execute
    /// * `input` - Optional stdin input for the code
    ///
    /// # Returns
    /// The execution result containing stdout, stderr, and exit code.
    pub async fn execute(&self, code: &str, input: Option<&str>) -> Result<ExecutionResult> {
        let code = code.to_string();
        let input = input.map(|s| s.to_string());
        let timeout = self.config.timeout;
        let epoch_interval = self.config.epoch_tick_interval;
        let max_memory = self.config.max_memory;
        let max_fuel = self.config.max_fuel;
        let engine = self.engine.clone();
        let module = self.module.clone();

        // Spawn the epoch ticker task
        let ticker_engine = engine.clone();
        let ticker_handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(epoch_interval);
            loop {
                interval.tick().await;
                ticker_engine.increment_epoch();
            }
        });

        // Clone engine for the blocking task
        let exec_engine = engine.clone();
        let exec_handle = tokio::task::spawn_blocking(move || {
            Self::execute_sync(&exec_engine, &module, &code, input.as_deref(), max_memory, max_fuel)
        });

        // Race between execution and timeout
        let result = tokio::select! {
            result = exec_handle => {
                ticker_handle.abort();
                match result {
                    Ok(inner_result) => inner_result,
                    Err(e) => Err(SandboxError::ExecutionFailed(format!("task panicked: {}", e))),
                }
            }
            _ = tokio::time::sleep(timeout) => {
                ticker_handle.abort();
                engine.increment_epoch(); // Force interrupt
                Err(SandboxError::Timeout(timeout))
            }
        };

        result
    }

    /// Synchronous execution (runs in blocking task).
    fn execute_sync(
        engine: &Engine,
        module: &Module,
        code: &str,
        input: Option<&str>,
        max_memory: u64,
        max_fuel: Option<u64>,
    ) -> Result<ExecutionResult> {
        // Set up I/O capture
        let io = SandboxIo::new(input);

        // Build WASI context with no filesystem or network access
        let wasi_ctx = WasiCtxBuilder::new()
            // Pass the Python code via command line argument
            .args(&["python", "-c", code])
            // No preopened directories (filesystem isolation)
            // No network access (WASI Preview 1 doesn't have sockets anyway)
            // Inherit nothing from host environment
            .build_p1();

        // Create store with resource limiter
        let store_data = StoreData::new(max_memory, wasi_ctx);
        let mut store = Store::new(engine, store_data);
        store.configure_limiter();

        // Set epoch deadline for timeout
        store.epoch_deadline_trap();
        store.set_epoch_deadline(1);

        // Set fuel limit if configured
        if let Some(fuel) = max_fuel {
            store.set_fuel(fuel).map_err(|e| {
                SandboxError::RuntimeInit(anyhow::anyhow!("failed to set fuel: {}", e))
            })?;
        }

        // Link WASI Preview 1
        let mut linker = Linker::new(engine);
        preview1::add_to_linker_sync(&mut linker, |data: &mut StoreData| &mut data.wasi)
            .map_err(|e| SandboxError::RuntimeInit(anyhow::anyhow!("failed to link WASI: {}", e)))?;

        // Instantiate the module
        let instance = linker.instantiate(&mut store, module).map_err(|e| {
            // Check if it was a resource limit issue
            if store.data().limiter.limit_exceeded() {
                return SandboxError::MemoryLimitExceeded(
                    "memory limit exceeded during instantiation".to_string(),
                );
            }
            SandboxError::ModuleLoad(anyhow::anyhow!("failed to instantiate: {}", e))
        })?;

        // Get the _start function (WASI entry point)
        let start = instance
            .get_typed_func::<(), ()>(&mut store, "_start")
            .map_err(|e| {
                SandboxError::ModuleLoad(anyhow::anyhow!("failed to get _start function: {}", e))
            })?;

        // Execute
        let exit_code = match start.call(&mut store, ()) {
            Ok(()) => 0,
            Err(e) => {
                // Check for various error conditions
                if store.data().limiter.limit_exceeded() {
                    return Err(SandboxError::MemoryLimitExceeded(
                        "memory limit exceeded during execution".to_string(),
                    ));
                }

                // Check for epoch interrupt (timeout)
                if e.to_string().contains("epoch") || e.to_string().contains("interrupt") {
                    return Err(SandboxError::Timeout(std::time::Duration::from_secs(0)));
                }

                // Check for WASI exit code
                if let Some(exit) = e.downcast_ref::<I32Exit>() {
                    exit.0
                } else {
                    // Other error
                    return Err(SandboxError::ExecutionFailed(e.to_string()));
                }
            }
        };

        Ok(ExecutionResult {
            stdout: io.stdout_str(),
            stderr: io.stderr_str(),
            exit_code,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    // Note: These tests require rustpython.wasm to be present
    // They are marked as ignored by default

    #[tokio::test]
    #[ignore = "requires rustpython.wasm"]
    async fn test_simple_execution() {
        let config = SandboxConfig::builder()
            .timeout(Duration::from_secs(5))
            .max_memory(32 * 1024 * 1024)
            .build();

        let sandbox = PythonSandbox::new(config).unwrap();
        let result = sandbox.execute("print(1 + 1)", None).await.unwrap();

        assert!(result.is_success());
        assert_eq!(result.stdout.trim(), "2");
    }

    #[tokio::test]
    #[ignore = "requires rustpython.wasm"]
    async fn test_timeout() {
        let config = SandboxConfig::builder()
            .timeout(Duration::from_millis(100))
            .build();

        let sandbox = PythonSandbox::new(config).unwrap();
        let result = sandbox.execute("while True: pass", None).await;

        assert!(matches!(result, Err(SandboxError::Timeout(_))));
    }
}
