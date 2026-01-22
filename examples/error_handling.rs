//! Example demonstrating error handling patterns.
//!
//! This example shows how to handle various types of errors:
//! - Python exceptions
//! - Timeouts
//! - Memory limits
//! - Configuration errors
//!
//! Run with: cargo run --example error_handling
//!
//! Note: Requires rustpython.wasm to be present in assets/

use std::time::Duration;
use wasm_python_sandbox_rs::error::parse_python_exception;
use wasm_python_sandbox_rs::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== Error Handling Example ===\n");

    let interpreter_path = "assets/rustpython.wasm";

    let config = SandboxConfig::builder()
        .timeout(Duration::from_secs(5))
        .max_memory(64 * 1024 * 1024)
        .interpreter_path(interpreter_path)
        .build();

    let sandbox = match PythonSandbox::new(config) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to create sandbox: {}", e);
            eprintln!("\nMake sure rustpython.wasm is present in the assets/ directory.");
            eprintln!("See assets/GET_RUSTPYTHON.txt for instructions.");
            return Err(e);
        }
    };

    // Example 1: ValueError
    println!("--- Test 1: Python ValueError ---");
    {
        let code = "int('not a number')";
        let result = sandbox.execute(code, None).await?;

        if !result.is_success() {
            println!("Python code failed with exit code: {}", result.exit_code);
            println!("stderr:\n{}", result.stderr);

            // Parse the Python exception
            if let Some(SandboxError::PythonException {
                exception_type,
                message,
                traceback,
            }) = parse_python_exception(&result.stderr)
            {
                println!("\nParsed exception:");
                println!("  Type: {}", exception_type);
                println!("  Message: {}", message);
                if let Some(tb) = traceback {
                    println!("  Has traceback: {} lines", tb.lines().count());
                }
            }
        }
    }
    println!();

    // Example 2: NameError
    println!("--- Test 2: Python NameError ---");
    {
        let code = "print(undefined_variable)";
        let result = sandbox.execute(code, None).await?;

        if !result.is_success() {
            if let Some(exc) = parse_python_exception(&result.stderr) {
                println!("Caught: {}", exc);
            }
        }
    }
    println!();

    // Example 3: TypeError
    println!("--- Test 3: Python TypeError ---");
    {
        let code = "'string' + 42";
        let result = sandbox.execute(code, None).await?;

        if !result.is_success() {
            if let Some(exc) = parse_python_exception(&result.stderr) {
                println!("Caught: {}", exc);
            }
        }
    }
    println!();

    // Example 4: ZeroDivisionError
    println!("--- Test 4: Python ZeroDivisionError ---");
    {
        let code = "print(1/0)";
        let result = sandbox.execute(code, None).await?;

        if !result.is_success() {
            if let Some(exc) = parse_python_exception(&result.stderr) {
                println!("Caught: {}", exc);
            }
        }
    }
    println!();

    // Example 5: Custom exception
    println!("--- Test 5: Custom Exception ---");
    {
        let code = r#"
class CustomError(Exception):
    pass

raise CustomError("This is a custom error with details")
"#;
        let result = sandbox.execute(code, None).await?;

        if !result.is_success() {
            println!("stderr:\n{}", result.stderr);
        }
    }
    println!();

    // Example 6: Syntax error
    println!("--- Test 6: Syntax Error ---");
    {
        let code = "if True print('missing colon')";
        let result = sandbox.execute(code, None).await?;

        if !result.is_success() {
            if let Some(exc) = parse_python_exception(&result.stderr) {
                println!("Caught: {}", exc);
            }
        }
    }
    println!();

    // Example 7: Handling successful execution
    println!("--- Test 7: Successful Execution ---");
    {
        let code = r#"
def calculate(x, y):
    """Safe calculation with error handling."""
    try:
        return x / y
    except ZeroDivisionError:
        return None

results = [
    calculate(10, 2),
    calculate(10, 0),
    calculate(15, 3),
]

for i, r in enumerate(results):
    if r is not None:
        print(f"Result {i+1}: {r}")
    else:
        print(f"Result {i+1}: Division by zero handled")
"#;
        let result = sandbox.execute(code, None).await?;

        if result.is_success() {
            println!("Execution successful!");
            println!("Output:\n{}", result.stdout);
        }
    }
    println!();

    // Example 8: Error classification helper
    println!("--- Test 8: Error Classification ---");
    {
        async fn classify_error(sandbox: &PythonSandbox, code: &str) -> String {
            match sandbox.execute(code, None).await {
                Ok(result) if result.is_success() => "Success".to_string(),
                Ok(result) => {
                    // Parse the Python error
                    if let Some(SandboxError::PythonException { exception_type, .. }) =
                        parse_python_exception(&result.stderr)
                    {
                        return format!("Python {}", exception_type);
                    }
                    format!("Python error (exit code {})", result.exit_code)
                }
                Err(e) if e.is_timeout() => "Timeout".to_string(),
                Err(e) if e.is_memory_limit() => "Memory limit".to_string(),
                Err(e) => format!("Sandbox error: {}", e),
            }
        }

        let test_cases = vec![
            ("print(42)", "Simple print"),
            ("int('x')", "Invalid conversion"),
            ("x = 1/0", "Division by zero"),
            ("def f(): f()\nf()", "Recursion limit"),
        ];

        println!("Classification results:");
        for (code, description) in test_cases {
            let classification = classify_error(&sandbox, code).await;
            println!("  {}: {}", description, classification);
        }
    }

    println!("\n=== Error Handling Example Complete ===");
    Ok(())
}
