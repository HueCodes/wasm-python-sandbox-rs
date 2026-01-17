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
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            max_memory: 64 * 1024 * 1024, // 64MB
            max_fuel: None,
            interpreter_path: PathBuf::from("assets/rustpython.wasm"),
            epoch_tick_interval: Duration::from_millis(10),
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
#[derive(Debug, Clone)]
pub struct SandboxConfigBuilder {
    timeout: Option<Duration>,
    max_memory: Option<u64>,
    max_fuel: Option<u64>,
    interpreter_path: Option<PathBuf>,
    epoch_tick_interval: Option<Duration>,
}

impl Default for SandboxConfigBuilder {
    fn default() -> Self {
        Self {
            timeout: None,
            max_memory: None,
            max_fuel: None,
            interpreter_path: None,
            epoch_tick_interval: None,
        }
    }
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
    pub fn epoch_tick_interval(mut self, interval: Duration) -> Self {
        self.epoch_tick_interval = Some(interval);
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
            epoch_tick_interval: self.epoch_tick_interval.unwrap_or(default.epoch_tick_interval),
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
}
