//! Verification tests for the GPU fault-injection infrastructure (issue
//! `ed575f15`, deliverable 3; design doc §8).
//!
//! The injectors themselves — [`common::OomInjector`] and
//! [`common::KernelErrorInjector`], plus the trivial [`common::Identity`] stage
//! and [`common::TinyBatch`] batch they wrap — live in the SHARED
//! [`tests/common/mod.rs`](./common/mod.rs) module so they are a single source
//! of truth: this file verifies they produce the expected error variants, and
//! the Phase C executor substitution test (issue `42eac5cc`) reuses the SAME
//! definitions via the same `mod common;` include — no copy-paste.
//!
//! - [`common::OomInjector`] forces
//!   `StageError::Recoverable(RecoverableError::OutOfMemory)` on the Nth call,
//!   so the executor can test its CPU-fallback substitution path.
//! - [`common::KernelErrorInjector`] forces
//!   `StageError::Fatal(FatalError::KernelLaunch)`, so the executor can test its
//!   fatal-abort path.
//!
//! # No GPU required
//!
//! The injectors are pure host structures; no real HIP device is needed. They
//! reference only the un-gated `gf2_sim::error` / `gf2_sim::stage` surface, so
//! these tests run in any environment where `gf2-sim` builds.

mod common;

use common::{Identity, KernelErrorInjector, OomInjector, TinyBatch};
use gf2_sim::error::{FatalError, RecoverableError, StageError};
use gf2_sim::stage::Stage;

/// `OomInjector` passes through on calls before `trigger_on`, then injects
/// the expected `RecoverableError::OutOfMemory` on the Nth call.
#[test]
fn test_oom_injector_passes_through_then_injects() {
    let inner = Identity;
    let inj = OomInjector::new(inner, 3);
    let input = TinyBatch(42);

    // Calls 1 and 2 pass through.
    let out1 = inj
        .process(&input, &mut ())
        .expect("call 1 must pass through");
    assert_eq!(out1, TinyBatch(42));

    let out2 = inj
        .process(&input, &mut ())
        .expect("call 2 must pass through");
    assert_eq!(out2, TinyBatch(42));

    // Call 3 injects OOM.
    let err = inj
        .process(&input, &mut ())
        .expect_err("call 3 must inject OOM");
    match err {
        StageError::Recoverable(RecoverableError::OutOfMemory {
            device_id,
            bytes_requested,
        }) => {
            assert_eq!(device_id, 0, "default OOM device_id must be 0");
            assert_eq!(
                bytes_requested,
                1024 * 1024 * 1024,
                "default OOM bytes_requested must be 1 GiB"
            );
        }
        other => panic!("expected RecoverableError::OutOfMemory, got {other:?}"),
    }

    // Call 4 resumes pass-through.
    let out4 = inj
        .process(&input, &mut ())
        .expect("call 4 must pass through again");
    assert_eq!(out4, TinyBatch(42));

    assert_eq!(
        inj.call_count(),
        4,
        "call counter must reflect all four invocations"
    );
}

/// `OomInjector` injects OOM on the very first call when `trigger_on == 1`.
#[test]
fn test_oom_injector_trigger_on_first_call() {
    let inj = OomInjector::new(Identity, 1);
    let err = inj
        .process(&TinyBatch(7), &mut ())
        .expect_err("trigger_on=1 must inject immediately");
    assert!(
        matches!(
            err,
            StageError::Recoverable(RecoverableError::OutOfMemory { .. })
        ),
        "first-call OOM must be RecoverableError::OutOfMemory, got {err:?}"
    );
}

/// Custom OOM params are carried into the injected error.
#[test]
fn test_oom_injector_custom_params() {
    let inj = OomInjector::new(Identity, 1).with_oom_params(3, 512 * 1024 * 1024);
    let err = inj
        .process(&TinyBatch(0), &mut ())
        .expect_err("must inject OOM");
    match err {
        StageError::Recoverable(RecoverableError::OutOfMemory {
            device_id,
            bytes_requested,
        }) => {
            assert_eq!(device_id, 3);
            assert_eq!(bytes_requested, 512 * 1024 * 1024);
        }
        other => panic!("expected OOM with custom params, got {other:?}"),
    }
}

/// `KernelErrorInjector` passes through on calls before `trigger_on`, then
/// injects `FatalError::KernelLaunch` on the Nth and subsequent calls.
#[test]
fn test_kernel_error_injector_passes_through_then_injects() {
    let inj = KernelErrorInjector::new(Identity, 2);
    let input = TinyBatch(99);

    // Call 1 passes through.
    let out1 = inj
        .process(&input, &mut ())
        .expect("call 1 must pass through");
    assert_eq!(out1, TinyBatch(99));

    // Call 2 injects KernelLaunch.
    let err = inj
        .process(&input, &mut ())
        .expect_err("call 2 must inject KernelLaunch");
    match err {
        StageError::Fatal(FatalError::KernelLaunch {
            hip_code,
            kernel,
            ref args,
        }) => {
            assert_eq!(hip_code, 7, "default hip_code must be 7");
            assert_eq!(kernel, "injected", "default kernel name must be 'injected'");
            assert_eq!(
                args, "fault-injection",
                "default args must be 'fault-injection'"
            );
        }
        other => panic!("expected FatalError::KernelLaunch, got {other:?}"),
    }

    // Call 3 also injects (fatal errors persist once triggered).
    let err3 = inj
        .process(&input, &mut ())
        .expect_err("call 3 must also inject KernelLaunch");
    assert!(
        matches!(err3, StageError::Fatal(FatalError::KernelLaunch { .. })),
        "all calls after trigger_on must inject, got {err3:?}"
    );

    assert_eq!(inj.call_count(), 3);
}

/// `KernelErrorInjector` injects on the very first call when `trigger_on == 1`.
#[test]
fn test_kernel_error_injector_trigger_on_first_call() {
    let inj = KernelErrorInjector::new(Identity, 1);
    let err = inj
        .process(&TinyBatch(0), &mut ())
        .expect_err("trigger_on=1 must inject immediately");
    assert!(
        matches!(err, StageError::Fatal(FatalError::KernelLaunch { .. })),
        "first-call inject must be Fatal::KernelLaunch, got {err:?}"
    );
}

/// Custom launch params are carried into the injected error.
#[test]
fn test_kernel_error_injector_custom_params() {
    let inj = KernelErrorInjector::new(Identity, 1).with_launch_params(
        301,
        "bcjr_decode",
        "gfx908 blob missing",
    );
    let err = inj
        .process(&TinyBatch(0), &mut ())
        .expect_err("must inject KernelLaunch");
    match err {
        StageError::Fatal(FatalError::KernelLaunch {
            hip_code,
            kernel,
            ref args,
        }) => {
            assert_eq!(hip_code, 301);
            assert_eq!(kernel, "bcjr_decode");
            assert_eq!(args, "gfx908 blob missing");
        }
        other => panic!("expected KernelLaunch with custom params, got {other:?}"),
    }
}

/// Verify that `OomInjector::new` panics on `trigger_on == 0`.
#[test]
#[should_panic(expected = "trigger_on must be >= 1")]
fn test_oom_injector_rejects_zero_trigger() {
    let _ = OomInjector::new(Identity, 0);
}

/// Verify that `KernelErrorInjector::new` panics on `trigger_on == 0`.
#[test]
#[should_panic(expected = "trigger_on must be >= 1")]
fn test_kernel_error_injector_rejects_zero_trigger() {
    let _ = KernelErrorInjector::new(Identity, 0);
}

/// Injected `StageError::Recoverable(OutOfMemory)` is distinct from
/// `StageError::Fatal(KernelLaunch)` — the two variants are different and
/// the executor must handle them differently.
#[test]
fn test_oom_and_kernel_error_are_distinct_variants() {
    let oom_inj = OomInjector::new(Identity, 1);
    let ker_inj = KernelErrorInjector::new(Identity, 1);
    let input = TinyBatch(0);

    let oom_err = oom_inj
        .process(&input, &mut ())
        .expect_err("OomInjector must produce an error");
    let ker_err = ker_inj
        .process(&input, &mut ())
        .expect_err("KernelErrorInjector must produce an error");

    assert!(
        matches!(
            oom_err,
            StageError::Recoverable(RecoverableError::OutOfMemory { .. })
        ),
        "OOM injector must produce Recoverable::OutOfMemory, got {oom_err:?}"
    );
    assert!(
        matches!(ker_err, StageError::Fatal(FatalError::KernelLaunch { .. })),
        "kernel-error injector must produce Fatal::KernelLaunch, got {ker_err:?}"
    );
}
