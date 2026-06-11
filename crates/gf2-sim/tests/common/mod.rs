//! Shared determinism-assertion helpers for the `gf2-sim` byte-identity
//! integration tests (design doc §11).
//!
//! Both [`parallel_determinism.rs`](../parallel_determinism.rs) (the direct
//! `frame_sim` path, issue `3fcb7025`) and [`determinism.rs`](../determinism.rs)
//! (the typestate preset production path, issue `48a0db6c`) compare
//! [`WorkerCounters`] for byte-identity across worker counts. The comparison is
//! the **single source of truth** for the four byte-identity columns and the
//! BER exclusion: it lives here so neither test binary re-implements (and
//! risks diverging) the column set the determinism contract pins.
//!
//! # The four byte-identity columns (design doc §11)
//!
//! The CPU-only / CPU-parallel contract pins exactly four columns as
//! byte-identical across worker counts `{1, 2, 4, 8, 24}` at a fixed seed:
//! `fer`, `frames`, `errors`, `mean_iters`. [`assert_four_columns_byte_identical`]
//! asserts all four (the two `f64` ratios via their exact bit patterns, plus
//! the underlying `total_iterations` whose ratio `mean_iters` is). **BER is
//! deliberately excluded** — see the function docs and the cited issue
//! `152388f4` / design-doc §11 "Always-excluded".
//!
//! # GPU fault-injection helpers (issue `ed575f15`, design doc §8)
//!
//! [`OomInjector`] and [`KernelErrorInjector`] are reusable `Stage` wrappers
//! that force a typed [`StageError`](gf2_sim::error::StageError) on the Nth
//! `process` invocation, plus the trivial [`Identity`] stage and [`TinyBatch`]
//! batch they wrap. They are the **single source of truth** for the fault
//! injectors: `tests/gpu_fault_injection.rs` (issue `ed575f15`, deliverable 3)
//! verifies they produce the expected error variants, and the Phase C executor
//! substitution test (issue `42eac5cc`) reuses them by the same `mod common;`
//! include — no copy-paste. They reference only the un-gated `gf2_sim::error`
//! / `gf2_sim::stage` items (no `feature = "hip"` types), so they compile into
//! the non-hip determinism test binaries that also include this module.

#![allow(dead_code)] // each test binary uses a subset of these helpers.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use gf2_sim::error::{FatalError, RecoverableError, StageError};
use gf2_sim::parallel::WorkerCounters;
use gf2_sim::stage::{BatchSize, ExecutionClass, Stage};

/// Asserts the four byte-identity columns of `actual` match `baseline`
/// (design doc §11 CPU-only / CPU-parallel contract).
///
/// The four columns the determinism contract pins as byte-identical across
/// worker counts are `frames`, `errors`, `fer`, and `mean_iters`. This helper
/// asserts all four:
///
/// * `frames` and `errors` — the integer-exact `u64` counters, asserted
///   directly.
/// * `fer` (`errors / frames`) and `mean_iters` (`total_iterations / frames`) —
///   derived `f64` ratios, asserted via their exact **bit patterns**
///   ([`f64::to_bits`]) so the check is strictly byte-identical, not merely
///   approximately equal. `total_iterations` (the numerator of `mean_iters`) is
///   asserted directly too, so a regression in either the ratio or its inputs is
///   caught.
///
/// # BER is excluded (issue `152388f4`, design-doc §11)
///
/// The bit-error-rate column (`total_bit_errors / total_bits`) is **NOT**
/// asserted here. Per design-doc §11 "Always-excluded", BER is a
/// non-associative `f32` horizontal reduction whose value depends on summation
/// order, so it is not byte-identical across worker counts (status-quo
/// amendment from issue `152388f4`). Callers may *record* BER for diagnostics
/// but must never assert it; this helper intentionally provides no BER
/// comparison so that exclusion cannot be circumvented by accident.
///
/// # Arguments
///
/// * `actual` — the counters from a non-baseline worker count.
/// * `baseline` — the 1-worker reference counters.
/// * `label` — a human-readable config/worker label for assertion messages.
///
/// # Panics
///
/// Panics (via `assert_eq!`) if any of the four byte-identity columns differ
/// between `actual` and `baseline`: `frames`, `errors`, the `fer` bit pattern,
/// or the `mean_iters` bit pattern (including its `total_iterations`
/// numerator). The panic message names the offending column and both values.
#[track_caller]
pub fn assert_four_columns_byte_identical(
    actual: &WorkerCounters,
    baseline: &WorkerCounters,
    label: &str,
) {
    // Column 1: frames (u64, integer-exact).
    assert_eq!(
        actual.frames, baseline.frames,
        "{label}: `frames` differs ({} vs baseline {})",
        actual.frames, baseline.frames
    );
    // Column 2: errors (u64, integer-exact).
    assert_eq!(
        actual.errors, baseline.errors,
        "{label}: `errors` differs ({} vs baseline {})",
        actual.errors, baseline.errors
    );
    // Column 3: fer = errors/frames, asserted by exact bit pattern.
    assert_eq!(
        actual.fer().to_bits(),
        baseline.fer().to_bits(),
        "{label}: `fer` bit pattern differs ({} vs baseline {})",
        actual.fer(),
        baseline.fer()
    );
    // Column 4: mean_iters = total_iterations/frames, asserted by exact bit
    // pattern, plus the integer numerator it is derived from.
    assert_eq!(
        actual.total_iterations, baseline.total_iterations,
        "{label}: `total_iterations` differs ({} vs baseline {})",
        actual.total_iterations, baseline.total_iterations
    );
    assert_eq!(
        actual.mean_iters().to_bits(),
        baseline.mean_iters().to_bits(),
        "{label}: `mean_iters` bit pattern differs ({} vs baseline {})",
        actual.mean_iters(),
        baseline.mean_iters()
    );

    // BER (total_bit_errors / total_bits) is intentionally NOT asserted — it is
    // always excluded from byte-identity (issue `152388f4`; design-doc §11
    // "Always-excluded"). No comparison is offered for it on purpose.
}

// ────────────────────────────────────────────────────────────────────────────
// Shared DVB-T2 graph-chain builder (BLOCKING-2, issue `8c8302c8`)
//
// Both `preset_vs_graph.rs` (structural + single-frame) and
// `preset_vs_graph_byte_identity.rs` (run-level 50-frame byte-identity) wire
// the same seven-stage DVB-T2 BICM chain by hand via the `Chain` graph API.
// This helper is the single source of truth for that wiring.
// ────────────────────────────────────────────────────────────────────────────

/// Builds a hand-wired DVB-T2 BICM chain via the graph [`gf2_sim::graph::Chain`]
/// API and returns a [`gf2_sim::pipeline::Pipeline`].
///
/// The chain is identical to what the typestate preset produces:
///
/// ```text
/// DvbT2Encode → BitInterleave → GrayQamMap → Awgn → GrayQamDemap → BitDeinterleave → DvbT2Decode
/// ```
///
/// `demap_n0` must be the caller-derived `N0 = 2*sigma^2` for the chosen
/// Es/N0 — use [`gf2_sim::channels::es_n0_db_to_n0`] to compute it
/// (the once-rounded SSOT).
///
/// # Arguments
///
/// * `rate` — DVB-T2 code rate.
/// * `modulation` — DVB-T2 modulation order.
/// * `decoder` — BP decoder configuration (algorithm + early termination).
/// * `demap` — soft demapper method.
/// * `es_n0_db` — channel Es/N0 in dB (used for the `Awgn` stage sigma).
/// * `demap_n0` — demapper noise variance `N0`; must equal
///   `es_n0_db_to_n0(es_n0_db)`.
/// * `seed` — simulation seed stored in the `PipelineConfig`.
/// * `parallelism` — worker count stored in the `PipelineConfig`.
///
/// # Panics
///
/// Panics if the chain cannot be built (invalid type connections) or if
/// `parallelism` is zero.
#[allow(clippy::too_many_arguments)]
pub fn build_dvb_t2_graph_chain(
    rate: gf2_coding::CodeRate,
    modulation: gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation,
    decoder: gf2_coding::ldpc::DecoderConfig,
    demap: gf2_coding::modem::DemapMethod,
    es_n0_db: f32,
    demap_n0: f32,
    seed: u64,
    parallelism: std::num::NonZeroUsize,
) -> gf2_sim::pipeline::Pipeline {
    let factory = gf2_sim::stages::dvb_t2_bicm_stages(rate, modulation, decoder, demap, demap_n0);

    let mut chain = gf2_sim::graph::Chain::new();
    let mut ids = Vec::with_capacity(7);
    for stage in factory.forward {
        ids.push(chain.add(stage));
    }
    ids.push(
        chain.add(gf2_sim::stage::erase(gf2_sim::channels::Awgn::new(
            es_n0_db,
            modulation.bits_per_cell(),
        ))),
    );
    for stage in factory.inverse {
        ids.push(chain.add(stage));
    }
    for pair in ids.windows(2) {
        chain
            .connect(pair[0], pair[1])
            .expect("each consecutive BICM hop is type-compatible");
    }

    let config = gf2_sim::PipelineConfig {
        seed,
        esn0_db_points: Vec::new(),
        target_errors: 0,
        max_frames: 0,
        heartbeat_every_frames: 0,
        checkpoint_dir: None,
        tracing_log_path: None,
        parallelism,
        gpu_enabled: false,
        strict_gpu: false,
        diagnostic_dump_dir: None,
        inject_gpu_oom_modulus: None,
    };

    chain
        .with_config(config)
        .build()
        .expect("the full BICM chain is a valid DAG")
}

// ────────────────────────────────────────────────────────────────────────────
// Shared tempdir helper
//
// Each test binary that uses this module gets its own independent directory
// namespace because `mod common` is textually included (not a separate crate),
// so `TMPDIR_COUNTER` is a per-binary static. The counter starts at 0 in each
// binary; the `pid` component prevents collisions across concurrent test
// binaries.
// ────────────────────────────────────────────────────────────────────────────

/// Creates a unique, empty temporary directory for use by a single test.
///
/// The directory name is `gf2sim-<prefix>-<pid>-<counter>` under the system
/// temp dir. No `tempfile` dev-dependency required.
///
/// # Panics
///
/// Panics if the directory cannot be created.
pub fn tempdir(prefix: &str) -> std::path::PathBuf {
    static TMPDIR_COUNTER: AtomicU64 = AtomicU64::new(0);
    let mut p = std::env::temp_dir();
    let unique = format!(
        "gf2sim-{}-{}-{}",
        prefix,
        std::process::id(),
        TMPDIR_COUNTER.fetch_add(1, Ordering::Relaxed)
    );
    p.push(unique);
    std::fs::create_dir_all(&p).expect("create unique tempdir");
    p
}

// ────────────────────────────────────────────────────────────────────────────
// GPU fault-injection helpers (issue `ed575f15`, deliverable 3; design doc §8)
//
// SSOT for the fault injectors: defined here so the verification tests
// (`gpu_fault_injection.rs`) and the Phase C executor substitution test
// (`42eac5cc`) reuse the SAME definitions via `mod common;` — no copy-paste.
// All items reference only the un-gated `gf2_sim::error` / `gf2_sim::stage`
// surface, so this module compiles cleanly into the non-hip test binaries
// (`determinism.rs`, `parallel_determinism.rs`) that also include it.
// ────────────────────────────────────────────────────────────────────────────

/// A trivial one-element batch used to exercise the injectors without depending
/// on the full DVB-T2 pipeline batch types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TinyBatch(pub u32);

impl BatchSize for TinyBatch {
    fn batch_size(&self) -> usize {
        1
    }
}

/// A trivial identity stage used as the wrapped inner stage for injector tests
/// (and by `42eac5cc` as a no-op stage to wrap); passes input through unchanged.
/// `Send + Sync` as the [`Stage`] bound requires.
#[derive(Clone)]
pub struct Identity;

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
/// let inj = OomInjector::new(identity_stage, 2);
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
/// let inj = KernelErrorInjector::new(identity_stage, 1);
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
