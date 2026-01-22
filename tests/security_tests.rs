//! Security tests to verify sandbox isolation.
//!
//! These tests attempt various escape techniques to verify the sandbox
//! properly restricts access to the host system.

use std::time::Duration;
use wasm_python_sandbox_rs::prelude::*;

/// Helper to create a test sandbox config.
fn test_config() -> SandboxConfig {
    SandboxConfig::builder()
        .timeout(Duration::from_secs(5))
        .max_memory(32 * 1024 * 1024)
        .build()
}

/// Test that infinite loops are properly terminated.
#[tokio::test]
#[ignore = "requires rustpython.wasm"]
async fn test_infinite_loop_timeout() {
    let config = SandboxConfig::builder()
        .timeout(Duration::from_millis(500))
        .max_memory(32 * 1024 * 1024)
        .build();

    let sandbox = PythonSandbox::new(config).unwrap();

    let result = sandbox.execute("while True: pass", None).await;
    assert!(
        matches!(result, Err(SandboxError::Timeout(_))),
        "infinite loop should timeout"
    );
}

/// Test that filesystem access is blocked.
#[tokio::test]
#[ignore = "requires rustpython.wasm"]
async fn test_filesystem_access_blocked() {
    let sandbox = PythonSandbox::new(test_config()).unwrap();

    // Try to read /etc/passwd
    let result = sandbox
        .execute(
            r#"
try:
    with open('/etc/passwd', 'r') as f:
        print(f.read())
    print('SECURITY_BREACH: file read succeeded')
except Exception as e:
    print(f'BLOCKED: {type(e).__name__}')
"#,
            None,
        )
        .await;

    if let Ok(res) = result {
        assert!(
            !res.stdout.contains("SECURITY_BREACH"),
            "filesystem access should be blocked"
        );
        assert!(res.stdout.contains("BLOCKED"), "should raise an exception");
    }
    // Execution error is also acceptable
}

/// Test that os module dangerous operations are restricted.
#[tokio::test]
#[ignore = "requires rustpython.wasm"]
async fn test_os_operations_blocked() {
    let sandbox = PythonSandbox::new(test_config()).unwrap();

    let result = sandbox
        .execute(
            r#"
import os
try:
    os.system('echo BREACH')
    print('SECURITY_BREACH: os.system succeeded')
except Exception as e:
    print(f'BLOCKED: {type(e).__name__}')
"#,
            None,
        )
        .await;

    if let Ok(res) = result {
        assert!(
            !res.stdout.contains("SECURITY_BREACH"),
            "os.system should be blocked"
        );
    }
    // Execution error is acceptable
}

/// Test that subprocess module is restricted.
#[tokio::test]
#[ignore = "requires rustpython.wasm"]
async fn test_subprocess_blocked() {
    let sandbox = PythonSandbox::new(test_config()).unwrap();

    let result = sandbox
        .execute(
            r#"
try:
    import subprocess
    subprocess.run(['echo', 'BREACH'])
    print('SECURITY_BREACH: subprocess succeeded')
except Exception as e:
    print(f'BLOCKED: {type(e).__name__}')
"#,
            None,
        )
        .await;

    if let Ok(res) = result {
        assert!(
            !res.stdout.contains("SECURITY_BREACH"),
            "subprocess should be blocked"
        );
    }
}

/// Test that network access is blocked.
#[tokio::test]
#[ignore = "requires rustpython.wasm"]
async fn test_network_access_blocked() {
    let sandbox = PythonSandbox::new(test_config()).unwrap();

    let result = sandbox
        .execute(
            r#"
try:
    import socket
    s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    s.connect(('8.8.8.8', 53))
    print('SECURITY_BREACH: network access succeeded')
except Exception as e:
    print(f'BLOCKED: {type(e).__name__}')
"#,
            None,
        )
        .await;

    if let Ok(res) = result {
        assert!(
            !res.stdout.contains("SECURITY_BREACH"),
            "network access should be blocked"
        );
    }
}

/// Test memory exhaustion protection.
#[tokio::test]
#[ignore = "requires rustpython.wasm"]
async fn test_memory_exhaustion_protection() {
    let config = SandboxConfig::builder()
        .timeout(Duration::from_secs(5))
        .max_memory(16 * 1024 * 1024) // 16MB limit
        .build();

    let sandbox = PythonSandbox::new(config).unwrap();

    let result = sandbox
        .execute(
            r#"
# Try to allocate a large list
data = []
for i in range(100000000):
    data.append('x' * 1000)
print('SECURITY_BREACH: memory exhaustion succeeded')
"#,
            None,
        )
        .await;

    // Should either get memory limit error or timeout
    match result {
        Ok(res) => {
            assert!(
                !res.stdout.contains("SECURITY_BREACH"),
                "memory exhaustion should be prevented"
            );
        }
        Err(e) => {
            // Memory limit or timeout error is expected
            assert!(
                matches!(e, SandboxError::MemoryLimitExceeded(_))
                    || matches!(e, SandboxError::Timeout(_))
                    || matches!(e, SandboxError::ExecutionFailed(_))
            );
        }
    }
}

/// Test ctypes/cffi escape attempts.
#[tokio::test]
#[ignore = "requires rustpython.wasm"]
async fn test_ctypes_blocked() {
    let sandbox = PythonSandbox::new(test_config()).unwrap();

    let result = sandbox
        .execute(
            r#"
try:
    import ctypes
    libc = ctypes.CDLL(None)
    print('SECURITY_BREACH: ctypes loaded')
except Exception as e:
    print(f'BLOCKED: {type(e).__name__}')
"#,
            None,
        )
        .await;

    if let Ok(res) = result {
        assert!(
            !res.stdout.contains("SECURITY_BREACH"),
            "ctypes should be blocked"
        );
    }
}

/// Test eval/exec with malicious code.
#[tokio::test]
#[ignore = "requires rustpython.wasm"]
async fn test_eval_exec_sandboxed() {
    let sandbox = PythonSandbox::new(test_config()).unwrap();

    let result = sandbox
        .execute(
            r#"
# Even eval/exec should be sandboxed
exec("import os; os.system('echo BREACH')")
print('SECURITY_BREACH: eval/exec bypass')
"#,
            None,
        )
        .await;

    if let Ok(res) = result {
        assert!(
            !res.stdout.contains("SECURITY_BREACH"),
            "eval/exec should remain sandboxed"
        );
    }
}

/// Test that importing __builtins__ manipulation is sandboxed.
#[tokio::test]
#[ignore = "requires rustpython.wasm"]
async fn test_builtins_manipulation() {
    let sandbox = PythonSandbox::new(test_config()).unwrap();

    let result = sandbox
        .execute(
            r#"
try:
    __builtins__.__import__('os').system('echo BREACH')
    print('SECURITY_BREACH: builtins bypass')
except Exception as e:
    print(f'BLOCKED: {type(e).__name__}')
"#,
            None,
        )
        .await;

    if let Ok(res) = result {
        assert!(
            !res.stdout.contains("SECURITY_BREACH"),
            "__builtins__ manipulation should be sandboxed"
        );
    }
}

/// Test pickle deserialization attacks.
#[tokio::test]
#[ignore = "requires rustpython.wasm"]
async fn test_pickle_attack() {
    let sandbox = PythonSandbox::new(test_config()).unwrap();

    let result = sandbox
        .execute(
            r#"
import pickle
import base64

# Malicious pickle that tries to run os.system
malicious = base64.b64decode(b'Y29zCnN5c3RlbQooUydlY2hvIEJSRUFDSCcKdFIu')
try:
    pickle.loads(malicious)
    print('SECURITY_BREACH: pickle attack succeeded')
except Exception as e:
    print(f'BLOCKED: {type(e).__name__}')
"#,
            None,
        )
        .await;

    if let Ok(res) = result {
        assert!(
            !res.stdout.contains("SECURITY_BREACH"),
            "pickle attacks should be blocked"
        );
    }
}
