//! Example demonstrating resource limiting capabilities.
//!
//! This example shows how to configure and handle:
//! - Timeouts for long-running code
//! - Memory limits
//! - Fuel (instruction) limits
//!
//! Run with: cargo run --example resource_limits
//!
//! Note: Requires rustpython.wasm to be present in assets/

use std::time::Duration;
use wasm_python_sandbox_rs::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== Resource Limits Example ===\n");

    let interpreter_path = "assets/rustpython.wasm";

    // Example 1: Timeout protection
    println!("--- Test 1: Timeout Protection ---");
    {
        let config = SandboxConfig::builder()
            .timeout(Duration::from_millis(500)) // Short timeout
            .max_memory(32 * 1024 * 1024)
            .interpreter_path(interpreter_path)
            .epoch_tick_interval(Duration::from_millis(5)) // Check frequently
            .build();

        let sandbox = PythonSandbox::new(config)?;

        // This infinite loop should be interrupted
        let code = "while True: pass";
        println!("Executing infinite loop with 500ms timeout...");

        match sandbox.execute(code, None).await {
            Ok(result) => {
                println!("Unexpected success: {:?}", result);
            }
            Err(e) if e.is_timeout() => {
                println!("Timeout triggered as expected: {}", e);
            }
            Err(e) => {
                println!("Unexpected error: {}", e);
            }
        }
    }
    println!();

    // Example 2: Memory limits
    println!("--- Test 2: Memory Limits ---");
    {
        let config = SandboxConfig::builder()
            .timeout(Duration::from_secs(10))
            .max_memory(16 * 1024 * 1024) // 16MB - relatively small
            .interpreter_path(interpreter_path)
            .build();

        let sandbox = PythonSandbox::new(config)?;

        // Try to allocate a large list
        let code = r#"
# Try to allocate a lot of memory
big_list = [i for i in range(10_000_000)]
print(f"Created list with {len(big_list)} elements")
"#;
        println!("Attempting to allocate large list with 16MB limit...");

        match sandbox.execute(code, None).await {
            Ok(result) => {
                println!("Success: {}", result.stdout.trim());
                println!("Peak memory: {} bytes", result.metadata.peak_memory);
            }
            Err(e) if e.is_memory_limit() => {
                println!("Memory limit triggered: {}", e);
            }
            Err(e) => {
                println!("Error: {}", e);
            }
        }
    }
    println!();

    // Example 3: Reasonable allocation within limits
    println!("--- Test 3: Reasonable Allocation ---");
    {
        let config = SandboxConfig::builder()
            .timeout(Duration::from_secs(10))
            .max_memory(64 * 1024 * 1024) // 64MB
            .interpreter_path(interpreter_path)
            .build();

        let sandbox = PythonSandbox::new(config)?;

        let code = r#"
# Reasonable allocation
data = [i * 2 for i in range(10000)]
result = sum(data)
print(f"Sum of doubled range: {result}")
"#;
        println!("Running reasonable computation with 64MB limit...");

        match sandbox.execute(code, None).await {
            Ok(result) => {
                println!("Success: {}", result.stdout.trim());
                println!(
                    "Peak memory: {} MB",
                    result.metadata.peak_memory / (1024 * 1024)
                );
                println!("Duration: {:?}", result.metadata.duration);
            }
            Err(e) => {
                println!("Error: {}", e);
            }
        }
    }
    println!();

    // Example 4: Fuel limiting (instruction count)
    println!("--- Test 4: Fuel Limiting ---");
    {
        // Note: Fuel limiting requires engine with fuel enabled
        let engine = SharedEngine::with_fuel()?;

        let config = SandboxConfig::builder()
            .timeout(Duration::from_secs(10))
            .max_memory(64 * 1024 * 1024)
            .max_fuel(10_000_000) // Allow 10 million instructions
            .interpreter_path(interpreter_path)
            .build();

        let options = SandboxOptions::with_engine(engine);
        let sandbox = PythonSandbox::new_with_options(config, options)?;

        let code = r#"
total = 0
for i in range(1000):
    total += i
print(f"Sum: {total}")
"#;
        println!("Running loop with fuel limit of 10M instructions...");

        match sandbox.execute(code, None).await {
            Ok(result) => {
                println!("Success: {}", result.stdout.trim());
                if let Some(fuel) = result.metadata.fuel_consumed {
                    println!("Fuel consumed: {} instructions", fuel);
                }
            }
            Err(e) if e.is_out_of_fuel() => {
                println!("Out of fuel: {}", e);
            }
            Err(e) => {
                println!("Error: {}", e);
            }
        }
    }
    println!();

    println!("=== Resource Limits Example Complete ===");
    Ok(())
}
