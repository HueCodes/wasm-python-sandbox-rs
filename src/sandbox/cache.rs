//! WASM module caching for improved performance.
//!
//! This module provides a thread-safe cache for compiled WASM modules,
//! enabling efficient reuse across multiple sandbox instances.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use wasmtime::{Engine, Module};

use crate::error::{Result, SandboxError};

/// A thread-safe cache for compiled WASM modules.
///
/// The cache stores compiled modules keyed by their filesystem path,
/// allowing multiple `PythonSandbox` instances to share the same
/// compiled module and avoid redundant compilation.
///
/// # Example
///
/// ```rust,ignore
/// use wasm_python_sandbox_rs::sandbox::cache::ModuleCache;
/// use wasmtime::Engine;
///
/// let engine = Engine::default();
/// let cache = ModuleCache::new();
///
/// // First call compiles the module
/// let module1 = cache.get_or_compile(&engine, "path/to/interpreter.wasm")?;
///
/// // Second call returns the cached module
/// let module2 = cache.get_or_compile(&engine, "path/to/interpreter.wasm")?;
///
/// // Both references point to the same module
/// assert!(Arc::ptr_eq(&module1, &module2));
/// ```
#[derive(Debug, Default)]
pub struct ModuleCache {
    /// The cached modules, keyed by canonical path.
    cache: RwLock<HashMap<PathBuf, Arc<Module>>>,
}

impl ModuleCache {
    /// Create a new empty module cache.
    pub fn new() -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
        }
    }

    /// Get a cached module or compile it if not present.
    ///
    /// The path is canonicalized before lookup to ensure consistent caching
    /// regardless of how the path is specified (relative, absolute, symlinks, etc.).
    ///
    /// # Arguments
    ///
    /// * `engine` - The Wasmtime engine to use for compilation
    /// * `path` - Path to the WASM module file
    ///
    /// # Returns
    ///
    /// An `Arc<Module>` that can be shared across threads.
    pub fn get_or_compile(&self, engine: &Engine, path: impl AsRef<Path>) -> Result<Arc<Module>> {
        let path = path.as_ref();

        // Canonicalize the path for consistent caching
        let canonical_path = std::fs::canonicalize(path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                SandboxError::InterpreterNotFound(path.display().to_string())
            } else {
                SandboxError::Io(e)
            }
        })?;

        // Try to get from cache first (read lock)
        {
            let cache = self.cache.read().unwrap();
            if let Some(module) = cache.get(&canonical_path) {
                return Ok(Arc::clone(module));
            }
        }

        // Not in cache, compile the module (outside any lock)
        let wasm_bytes = std::fs::read(&canonical_path).map_err(SandboxError::Io)?;

        let module = Module::new(engine, &wasm_bytes).map_err(|e| {
            SandboxError::ModuleLoad(anyhow::anyhow!("failed to compile module: {}", e))
        })?;

        let module = Arc::new(module);

        // Insert into cache (write lock)
        {
            let mut cache = self.cache.write().unwrap();
            // Double-check pattern: another thread might have compiled while we were
            if let Some(existing) = cache.get(&canonical_path) {
                return Ok(Arc::clone(existing));
            }
            cache.insert(canonical_path, Arc::clone(&module));
        }

        Ok(module)
    }

    /// Check if a module is cached.
    pub fn contains(&self, path: impl AsRef<Path>) -> bool {
        let path = path.as_ref();
        if let Ok(canonical) = std::fs::canonicalize(path) {
            let cache = self.cache.read().unwrap();
            cache.contains_key(&canonical)
        } else {
            false
        }
    }

    /// Remove a module from the cache.
    ///
    /// Returns `true` if the module was present and removed.
    pub fn remove(&self, path: impl AsRef<Path>) -> bool {
        let path = path.as_ref();
        if let Ok(canonical) = std::fs::canonicalize(path) {
            let mut cache = self.cache.write().unwrap();
            cache.remove(&canonical).is_some()
        } else {
            false
        }
    }

    /// Clear all cached modules.
    pub fn clear(&self) {
        let mut cache = self.cache.write().unwrap();
        cache.clear();
    }

    /// Get the number of cached modules.
    pub fn len(&self) -> usize {
        let cache = self.cache.read().unwrap();
        cache.len()
    }

    /// Check if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Global module cache for convenient access.
///
/// This cache is shared across all sandbox instances and provides
/// automatic module reuse without explicit cache management.
static GLOBAL_CACHE: std::sync::LazyLock<ModuleCache> = std::sync::LazyLock::new(ModuleCache::new);

/// Get the global module cache.
///
/// This cache is automatically used by `PythonSandbox::new()` when
/// caching is enabled (the default).
pub fn global_cache() -> &'static ModuleCache {
    &GLOBAL_CACHE
}

/// A shared engine that can be reused across sandbox instances.
///
/// Wraps an `Arc<Engine>` for thread-safe sharing.
#[derive(Clone)]
pub struct SharedEngine {
    engine: Arc<Engine>,
}

impl std::fmt::Debug for SharedEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SharedEngine")
            .field("engine", &"<wasmtime::Engine>")
            .finish()
    }
}

impl SharedEngine {
    /// Create a new shared engine with the default configuration.
    pub fn new() -> Result<Self> {
        let config = Self::default_config(false)?;
        let engine = Engine::new(&config)
            .map_err(|e| SandboxError::RuntimeInit(anyhow::anyhow!("{}", e)))?;
        Ok(Self {
            engine: Arc::new(engine),
        })
    }

    /// Create a new shared engine with fuel consumption enabled.
    pub fn with_fuel() -> Result<Self> {
        let config = Self::default_config(true)?;
        let engine = Engine::new(&config)
            .map_err(|e| SandboxError::RuntimeInit(anyhow::anyhow!("{}", e)))?;
        Ok(Self {
            engine: Arc::new(engine),
        })
    }

    /// Create a new shared engine from an existing engine configuration.
    pub fn from_config(config: &wasmtime::Config) -> Result<Self> {
        let engine =
            Engine::new(config).map_err(|e| SandboxError::RuntimeInit(anyhow::anyhow!("{}", e)))?;
        Ok(Self {
            engine: Arc::new(engine),
        })
    }

    /// Create a shared engine wrapper from an existing Arc<Engine>.
    pub fn from_arc(engine: Arc<Engine>) -> Self {
        Self { engine }
    }

    /// Get a reference to the underlying engine.
    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    /// Get the Arc<Engine> for sharing.
    pub fn arc(&self) -> Arc<Engine> {
        Arc::clone(&self.engine)
    }

    /// Create the default engine configuration.
    fn default_config(enable_fuel: bool) -> Result<wasmtime::Config> {
        let mut config = wasmtime::Config::new();
        config.epoch_interruption(true);
        config.consume_fuel(enable_fuel);
        Ok(config)
    }
}

impl Default for SharedEngine {
    fn default() -> Self {
        Self::new().expect("failed to create default engine")
    }
}

impl std::ops::Deref for SharedEngine {
    type Target = Engine;

    fn deref(&self) -> &Self::Target {
        &self.engine
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_cache_new() {
        let cache = ModuleCache::new();
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn test_shared_engine_creation() {
        let engine = SharedEngine::new().unwrap();
        // Engine should be usable - increment_epoch returns ()
        engine.engine().increment_epoch();
    }

    #[test]
    fn test_shared_engine_with_fuel() {
        let engine = SharedEngine::with_fuel().unwrap();
        engine.engine().increment_epoch();
    }

    #[test]
    fn test_shared_engine_clone() {
        let engine1 = SharedEngine::new().unwrap();
        let engine2 = engine1.clone();

        // Both should reference the same underlying engine
        assert!(Arc::ptr_eq(&engine1.arc(), &engine2.arc()));
    }
}
