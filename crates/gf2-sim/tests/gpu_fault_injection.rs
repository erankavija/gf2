//! Fault-injection test infrastructure for GPU error signalling (issue
//! `ed575f15`, deliverable 3; design doc §8).
//!
//! Provides two injectors that wrap a `Stage` and force a typed error on the
//! Nth invocation of `process`:
//!
//! - [`OomInjector`] — forces `StageError::Recoverable(RecoverableError::OutOfMemory)`
//!   so the executor can test its CPU-fallback substitution path.
//! - [`KernelErrorInjector`] — forces `StageError::Fatal(FatalError::KernelLaunch)`
//!   so the executor can test its fatal-abort path.
//!
//! # Where these live
//!
//! The injectors are defined in this integration-test file. Phase C (`42eac5cc`)
//! can import them by adding this file as a common module (e.g. via a
//! `tests/common/` re-export) or by copying the struct definitions. For now they
//! live here and are re-exported from the `injectors` inline module so
//! `42eac5cc`'s test file can include this file or reference the module path.
//!
//! # Usage example
//!
//! ```rust,ignore
//! // Construct a real CPU stage, then wrap it to inject OOM on the 2nd call.
//! let inner = some_cpu_stage();
//! let mut injector = OomInjector::new(inner, 2);
//! assert!(injector.process(&input, &mut ()).is_ok());   // call 1: passes through
//! let err = injector.process(&input, &mut ()).unwrap_err(); // call 2: OOM
//! match err {
//!     StageError::Recoverable(RecoverableError::OutOfMemory { .. }) => {}
//!     _ => panic!("expected OOM"),
//! }
//! ```
//!
//! # No GPU required
//!
//! These injectors are pure host structures; no real HIP device is needed.
//! They compile and run in any environment where `gf2-sim` builds.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use gf2_sim::error::{FatalError, RecoverableError, StageError};
use gf2_sim::stage::{BatchSize, ExecutionClass, Stage};

// ────────────────────────────────────────────────────────────────────────────
// Minimal batch type for self-contained tests
// ────────────────────────────────────────────────────────────────────────────

/// A trivial one-element batch used to test injector behaviour without
/// depending on the full DVB-T2 pipeline batch types.
#[derive(Debug, Clone, PartialEq, Eq)]
struct TinyBatch(u32);

impl BatchSize for TinyBatch {
    fn batch_size(&self) -> usize {
        1
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Passthrough stage for wrapping
// ────────────────────────────────────────────────────────────────────────────

/// A trivial identity stage used as the wrapped inner stage for injector
/// tests; passes input through unchanged. `Send + Sync` as required.
#[derive(Clone)]
struct Identity;

impl Stage<TinyBatch, TinyBatch> for Identity {
    type Scratch = ();
    type CpuFallback = Self;

    fn process(&self, input: &TinyBatch, _scratch: &mut ()) -> Result<TinyBatch, StageError> {
        Ok(input.clone())
    }

    fn execution_class(&self) -> ExecutionClass {
        ExecutionClass::CpuOnly
    }

    fn cpu_fallback(&self) -> Option<&Self> {
        Some(self)
    }
}

// ────────────────────────────────────────────────────────────────────────────
// OomInjector
// ────────────────────────────────────────────────────────────────────────────

/// Wraps any `Stage<I, O>` and forces
/// `StageError::Recoverable(RecoverableError::OutOfMemory)` on the `trigger_on`th
/// invocation of `process` (1-indexed).
///
/// Invocations before and after `trigger_on` pass through to the inner stage.
/// The call count is shared behind an `Arc<AtomicU64>` so the injector can be
/// cloned for multi-worker tests while sharing the same trigger counter.
///
/// # Examples (conceptual)
///
/// ```rust,ignore
/// use gf2_sim::error::{RecoverableError, StageError};
/// // Inject OOM on the 2nd call.
/// let mut inj = OomInjector::new(identity_stage, 2);
/// assert!(inj.process(&input, &mut ()).is_ok()); // 1st call passes through
/// let err = inj.process(&input, &mut ()).unwrap_err(); // 2nd call: OOM
/// matches!(err, StageError::Recoverable(RecoverableError::OutOfMemory { .. }));
/// ```
///
/// # For 42eac5cc
///
/// `42eac5cc` should construct this injector with any `Stage<I, O>` of the
/// same input/output batch types as the real GPU stage it substitutes. The
/// `device_id` and `bytes_requested` in the injected error are configurable
/// via [`OomInjector::with_oom_params`]; the default is device 0, 1 GiB.
pub struct OomInjector<I, O, S: Stage<I, O>> {
    inner: S,
    call_count: Arc<AtomicU64>,
    trigger_on: u64,
    device_id: i32,
    bytes_requested: usize,
    _marker: std::marker::PhantomData<fn(I) -> O>,
}

impl<I, O, S: Stage<I, O> + Clone> OomInjector<I, O, S> {
    /// Constructs an injector that forces OOM on the `trigger_on`th call
    /// (1-indexed). Calls before and after pass through.
    ///
    /// # Arguments
    ///
    /// * `inner` — the wrapped stage.
    /// * `trigger_on` — 1-indexed call number at which to inject OOM; must be
    ///   `>= 1`.
    ///
    /// # Panics
    ///
    /// Panics if `trigger_on == 0`.
    pub fn new(inner: S, trigger_on: u64) -> Self {
        assert!(trigger_on >= 1, "OomInjector: trigger_on must be >= 1");
        Self {
            inner,
            call_count: Arc::new(AtomicU64::new(0)),
            trigger_on,
            device_id: 0,
            bytes_requested: 1024 * 1024 * 1024, // 1 GiB default
            _marker: std::marker::PhantomData,
        }
    }

    /// Overrides the device_id and bytes_requested carried by the injected
    /// `RecoverableError::OutOfMemory`.
    pub fn with_oom_params(mut self, device_id: i32, bytes_requested: usize) -> Self {
        self.device_id = device_id;
        self.bytes_requested = bytes_requested;
        self
    }

    /// Shared call counter — lets the caller inspect how many calls were made.
    pub fn call_count(&self) -> u64 {
        self.call_count.load(Ordering::SeqCst)
    }
}

impl<I, O, S> Stage<I, O> for OomInjector<I, O, S>
where
    I: Send + Sync + std::any::Any,
    O: Send + Sync + std::any::Any,
    S: Stage<I, O> + Clone,
{
    type Scratch = S::Scratch;
    type CpuFallback = S::CpuFallback;

    fn process(&self, input: &I, scratch: &mut S::Scratch) -> Result<O, StageError> {
        let n = self.call_count.fetch_add(1, Ordering::SeqCst) + 1;
        if n == self.trigger_on {
            return Err(StageError::Recoverable(RecoverableError::OutOfMemory {
                device_id: self.device_id,
                bytes_requested: self.bytes_requested,
            }));
        }
        self.inner.process(input, scratch)
    }

    fn execution_class(&self) -> ExecutionClass {
        self.inner.execution_class()
    }

    fn cpu_fallback(&self) -> Option<&S::CpuFallback> {
        self.inner.cpu_fallback()
    }
}

// ────────────────────────────────────────────────────────────────────────────
// KernelErrorInjector
// ────────────────────────────────────────────────────────────────────────────

/// Wraps any `Stage<I, O>` and forces
/// `StageError::Fatal(FatalError::KernelLaunch)` on the `trigger_on`th
/// invocation of `process` (1-indexed).
///
/// Invocations before `trigger_on` pass through; invocations on and after
/// `trigger_on` all return a fatal `KernelLaunch` error (since the executor
/// must abort on the first fatal, later calls are academic but consistent).
///
/// # Examples (conceptual)
///
/// ```rust,ignore
/// use gf2_sim::error::{FatalError, StageError};
/// // Inject a kernel launch failure on the 1st call.
/// let mut inj = KernelErrorInjector::new(identity_stage, 1);
/// let err = inj.process(&input, &mut ()).unwrap_err();
/// matches!(err, StageError::Fatal(FatalError::KernelLaunch { .. }));
/// ```
///
/// # For 42eac5cc
///
/// Use this to verify the executor aborts and propagates the fatal error
/// rather than substituting a CPU fallback (fatal errors are not recoverable).
/// The injected `hip_code`, `kernel`, and `args` are configurable via
/// [`KernelErrorInjector::with_launch_params`]; the default is code 7
/// (`hipErrorLaunchFailure`), kernel `"injected"`, args `"fault-injection"`.
pub struct KernelErrorInjector<I, O, S: Stage<I, O>> {
    inner: S,
    call_count: Arc<AtomicU64>,
    trigger_on: u64,
    hip_code: i32,
    kernel: &'static str,
    args: String,
    _marker: std::marker::PhantomData<fn(I) -> O>,
}

impl<I, O, S: Stage<I, O> + Clone> KernelErrorInjector<I, O, S> {
    /// Constructs an injector that forces a `KernelLaunch` fatal error on the
    /// `trigger_on`th call (1-indexed).
    ///
    /// # Panics
    ///
    /// Panics if `trigger_on == 0`.
    pub fn new(inner: S, trigger_on: u64) -> Self {
        assert!(
            trigger_on >= 1,
            "KernelErrorInjector: trigger_on must be >= 1"
        );
        Self {
            inner,
            call_count: Arc::new(AtomicU64::new(0)),
            trigger_on,
            hip_code: 7, // hipErrorLaunchFailure
            kernel: "injected",
            args: "fault-injection".to_string(),
            _marker: std::marker::PhantomData,
        }
    }

    /// Overrides the hip_code, kernel name, and args carried by the injected
    /// `FatalError::KernelLaunch`.
    pub fn with_launch_params(
        mut self,
        hip_code: i32,
        kernel: &'static str,
        args: impl Into<String>,
    ) -> Self {
        self.hip_code = hip_code;
        self.kernel = kernel;
        self.args = args.into();
        self
    }

    /// Shared call counter.
    pub fn call_count(&self) -> u64 {
        self.call_count.load(Ordering::SeqCst)
    }
}

impl<I, O, S> Stage<I, O> for KernelErrorInjector<I, O, S>
where
    I: Send + Sync + std::any::Any,
    O: Send + Sync + std::any::Any,
    S: Stage<I, O> + Clone,
{
    type Scratch = S::Scratch;
    type CpuFallback = S::CpuFallback;

    fn process(&self, input: &I, scratch: &mut S::Scratch) -> Result<O, StageError> {
        let n = self.call_count.fetch_add(1, Ordering::SeqCst) + 1;
        if n >= self.trigger_on {
            return Err(StageError::Fatal(FatalError::KernelLaunch {
                hip_code: self.hip_code,
                kernel: self.kernel,
                args: self.args.clone(),
            }));
        }
        self.inner.process(input, scratch)
    }

    fn execution_class(&self) -> ExecutionClass {
        self.inner.execution_class()
    }

    fn cpu_fallback(&self) -> Option<&S::CpuFallback> {
        self.inner.cpu_fallback()
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Tests: verify the injectors produce the expected error variants
// ────────────────────────────────────────────────────────────────────────────

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
