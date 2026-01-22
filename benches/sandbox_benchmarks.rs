//! Benchmarks for the Python sandbox.
//!
//! Run with: cargo bench
//!
//! These benchmarks require rustpython.wasm to be present at assets/rustpython.wasm

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::time::Duration;
use tokio::runtime::Runtime;
use wasm_python_sandbox_rs::prelude::*;

/// Get the path to the interpreter, checking if it exists.
fn get_interpreter_path() -> Option<std::path::PathBuf> {
    let path = std::path::PathBuf::from("assets/rustpython.wasm");
    if path.exists() {
        Some(path)
    } else {
        None
    }
}

/// Benchmark cold start (new sandbox creation without cache).
fn bench_cold_start(c: &mut Criterion) {
    let Some(interpreter_path) = get_interpreter_path() else {
        eprintln!("Skipping cold_start benchmark: rustpython.wasm not found");
        return;
    };

    let mut group = c.benchmark_group("cold_start");
    group.sample_size(10); // Reduced sample size due to compilation time

    group.bench_function("sandbox_creation_no_cache", |b| {
        b.iter(|| {
            // Clear the global cache before each iteration
            global_cache().clear();

            let config = SandboxConfig::builder()
                .interpreter_path(&interpreter_path)
                .timeout(Duration::from_secs(30))
                .max_memory(64 * 1024 * 1024)
                .build();

            let options = SandboxOptions::no_cache();
            let sandbox = PythonSandbox::new_with_options(config, options).unwrap();
            black_box(sandbox)
        });
    });

    group.finish();
}

/// Benchmark warm execution (cached module).
fn bench_warm_execution(c: &mut Criterion) {
    let Some(interpreter_path) = get_interpreter_path() else {
        eprintln!("Skipping warm_execution benchmark: rustpython.wasm not found");
        return;
    };

    // Pre-warm the cache
    let config = SandboxConfig::builder()
        .interpreter_path(&interpreter_path)
        .timeout(Duration::from_secs(30))
        .max_memory(64 * 1024 * 1024)
        .build();
    let _ = PythonSandbox::new(config.clone()).unwrap();

    let mut group = c.benchmark_group("warm_execution");

    group.bench_function("sandbox_creation_with_cache", |b| {
        b.iter(|| {
            let sandbox = PythonSandbox::new(config.clone()).unwrap();
            assert!(sandbox.is_using_cached_module());
            black_box(sandbox)
        });
    });

    group.finish();
}

/// Benchmark Python code execution.
fn bench_execution(c: &mut Criterion) {
    let Some(interpreter_path) = get_interpreter_path() else {
        eprintln!("Skipping execution benchmark: rustpython.wasm not found");
        return;
    };

    let rt = Runtime::new().unwrap();
    let config = SandboxConfig::builder()
        .interpreter_path(&interpreter_path)
        .timeout(Duration::from_secs(30))
        .max_memory(64 * 1024 * 1024)
        .build();

    let sandbox = PythonSandbox::new(config).unwrap();

    let mut group = c.benchmark_group("execution");

    // Simple arithmetic
    group.bench_function("simple_print", |b| {
        b.iter(|| {
            let result = rt.block_on(sandbox.execute("print(1 + 1)", None)).unwrap();
            black_box(result)
        });
    });

    // Loop computation
    group.bench_function("loop_100", |b| {
        b.iter(|| {
            let result = rt
                .block_on(
                    sandbox.execute("sum = 0\nfor i in range(100): sum += i\nprint(sum)", None),
                )
                .unwrap();
            black_box(result)
        });
    });

    // String manipulation
    group.bench_function("string_ops", |b| {
        b.iter(|| {
            let result = rt
                .block_on(sandbox.execute("s = 'hello' * 100\nprint(len(s))", None))
                .unwrap();
            black_box(result)
        });
    });

    group.finish();
}

/// Benchmark concurrent execution throughput.
fn bench_concurrent_execution(c: &mut Criterion) {
    let Some(interpreter_path) = get_interpreter_path() else {
        eprintln!("Skipping concurrent benchmark: rustpython.wasm not found");
        return;
    };

    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("concurrent");
    group.sample_size(10);

    for concurrency in [1, 2, 4, 8].iter() {
        group.throughput(Throughput::Elements(*concurrency as u64));
        group.bench_with_input(
            BenchmarkId::new("executions", concurrency),
            concurrency,
            |b, &concurrency| {
                b.iter(|| {
                    rt.block_on(async {
                        let mut handles = Vec::new();

                        for _ in 0..concurrency {
                            let path = interpreter_path.clone();
                            let handle = tokio::spawn(async move {
                                let config = SandboxConfig::builder()
                                    .interpreter_path(&path)
                                    .timeout(Duration::from_secs(30))
                                    .max_memory(64 * 1024 * 1024)
                                    .build();
                                let sandbox = PythonSandbox::new(config).unwrap();
                                sandbox.execute("print(1 + 1)", None).await.unwrap()
                            });
                            handles.push(handle);
                        }

                        for handle in handles {
                            let result = handle.await.unwrap();
                            black_box(result);
                        }
                    });
                });
            },
        );
    }

    group.finish();
}

/// Benchmark with shared engine for better resource utilization.
fn bench_shared_engine(c: &mut Criterion) {
    let Some(interpreter_path) = get_interpreter_path() else {
        eprintln!("Skipping shared_engine benchmark: rustpython.wasm not found");
        return;
    };

    let rt = Runtime::new().unwrap();
    let shared_engine = SharedEngine::new().unwrap();

    let mut group = c.benchmark_group("shared_engine");

    group.bench_function("multiple_sandboxes_shared_engine", |b| {
        b.iter(|| {
            rt.block_on(async {
                let mut handles = Vec::new();

                for _ in 0..4 {
                    let path = interpreter_path.clone();
                    let engine = shared_engine.clone();
                    let handle = tokio::spawn(async move {
                        let config = SandboxConfig::builder()
                            .interpreter_path(&path)
                            .timeout(Duration::from_secs(30))
                            .max_memory(64 * 1024 * 1024)
                            .build();
                        let options = SandboxOptions::with_engine(engine);
                        let sandbox = PythonSandbox::new_with_options(config, options).unwrap();
                        sandbox.execute("print(1 + 1)", None).await.unwrap()
                    });
                    handles.push(handle);
                }

                for handle in handles {
                    let result = handle.await.unwrap();
                    black_box(result);
                }
            });
        });
    });

    group.finish();
}

/// Benchmark epoch vs fuel limiting (when fuel is implemented).
fn bench_limiting_mechanisms(c: &mut Criterion) {
    let Some(interpreter_path) = get_interpreter_path() else {
        eprintln!("Skipping limiting benchmark: rustpython.wasm not found");
        return;
    };

    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("limiting");

    // Epoch-only limiting
    group.bench_function("epoch_only", |b| {
        let config = SandboxConfig::builder()
            .interpreter_path(&interpreter_path)
            .timeout(Duration::from_secs(30))
            .max_memory(64 * 1024 * 1024)
            .epoch_tick_interval(Duration::from_millis(10))
            .build();
        let sandbox = PythonSandbox::new(config).unwrap();

        b.iter(|| {
            let result = rt
                .block_on(sandbox.execute("for i in range(1000): pass", None))
                .unwrap();
            black_box(result)
        });
    });

    // Epoch + fuel limiting
    group.bench_function("epoch_and_fuel", |b| {
        let config = SandboxConfig::builder()
            .interpreter_path(&interpreter_path)
            .timeout(Duration::from_secs(30))
            .max_memory(64 * 1024 * 1024)
            .epoch_tick_interval(Duration::from_millis(10))
            .max_fuel(100_000_000) // High fuel limit
            .build();

        // Need a new engine with fuel enabled
        let engine = SharedEngine::with_fuel().unwrap();
        let options = SandboxOptions::with_engine(engine);
        let sandbox = PythonSandbox::new_with_options(config, options).unwrap();

        b.iter(|| {
            let result = rt
                .block_on(sandbox.execute("for i in range(1000): pass", None))
                .unwrap();
            black_box(result)
        });
    });

    group.finish();
}

/// Benchmark memory limit edge cases.
fn bench_memory_limits(c: &mut Criterion) {
    let Some(interpreter_path) = get_interpreter_path() else {
        eprintln!("Skipping memory_limits benchmark: rustpython.wasm not found");
        return;
    };

    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("memory_limits");
    group.sample_size(10);

    for memory_mb in [16, 32, 64, 128].iter() {
        group.bench_with_input(
            BenchmarkId::new("allocation", format!("{}MB", memory_mb)),
            memory_mb,
            |b, &memory_mb| {
                let config = SandboxConfig::builder()
                    .interpreter_path(&interpreter_path)
                    .timeout(Duration::from_secs(30))
                    .max_memory(memory_mb as u64 * 1024 * 1024)
                    .build();
                let sandbox = PythonSandbox::new(config).unwrap();

                b.iter(|| {
                    // Allocate some memory but stay within limits
                    let result = rt
                        .block_on(
                            sandbox.execute(
                                "data = [i for i in range(10000)]\nprint(len(data))",
                                None,
                            ),
                        )
                        .unwrap();
                    black_box(result)
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_cold_start,
    bench_warm_execution,
    bench_execution,
    bench_concurrent_execution,
    bench_shared_engine,
    bench_limiting_mechanisms,
    bench_memory_limits,
);

criterion_main!(benches);
