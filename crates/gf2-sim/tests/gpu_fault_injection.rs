//! Checks the call on which [`common::OomInjector`] and
//! [`common::KernelErrorInjector`] inject their error variant, and the
//! variant's fields.

mod common;

use common::{Identity, KernelErrorInjector, OomInjector, TinyBatch};
use gf2_sim::error::{FatalError, RecoverableError, StageError};
use gf2_sim::stage::Stage;

#[test]
fn test_oom_injector_passes_through_then_injects() {
    let inner = Identity;
    let inj = OomInjector::new(inner, 3);
    let input = TinyBatch(42);

    let out1 = inj
        .process(&input, &mut ())
        .expect("call 1 must pass through");
    assert_eq!(out1, TinyBatch(42));

    let out2 = inj
        .process(&input, &mut ())
        .expect("call 2 must pass through");
    assert_eq!(out2, TinyBatch(42));

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

#[test]
fn test_kernel_error_injector_passes_through_then_injects() {
    let inj = KernelErrorInjector::new(Identity, 2);
    let input = TinyBatch(99);

    let out1 = inj
        .process(&input, &mut ())
        .expect("call 1 must pass through");
    assert_eq!(out1, TinyBatch(99));

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

    let err3 = inj
        .process(&input, &mut ())
        .expect_err("call 3 must also inject KernelLaunch");
    assert!(
        matches!(err3, StageError::Fatal(FatalError::KernelLaunch { .. })),
        "all calls after trigger_on must inject, got {err3:?}"
    );

    assert_eq!(inj.call_count(), 3);
}

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

#[test]
#[should_panic(expected = "trigger_on must be >= 1")]
fn test_oom_injector_rejects_zero_trigger() {
    let _ = OomInjector::new(Identity, 0);
}

#[test]
#[should_panic(expected = "trigger_on must be >= 1")]
fn test_kernel_error_injector_rejects_zero_trigger() {
    let _ = KernelErrorInjector::new(Identity, 0);
}

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
