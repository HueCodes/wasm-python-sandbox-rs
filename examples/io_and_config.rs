//! Example demonstrating stdin, environment variables, and prelude usage.
//!
//! This example shows how to:
//! - Provide stdin input to Python code
//! - Set environment variables
//! - Use prelude scripts for setup code
//!
//! Run with: cargo run --example io_and_config
//!
//! Note: Requires rustpython.wasm to be present in assets/

use std::time::Duration;
use wasm_python_sandbox_rs::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== I/O and Configuration Example ===\n");

    let interpreter_path = "assets/rustpython.wasm";

    // Example 1: Stdin input
    println!("--- Test 1: Reading from Stdin ---");
    {
        let config = SandboxConfig::builder()
            .timeout(Duration::from_secs(5))
            .max_memory(64 * 1024 * 1024)
            .interpreter_path(interpreter_path)
            .stdin("Alice\n25\nEngineer")
            .build();

        let sandbox = PythonSandbox::new(config)?;

        let code = r#"
name = input()
age = input()
job = input()
print(f"Hello {name}! You are {age} years old and work as a {job}.")
"#;

        let result = sandbox.execute(code, None).await?;
        println!("Output: {}", result.stdout.trim());
    }
    println!();

    // Example 2: Environment variables
    println!("--- Test 2: Environment Variables ---");
    {
        let config = SandboxConfig::builder()
            .timeout(Duration::from_secs(5))
            .max_memory(64 * 1024 * 1024)
            .interpreter_path(interpreter_path)
            .env("API_KEY", "secret-key-12345")
            .env("DEBUG_MODE", "true")
            .env("MAX_RETRIES", "3")
            .envs([("APP_NAME", "MySandboxApp"), ("VERSION", "1.0.0")])
            .build();

        let sandbox = PythonSandbox::new(config)?;

        let code = r#"
import os

print("Environment variables:")
for key in ['API_KEY', 'DEBUG_MODE', 'MAX_RETRIES', 'APP_NAME', 'VERSION']:
    value = os.environ.get(key, 'NOT SET')
    # Mask sensitive values
    if 'KEY' in key or 'SECRET' in key:
        value = value[:4] + '...' if len(value) > 4 else '****'
    print(f"  {key}: {value}")
"#;

        let result = sandbox.execute(code, None).await?;
        println!("{}", result.stdout);
    }
    println!();

    // Example 3: Prelude script
    println!("--- Test 3: Prelude Script ---");
    {
        let prelude = r#"
# Define helper functions and constants available to all user code
import math

def safe_sqrt(x):
    """Return square root or None if negative."""
    if x < 0:
        return None
    return math.sqrt(x)

def clamp(value, min_val, max_val):
    """Clamp value to range [min_val, max_val]."""
    return max(min_val, min(max_val, value))

# Constants
PI = math.pi
E = math.e
GOLDEN_RATIO = (1 + math.sqrt(5)) / 2
"#;

        let config = SandboxConfig::builder()
            .timeout(Duration::from_secs(5))
            .max_memory(64 * 1024 * 1024)
            .interpreter_path(interpreter_path)
            .prelude(prelude)
            .build();

        let sandbox = PythonSandbox::new(config)?;

        // User code can use the prelude functions directly
        let code = r#"
# Use prelude helpers without importing
print(f"safe_sqrt(16) = {safe_sqrt(16)}")
print(f"safe_sqrt(-4) = {safe_sqrt(-4)}")
print(f"clamp(150, 0, 100) = {clamp(150, 0, 100)}")
print(f"PI = {PI:.6f}")
print(f"GOLDEN_RATIO = {GOLDEN_RATIO:.6f}")
"#;

        let result = sandbox.execute(code, None).await?;
        println!("{}", result.stdout);
    }
    println!();

    // Example 4: Combined usage
    println!("--- Test 4: Combined Stdin + Env + Prelude ---");
    {
        let prelude = r#"
import os

def get_config(key, default=None):
    """Get configuration from environment."""
    return os.environ.get(key, default)

def process_input(data):
    """Process input according to configuration."""
    uppercase = get_config('UPPERCASE', 'false').lower() == 'true'
    prefix = get_config('PREFIX', '')

    if uppercase:
        data = data.upper()
    if prefix:
        data = f"{prefix}: {data}"
    return data
"#;

        let config = SandboxConfig::builder()
            .timeout(Duration::from_secs(5))
            .max_memory(64 * 1024 * 1024)
            .interpreter_path(interpreter_path)
            .stdin("hello world\ngoodbye world")
            .env("UPPERCASE", "true")
            .env("PREFIX", "[PROCESSED]")
            .prelude(prelude)
            .build();

        let sandbox = PythonSandbox::new(config)?;

        let code = r#"
# Read lines from stdin and process them
line1 = input()
line2 = input()

print(process_input(line1))
print(process_input(line2))
"#;

        let result = sandbox.execute(code, None).await?;
        println!("{}", result.stdout);
    }
    println!();

    // Example 5: Using input parameter
    println!("--- Test 5: Using execute() input Parameter ---");
    {
        let config = SandboxConfig::builder()
            .timeout(Duration::from_secs(5))
            .max_memory(64 * 1024 * 1024)
            .interpreter_path(interpreter_path)
            .build();

        let sandbox = PythonSandbox::new(config)?;

        // Different inputs for the same code
        let code = "name = input()\nprint(f'Hello, {name}!')";

        for name in ["Alice", "Bob", "Charlie"] {
            let result = sandbox.execute(code, Some(name)).await?;
            println!("{}", result.stdout.trim());
        }
    }

    println!("\n=== I/O and Configuration Example Complete ===");
    Ok(())
}
