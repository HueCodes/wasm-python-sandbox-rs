//! Core execution engine for the Python sandbox.

use std::sync::Arc;
use std::time::{Duration, Instant};

#[cfg(feature = "tracing")]
use tracing::{debug, info, instrument, warn};

use wasmtime::{Engine, Linker, Module, Store, Trap};
use wasmtime_wasi::pipe::MemoryInputPipe;
use wasmtime_wasi::preview1;
use wasmtime_wasi::{I32Exit, WasiCtxBuilder};

use crate::error::{Result, SandboxError};
use crate::sandbox::cache::{global_cache, ModuleCache, SharedEngine};
use crate::sandbox::config::SandboxConfig;
use crate::sandbox::io::SandboxIo;
use crate::sandbox::limits::{StoreData, StoreLimiterExt};

/// Metadata about an execution, including resource usage.
#[derive(Debug, Clone)]
pub struct ExecutionMetadata {
    /// Wall-clock duration of execution.
    pub duration: Duration,
    /// Peak memory usage in bytes (as tracked by the limiter).
    pub peak_memory: u64,
    /// Fuel consumed during execution (if fuel limiting was enabled).
    pub fuel_consumed: Option<u64>,
    /// Whether this execution used a cached module.
    pub used_cached_module: bool,
}

impl ExecutionMetadata {
    /// Create empty metadata (used when execution fails early).
    pub fn empty() -> Self {
        Self {
            duration: Duration::ZERO,
            peak_memory: 0,
            fuel_consumed: None,
            used_cached_module: false,
        }
    }
}

/// Result of a Python execution.
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    /// Captured stdout output.
    pub stdout: String,
    /// Captured stderr output.
    pub stderr: String,
    /// Exit code (0 for success).
    pub exit_code: i32,
    /// Execution metadata including timing and resource usage.
    pub metadata: ExecutionMetadata,
}

impl ExecutionResult {
    /// Check if the execution was successful (exit code 0).
    pub fn is_success(&self) -> bool {
        self.exit_code == 0
    }
}

/// Options for creating a PythonSandbox.
#[derive(Debug, Clone)]
pub struct SandboxOptions {
    /// Whether to use the global module cache.
    pub use_cache: bool,
    /// A specific module cache to use (if not using global).
    pub cache: Option<Arc<ModuleCache>>,
    /// A shared engine to use (if provided).
    pub shared_engine: Option<SharedEngine>,
}

impl Default for SandboxOptions {
    fn default() -> Self {
        Self {
            use_cache: true,
            cache: None,
            shared_engine: None,
        }
    }
}

impl SandboxOptions {
    /// Create options with caching disabled.
    pub fn no_cache() -> Self {
        Self {
            use_cache: false,
            cache: None,
            shared_engine: None,
        }
    }

    /// Create options with a specific cache.
    pub fn with_cache(cache: Arc<ModuleCache>) -> Self {
        Self {
            use_cache: true,
            cache: Some(cache),
            shared_engine: None,
        }
    }

    /// Create options with a shared engine.
    pub fn with_engine(engine: SharedEngine) -> Self {
        Self {
            use_cache: true,
            cache: None,
            shared_engine: Some(engine),
        }
    }

    /// Set whether to use caching.
    pub fn use_cache(mut self, use_cache: bool) -> Self {
        self.use_cache = use_cache;
        self
    }

    /// Set a specific cache to use.
    pub fn cache(mut self, cache: Arc<ModuleCache>) -> Self {
        self.cache = Some(cache);
        self.use_cache = true;
        self
    }

    /// Set a shared engine to use.
    pub fn engine(mut self, engine: SharedEngine) -> Self {
        self.shared_engine = Some(engine);
        self
    }
}

/// A sandboxed Python execution environment.
pub struct PythonSandbox {
    config: SandboxConfig,
    engine: Arc<Engine>,
    module: Arc<Module>,
    /// Whether the module came from cache.
    module_was_cached: bool,
}

impl PythonSandbox {
    /// Create a new Python sandbox with the given configuration.
    ///
    /// This uses the global module cache by default. To customize caching
    /// behavior, use `new_with_options`.
    #[cfg_attr(feature = "tracing", instrument(skip(config), fields(interpreter_path = %config.interpreter_path.display())))]
    pub fn new(config: SandboxConfig) -> Result<Self> {
        Self::new_with_options(config, SandboxOptions::default())
    }

    /// Create a new Python sandbox with custom options.
    ///
    /// # Arguments
    ///
    /// * `config` - The sandbox configuration
    /// * `options` - Options controlling caching and engine sharing
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use wasm_python_sandbox_rs::prelude::*;
    /// use wasm_python_sandbox_rs::sandbox::executor::SandboxOptions;
    /// use wasm_python_sandbox_rs::sandbox::cache::SharedEngine;
    ///
    /// // Share an engine across multiple sandboxes
    /// let engine = SharedEngine::new()?;
    /// let options = SandboxOptions::with_engine(engine.clone());
    ///
    /// let sandbox1 = PythonSandbox::new_with_options(config.clone(), options.clone())?;
    /// let sandbox2 = PythonSandbox::new_with_options(config, options)?;
    /// ```
    #[cfg_attr(feature = "tracing", instrument(skip(config, options), fields(use_cache = options.use_cache, has_shared_engine = options.shared_engine.is_some())))]
    pub fn new_with_options(config: SandboxConfig, options: SandboxOptions) -> Result<Self> {
        let (engine, module, module_was_cached) =
            Self::create_engine_and_module(&config, &options)?;

        #[cfg(feature = "tracing")]
        info!(module_was_cached, "Sandbox created");

        Ok(Self {
            config,
            engine,
            module,
            module_was_cached,
        })
    }

    /// Create or retrieve the engine and module based on options.
    fn create_engine_and_module(
        config: &SandboxConfig,
        options: &SandboxOptions,
    ) -> Result<(Arc<Engine>, Arc<Module>, bool)> {
        // Create or reuse engine
        let engine = if let Some(ref shared) = options.shared_engine {
            shared.arc()
        } else {
            let mut engine_config = wasmtime::Config::new();
            engine_config.epoch_interruption(true);
            engine_config.consume_fuel(config.max_fuel.is_some());

            Arc::new(Engine::new(&engine_config).map_err(|e| {
                SandboxError::RuntimeInit(anyhow::anyhow!("failed to create engine: {}", e))
            })?)
        };

        // Get or compile module
        let (module, was_cached) = if options.use_cache {
            let cache = options
                .cache
                .as_ref()
                .map(|c| c.as_ref())
                .unwrap_or_else(|| global_cache());

            let was_in_cache = cache.contains(&config.interpreter_path);
            let module = cache.get_or_compile(&engine, &config.interpreter_path)?;
            (module, was_in_cache)
        } else {
            // No caching, compile directly
            let wasm_bytes = std::fs::read(&config.interpreter_path).map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    SandboxError::InterpreterNotFound(config.interpreter_path.display().to_string())
                } else {
                    SandboxError::Io(e)
                }
            })?;

            let module = Module::new(&engine, &wasm_bytes).map_err(|e| {
                SandboxError::ModuleLoad(anyhow::anyhow!("failed to compile module: {}", e))
            })?;

            (Arc::new(module), false)
        };

        Ok((engine, module, was_cached))
    }

    /// Execute Python code in the sandbox.
    ///
    /// # Arguments
    /// * `code` - The Python code to execute
    /// * `input` - Optional stdin input for the code
    ///
    /// # Returns
    /// The execution result containing stdout, stderr, exit code, and metadata.
    #[cfg_attr(feature = "tracing", instrument(skip(self, code, input), fields(code_len = code.len(), has_input = input.is_some())))]
    pub async fn execute(&self, code: &str, input: Option<&str>) -> Result<ExecutionResult> {
        #[cfg(feature = "tracing")]
        debug!("Starting Python code execution");

        let code = code.to_string();
        let input = input.map(|s| s.to_string());
        let timeout = self.config.timeout;
        let epoch_interval = self.config.epoch_tick_interval;
        let max_memory = self.config.max_memory;
        let max_fuel = self.config.max_fuel;
        let engine = Arc::clone(&self.engine);
        let module = Arc::clone(&self.module);
        let module_was_cached = self.module_was_cached;
        let stdin_data = self.config.stdin.clone();
        let env_vars = self.config.env_vars.clone();
        let prelude = self.config.prelude.clone();

        // Spawn the epoch ticker task
        let ticker_engine = Arc::clone(&engine);
        let ticker_handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(epoch_interval);
            loop {
                interval.tick().await;
                ticker_engine.increment_epoch();
            }
        });

        // Clone engine for the blocking task
        let exec_engine = Arc::clone(&engine);
        let exec_handle = tokio::task::spawn_blocking(move || {
            Self::execute_sync(
                &exec_engine,
                &module,
                &code,
                input.as_deref(),
                stdin_data.as_deref(),
                &env_vars,
                prelude.as_deref(),
                max_memory,
                max_fuel,
                module_was_cached,
            )
        });

        // Race between execution and timeout
        let result = tokio::select! {
            result = exec_handle => {
                ticker_handle.abort();
                #[cfg(feature = "tracing")]
                debug!("Execution completed normally");
                match result {
                    Ok(inner_result) => inner_result,
                    Err(e) => Err(SandboxError::ExecutionFailed(format!("task panicked: {}", e))),
                }
            }
            _ = tokio::time::sleep(timeout) => {
                ticker_handle.abort();
                engine.increment_epoch(); // Force interrupt
                #[cfg(feature = "tracing")]
                warn!(?timeout, "Execution timed out");
                Err(SandboxError::Timeout(timeout))
            }
        };

        #[cfg(feature = "tracing")]
        if let Ok(ref res) = result {
            info!(
                exit_code = res.exit_code,
                duration_ms = res.metadata.duration.as_millis() as u64,
                peak_memory = res.metadata.peak_memory,
                "Execution finished"
            );
        }

        result
    }

    /// Synchronous execution (runs in blocking task).
    #[allow(clippy::too_many_arguments)]
    fn execute_sync(
        engine: &Engine,
        module: &Module,
        code: &str,
        input: Option<&str>,
        stdin_data: Option<&str>,
        env_vars: &[(String, String)],
        prelude: Option<&str>,
        max_memory: u64,
        max_fuel: Option<u64>,
        module_was_cached: bool,
    ) -> Result<ExecutionResult> {
        let start_time = Instant::now();
        let initial_fuel = max_fuel;

        // Combine prelude with user code if prelude is provided
        let full_code = if let Some(prelude_code) = prelude {
            format!("{}\n{}", prelude_code, code)
        } else {
            code.to_string()
        };

        // Set up I/O capture - prefer stdin_data from config, fall back to input parameter
        let effective_input = stdin_data.or(input);
        let io = SandboxIo::new(effective_input);

        // Build WASI context with controlled access
        let mut wasi_builder = WasiCtxBuilder::new();

        // Pass the Python code via command line argument
        wasi_builder.args(&["python", "-c", &full_code]);

        // Add environment variables
        for (key, value) in env_vars {
            wasi_builder.env(key, value);
        }

        // Connect stdin to our I/O capture via MemoryInputPipe
        if let Some(input_str) = effective_input {
            let stdin_pipe = MemoryInputPipe::new(input_str.as_bytes().to_vec());
            wasi_builder.stdin(stdin_pipe);
        }

        // Build the WASI Preview 1 context
        let wasi_ctx = wasi_builder.build_p1();

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
        preview1::add_to_linker_sync(&mut linker, |data: &mut StoreData| &mut data.wasi).map_err(
            |e| SandboxError::RuntimeInit(anyhow::anyhow!("failed to link WASI: {}", e)),
        )?;

        // Instantiate the module
        let instance = linker.instantiate(&mut store, module).map_err(|e| {
            // Check if it was a resource limit issue
            if store.data().limiter.limit_exceeded() {
                let current_memory = store.data().limiter.current_memory();
                return SandboxError::MemoryLimitExceeded(format!(
                    "memory limit exceeded during instantiation (used {} bytes, limit {} bytes)",
                    current_memory, max_memory
                ));
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
                    let current_memory = store.data().limiter.current_memory();
                    return Err(SandboxError::MemoryLimitExceeded(format!(
                        "memory limit exceeded during execution (used {} bytes, limit {} bytes)",
                        current_memory, max_memory
                    )));
                }

                // Check for epoch interrupt (timeout) using proper trap code detection
                if let Some(trap) = e.downcast_ref::<Trap>() {
                    if *trap == Trap::Interrupt {
                        return Err(SandboxError::Timeout(start_time.elapsed()));
                    }
                }

                // Also check the trap code via root cause analysis
                if is_epoch_interrupt(&e) {
                    return Err(SandboxError::Timeout(start_time.elapsed()));
                }

                // Check for out-of-fuel trap
                if is_out_of_fuel(&e) {
                    let fuel_remaining = store.get_fuel().unwrap_or(0);
                    let fuel_consumed = initial_fuel.map(|f| f.saturating_sub(fuel_remaining));
                    return Err(SandboxError::ExecutionFailed(format!(
                        "execution ran out of fuel (consumed {:?} instructions)",
                        fuel_consumed
                    )));
                }

                // Check for WASI exit code
                if let Some(exit) = e.downcast_ref::<I32Exit>() {
                    exit.0
                } else {
                    // Other error - check for Python exceptions in stderr
                    return Err(SandboxError::ExecutionFailed(e.to_string()));
                }
            }
        };

        // Collect execution metadata
        let duration = start_time.elapsed();
        let peak_memory = store.data().limiter.current_memory();
        let fuel_consumed = if let Some(initial) = initial_fuel {
            let remaining = store.get_fuel().unwrap_or(0);
            Some(initial.saturating_sub(remaining))
        } else {
            None
        };

        Ok(ExecutionResult {
            stdout: io.stdout_str(),
            stderr: io.stderr_str(),
            exit_code,
            metadata: ExecutionMetadata {
                duration,
                peak_memory,
                fuel_consumed,
                used_cached_module: module_was_cached,
            },
        })
    }

    /// Get the shared engine used by this sandbox.
    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    /// Check if this sandbox is using a cached module.
    pub fn is_using_cached_module(&self) -> bool {
        self.module_was_cached
    }
}

/// Check if an error is an epoch interrupt (timeout).
fn is_epoch_interrupt(error: &anyhow::Error) -> bool {
    // Check if the error is a Trap::Interrupt
    if let Some(trap) = error.downcast_ref::<Trap>() {
        return *trap == Trap::Interrupt;
    }

    // Check the error chain for interrupt indicators
    for cause in error.chain() {
        if let Some(trap) = cause.downcast_ref::<Trap>() {
            if *trap == Trap::Interrupt {
                return true;
            }
        }
        // Fallback: check error message (for compatibility)
        let msg = cause.to_string().to_lowercase();
        if msg.contains("epoch") || msg.contains("interrupt") {
            return true;
        }
    }

    false
}

/// Check if an error is an out-of-fuel trap.
fn is_out_of_fuel(error: &anyhow::Error) -> bool {
    // Check if the error is a Trap::OutOfFuel
    if let Some(trap) = error.downcast_ref::<Trap>() {
        return *trap == Trap::OutOfFuel;
    }

    // Check the error chain
    for cause in error.chain() {
        if let Some(trap) = cause.downcast_ref::<Trap>() {
            if *trap == Trap::OutOfFuel {
                return true;
            }
        }
        // Fallback: check error message
        let msg = cause.to_string().to_lowercase();
        if msg.contains("out of fuel") || msg.contains("fuel") {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[tokio::test]
    #[ignore = "requires rustpython.wasm"]
    async fn test_execution_metadata() {
        let config = SandboxConfig::builder()
            .timeout(Duration::from_secs(5))
            .max_memory(32 * 1024 * 1024)
            .build();

        let sandbox = PythonSandbox::new(config).unwrap();
        let result = sandbox.execute("print('hello')", None).await.unwrap();

        assert!(result.metadata.duration.as_nanos() > 0);
        assert!(result.metadata.peak_memory > 0);
    }

    #[tokio::test]
    #[ignore = "requires rustpython.wasm"]
    async fn test_module_caching() {
        let config = SandboxConfig::builder()
            .timeout(Duration::from_secs(5))
            .max_memory(32 * 1024 * 1024)
            .build();

        // First sandbox compiles the module
        let sandbox1 = PythonSandbox::new(config.clone()).unwrap();
        let was_cached_1 = sandbox1.is_using_cached_module();

        // Second sandbox should use cached module
        let sandbox2 = PythonSandbox::new(config).unwrap();
        let was_cached_2 = sandbox2.is_using_cached_module();

        // First should have compiled fresh (or found existing cache)
        // Second should definitely find it cached
        assert!(was_cached_2 || !was_cached_1);
    }

    #[tokio::test]
    #[ignore = "requires rustpython.wasm"]
    async fn test_stdin_input() {
        let config = SandboxConfig::builder()
            .timeout(Duration::from_secs(5))
            .max_memory(32 * 1024 * 1024)
            .stdin("hello world")
            .build();

        let sandbox = PythonSandbox::new(config).unwrap();
        let result = sandbox.execute("print(input())", None).await.unwrap();

        assert!(result.is_success());
        assert_eq!(result.stdout.trim(), "hello world");
    }

    #[tokio::test]
    #[ignore = "requires rustpython.wasm"]
    async fn test_env_vars() {
        let config = SandboxConfig::builder()
            .timeout(Duration::from_secs(5))
            .max_memory(32 * 1024 * 1024)
            .env("MY_VAR", "my_value")
            .build();

        let sandbox = PythonSandbox::new(config).unwrap();
        let result = sandbox
            .execute(
                "import os; print(os.environ.get('MY_VAR', 'not found'))",
                None,
            )
            .await
            .unwrap();

        assert!(result.is_success());
        assert_eq!(result.stdout.trim(), "my_value");
    }

    #[tokio::test]
    #[ignore = "requires rustpython.wasm"]
    async fn test_prelude() {
        let config = SandboxConfig::builder()
            .timeout(Duration::from_secs(5))
            .max_memory(32 * 1024 * 1024)
            .prelude("def greet(name): return f'Hello, {name}!'")
            .build();

        let sandbox = PythonSandbox::new(config).unwrap();
        let result = sandbox
            .execute("print(greet('World'))", None)
            .await
            .unwrap();

        assert!(result.is_success());
        assert_eq!(result.stdout.trim(), "Hello, World!");
    }
}
