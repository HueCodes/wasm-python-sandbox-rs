//! Example of concurrent Python code execution with shared engine.
//!
//! This example demonstrates how to efficiently run multiple Python
//! executions concurrently by sharing the engine and using module caching.
//!
//! Run with: cargo run --example concurrent_execution
//!
//! Note: Requires rustpython.wasm to be present in assets/

use std::time::{Duration, Instant};
use wasm_python_sandbox_rs::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== Concurrent Execution Example ===\n");

    // Create a shared engine for better resource utilization
    let shared_engine = SharedEngine::new()?;
    println!("Created shared engine");

    let interpreter_path = "assets/rustpython.wasm";

    // Pre-warm the cache by creating the first sandbox
    let config = SandboxConfig::builder()
        .timeout(Duration::from_secs(10))
        .max_memory(32 * 1024 * 1024)
        .interpreter_path(interpreter_path)
        .build();

    let options = SandboxOptions::with_engine(shared_engine.clone());
    let _ = PythonSandbox::new_with_options(config.clone(), options.clone())?;
    println!("Pre-warmed module cache\n");

    // Define some Python tasks to run concurrently
    let tasks = vec![
        (
            "Task 1",
            "sum([i**2 for i in range(100)])",
            "Sum of squares",
        ),
        (
            "Task 2",
            "len([x for x in range(1000) if x % 3 == 0])",
            "Count divisible by 3",
        ),
        (
            "Task 3",
            "''.join([chr(65 + i % 26) for i in range(50)])",
            "Generate letters",
        ),
        (
            "Task 4",
            "max([i * (100 - i) for i in range(101)])",
            "Maximum product",
        ),
    ];

    println!("Starting {} concurrent tasks...\n", tasks.len());
    let start = Instant::now();

    // Spawn all tasks concurrently
    let mut handles = Vec::new();
    for (name, code, description) in tasks {
        let engine = shared_engine.clone();
        let config = SandboxConfig::builder()
            .timeout(Duration::from_secs(10))
            .max_memory(32 * 1024 * 1024)
            .interpreter_path(interpreter_path)
            .build();

        let handle = tokio::spawn(async move {
            let options = SandboxOptions::with_engine(engine);
            let sandbox = PythonSandbox::new_with_options(config, options)?;

            // Wrap code to print result
            let full_code = format!("print({})", code);
            let result = sandbox.execute(&full_code, None).await?;

            Ok::<_, wasm_python_sandbox_rs::SandboxError>((
                name,
                description,
                result.stdout.trim().to_string(),
                result.metadata.duration,
            ))
        });
        handles.push(handle);
    }

    // Collect results
    println!("Results:");
    println!("{:-<60}", "");
    for handle in handles {
        match handle.await {
            Ok(Ok((name, description, output, duration))) => {
                println!(
                    "{}: {} = {} (took {:?})",
                    name, description, output, duration
                );
            }
            Ok(Err(e)) => {
                println!("Task error: {}", e);
            }
            Err(e) => {
                println!("Join error: {}", e);
            }
        }
    }
    println!("{:-<60}", "");

    let total_time = start.elapsed();
    println!("\nTotal wall-clock time: {:?}", total_time);
    println!("(Tasks ran concurrently, so total time < sum of individual times)");

    Ok(())
}
