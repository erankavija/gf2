//! Helpers shared by the `gf2-sim` integration tests. They use only items
//! available without the `hip` feature.

#![allow(dead_code)] // each test binary uses a subset of these helpers.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use gf2_sim::error::{FatalError, RecoverableError, StageError};
use gf2_sim::executor::SnrPointResult;
use gf2_sim::parallel::WorkerCounters;
use gf2_sim::stage::{BatchSize, ExecutionClass, Stage};

/// Asserts `frames`, `errors`, `total_iterations` and the bit patterns of
/// `fer` and `mean_iters` equal between `actual` and `baseline`. BER is not
/// compared.
///
/// # Panics
///
/// Panics if a compared column differs.
#[track_caller]
pub fn assert_four_columns_byte_identical(
    actual: &WorkerCounters,
    baseline: &WorkerCounters,
    label: &str,
) {
    assert_eq!(
        actual.frames, baseline.frames,
        "{label}: `frames` differs ({} vs baseline {})",
        actual.frames, baseline.frames
    );
    assert_eq!(
        actual.errors, baseline.errors,
        "{label}: `errors` differs ({} vs baseline {})",
        actual.errors, baseline.errors
    );
    assert_eq!(
        actual.fer().to_bits(),
        baseline.fer().to_bits(),
        "{label}: `fer` bit pattern differs ({} vs baseline {})",
        actual.fer(),
        baseline.fer()
    );
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
}

/// The [`WorkerCounters`] columns of `p`, the operand the column assertions take.
pub fn snr_point_to_counters(p: &SnrPointResult) -> WorkerCounters {
    WorkerCounters {
        frames: p.frames,
        errors: p.errors,
        total_iterations: p.total_iterations,
        total_bits: p.total_bits,
        total_bit_errors: p.total_bit_errors,
    }
}

/// Asserts `frames`, `errors` and the `fer` bit pattern equal between a
/// GPU-bearing arm (`actual`) and its CPU-only `baseline`; logs `mean_iters`
/// without asserting it.
///
/// # Panics
///
/// Panics if a compared column differs.
#[track_caller]
pub fn assert_three_columns_byte_identical_log_mean_iters(
    actual: &WorkerCounters,
    baseline: &WorkerCounters,
    label: &str,
) {
    assert_eq!(
        baseline.frames, actual.frames,
        "[{label}] BYTE-IDENTITY VIOLATION: column `frames` diverged \
         (CPU={} GPU={}). ESCALATE per §11 HARD trigger.",
        baseline.frames, actual.frames
    );
    assert_eq!(
        baseline.errors, actual.errors,
        "[{label}] BYTE-IDENTITY VIOLATION: column `errors` (frame errors) diverged \
         (CPU={} GPU={}). ESCALATE per §11 HARD trigger.",
        baseline.errors, actual.errors
    );
    assert_eq!(
        baseline.fer().to_bits(),
        actual.fer().to_bits(),
        "[{label}] BYTE-IDENTITY VIOLATION: column `fer` bit pattern diverged \
         (CPU={} GPU={}). ESCALATE per §11 HARD trigger.",
        baseline.fer(),
        actual.fer()
    );

    eprintln!(
        "[{label}] mean_iters (LOGGED, NOT asserted — §11 CPU-vs-GPU exclusion): \
         CPU {:.4}, GPU {:.4}, diff {:+.4}",
        baseline.mean_iters(),
        actual.mean_iters(),
        actual.mean_iters() - baseline.mean_iters(),
    );
}

/// Builds the DVB-T2 BICM chain through the [`gf2_sim::graph::Chain`] API.
/// `demap_n0` must equal [`gf2_sim::channels::es_n0_db_to_n0`]`(es_n0_db)`.
///
/// # Panics
///
/// Panics if the chain fails to build.
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

/// Creates a unique, empty temporary directory for use by a single test.
///
/// # Panics
///
/// Panics if the directory cannot be created.
pub fn tempdir(prefix: &str) -> gf2_core::test_scratch::Scratch {
    gf2_core::test_scratch::scratch(&format!("gf2sim-{prefix}"))
}

/// One-element batch for exercising the injectors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TinyBatch(pub u32);

impl BatchSize for TinyBatch {
    fn batch_size(&self) -> usize {
        1
    }
}

/// Pass-through stage for the injectors to wrap.
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

/// Wraps a stage and returns `RecoverableError::OutOfMemory` on the
/// `trigger_on`th `process` call (1-indexed); other calls pass through.
pub struct OomInjector<I, O, S: Stage<I, O>> {
    inner: S,
    call_count: Arc<AtomicU64>,
    trigger_on: u64,
    device_id: i32,
    bytes_requested: usize,
    _marker: std::marker::PhantomData<fn(I) -> O>,
}

impl<I, O, S: Stage<I, O> + Clone> OomInjector<I, O, S> {
    /// Wraps `inner`; the injected error reports device 0 and 1 GiB requested.
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
            bytes_requested: 1024 * 1024 * 1024,
            _marker: std::marker::PhantomData,
        }
    }

    /// Sets the device id and byte count the injected error reports.
    pub fn with_oom_params(mut self, device_id: i32, bytes_requested: usize) -> Self {
        self.device_id = device_id;
        self.bytes_requested = bytes_requested;
        self
    }

    /// `process` calls so far, the injected one included.
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

/// Wraps a stage and returns `FatalError::KernelLaunch` on every `process`
/// call from the `trigger_on`th (1-indexed) onward.
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
    /// Wraps `inner`; the injected error reports HIP code 7 (`hipErrorLaunchFailure`).
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

    /// Sets the HIP code, kernel name and argument text the injected error reports.
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

    /// `process` calls so far, the failing ones included.
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
