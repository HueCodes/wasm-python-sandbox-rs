# wasm-python-sandbox-rs

Secure Python execution in WebAssembly using RustPython and Wasmtime.

## Features

- Memory limits, timeouts, and fuel-based CPU limits
- Filesystem and network isolation via WASI
- Module caching for performance
- Stdin, environment variables, and prelude injection

## Setup

```bash
# Build RustPython WASM interpreter
git clone https://github.com/RustPython/RustPython
cd RustPython
cargo build --release --target wasm32-wasip1 --features freeze-stdlib
cp target/wasm32-wasip1/release/rustpython.wasm /path/to/assets/
```

## Usage

```rust
use wasm_python_sandbox_rs::prelude::*;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<()> {
    let config = SandboxConfig::builder()
        .timeout(Duration::from_secs(5))
        .max_memory(32 * 1024 * 1024)
        .build();

    let sandbox = PythonSandbox::new(config)?;
    let result = sandbox.execute("print(1 + 1)", None).await?;

    println!("{}", result.stdout);
    Ok(())
}
```

## Configuration

```rust
SandboxConfig::builder()
    .timeout(Duration::from_secs(5))
    .max_memory(32 * 1024 * 1024)
    .max_fuel(1_000_000)          // optional instruction limit
    .stdin("input data")
    .env("KEY", "value")
    .prelude("def helper(): pass")
    .build()
```

## Examples

```bash
cargo run --example basic_execution
cargo run --example concurrent_execution
cargo run --example resource_limits
```

## License

MIT
