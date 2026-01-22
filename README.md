# wasm-python-sandbox-rs

Secure Python execution in WebAssembly isolation using RustPython and Wasmtime.

## Features

- **Memory limits** - Configurable maximum memory allocation
- **Timeout protection** - Epoch-based interruption for infinite loop protection
- **Filesystem isolation** - No access to the host filesystem
- **Network isolation** - No network access (WASI Preview 1)
- **Process isolation** - Cannot spawn subprocesses
- **Module caching** - Compiled WASM modules cached for performance
- **Stdin support** - Provide input to Python code
- **Environment variables** - Pass controlled env vars to sandbox
- **Prelude injection** - Define helper functions available to user code
- **Execution metadata** - Track duration, memory usage, fuel consumed

## Prerequisites

Download or build `rustpython.wasm` and place it in `assets/rustpython.wasm`:

```bash
# Build from source
git clone https://github.com/RustPython/RustPython
cd RustPython
cargo build --release --target wasm32-wasip1 --features freeze-stdlib
cp target/wasm32-wasip1/release/rustpython.wasm /path/to/assets/
```

## Usage

### Basic Example

```rust
use wasm_python_sandbox_rs::prelude::*;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<()> {
    let config = SandboxConfig::builder()
        .timeout(Duration::from_secs(5))
        .max_memory(32 * 1024 * 1024)  // 32MB
        .build();

    let sandbox = PythonSandbox::new(config)?;
    let result = sandbox.execute("print(1 + 1)", None).await?;

    println!("stdout: {}", result.stdout);  // "2\n"
    println!("duration: {:?}", result.metadata.duration);

    Ok(())
}
```

### With Stdin, Env Vars, and Prelude

```rust
let config = SandboxConfig::builder()
    .timeout(Duration::from_secs(5))
    .max_memory(64 * 1024 * 1024)
    .stdin("Alice\n25")
    .env("GREETING", "Hello")
    .prelude("def greet(name): return f'{GREETING}, {name}!'")
    .build();

let sandbox = PythonSandbox::new(config)?;
let result = sandbox.execute(r#"
import os
name = input()
age = input()
GREETING = os.environ.get('GREETING', 'Hi')
print(greet(name))
"#, None).await?;
```

### Shared Engine for Concurrent Execution

```rust
use wasm_python_sandbox_rs::prelude::*;

let engine = SharedEngine::new()?;

let handles: Vec<_> = (0..4).map(|i| {
    let engine = engine.clone();
    tokio::spawn(async move {
        let config = SandboxConfig::builder()
            .timeout(Duration::from_secs(5))
            .build();
        let options = SandboxOptions::with_engine(engine);
        let sandbox = PythonSandbox::new_with_options(config, options)?;
        sandbox.execute(&format!("print({})", i), None).await
    })
}).collect();
```

### Configuration Options

```rust
SandboxConfig::builder()
    .timeout(Duration::from_secs(5))       // Max execution time
    .max_memory(32 * 1024 * 1024)          // Memory limit in bytes
    .max_fuel(1_000_000)                   // Instruction limit (optional)
    .interpreter_path("path/to/rustpython.wasm")
    .epoch_tick_interval(Duration::from_millis(10))
    .stdin("input data")                   // Stdin for Python
    .env("KEY", "value")                   // Environment variable
    .prelude("def helper(): pass")         // Setup code
    .build()
```

## Running Tests

```bash
cargo test --lib               # Unit tests (no wasm needed)
cargo test -- --ignored        # Integration tests (requires rustpython.wasm)
```

## Running Examples

```bash
cargo run --example basic_execution
cargo run --example concurrent_execution
cargo run --example resource_limits
cargo run --example io_and_config
cargo run --example error_handling
```

## Benchmarks

```bash
cargo bench   # Requires rustpython.wasm
```

## Optional Features

```toml
[dependencies]
wasm-python-sandbox-rs = { version = "0.1", features = ["tracing"] }
```

- `tracing` - Enable tracing instrumentation for debugging

## Security Model

| Threat | Mitigation |
|--------|------------|
| Infinite loops | Epoch interruption + tokio timeout |
| Memory exhaustion | ResourceLimiter (configurable cap) |
| CPU exhaustion | Fuel limiting (optional) |
| Filesystem access | No preopened directories in WASI |
| Network access | WASI Preview 1 has no sockets |
| Process spawning | WASI doesn't support fork/exec |
| Native code loading | WASM boundary prevents ctypes/cffi |

## License

MIT
