//! Basic example of executing Python code in the sandbox.
//!
//! Run with: cargo run --example basic_execution
//!
//! Note: Requires rustpython.wasm to be present in assets/

use std::time::Duration;
use wasm_python_sandbox_rs::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    // Configure the sandbox
    let config = SandboxConfig::builder()
        .timeout(Duration::from_secs(5))
        .max_memory(32 * 1024 * 1024) // 32MB
        .interpreter_path("assets/rustpython.wasm")
        .build();

    println!("Creating sandbox with config: {:?}", config);

    // Create the sandbox
    let sandbox = match PythonSandbox::new(config) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to create sandbox: {}", e);
            eprintln!("Make sure rustpython.wasm is present in the assets/ directory");
            return Err(e);
        }
    };

    // Execute simple arithmetic
    println!("\n=== Test 1: Simple arithmetic ===");
    match sandbox.execute("print(1 + 1)", None).await {
        Ok(result) => {
            println!("stdout: {}", result.stdout);
            println!("stderr: {}", result.stderr);
            println!("exit_code: {}", result.exit_code);
            println!("duration: {:?}", result.metadata.duration);
            println!("peak_memory: {} bytes", result.metadata.peak_memory);
        }
        Err(e) => eprintln!("Error: {}", e),
    }

    // Execute with a loop
    println!("\n=== Test 2: Loop execution ===");
    let code = r#"
for i in range(5):
    print(f"Count: {i}")
"#;
    match sandbox.execute(code, None).await {
        Ok(result) => {
            println!("stdout:\n{}", result.stdout);
            println!("exit_code: {}", result.exit_code);
        }
        Err(e) => eprintln!("Error: {}", e),
    }

    // Test error handling
    println!("\n=== Test 3: Python error ===");
    match sandbox
        .execute("raise ValueError('test error')", None)
        .await
    {
        Ok(result) => {
            println!("stderr:\n{}", result.stderr);
            println!("exit_code: {}", result.exit_code);
        }
        Err(e) => eprintln!("Execution error: {}", e),
    }

    Ok(())
}
