//! The measurement protocol: one timed cell of the `(q, n, backend)` grid.
//!
//! # What is timed
//!
//! A cell times the *composite campaign hot path*, not the kernel alone. One
//! repetition performs, on a batch of `M` matrices:
//!
//! 1. **generate** — draw `M` uniform matrices from [`crate::sampler`] and
//!    build the packed matrix type each kernel consumes;
//! 2. **evaluate** — run the backend over the batch, one permanent per matrix;
//! 3. **reduce** — histogram the `M` permanent values over the `q` residue
//!    classes, of which bin 0 is the zero count;
//! 4. **store** — append the shard record (seed stream, counts) to a file and
//!    flush it, which is the campaign's per-shard durability point.
//!
//! The composite rate is `M / (t_gen + t_eval + t_reduce + t_store)`; the four
//! component times are reported separately so the study can attribute cost.
//! The envelope in the study is derived from the composite rate.
//!
//! GPU rows additionally carry event-measured `kernel_device_s`,
//! `h2d_device_s`, `d2h_device_s`, and `device_submission_to_kernel_s` totals
//! from the permanent dispatch stream, plus `host_submission_s` from the host
//! submission call. These are independent columns: the device-event values
//! never use host timestamps, and the host submission value is never combined
//! with a device event. CPU, unsupported, and unexecuted rows leave all five
//! columns empty and state why in `phase_timing_note`; they never substitute a
//! wall-clock evaluation duration or zero.
//!
//! # Repetition and censoring policy
//!
//! Every cell first runs an untimed warm-up of at least [`WARMUP_SECONDS`], on
//! the assumption that clocks and the GPU reach steady state within it - an
//! assumption the sustained runs bound rather than verify - drawing from a
//! purpose tag distinct from timed work, so the timed sample stays independent of how many warm-up
//! repetitions the machine happened to fit. It then repeats until it has both
//! at least [`MIN_REPS`] repetitions and at least [`MIN_TIMED_SECONDS`] of timed
//! work, stopping at [`MAX_CELL_SECONDS`].
//!
//! Three outcomes are distinguishable in the CSV: `measured`, `unsupported`
//! (a kernel bound forbids the cell), and `censored` (not attempted).
//!
//! # The censoring contract
//!
//! This is the single normative statement of what a censored row means; the
//! study and the CSV preamble restate it and must not diverge from it.
//!
//! A censored row carries **no measured rate**: `composite_matrices_per_s` is
//! `NaN`. What it carries is `projected_matrices_per_s`, an **estimate** formed
//! by scaling a *measured batched rate* from another `n` on the same
//! `(q, backend, batch size)` through Ryser's exact `n * 2^n` work model, with
//! the reference size in `projection_reference_n`.
//!
//! A cell is censored when that projection implies a repetition longer than
//! [`MAX_CELL_SECONDS`] — `M / rate` for a fixed-batch cell, `1 / rate` for an
//! adaptive one, which sizes its own batch and so can only be defeated by a
//! single unaffordable matrix. A cell with no projection reference falls back
//! to a single-matrix probe and is censored only if that one matrix already
//! exceeds the cap.
//!
//! ## Why no bound is derived from the probe
//!
//! [`CellResult::probe_matrix_s`] is a single-matrix **latency**, and no bound
//! on a batched rate follows from it in either direction. `1 / probe`
//! understates the device by orders of magnitude, since one matrix occupies one
//! compute unit. `W / probe`, with `W` the compute-unit count, is **not an
//! upper bound either**: a compute unit hosts several workgroups at once, and
//! the probe pays per-launch costs that a real batch amortises. An earlier
//! version of this harness published `W / probe` as an upper bound and the
//! grid's own measurements exceeded it. The study's section 4.3 records that
//! falsification and states which of its numbers a current grid still carries:
//! a cell with a projection reference records no probe, so the probe side of
//! that comparison survives only in the superseded receipt it came from.
//!
//! ## What the projection is worth
//!
//! Scaling a measured batched rate by the work ratio is checkable on any chain
//! with both ends measured, which includes the `q = 3` GPU chain and, for the
//! fields the censored cells belong to, the `q` in `{5, 7}` GPU steps up to
//! `n = 20`. Every one of those steps runs **pessimistic**, so a censored
//! cell's true rate is at least its projection. The magnitude beyond `n = 20`
//! is not validated, and the sign is kernel-specific: the generic Ryser path,
//! whose per-step cost matches the model, projects high. Magnitudes are
//! re-derived from each grid's own chains in the study, never quoted here.

use std::fmt::Write as _;
use std::time::Instant;

use gf2_algebra::packed::bipedal3::Bipedal3Matrix;
use gf2_algebra::packed::packed5::Packed5Matrix;
use gf2_algebra::packed::packed7::Packed7Matrix;

use crate::backend::{
    count_zeros, evaluate, evaluate_timed, support, Backend, Batch, PhaseTiming, Support,
};
use crate::env::{pin_thread, ThermalSample};
use crate::sampler::{MatrixSampler, MeasurementPurpose};
use crate::schedule::{scheduled_backend, SchedulePhase};

/// Untimed work run before a cell's first timed repetition.
pub const WARMUP_SECONDS: f64 = 3.0;
/// Timed repetitions every cell runs at minimum.
pub const MIN_REPS: usize = 5;
/// Timed wall-clock every cell accumulates at minimum.
pub const MIN_TIMED_SECONDS: f64 = 5.0;
/// Timed wall-clock after which a cell stops accepting further repetitions.
pub const MAX_CELL_SECONDS: f64 = 120.0;
/// Batch wall-clock each cell's `M` is calibrated to hit.
pub const TARGET_REP_SECONDS: f64 = 2.0;
/// Ceiling on `M`, to bound a cell's resident matrix memory.
pub const MAX_BATCH: usize = 65_536;
/// Minimum matrices per rayon worker in an adaptively sized batch, so the tail
/// of each batch does not leave most of the pool idle.
pub const MATRICES_PER_WORKER: usize = 4;
/// Physical core the single-thread cells are pinned to.
pub const PINNED_CORE: usize = 0;
/// Stream indices reserved to each sustained run within its named purpose.
pub const SUSTAINED_INDICES_PER_RUN: u64 = 100_000;
const PHASE_TIMING_NOT_RUN: &str = "event timing unavailable: cell did not run";

/// Result of applying the harness's canonical warm-up and timed-repetition
/// policy to a measurement shape.
///
/// The closures receive deterministic stream indices.  Their caller supplies
/// the named purpose through the schedule adapter, so this policy owns timing
/// but never manufactures a sampler domain or a candidate list.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TimedRepetitions {
    /// Number of timed repetitions completed.
    pub reps: usize,
    /// Host wall-clock elapsed while the timed closure ran.
    pub wall_s: f64,
}

/// Run a measurement shape under the protocol's committed warm-up and timed
/// repetition policy.
///
/// The warm-up and timed closures are deliberately separate because their
/// sampler purposes must be distinct. `after_warmup` preserves the grid
/// protocol's thermal sampling boundary without duplicating the timing policy.
/// The timed loop stops under precisely the same minimum-repetition,
/// minimum-duration, and maximum-duration policy used by grid cells. A shape
/// that needs pre-execution censoring keeps that decision in its own semantic
/// cost model, as [`run_cell`] does for Ryser.
pub fn run_timed_repetitions(
    first_index: u64,
    mut warmup: impl FnMut(u64),
    mut after_warmup: impl FnMut(),
    mut timed: impl FnMut(u64),
) -> TimedRepetitions {
    let mut warm_index = first_index;
    let warm_start = Instant::now();
    loop {
        warmup(warm_index);
        warm_index = warm_index.checked_add(1).expect("warm-up index overflow");
        if warm_start.elapsed().as_secs_f64() >= WARMUP_SECONDS {
            break;
        }
    }
    after_warmup();

    let mut index = first_index;
    let timed_start = Instant::now();
    let mut reps = 0;
    loop {
        timed(index);
        index = index
            .checked_add(1)
            .expect("timed repetition index overflow");
        reps += 1;
        let elapsed = timed_start.elapsed().as_secs_f64();
        if (reps >= MIN_REPS && elapsed >= MIN_TIMED_SECONDS) || elapsed >= MAX_CELL_SECONDS {
            return TimedRepetitions {
                reps,
                wall_s: elapsed,
            };
        }
    }
}

/// How a cell finished.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Measured,
    Unsupported,
    Censored,
}

impl Outcome {
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Outcome::Measured => "measured",
            Outcome::Unsupported => "unsupported",
            Outcome::Censored => "censored",
        }
    }
}

/// Everything one grid cell contributes to the CSV.
#[derive(Clone, Debug)]
pub struct CellResult {
    pub q: u64,
    pub n: usize,
    pub backend: &'static str,
    pub outcome: Outcome,
    pub note: String,
    pub batch_size: usize,
    pub reps: usize,
    pub matrices: u64,
    pub zeros: u64,
    /// Summed composite time over every timed repetition.
    pub total_s: f64,
    pub gen_s: f64,
    pub eval_s: f64,
    pub reduce_s: f64,
    pub store_s: f64,
    /// Summed device-event kernel-only duration. It excludes allocation,
    /// transfer, host serialisation, and host submission time.
    pub kernel_device_s: Option<f64>,
    /// Summed device-event host-to-device transfer duration.
    pub h2d_device_s: Option<f64>,
    /// Summed device-event device-to-host transfer duration.
    pub d2h_device_s: Option<f64>,
    /// Summed host-clock duration of the GPU submission wrapper call.
    pub host_submission_s: Option<f64>,
    /// Summed device-event duration from submission marker to kernel start.
    pub device_submission_to_kernel_s: Option<f64>,
    /// Why phase timing columns are absent. Empty only when every timing field
    /// was supplied by the event-instrumented GPU boundary.
    pub phase_timing_note: String,
    /// `matrices / total_s`, or `NaN` for a cell that carries no rate.
    pub composite_rate: f64,
    /// Wall-clock of the single-matrix probe that sized the batch, where one
    /// was run. It is a latency, not a throughput, and no bound on the cell's
    /// batched rate may be derived from it — see the module docs.
    pub probe_matrix_s: f64,
    /// For a censored cell, the estimated batched rate obtained by scaling the
    /// measured rate at [`Self::projection_reference_n`] through Ryser's
    /// `n * 2^n` work model. An **estimate**, not a measurement or a bound.
    pub projected_rate: f64,
    /// The `n` whose measured rate the projection came from, or 0 if none.
    pub projection_reference_n: usize,
    /// Named purpose of every timed repetition.
    pub timed_purpose: MeasurementPurpose,
    /// First index of the *timed* repetitions. Warm-up has a distinct purpose,
    /// so this is independent of its wall-clock-dependent repetition count.
    pub timed_index_first: u64,
    /// `matrices / eval_s`: the host-clock evaluation rate, for attribution
    /// only. GPU kernel-only rate must instead be derived from
    /// `kernel_device_s` when that event measurement is present.
    pub eval_rate: f64,
    pub rep_min_s: f64,
    pub rep_max_s: f64,
    /// Sample standard deviation of the per-repetition composite time.
    pub rep_sd_s: f64,
    pub threads: usize,
    pub pinned_core: String,
    pub seed_root: u64,
    pub seed_index_first: u64,
    pub cpu_mhz_mean: f64,
    pub cpu_temp_c: f64,
    pub gpu_temp_c: f64,
    /// Position of this cell in the randomised execution order.
    pub order_index: usize,
}

/// CSV header for [`CellResult::to_csv_row`].
pub const CELL_CSV_HEADER: &str = "q,n,backend,outcome,batch_size,reps,matrices,zeros,\
total_s,gen_s,eval_s,reduce_s,store_s,composite_matrices_per_s,eval_matrices_per_s,\
kernel_device_s,h2d_device_s,d2h_device_s,host_submission_s,device_submission_to_kernel_s,\
phase_timing_note,\
probe_matrix_s,projected_matrices_per_s,projection_reference_n,timed_purpose,timed_index_first,\
rep_min_s,rep_max_s,rep_sd_s,threads,pinned_core,seed_root,seed_index_first,\
cpu_mhz_mean,cpu_temp_c,gpu_temp_c,order_index,note";

impl CellResult {
    #[must_use]
    pub fn to_csv_row(&self) -> String {
        let mut s = String::new();
        let _ = write!(
            s,
            "{},{},{},{},{},{},{},{},{:.6},{:.6},{:.6},{:.6},{:.6},{:.4},{:.4},\
{},{},{},{},{},{},\
{:.6},{:.6},{},{},{},{:.6},{:.6},{:.6},{},{},0x{:016x},{},{:.1},{:.1},{:.1},{},{}",
            self.q,
            self.n,
            self.backend,
            self.outcome.name(),
            self.batch_size,
            self.reps,
            self.matrices,
            self.zeros,
            self.total_s,
            self.gen_s,
            self.eval_s,
            self.reduce_s,
            self.store_s,
            self.composite_rate,
            self.eval_rate,
            optional_seconds(self.kernel_device_s),
            optional_seconds(self.h2d_device_s),
            optional_seconds(self.d2h_device_s),
            optional_seconds(self.host_submission_s),
            optional_seconds(self.device_submission_to_kernel_s),
            self.phase_timing_note,
            self.probe_matrix_s,
            self.projected_rate,
            self.projection_reference_n,
            self.timed_purpose.name(),
            self.timed_index_first,
            self.rep_min_s,
            self.rep_max_s,
            self.rep_sd_s,
            self.threads,
            self.pinned_core,
            self.seed_root,
            self.seed_index_first,
            self.cpu_mhz_mean,
            self.cpu_temp_c,
            self.gpu_temp_c,
            self.order_index,
            self.note,
        );
        s
    }
}

/// Render an optional event or host-clock duration without inventing zero for
/// a phase that was not measured.
fn optional_seconds(seconds: Option<f64>) -> String {
    seconds.map_or_else(String::new, |value| format!("{value:.6}"))
}

/// Build `m` matrices of order `n` over `F_q` from `sampler`, in the
/// representation `backend` consumes.
///
/// [`Backend::RyserGeneric`] takes the sampler's row-major `Fp` output
/// directly; every other backend takes a packed matrix, and the packing is
/// charged to this generation phase because those kernels cannot run without
/// it. Both forms draw the same entries from the same stream, so a cell's
/// sample does not depend on which backend measures it.
fn generate(backend: Backend, q: u64, n: usize, m: usize, sampler: &mut MatrixSampler) -> Batch {
    if backend == Backend::RyserGeneric {
        return match q {
            3 => Batch::RawF3(n, (0..m).map(|_| sampler.next_matrix::<3>(n)).collect()),
            5 => Batch::RawF5(n, (0..m).map(|_| sampler.next_matrix::<5>(n)).collect()),
            7 => Batch::RawF7(n, (0..m).map(|_| sampler.next_matrix::<7>(n)).collect()),
            _ => panic!("generate: unsupported q = {q}"),
        };
    }
    match q {
        3 => Batch::F3(
            (0..m)
                .map(|_| {
                    let data = sampler.next_matrix::<3>(n);
                    Bipedal3Matrix::from_row_major(&data, n, n)
                })
                .collect(),
        ),
        5 => Batch::F5(
            (0..m)
                .map(|_| {
                    let data = sampler.next_matrix::<5>(n);
                    Packed5Matrix::from_row_major(&data, n, n)
                })
                .collect(),
        ),
        7 => Batch::F7(
            (0..m)
                .map(|_| {
                    let data = sampler.next_matrix::<7>(n);
                    Packed7Matrix::from_row_major(&data, n, n)
                })
                .collect(),
        ),
        _ => panic!("generate: unsupported q = {q}"),
    }
}

/// Histogram `values` over the `q` residue classes.
fn histogram(q: u64, values: &[u64]) -> Vec<u64> {
    let mut hist = vec![0u64; q as usize];
    for &v in values {
        hist[v as usize] += 1;
    }
    hist
}

/// One composite repetition. Returns `(component times, zero count)`.
struct RepTimes {
    gen_s: f64,
    eval_s: f64,
    reduce_s: f64,
    store_s: f64,
    phase_timing: PhaseTiming,
}

impl RepTimes {
    fn total(&self) -> f64 {
        self.gen_s + self.eval_s + self.reduce_s + self.store_s
    }
}

fn one_rep(
    q: u64,
    n: usize,
    m: usize,
    backend: Backend,
    sampler: &mut MatrixSampler,
    index: u64,
    sink: &mut dyn std::io::Write,
) -> (RepTimes, u64) {
    let t0 = Instant::now();
    let batch = generate(backend, q, n, m, sampler);
    let gen_s = t0.elapsed().as_secs_f64();

    let t1 = Instant::now();
    let evaluation = evaluate_timed(backend, &batch);
    let eval_s = t1.elapsed().as_secs_f64();

    let t2 = Instant::now();
    let hist = histogram(q, &evaluation.values);
    let zeros = count_zeros(&evaluation.values);
    let reduce_s = t2.elapsed().as_secs_f64();

    let t3 = Instant::now();
    let mut line = format!("{q},{n},{m},{index},{zeros}");
    for h in &hist {
        let _ = write!(line, ",{h}");
    }
    let _ = writeln!(sink, "{line}");
    let _ = sink.flush();
    let store_s = t3.elapsed().as_secs_f64();

    (
        RepTimes {
            gen_s,
            eval_s,
            reduce_s,
            store_s,
            phase_timing: evaluation.phase_timing,
        },
        zeros,
    )
}

/// Configuration for one cell.
pub struct CellSpec {
    pub q: u64,
    pub n: usize,
    pub backend: Backend,
    /// Fixed batch size, or `None` to calibrate one from a single matrix.
    pub batch_size: Option<usize>,
    pub seed_root: u64,
    /// First stream index this cell consumes within each of its named purposes.
    pub seed_index: u64,
    pub order_index: usize,
}

/// Ryser's per-matrix work at order `n`, up to a constant: `n * 2^n`.
#[must_use]
pub fn ryser_work(n: usize) -> f64 {
    n as f64 * (n as f64).exp2()
}

/// Measured single-matrix probes, keyed by `(q, backend, n)`.
///
/// A cell whose exact `(q, backend, n)` was already probed reuses that number
/// instead of re-measuring it: the two GPU batch-size variants of one `(q, n)`
/// share a probe, and at `q = 7, n = 28` that probe costs about 42 minutes.
pub type ProbeCache = std::collections::HashMap<(u64, &'static str, usize), f64>;

/// Measured *batched* composite rates, keyed by `(q, backend, batch size)`.
///
/// This is what censoring projects from. `batch_size` is the cell's requested
/// size, or 0 for the adaptive backends, so a projection only ever compares
/// like with like.
pub type RateCache = std::collections::HashMap<(u64, &'static str, usize), (usize, f64)>;

/// The measured probe at exactly `(q, backend, n)`, if one exists.
#[must_use]
pub fn exact_probe(probes: &ProbeCache, q: u64, backend: Backend, n: usize) -> Option<f64> {
    probes.get(&(q, backend.name(), n)).copied()
}

/// Record a measured batched rate, keeping the reference closest to `n` from
/// below — the nearest size minimises the extrapolation distance.
pub fn record_rate(
    rates: &mut RateCache,
    q: u64,
    backend: Backend,
    batch_key: usize,
    n: usize,
    rate: f64,
) {
    if !rate.is_finite() || rate <= 0.0 {
        return;
    }
    rates
        .entry((q, backend.name(), batch_key))
        .and_modify(|e| {
            if n > e.0 {
                *e = (n, rate);
            }
        })
        .or_insert((n, rate));
}

/// Project this cell's batched rate from the nearest measured rate on the same
/// `(q, backend, batch size)`.
///
/// Returns `(reference n, projected rate)`. See the module docs for why this,
/// and not any function of the single-matrix probe, is the defensible estimate.
#[must_use]
pub fn project_rate(
    rates: &RateCache,
    q: u64,
    backend: Backend,
    batch_key: usize,
    n: usize,
) -> Option<(usize, f64)> {
    rates
        .get(&(q, backend.name(), batch_key))
        .filter(|(ref_n, _)| *ref_n != n)
        .map(|(ref_n, ref_rate)| (*ref_n, ref_rate * ryser_work(*ref_n) / ryser_work(n)))
}

/// Run one cell of the grid end to end.
///
/// `sink` receives the shard records written during the timed `store` phase;
/// pass a handle to a scratch file so the measured cost is a real filesystem
/// write rather than a discard.
///
/// `probes` caches single-matrix latencies so a repeated `(q, backend, n)` is
/// not re-measured. `rates` accumulates measured batched rates and is the sole
/// basis for censoring; both are updated in place.
pub fn run_cell(
    spec: &CellSpec,
    sink: &mut dyn std::io::Write,
    probes: &mut ProbeCache,
    rates: &mut RateCache,
) -> CellResult {
    let CellSpec {
        q,
        n,
        backend,
        seed_root,
        seed_index,
        order_index,
        ..
    } = *spec;

    let mut result = CellResult {
        q,
        n,
        backend: backend.name(),
        outcome: Outcome::Unsupported,
        note: String::new(),
        // Seeded with the requested batch size so that unsupported and censored
        // rows still identify which of a backend's batch-size variants they
        // are; the adaptive backends overwrite it once the probe sizes it.
        batch_size: spec.batch_size.unwrap_or(0),
        reps: 0,
        matrices: 0,
        zeros: 0,
        total_s: 0.0,
        gen_s: 0.0,
        eval_s: 0.0,
        reduce_s: 0.0,
        store_s: 0.0,
        kernel_device_s: None,
        h2d_device_s: None,
        d2h_device_s: None,
        host_submission_s: None,
        device_submission_to_kernel_s: None,
        phase_timing_note: if backend == Backend::Gpu {
            PHASE_TIMING_NOT_RUN.to_string()
        } else {
            "event timing unavailable: backend is not GPU/HIP".to_string()
        },
        composite_rate: f64::NAN,
        eval_rate: f64::NAN,
        probe_matrix_s: f64::NAN,
        projected_rate: f64::NAN,
        projection_reference_n: 0,
        timed_purpose: scheduled_backend(backend, SchedulePhase::GridTimed).purpose(),
        timed_index_first: 0,
        rep_min_s: f64::NAN,
        rep_max_s: f64::NAN,
        rep_sd_s: f64::NAN,
        threads: if backend.is_multithreaded() {
            rayon::current_num_threads()
        } else {
            1
        },
        pinned_core: String::new(),
        seed_root,
        seed_index_first: seed_index,
        cpu_mhz_mean: f64::NAN,
        cpu_temp_c: f64::NAN,
        gpu_temp_c: f64::NAN,
        order_index,
    };

    if let Support::Unsupported(reason) = support(backend, q, n) {
        result.phase_timing_note = format!("event timing unavailable: {reason}");
        result.note = reason;
        return result;
    }
    if matches!(backend, Backend::Avx2 | Backend::RayonAvx2) && crate::backend::avx2_fns().is_none()
    {
        result.note = "AVX2 not detected at runtime".to_string();
        return result;
    }

    // Single-thread cells run pinned to one physical core; multithreaded cells
    // release the mask so the rayon pool sees the whole machine.
    let pin_target = if backend.is_multithreaded() {
        None
    } else {
        Some(PINNED_CORE)
    };
    let pinned_ok = pin_thread(pin_target);
    result.pinned_core = match (pin_target, pinned_ok) {
        (Some(c), true) => c.to_string(),
        (None, true) => "all".to_string(),
        (_, false) => "pin-failed".to_string(),
    };

    let mut devnull = std::io::sink();

    // Censoring decision, made from a measured batched rate at another n on the
    // same (q, backend, batch size). This is the only inference used: no
    // function of the single-matrix probe bounds a batched rate (module docs).
    let batch_key = spec.batch_size.unwrap_or(0);
    if let Some((ref_n, projected)) = project_rate(rates, q, backend, batch_key, n) {
        result.projected_rate = projected;
        result.projection_reference_n = ref_n;
        let projected_rep_s = match spec.batch_size {
            // Fixed-batch cells run M matrices per repetition whatever the cost.
            Some(m) => m as f64 / projected.max(1e-12),
            // Adaptive cells size themselves, so the only unaffordable case is a
            // single matrix already exceeding the cap.
            None => 1.0 / projected.max(1e-12),
        };
        if projected_rep_s > MAX_CELL_SECONDS {
            result.outcome = Outcome::Censored;
            result.note = format!(
                "not attempted: the measured rate at n={ref_n} projects to {projected:.4} \
matrices/s at n={n} under Ryser's n*2^n work model, so one repetition would take \
{projected_rep_s:.0} s against the {MAX_CELL_SECONDS:.0} s cap. The projection is an \
ESTIMATE; where it can be checked against a measurement, on the q=3 GPU chain, it lands \
LOW, so this cell's true rate is expected to be somewhat higher - an extrapolation from \
that chain, not a measurement of this cell. The magnitude is re-derived from this file's \
own q=3 chain in the study. The cell carries no measured rate"
            );
            return result;
        }
    }

    // Probe: one matrix, to size the batch for the adaptive backends. Fixed-batch
    // cells need no probe at all once a projection exists. A probe already
    // measured for this exact (q, backend, n) is reused, since the two GPU
    // batch-size variants of one (q, n) would otherwise pay for it twice.
    let needs_probe = spec.batch_size.is_none() || result.projection_reference_n == 0;
    let per_matrix_s = if needs_probe {
        match exact_probe(probes, q, backend, n) {
            Some(cached) => cached,
            None => {
                let cal = Instant::now();
                let mut sampler = MatrixSampler::new(
                    seed_root,
                    q,
                    n,
                    scheduled_backend(backend, SchedulePhase::GridProbe).purpose(),
                    seed_index,
                );
                let _ = one_rep(q, n, 1, backend, &mut sampler, seed_index, &mut devnull);
                let measured = cal.elapsed().as_secs_f64();
                probes.insert((q, backend.name(), n), measured);
                measured
            }
        }
    } else {
        f64::NAN
    };
    result.probe_matrix_s = per_matrix_s;

    // Fallback for a cell with no projection reference yet: a single matrix
    // costing more than the whole per-cell budget cannot be batched into it.
    if per_matrix_s.is_finite() && per_matrix_s > MAX_CELL_SECONDS {
        result.outcome = Outcome::Censored;
        result.note = format!(
            "not attempted: one matrix alone took {per_matrix_s:.1} s, beyond the \
{MAX_CELL_SECONDS:.0} s per-cell cap, and no measured rate at another n was available to \
project from. This is a latency, not a throughput: no bound on the batched rate follows \
from it, and the cell carries no rate"
        );
        return result;
    }

    let m = spec.batch_size.unwrap_or_else(|| {
        let target = (TARGET_REP_SECONDS / per_matrix_s.max(1e-9)).ceil() as usize;
        // A rayon batch is floored at several matrices per worker, not at one:
        // with a batch barely larger than the pool, the tail of every batch
        // leaves most workers idle. The floor keeps the grid and a real campaign
        // on the same footing. No committed receipt measures the size of that
        // penalty — the sweep that once did belongs to a discarded receipt set —
        // so no magnitude is quoted here.
        let floor = if backend.is_multithreaded() {
            MATRICES_PER_WORKER * rayon::current_num_threads()
        } else {
            1
        };
        target.clamp(floor, MAX_BATCH)
    });
    result.batch_size = m;

    let mut rep_totals: Vec<f64> = Vec::new();
    let mut thermal = None;
    // `run_timed_repetitions` keeps the warm-up, minimum repetitions,
    // minimum timed duration, and cap canonical for every timing shape.  The
    // distinct scheduled purposes ensure a wall-clock-dependent warm-up count
    // cannot alter the timed sample's address sequence.
    result.timed_index_first = seed_index;
    let policy = run_timed_repetitions(
        seed_index,
        |warm_index| {
            let mut s = MatrixSampler::new(
                seed_root,
                q,
                n,
                scheduled_backend(backend, SchedulePhase::GridWarmup).purpose(),
                warm_index,
            );
            let _ = one_rep(q, n, m, backend, &mut s, warm_index, &mut devnull);
        },
        || thermal = Some(ThermalSample::probe()),
        |index| {
            let mut s = MatrixSampler::new(
                seed_root,
                q,
                n,
                scheduled_backend(backend, SchedulePhase::GridTimed).purpose(),
                index,
            );
            let (times, zeros) = one_rep(q, n, m, backend, &mut s, index, sink);
            rep_totals.push(times.total());
            result.gen_s += times.gen_s;
            result.eval_s += times.eval_s;
            result.reduce_s += times.reduce_s;
            result.store_s += times.store_s;
            result.matrices += m as u64;
            result.zeros += zeros;
            result.reps += 1;
            accumulate_gpu_phase_timings(&mut result, times.phase_timing);
        },
    );
    let thermal = thermal.expect("canonical timing policy runs its post-warm-up hook");
    result.cpu_mhz_mean = thermal.cpu_mhz_mean;
    result.cpu_temp_c = thermal.cpu_temp_c;
    result.gpu_temp_c = thermal.gpu_temp_c;
    debug_assert_eq!(result.reps, policy.reps);

    result.total_s = rep_totals.iter().sum();
    // Rates come from summed time over summed matrices, never from averaging
    // per-repetition rates: the mean of reciprocals is not the reciprocal of
    // the mean.
    result.composite_rate = result.matrices as f64 / result.total_s;
    result.eval_rate = result.matrices as f64 / result.eval_s;
    result.rep_min_s = rep_totals.iter().copied().fold(f64::INFINITY, f64::min);
    result.rep_max_s = rep_totals.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    result.rep_sd_s = sample_sd(&rep_totals);
    result.outcome = Outcome::Measured;
    record_rate(rates, q, backend, batch_key, n, result.composite_rate);
    result
}

/// Accumulate the independent timings from each event-instrumented GPU
/// dispatch. A cell has one backend, so it never combines a device-clock
/// duration with a host timestamp or a non-GPU evaluation wall clock.
fn accumulate_gpu_phase_timings(result: &mut CellResult, phase_timing: PhaseTiming) {
    match phase_timing {
        PhaseTiming::Measured(timings)
            if result.phase_timing_note.is_empty()
                || result.phase_timing_note == PHASE_TIMING_NOT_RUN =>
        {
            result.kernel_device_s = add_duration(result.kernel_device_s, timings.kernel);
            result.h2d_device_s = add_duration(result.h2d_device_s, timings.h2d);
            result.d2h_device_s = add_duration(result.d2h_device_s, timings.d2h);
            result.host_submission_s =
                add_duration(result.host_submission_s, timings.host_submission);
            result.device_submission_to_kernel_s = add_duration(
                result.device_submission_to_kernel_s,
                timings.device_submission_to_kernel,
            );
            result.phase_timing_note.clear();
        }
        PhaseTiming::Measured(_) | PhaseTiming::NotApplicable => {}
        PhaseTiming::Unavailable(reason) => {
            result.kernel_device_s = None;
            result.h2d_device_s = None;
            result.d2h_device_s = None;
            result.host_submission_s = None;
            result.device_submission_to_kernel_s = None;
            result.phase_timing_note = reason;
        }
    }
}

fn add_duration(total: Option<f64>, duration: std::time::Duration) -> Option<f64> {
    Some(total.unwrap_or(0.0) + duration.as_secs_f64())
}

/// Sample standard deviation, or `NaN` for fewer than two observations.
#[must_use]
pub fn sample_sd(xs: &[f64]) -> f64 {
    if xs.len() < 2 {
        return f64::NAN;
    }
    let mean = xs.iter().sum::<f64>() / xs.len() as f64;
    let var = xs.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (xs.len() - 1) as f64;
    var.sqrt()
}

/// Load the machine to thermal steady state before the grid starts.
///
/// Runs `permanent_bipedal3_singleword` across every rayon worker for
/// `seconds`, which is the same instruction mix the grid measures.
pub fn warm_machine(seconds: f64, seed_root: u64) {
    pin_thread(None);
    let start = Instant::now();
    let mut stream = 0u64;
    while start.elapsed().as_secs_f64() < seconds {
        stream += 1;
        let mut s = MatrixSampler::new(seed_root, 3, 22, MeasurementPurpose::MachineWarmup, stream);
        let batch = generate(Backend::Rayon, 3, 22, rayon::current_num_threads(), &mut s);
        let _ = evaluate(Backend::Rayon, &batch);
    }
}

/// Sustained streaming throughput: run the composite hot path continuously for
/// `seconds` and report the achieved rate over the whole window.
///
/// This is the check on the short-cell projections. A short cell can ride a
/// boost window; a minutes-scale run cannot.
pub struct SustainedResult {
    pub q: u64,
    pub n: usize,
    pub backend: &'static str,
    pub batch_size: usize,
    pub shards: usize,
    pub matrices: u64,
    pub zeros: u64,
    pub wall_s: f64,
    pub sustained_rate: f64,
    pub first_quarter_rate: f64,
    pub last_quarter_rate: f64,
    pub cpu_mhz_start: f64,
    pub cpu_mhz_end: f64,
    pub cpu_temp_start_c: f64,
    pub cpu_temp_end_c: f64,
    pub gpu_temp_end_c: f64,
    pub seed_root: u64,
    /// Named purpose of every sustained shard.
    pub purpose: MeasurementPurpose,
    /// First index this run drew from. Runs reserve disjoint index ranges
    /// within their named purpose, so two runs at the same `(q, n)` are
    /// independently addressed.
    pub index_first: u64,
}

/// CSV header for [`SustainedResult::to_csv_row`].
pub const SUSTAINED_CSV_HEADER: &str = "q,n,backend,batch_size,shards,matrices,zeros,wall_s,\
sustained_matrices_per_s,first_quarter_matrices_per_s,last_quarter_matrices_per_s,\
cpu_mhz_start,cpu_mhz_end,cpu_temp_start_c,cpu_temp_end_c,gpu_temp_end_c,seed_root,purpose,index_first";

impl SustainedResult {
    #[must_use]
    pub fn to_csv_row(&self) -> String {
        format!(
            "{},{},{},{},{},{},{},{:.3},{:.4},{:.4},{:.4},{:.1},{:.1},{:.1},{:.1},{:.1},0x{:016x},{},{}",
            self.q,
            self.n,
            self.backend,
            self.batch_size,
            self.shards,
            self.matrices,
            self.zeros,
            self.wall_s,
            self.sustained_rate,
            self.first_quarter_rate,
            self.last_quarter_rate,
            self.cpu_mhz_start,
            self.cpu_mhz_end,
            self.cpu_temp_start_c,
            self.cpu_temp_end_c,
            self.gpu_temp_end_c,
            self.seed_root,
            self.purpose.name(),
            self.index_first,
        )
    }
}

/// Run the composite hot path continuously for `seconds`.
/// Configuration for one sustained streaming run.
pub struct SustainedSpec {
    pub q: u64,
    pub n: usize,
    pub backend: Backend,
    pub batch_size: usize,
    pub seconds: f64,
    pub seed_root: u64,
    /// Selects a reserved index range within the sustained purpose, so two runs at the same `(q, n)` never
    /// draw the same matrices. Without it every run started at the same stream
    /// and two runs at one `(q, n)` produced overlapping samples whose zero
    /// counts could not be pooled — the shorter run's matrices were a prefix of
    /// the longer one's.
    pub run_index: u64,
}

/// Stream the composite hot path for a fixed window.
pub fn run_sustained(spec: &SustainedSpec, sink: &mut dyn std::io::Write) -> SustainedResult {
    let SustainedSpec {
        q,
        n,
        backend,
        batch_size,
        seconds,
        seed_root,
        run_index,
    } = *spec;
    pin_thread(if backend.is_multithreaded() {
        None
    } else {
        Some(PINNED_CORE)
    });

    let start_thermal = ThermalSample::probe();
    let mut shard_times: Vec<f64> = Vec::new();
    let mut matrices = 0u64;
    let mut zeros = 0u64;
    let index_base = run_index
        .checked_mul(SUSTAINED_INDICES_PER_RUN)
        .expect("sustained run index overflow");
    let mut index = index_base;

    let start = Instant::now();
    while start.elapsed().as_secs_f64() < seconds {
        let mut s = MatrixSampler::new(
            seed_root,
            q,
            n,
            scheduled_backend(backend, SchedulePhase::Sustained).purpose(),
            index,
        );
        let t = Instant::now();
        let (_, z) = one_rep(q, n, batch_size, backend, &mut s, index, sink);
        index = index
            .checked_add(1)
            .expect("sustained shard index overflow");
        shard_times.push(t.elapsed().as_secs_f64());
        matrices += batch_size as u64;
        zeros += z;
    }
    let wall_s = start.elapsed().as_secs_f64();
    let end_thermal = ThermalSample::probe();

    // First and last quarter of ELAPSED TIME, not of the shard count: shard
    // durations vary, so a count-based split does not partition the window and
    // would misreport drift whenever the two differ. Each side takes whole
    // shards until their accumulated time reaches a quarter of the timed total,
    // and always at least one shard.
    let timed_total: f64 = shard_times.iter().sum();
    let cut = timed_total / 4.0;
    let mut first = 0.0;
    let mut first_shards = 0usize;
    for &t in &shard_times {
        first += t;
        first_shards += 1;
        if first >= cut {
            break;
        }
    }
    let mut last = 0.0;
    let mut last_shards = 0usize;
    for &t in shard_times.iter().rev() {
        last += t;
        last_shards += 1;
        if last >= cut {
            break;
        }
    }

    SustainedResult {
        q,
        n,
        backend: backend.name(),
        batch_size,
        shards: shard_times.len(),
        matrices,
        zeros,
        wall_s,
        sustained_rate: matrices as f64 / wall_s,
        first_quarter_rate: (first_shards * batch_size) as f64 / first,
        last_quarter_rate: (last_shards * batch_size) as f64 / last,
        cpu_mhz_start: start_thermal.cpu_mhz_mean,
        cpu_mhz_end: end_thermal.cpu_mhz_mean,
        cpu_temp_start_c: start_thermal.cpu_temp_c,
        cpu_temp_end_c: end_thermal.cpu_temp_c,
        gpu_temp_end_c: end_thermal.gpu_temp_c,
        seed_root,
        purpose: scheduled_backend(backend, SchedulePhase::Sustained).purpose(),
        index_first: index_base,
    }
}

/// Deterministic in-place shuffle used to randomise cell execution order.
///
/// Fisher–Yates driven by a ChaCha20 stream so the order is recorded by its
/// seed and reproducible.
pub fn shuffle<T>(items: &mut [T], seed_root: u64) {
    let mut sampler = MatrixSampler::new(seed_root, 0xFFFF_FFFF, 0, MeasurementPurpose::Shuffle, 0);
    for i in (1..items.len()).rev() {
        // Rejection-sample an index in 0..=i to avoid modulo bias.
        let bound = (i + 1) as u64;
        let limit = u64::MAX - (u64::MAX % bound);
        let j = loop {
            let mut bytes = [0u8; 8];
            for b in &mut bytes {
                *b = sampler.next_raw_byte();
            }
            let x = u64::from_le_bytes(bytes);
            if x < limit {
                break x % bound;
            }
        };
        items.swap(i, j as usize);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::GpuPhaseTimings;
    use std::collections::BTreeMap;

    fn example_result() -> CellResult {
        CellResult {
            q: 3,
            n: 12,
            backend: "gpu_hip",
            outcome: Outcome::Measured,
            note: String::new(),
            batch_size: 256,
            reps: 5,
            matrices: 1_280,
            zeros: 427,
            total_s: 12.0,
            gen_s: 1.0,
            eval_s: 9.0,
            reduce_s: 1.0,
            store_s: 1.0,
            kernel_device_s: Some(0.25),
            h2d_device_s: Some(0.125),
            d2h_device_s: Some(0.0625),
            host_submission_s: Some(0.015625),
            device_submission_to_kernel_s: Some(0.03125),
            phase_timing_note: String::new(),
            composite_rate: 106.666_666_666_7,
            probe_matrix_s: f64::NAN,
            projected_rate: f64::NAN,
            projection_reference_n: 0,
            timed_purpose: MeasurementPurpose::GridTimed,
            timed_index_first: 42,
            eval_rate: 142.222_222_222_2,
            rep_min_s: 2.0,
            rep_max_s: 3.0,
            rep_sd_s: 0.5,
            threads: 1,
            pinned_core: "0".to_string(),
            seed_root: 0xB488_F02C_0000_0001,
            seed_index_first: 42,
            cpu_mhz_mean: 3_600.0,
            cpu_temp_c: 60.0,
            gpu_temp_c: 70.0,
            order_index: 2,
        }
    }

    fn csv_fields(result: &CellResult) -> BTreeMap<&'static str, String> {
        let header = CELL_CSV_HEADER.split(',');
        let row = result.to_csv_row();
        header.zip(row.split(',').map(str::to_string)).collect()
    }

    #[test]
    fn cell_csv_schema_is_canonical() {
        assert_eq!(
            CELL_CSV_HEADER,
            "q,n,backend,outcome,batch_size,reps,matrices,zeros,total_s,gen_s,eval_s,reduce_s,store_s,composite_matrices_per_s,eval_matrices_per_s,kernel_device_s,h2d_device_s,d2h_device_s,host_submission_s,device_submission_to_kernel_s,phase_timing_note,probe_matrix_s,projected_matrices_per_s,projection_reference_n,timed_purpose,timed_index_first,rep_min_s,rep_max_s,rep_sd_s,threads,pinned_core,seed_root,seed_index_first,cpu_mhz_mean,cpu_temp_c,gpu_temp_c,order_index,note"
        );
    }

    #[test]
    fn csv_keeps_phase_clocks_distinct_from_evaluation_wall_clock() {
        let fields = csv_fields(&example_result());

        assert_eq!(fields["eval_s"], "9.000000");
        assert_eq!(fields["kernel_device_s"], "0.250000");
        assert_eq!(fields["h2d_device_s"], "0.125000");
        assert_eq!(fields["d2h_device_s"], "0.062500");
        assert_eq!(fields["host_submission_s"], "0.015625");
        assert_eq!(fields["device_submission_to_kernel_s"], "0.031250");
        assert!(fields["phase_timing_note"].is_empty());
        assert_ne!(fields["kernel_device_s"], fields["eval_s"]);
    }

    #[test]
    fn unavailable_phase_measurements_are_blank_with_a_reason() {
        let mut result = example_result();
        result.backend = "cpu_scalar";
        result.kernel_device_s = None;
        result.h2d_device_s = None;
        result.d2h_device_s = None;
        result.host_submission_s = None;
        result.device_submission_to_kernel_s = None;
        result.phase_timing_note = "event timing unavailable: backend is not GPU/HIP".to_string();

        let fields = csv_fields(&result);
        for column in [
            "kernel_device_s",
            "h2d_device_s",
            "d2h_device_s",
            "host_submission_s",
            "device_submission_to_kernel_s",
        ] {
            assert!(fields[column].is_empty(), "{column} must remain absent");
        }
        assert_eq!(
            fields["phase_timing_note"],
            "event timing unavailable: backend is not GPU/HIP"
        );
    }

    #[test]
    fn instrumentation_failure_clears_partial_phases_and_keeps_its_reason() {
        let mut result = example_result();
        let reason =
            "event timing unavailable: create instrumented permanent stream: no HIP device; \
values came from synchronous gpu_hip dispatch";

        accumulate_gpu_phase_timings(&mut result, PhaseTiming::Unavailable(reason.to_string()));
        accumulate_gpu_phase_timings(
            &mut result,
            PhaseTiming::Measured(GpuPhaseTimings {
                h2d: std::time::Duration::from_secs(1),
                kernel: std::time::Duration::from_secs(1),
                d2h: std::time::Duration::from_secs(1),
                host_submission: std::time::Duration::from_secs(1),
                device_submission_to_kernel: std::time::Duration::from_secs(1),
            }),
        );

        let fields = csv_fields(&result);
        for column in [
            "kernel_device_s",
            "h2d_device_s",
            "d2h_device_s",
            "host_submission_s",
            "device_submission_to_kernel_s",
        ] {
            assert!(fields[column].is_empty(), "{column} must remain absent");
        }
        assert_eq!(fields["phase_timing_note"], reason);
    }
}
