//! Sandbox configuration with builder pattern.

use std::path::PathBuf;
use std::time::Duration;

/// Configuration for the Python sandbox.
#[derive(Debug, Clone)]
pub struct SandboxConfig {
    /// Maximum execution time before timeout.
    pub timeout: Duration,
    /// Maximum memory in bytes.
    pub max_memory: u64,
    /// Maximum fuel (instruction count limit).
    pub max_fuel: Option<u64>,
    /// Path to the RustPython wasm file.
    pub interpreter_path: PathBuf,
    /// Epoch interruption interval for cooperative timeout.
    pub epoch_tick_interval: Duration,
    /// Stdin data to provide to the sandbox.
    pub stdin: Option<String>,
    /// Environment variables to set in the sandbox.
    pub env_vars: Vec<(String, String)>,
    /// Prelude Python code to run before user code.
    pub prelude: Option<String>,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            max_memory: 64 * 1024 * 1024, // 64MB
            max_fuel: None,
            interpreter_path: PathBuf::from("assets/rustpython.wasm"),
            epoch_tick_interval: Duration::from_millis(10),
            stdin: None,
            env_vars: Vec::new(),
            prelude: None,
        }
    }
}

impl SandboxConfig {
    /// Create a new builder for SandboxConfig.
    pub fn builder() -> SandboxConfigBuilder {
        SandboxConfigBuilder::default()
    }
}

/// Builder for creating SandboxConfig instances.
#[derive(Debug, Clone, Default)]
pub struct SandboxConfigBuilder {
    timeout: Option<Duration>,
    max_memory: Option<u64>,
    max_fuel: Option<u64>,
    interpreter_path: Option<PathBuf>,
    epoch_tick_interval: Option<Duration>,
    stdin: Option<String>,
    env_vars: Vec<(String, String)>,
    prelude: Option<String>,
}

impl SandboxConfigBuilder {
    /// Set the maximum execution timeout.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Set the maximum memory limit in bytes.
    pub fn max_memory(mut self, bytes: u64) -> Self {
        self.max_memory = Some(bytes);
        self
    }

    /// Set the maximum fuel (instruction count).
    ///
    /// When fuel is exhausted, execution will stop with an error.
    /// This provides deterministic limiting of computation.
    pub fn max_fuel(mut self, fuel: u64) -> Self {
        self.max_fuel = Some(fuel);
        self
    }

    /// Set the path to the RustPython wasm interpreter.
    pub fn interpreter_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.interpreter_path = Some(path.into());
        self
    }

    /// Set the epoch tick interval for timeout checking.
    ///
    /// Smaller intervals provide more responsive timeout detection
    /// but incur slightly more overhead.
    pub fn epoch_tick_interval(mut self, interval: Duration) -> Self {
        self.epoch_tick_interval = Some(interval);
        self
    }

    /// Set stdin data to provide to the Python code.
    ///
    /// This data will be available via `input()` or reading from `sys.stdin`.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let config = SandboxConfig::builder()
    ///     .stdin("line 1\nline 2\nline 3")
    ///     .build();
    ///
    /// // Python code can then read:
    /// // line1 = input()  # "line 1"
    /// // line2 = input()  # "line 2"
    /// ```
    pub fn stdin(mut self, data: impl Into<String>) -> Self {
        self.stdin = Some(data.into());
        self
    }

    /// Add an environment variable to the sandbox.
    ///
    /// These variables will be accessible via `os.environ` in Python.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let config = SandboxConfig::builder()
    ///     .env("API_KEY", "test-key")
    ///     .env("DEBUG", "true")
    ///     .build();
    /// ```
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env_vars.push((key.into(), value.into()));
        self
    }

    /// Add multiple environment variables to the sandbox.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let config = SandboxConfig::builder()
    ///     .envs([("KEY1", "value1"), ("KEY2", "value2")])
    ///     .build();
    /// ```
    pub fn envs<K, V, I>(mut self, vars: I) -> Self
    where
        K: Into<String>,
        V: Into<String>,
        I: IntoIterator<Item = (K, V)>,
    {
        for (k, v) in vars {
            self.env_vars.push((k.into(), v.into()));
        }
        self
    }

    /// Set a prelude script to run before user code.
    ///
    /// The prelude is executed in the same context as the user code,
    /// so any definitions (functions, classes, variables) will be
    /// available to the user code.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let config = SandboxConfig::builder()
    ///     .prelude(r#"
    /// def safe_divide(a, b):
    ///     if b == 0:
    ///         return None
    ///     return a / b
    ///
    /// ALLOWED_OPERATIONS = ['add', 'subtract', 'multiply', 'divide']
    /// "#)
    ///     .build();
    ///
    /// // User code can then use safe_divide() and ALLOWED_OPERATIONS
    /// ```
    pub fn prelude(mut self, code: impl Into<String>) -> Self {
        self.prelude = Some(code.into());
        self
    }

    /// Build the SandboxConfig.
    pub fn build(self) -> SandboxConfig {
        let default = SandboxConfig::default();
        SandboxConfig {
            timeout: self.timeout.unwrap_or(default.timeout),
            max_memory: self.max_memory.unwrap_or(default.max_memory),
            max_fuel: self.max_fuel.or(default.max_fuel),
            interpreter_path: self.interpreter_path.unwrap_or(default.interpreter_path),
            epoch_tick_interval: self
                .epoch_tick_interval
                .unwrap_or(default.epoch_tick_interval),
            stdin: self.stdin,
            env_vars: self.env_vars,
            prelude: self.prelude,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = SandboxConfig::default();
        assert_eq!(config.timeout, Duration::from_secs(30));
        assert_eq!(config.max_memory, 64 * 1024 * 1024);
        assert!(config.stdin.is_none());
        assert!(config.env_vars.is_empty());
        assert!(config.prelude.is_none());
    }

    #[test]
    fn test_builder() {
        let config = SandboxConfig::builder()
            .timeout(Duration::from_secs(5))
            .max_memory(32 * 1024 * 1024)
            .max_fuel(1_000_000)
            .build();

        assert_eq!(config.timeout, Duration::from_secs(5));
        assert_eq!(config.max_memory, 32 * 1024 * 1024);
        assert_eq!(config.max_fuel, Some(1_000_000));
    }

    #[test]
    fn test_builder_stdin() {
        let config = SandboxConfig::builder().stdin("hello world").build();

        assert_eq!(config.stdin, Some("hello world".to_string()));
    }

    #[test]
    fn test_builder_env_vars() {
        let config = SandboxConfig::builder()
            .env("KEY1", "value1")
            .env("KEY2", "value2")
            .build();

        assert_eq!(config.env_vars.len(), 2);
        assert_eq!(
            config.env_vars[0],
            ("KEY1".to_string(), "value1".to_string())
        );
        assert_eq!(
            config.env_vars[1],
            ("KEY2".to_string(), "value2".to_string())
        );
    }

    #[test]
    fn test_builder_envs() {
        let config = SandboxConfig::builder()
            .envs([("A", "1"), ("B", "2"), ("C", "3")])
            .build();

        assert_eq!(config.env_vars.len(), 3);
    }

    #[test]
    fn test_builder_prelude() {
        let config = SandboxConfig::builder()
            .prelude("def helper(): pass")
            .build();

        assert_eq!(config.prelude, Some("def helper(): pass".to_string()));
    }
}
