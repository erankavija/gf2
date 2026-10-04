//! `dispatch_with_fallback` decision tree: a recoverable OOM runs the CPU
//! fallback, a fatal error propagates and writes a JSON diagnostic dump, and
//! `strict_gpu` promotes OOM to `FatalError::OutOfMemory` without fallback.
//! Uses the host-only injectors of `tests/common`; no GPU is required.

mod common;

use common::{Identity, KernelErrorInjector, OomInjector, TinyBatch};
use gf2_sim::error::{FatalError, RecoverableError, StageError};
use gf2_sim::executor::failure::{default_dump_dir, dispatch_with_fallback, FaultContext};
use gf2_sim::stage::{erase, Stage};

use std::path::PathBuf;

fn test_dump_dir(tag: &str) -> (gf2_core::test_scratch::Scratch, PathBuf) {
    let scratch = gf2_core::test_scratch::scratch(&format!("gf2sim-failmode-{tag}"));
    let dump_dir = scratch.path().join("dump");
    (scratch, dump_dir)
}

fn ctx() -> FaultContext {
    FaultContext {
        batch_id: 7,
        snr_idx: 3,
        device_id: 0,
        worker_idx: 0,
    }
}

#[test]
fn test_oom_fallback_output_matches_cpu_only_path() {
    let (_scratch, dir) = test_dump_dir("oom-fallback");
    let input = TinyBatch(55);

    let gpu_result: Result<TinyBatch, StageError> =
        Err(StageError::Recoverable(RecoverableError::OutOfMemory {
            device_id: 0,
            bytes_requested: 4096,
        }));

    let identity = Identity;
    let fallback = || identity.process(&input, &mut ());

    let fallback_out = dispatch_with_fallback(gpu_result, fallback, ctx(), false, &dir)
        .expect("OOM non-strict must succeed via fallback");

    let cpu_out = Identity.process(&input, &mut ()).unwrap();

    assert_eq!(
        fallback_out, cpu_out,
        "fallback output must be byte-identical to CPU-only output (§11 3-column contract)"
    );
    assert!(
        !dir.exists(),
        "no dump dir must be created for non-strict OOM with successful fallback"
    );
}

#[test]
fn test_oom_injector_dispatched_via_dispatch_with_fallback() {
    let (_scratch, dir) = test_dump_dir("oom-injector");
    let input = TinyBatch(99);

    let inj = OomInjector::new(Identity, 1);
    let gpu_result = inj.process(&input, &mut ());
    assert!(
        matches!(
            gpu_result,
            Err(StageError::Recoverable(
                RecoverableError::OutOfMemory { .. }
            ))
        ),
        "injector must produce OOM on call 1"
    );

    let fallback = || Identity.process(&input, &mut ());
    let out = dispatch_with_fallback(gpu_result, fallback, ctx(), false, &dir)
        .expect("OOM + successful fallback must succeed");
    assert_eq!(out, TinyBatch(99), "fallback must return identity output");

    assert!(!dir.exists());
}

#[test]
fn test_oom_fallback_also_fails_produces_dump_and_cpu_fallback_also_failed() {
    let (_scratch, dir) = test_dump_dir("oom-fb-fail");
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
}

#[test]
fn test_fatal_kernel_error_writes_dump_and_propagates() {
    let (_scratch, dir) = test_dump_dir("fatal-kernel");
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

    assert!(
        matches!(
            err,
            StageError::Fatal(FatalError::KernelLaunch { hip_code: 301, .. })
        ),
        "fatal must propagate as KernelLaunch(301), got {err:?}"
    );

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

    let path = entries[0].path();
    let content = std::fs::read_to_string(&path).expect("dump file must be readable");
    let v: serde_json::Value = serde_json::from_str(&content).expect("dump must be valid JSON");
    assert_eq!(v["event"], "hard_fail", "event field must be 'hard_fail'");
    assert_eq!(v["hip_code"], 301_i64, "hip_code must be 301");
    assert_eq!(v["kernel"], "bcjr_decode", "kernel name must be preserved");
    assert_eq!(v["snr_idx"], 3_i64, "snr_idx must match context");
    assert_eq!(v["batch_id"], 7_i64, "batch_id must match context");
    assert_eq!(v["device_id"], 0_i64, "device_id must match context");
}

#[test]
fn test_kernel_error_injector_via_common_mod_writes_dump() {
    let (_scratch, dir) = test_dump_dir("kernel-injector");
    let input = TinyBatch(0);

    let inj = KernelErrorInjector::new(Identity, 1).with_launch_params(
        7,
        "ldpc_bp",
        "injected for 42eac5cc test",
    );
    let gpu_result = inj.process(&input, &mut ());
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
}

#[test]
fn test_strict_gpu_promotes_oom_to_fatal_without_fallback() {
    let (_scratch, dir) = test_dump_dir("strict-gpu");
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

    let entries: Vec<_> = std::fs::read_dir(&dir)
        .expect("dump dir must exist after strict OOM")
        .filter_map(|e| e.ok())
        .collect();
    assert!(
        !entries.is_empty(),
        "dump file must be written on strict OOM"
    );
}

#[test]
fn test_strict_gpu_with_oom_injector_from_common_mod() {
    let (_scratch, dir) = test_dump_dir("strict-oom-injector");
    let input = TinyBatch(5);

    let inj = OomInjector::new(Identity, 1);
    let gpu_result = inj.process(&input, &mut ());

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
}

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

#[test]
fn test_erased_oom_injector_cpu_fallback_process_any() {
    let input = TinyBatch(77);
    let inj = OomInjector::new(Identity, 1);
    let erased = erase(inj);

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
