//! Resource limiting for the Wasm sandbox.

use wasmtime::{ResourceLimiter, Store};

/// Resource limiter that enforces memory and table size limits.
pub struct SandboxLimiter {
    /// Maximum memory in bytes.
    max_memory: u64,
    /// Current memory allocation.
    current_memory: u64,
    /// Peak memory allocation (highest ever seen).
    peak_memory: u64,
    /// Maximum table elements.
    max_table_elements: u64,
    /// Whether the limit has been exceeded.
    limit_exceeded: bool,
}

impl SandboxLimiter {
    /// Create a new resource limiter with the specified memory limit.
    pub fn new(max_memory: u64) -> Self {
        Self {
            max_memory,
            current_memory: 0,
            peak_memory: 0,
            max_table_elements: 10_000, // Reasonable default
            limit_exceeded: false,
        }
    }

    /// Check if any limit has been exceeded.
    pub fn limit_exceeded(&self) -> bool {
        self.limit_exceeded
    }

    /// Get the current memory usage.
    pub fn current_memory(&self) -> u64 {
        self.current_memory
    }

    /// Get the peak memory usage (highest ever observed).
    pub fn peak_memory(&self) -> u64 {
        self.peak_memory
    }

    /// Get the configured maximum memory.
    pub fn max_memory(&self) -> u64 {
        self.max_memory
    }
}

impl ResourceLimiter for SandboxLimiter {
    fn memory_growing(
        &mut self,
        _current: usize,
        desired: usize,
        _maximum: Option<usize>,
    ) -> anyhow::Result<bool> {
        let desired_bytes = desired as u64;

        if desired_bytes > self.max_memory {
            self.limit_exceeded = true;
            return Ok(false);
        }

        self.current_memory = desired_bytes;
        // Track peak memory
        if desired_bytes > self.peak_memory {
            self.peak_memory = desired_bytes;
        }
        Ok(true)
    }

    fn table_growing(
        &mut self,
        _current: usize,
        desired: usize,
        _maximum: Option<usize>,
    ) -> anyhow::Result<bool> {
        if desired as u64 > self.max_table_elements {
            self.limit_exceeded = true;
            return Ok(false);
        }
        Ok(true)
    }
}

/// Store data that includes the resource limiter and execution context.
pub struct StoreData {
    /// The resource limiter.
    pub limiter: SandboxLimiter,
    /// WASI Preview 1 context for the sandbox.
    pub wasi: wasmtime_wasi::preview1::WasiP1Ctx,
}

impl StoreData {
    /// Create new store data with the given memory limit and WASI context.
    pub fn new(max_memory: u64, wasi: wasmtime_wasi::preview1::WasiP1Ctx) -> Self {
        Self {
            limiter: SandboxLimiter::new(max_memory),
            wasi,
        }
    }
}

/// Extension trait for Store to configure resource limiting.
pub trait StoreLimiterExt {
    /// Configure the store with resource limiting enabled.
    fn configure_limiter(&mut self);
}

impl StoreLimiterExt for Store<StoreData> {
    fn configure_limiter(&mut self) {
        self.limiter(|data| &mut data.limiter);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_limiter_allows_within_limit() {
        let mut limiter = SandboxLimiter::new(1024 * 1024); // 1MB

        let result = limiter.memory_growing(0, 512 * 1024, None).unwrap();
        assert!(result);
        assert!(!limiter.limit_exceeded());
    }

    #[test]
    fn test_limiter_denies_over_limit() {
        let mut limiter = SandboxLimiter::new(1024 * 1024); // 1MB

        let result = limiter.memory_growing(0, 2 * 1024 * 1024, None).unwrap();
        assert!(!result);
        assert!(limiter.limit_exceeded());
    }
}
