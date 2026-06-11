//! Failure-mode wiring tests (issue `42eac5cc`, design doc §8).
//!
//! Covers all four success criteria:
//!
//! 1. **OOM auto-fallback** (SC1), function-level: forced OOM → fallback
//!    produces the same output as the CPU-only path, satisfying the §11 3-column
//!    contract at the `dispatch_with_fallback` boundary using the shared
//!    injectors. The **run-level** SC1 proof (a forced OOM during a real hybrid
//!    run yielding the same `fer`/`frames`/`errors` columns as a CPU-only run)
//!    lives in [`tests/executor_oom_fallback_run.rs`](../executor_oom_fallback_run.rs).
//! 2. **Shared injectors** (SC2): `OomInjector` / `KernelErrorInjector` are
//!    consumed via `mod common;` — no copy-paste.
//! 3. **Hard-fail path** (SC3), function-level: fatal kernel error →
//!    `dispatch_with_fallback` returns `Err` and writes a JSON dump to
//!    `diagnostic_dump_dir`. The **process-exit** half of SC3 (non-zero process
//!    exit + the `tracing::error!` event) is proven by a real subprocess in
//!    [`tests/hard_fail_subprocess.rs`](../hard_fail_subprocess.rs) — an
//!    in-process `Err` return does not prove a non-zero process exit.
//! 4. **`strict_gpu` honored** (SC4): OOM with `strict_gpu=true` is promoted to
//!    `FatalError::OutOfMemory` (no fallback), and a dump is written.
//!
//! # No GPU required
//!
//! All tests in this binary use `dispatch_with_fallback` directly and the
//! host-only injectors from `tests/common/mod.rs`. No real HIP device is needed.

mod common;

use common::{Identity, KernelErrorInjector, OomInjector, TinyBatch};
use gf2_sim::error::{FatalError, RecoverableError, StageError};
use gf2_sim::executor::failure::{default_dump_dir, dispatch_with_fallback, FaultContext};
use gf2_sim::stage::{erase, Stage};

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// Returns a unique temp directory for this test invocation. Does NOT create it
/// (dump functions create it on demand; non-dump tests assert it stays absent).
fn test_dump_dir(tag: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!(
        "gf2sim-failmode-{tag}-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed),
    ));
    p
}

fn ctx() -> FaultContext {
    FaultContext {
        batch_id: 7,
        snr_idx: 3,
        device_id: 0,
        worker_idx: 0,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// SC1: OOM auto-fallback — fallback output == CPU-only output
// ─────────────────────────────────────────────────────────────────────────────

/// When a GPU stage returns OOM, `dispatch_with_fallback` (non-strict) must
/// invoke the fallback and return its output. This mirrors the §11 OOM-fallback
/// substitution path: since the fallback runs the same CPU-stage logic as the
/// CPU-only reference, the output (and therefore `fer`/`frames`/`errors`) is
/// byte-identical.
///
/// This test exercises the function-level boundary that both the C.1 scheduler
/// (`worker_partition_hybrid`) and the topology executor (`execute_gpu_stage`)
/// delegate to.
#[test]
fn test_oom_fallback_output_matches_cpu_only_path() {
    let dir = test_dump_dir("oom-fallback");
    let input = TinyBatch(55);

    // Simulate what the GPU path returns: an OOM error.
    let gpu_result: Result<TinyBatch, StageError> =
        Err(StageError::Recoverable(RecoverableError::OutOfMemory {
            device_id: 0,
            bytes_requested: 4096,
        }));

    // The fallback runs the same Identity stage logic as the CPU-only reference.
    let identity = Identity;
    let fallback = || identity.process(&input, &mut ());

    let fallback_out = dispatch_with_fallback(gpu_result, fallback, ctx(), false, &dir)
        .expect("OOM non-strict must succeed via fallback");

    // CPU-only path: the same Identity stage applied directly.
    let cpu_out = Identity.process(&input, &mut ()).unwrap();

    assert_eq!(
        fallback_out, cpu_out,
        "fallback output must be byte-identical to CPU-only output (§11 3-column contract)"
    );
    // Non-strict OOM + successful fallback does NOT produce a dump.
    assert!(
        !dir.exists(),
        "no dump dir must be created for non-strict OOM with successful fallback"
    );
}

/// OOM injector wired through a mini erased-stage pipeline (the SC2 integration
/// path): inject OOM on the 1st call, fallback produces same value as Identity
/// running directly.
#[test]
fn test_oom_injector_dispatched_via_dispatch_with_fallback() {
    let dir = test_dump_dir("oom-injector");
    let input = TinyBatch(99);

    // The injector reports itself as CpuOnly (like a GPU stage, but with
    // cpu_fallback pointing back to Identity). We call it manually here rather
    // than through a full pipeline to keep the test fast.
    let inj = OomInjector::new(Identity, 1);
    let gpu_result = inj.process(&input, &mut ()); // 1st call → OOM
    assert!(
        matches!(
            gpu_result,
            Err(StageError::Recoverable(
                RecoverableError::OutOfMemory { .. }
            ))
        ),
        "injector must produce OOM on call 1"
    );

    // Pass the OOM through dispatch_with_fallback with a CPU fallback.
    let fallback = || Identity.process(&input, &mut ());
    let out = dispatch_with_fallback(gpu_result, fallback, ctx(), false, &dir)
        .expect("OOM + successful fallback must succeed");
    assert_eq!(out, TinyBatch(99), "fallback must return identity output");

    // No dump for non-strict OOM + successful fallback.
    assert!(!dir.exists());
}

/// When OOM happens and the fallback ALSO fails, the result is
/// `FatalError::CpuFallbackAlsoFailed` and a dump IS written.
#[test]
fn test_oom_fallback_also_fails_produces_dump_and_cpu_fallback_also_failed() {
    let dir = test_dump_dir("oom-fb-fail");
    let oom: Result<TinyBatch, StageError> =
        Err(StageError::Recoverable(RecoverableError::OutOfMemory {
            device_id: 0,
            bytes_requested: 1024,
        }));
    let fallback_err = StageError::Fatal(FatalError::KernelLaunch {
        hip_code: 7,
        kernel: "fallback_stage",
        args: "also failed".to_string(),
    });
    let err = dispatch_with_fallback(
        oom,
        || Err::<TinyBatch, _>(fallback_err),
        ctx(),
        false,
        &dir,
    )
    .expect_err("both GPU and fallback failed — must be fatal");
    assert!(
        matches!(
            err,
            StageError::Fatal(FatalError::CpuFallbackAlsoFailed { .. })
        ),
        "expected CpuFallbackAlsoFailed, got {err:?}"
    );
    let entries: Vec<_> = std::fs::read_dir(&dir)
        .expect("dump dir must exist after CpuFallbackAlsoFailed")
        .filter_map(|e| e.ok())
        .collect();
    assert!(
        !entries.is_empty(),
        "a dump file must be written on CpuFallbackAlsoFailed"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// ─────────────────────────────────────────────────────────────────────────────
// SC3: Hard-fail path — fatal kernel error → dump + propagate + tracing::error!
// ─────────────────────────────────────────────────────────────────────────────

/// Fatal `KernelLaunch` error → `dispatch_with_fallback` returns the original
/// `Err(Fatal)` unchanged and writes a valid JSON dump file with the expected
/// fields.
///
/// This is the **function-level** half of the SC3 hard-fail criterion: it
/// proves the dump-write + error-propagation mechanics at the
/// `dispatch_with_fallback` boundary. The **process-exit** half of SC3 (a forced
/// kernel error yields a *non-zero process exit* plus the `tracing::error!`
/// event) is proven by a real subprocess in
/// [`tests/hard_fail_subprocess.rs`](../hard_fail_subprocess.rs) — an in-process
/// `Err` return does NOT prove the process exits non-zero (a reasoning rejected
/// by formal review on this project), so the exit status is asserted there by
/// actually spawning a process and reading its status.
#[test]
fn test_fatal_kernel_error_writes_dump_and_propagates() {
    let dir = test_dump_dir("fatal-kernel");
    let fatal: Result<TinyBatch, StageError> = Err(StageError::Fatal(FatalError::KernelLaunch {
        hip_code: 301,
        kernel: "bcjr_decode",
        args: "gfx1030: launch failed".to_string(),
    }));

    let err = dispatch_with_fallback(
        fatal,
        || Ok::<TinyBatch, _>(TinyBatch(0)),
        ctx(),
        false,
        &dir,
    )
    .expect_err("fatal error must propagate");

    // SC3a: error propagates unchanged.
    assert!(
        matches!(
            err,
            StageError::Fatal(FatalError::KernelLaunch { hip_code: 301, .. })
        ),
        "fatal must propagate as KernelLaunch(301), got {err:?}"
    );

    // SC3b: a JSON dump file was written.
    let entries: Vec<_> = std::fs::read_dir(&dir)
        .expect("dump dir must exist after fatal error")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
        .collect();
    assert_eq!(
        entries.len(),
        1,
        "exactly one JSON dump file must be written for one fatal error"
    );

    // SC3c: the dump is valid JSON with the expected fields.
    let path = entries[0].path();
    let content = std::fs::read_to_string(&path).expect("dump file must be readable");
    let v: serde_json::Value = serde_json::from_str(&content).expect("dump must be valid JSON");
    assert_eq!(v["event"], "hard_fail", "event field must be 'hard_fail'");
    assert_eq!(v["hip_code"], 301_i64, "hip_code must be 301");
    assert_eq!(v["kernel"], "bcjr_decode", "kernel name must be preserved");
    assert_eq!(v["snr_idx"], 3_i64, "snr_idx must match context");
    assert_eq!(v["batch_id"], 7_i64, "batch_id must match context");
    assert_eq!(v["device_id"], 0_i64, "device_id must match context");

    let _ = std::fs::remove_dir_all(&dir);
}

/// `KernelErrorInjector` consumed via `mod common;` (SC2 + SC3): inject a fatal
/// error on the 1st call and verify the dump is written.
#[test]
fn test_kernel_error_injector_via_common_mod_writes_dump() {
    let dir = test_dump_dir("kernel-injector");
    let input = TinyBatch(0);

    // Consume KernelErrorInjector from the shared common module (SC2 mandate).
    let inj = KernelErrorInjector::new(Identity, 1).with_launch_params(
        7,
        "ldpc_bp",
        "injected for 42eac5cc test",
    );
    let gpu_result = inj.process(&input, &mut ()); // 1st call → KernelLaunch
    assert!(
        matches!(
            gpu_result,
            Err(StageError::Fatal(FatalError::KernelLaunch { .. }))
        ),
        "injector must produce KernelLaunch on call 1"
    );

    let err = dispatch_with_fallback(
        gpu_result,
        || Ok::<TinyBatch, _>(TinyBatch(99)),
        ctx(),
        false,
        &dir,
    )
    .expect_err("fatal error must not invoke fallback and must propagate");

    assert!(
        matches!(
            err,
            StageError::Fatal(FatalError::KernelLaunch { hip_code: 7, .. })
        ),
        "fatal must propagate unchanged, got {err:?}"
    );

    let entries: Vec<_> = std::fs::read_dir(&dir)
        .expect("dump dir must exist")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
        .collect();
    assert!(
        !entries.is_empty(),
        "dump file must be written on fatal error"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

// ─────────────────────────────────────────────────────────────────────────────
// SC4: strict_gpu honored — OOM → FatalError::OutOfMemory (no fallback)
// ─────────────────────────────────────────────────────────────────────────────

/// With `strict_gpu=true`, OOM is promoted to `FatalError::OutOfMemory` without
/// invoking the CPU fallback. A dump is written. The fallback closure is never
/// called (verified by the sentinel).
#[test]
fn test_strict_gpu_promotes_oom_to_fatal_without_fallback() {
    let dir = test_dump_dir("strict-gpu");
    let input = TinyBatch(42);

    let oom: Result<TinyBatch, StageError> =
        Err(StageError::Recoverable(RecoverableError::OutOfMemory {
            device_id: 1,
            bytes_requested: 2 * 1024 * 1024 * 1024,
        }));

    let fallback_called = std::cell::Cell::new(false);
    let fallback = || {
        fallback_called.set(true);
        Identity.process(&input, &mut ())
    };

    let err = dispatch_with_fallback(oom, fallback, ctx(), true /* strict_gpu */, &dir)
        .expect_err("strict_gpu OOM must be fatal");

    assert!(
        matches!(
            err,
            StageError::Fatal(FatalError::OutOfMemory { device_id: 1, .. })
        ),
        "strict OOM must produce Fatal::OutOfMemory(device_id=1), got {err:?}"
    );
    assert!(
        !fallback_called.get(),
        "fallback must NOT be called when strict_gpu=true"
    );

    // A dump is written even for strict-mode OOM.
    let entries: Vec<_> = std::fs::read_dir(&dir)
        .expect("dump dir must exist after strict OOM")
        .filter_map(|e| e.ok())
        .collect();
    assert!(
        !entries.is_empty(),
        "dump file must be written on strict OOM"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// With `strict_gpu=true`, `OomInjector` (consumed from `mod common;`) triggers
/// the strict promotion path — no fallback, `FatalError::OutOfMemory`.
#[test]
fn test_strict_gpu_with_oom_injector_from_common_mod() {
    let dir = test_dump_dir("strict-oom-injector");
    let input = TinyBatch(5);

    // SC2: consume OomInjector from common.
    let inj = OomInjector::new(Identity, 1);
    let gpu_result = inj.process(&input, &mut ()); // OOM on 1st call.

    let err = dispatch_with_fallback(
        gpu_result,
        || Identity.process(&input, &mut ()),
        ctx(),
        true, // strict_gpu
        &dir,
    )
    .expect_err("strict OOM must be fatal");

    assert!(
        matches!(err, StageError::Fatal(FatalError::OutOfMemory { .. })),
        "strict_gpu OOM must be FatalError::OutOfMemory, got {err:?}"
    );

    let entries: Vec<_> = std::fs::read_dir(&dir)
        .expect("dump dir must exist after strict OOM")
        .filter_map(|e| e.ok())
        .collect();
    assert!(!entries.is_empty(), "dump must be written on strict OOM");
    let _ = std::fs::remove_dir_all(&dir);
}

// ─────────────────────────────────────────────────────────────────────────────
// default_dump_dir sanity
// ─────────────────────────────────────────────────────────────────────────────

/// `default_dump_dir()` returns a non-empty path (the default directory is
/// deterministic and does not change across invocations).
#[test]
fn test_default_dump_dir_is_non_empty() {
    let dir = default_dump_dir();
    assert!(
        dir.to_str().is_some_and(|s| !s.is_empty()),
        "default_dump_dir must return a non-empty path"
    );
    assert!(
        dir.to_str().unwrap().contains("diagnostic-dumps"),
        "default_dump_dir should contain 'diagnostic-dumps'"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Erase / pipeline integration smoke test (SC2 in erased-stage form)
// ─────────────────────────────────────────────────────────────────────────────

/// Verify that `erase(OomInjector(...))` compiles and its `cpu_fallback_process_any`
/// invokes the fallback correctly. This is the integration path
/// `execute_gpu_stage` in topology.rs uses (SC2: injectors via `mod common;`).
#[test]
fn test_erased_oom_injector_cpu_fallback_process_any() {
    let input = TinyBatch(77);
    let inj = OomInjector::new(Identity, 1); // OOM on first call.
    let erased = erase(inj);

    // The erased stage's cpu_fallback_process_any must call Identity::process.
    let result = erased
        .cpu_fallback_process_any(&input, &mut ())
        .expect("OomInjector's cpu_fallback (Identity) must be present");
    let out = result.expect("Identity fallback must succeed");
    let out_tiny = out
        .as_any()
        .downcast_ref::<TinyBatch>()
        .expect("output must be TinyBatch");
    assert_eq!(*out_tiny, TinyBatch(77), "fallback must pass input through");
}
