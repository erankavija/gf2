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
//! # Repetition and censoring policy
//!
//! Every cell first runs an untimed warm-up of at least [`WARMUP_SECONDS`], on
//! the assumption that clocks and the GPU reach steady state within it - an
//! assumption the sustained runs bound rather than verify - drawing from a
//! stream sub-range
//! reserved for it so the timed sample stays independent of how many warm-up
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

use crate::backend::{count_zeros, evaluate, support, Backend, Batch, Support};
use crate::env::{pin_thread, ThermalSample};
use crate::sampler::MatrixSampler;

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
/// Offset into a cell's reserved stream range at which warm-up draws begin.
///
/// Warm-up runs for a wall-clock duration, so its repetition count varies with
/// machine speed. Giving it its own sub-range keeps the timed sub-range —
/// which starts at the cell's base stream — independent of that variation, so
/// a recorded seed regenerates exactly the recorded sample.
pub const WARMUP_STREAM_OFFSET: u64 = 50_000;
/// First stream index used by `sustained`, offset per run so that two runs at
/// the same `(q, n)` never draw the same matrices.
///
/// Chosen far above the grid's reserved space rather than merely beside it. The
/// grid hands cell `i` the range `1 + i * STREAMS_PER_CELL`, so with a base of
/// `1_000_000` the two allocations were commensurate and collided in index
/// space; they stayed disjoint in practice only because no colliding pair
/// shared a `(q, n)`. Disjointness by construction is worth more than
/// disjointness by audit.
pub const SUSTAINED_STREAM_BASE: u64 = 1_000_000_000;
/// Stream indices reserved to each sustained run.
pub const SUSTAINED_STREAMS_PER_RUN: u64 = 100_000;

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
    /// First stream index of the *timed* repetitions. Warm-up uses a disjoint
    /// sub-range, so this is `seed_stream + 1` for every measured cell and the
    /// timed sample regenerates from it without knowing the warm-up count.
    pub timed_stream_first: u64,
    /// `matrices / eval_s`: the kernel-only rate, for attribution only.
    pub eval_rate: f64,
    pub rep_min_s: f64,
    pub rep_max_s: f64,
    /// Sample standard deviation of the per-repetition composite time.
    pub rep_sd_s: f64,
    pub threads: usize,
    pub pinned_core: String,
    pub seed_root: u64,
    pub seed_stream_first: u64,
    pub cpu_mhz_mean: f64,
    pub cpu_temp_c: f64,
    pub gpu_temp_c: f64,
    /// Position of this cell in the randomised execution order.
    pub order_index: usize,
}

/// CSV header for [`CellResult::to_csv_row`].
pub const CELL_CSV_HEADER: &str = "q,n,backend,outcome,batch_size,reps,matrices,zeros,\
total_s,gen_s,eval_s,reduce_s,store_s,composite_matrices_per_s,eval_matrices_per_s,\
probe_matrix_s,projected_matrices_per_s,projection_reference_n,timed_stream_first,\
rep_min_s,rep_max_s,rep_sd_s,threads,pinned_core,seed_root,seed_stream_first,\
cpu_mhz_mean,cpu_temp_c,gpu_temp_c,order_index,note";

impl CellResult {
    #[must_use]
    pub fn to_csv_row(&self) -> String {
        let mut s = String::new();
        let _ = write!(
            s,
            "{},{},{},{},{},{},{},{},{:.6},{:.6},{:.6},{:.6},{:.6},{:.4},{:.4},\
{:.6},{:.6},{},{},{:.6},{:.6},{:.6},{},{},0x{:016x},{},{:.1},{:.1},{:.1},{},{}",
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
            self.probe_matrix_s,
            self.projected_rate,
            self.projection_reference_n,
            self.timed_stream_first,
            self.rep_min_s,
            self.rep_max_s,
            self.rep_sd_s,
            self.threads,
            self.pinned_core,
            self.seed_root,
            self.seed_stream_first,
            self.cpu_mhz_mean,
            self.cpu_temp_c,
            self.gpu_temp_c,
            self.order_index,
            self.note,
        );
        s
    }
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
    stream: u64,
    sink: &mut dyn std::io::Write,
) -> (RepTimes, u64) {
    let t0 = Instant::now();
    let batch = generate(backend, q, n, m, sampler);
    let gen_s = t0.elapsed().as_secs_f64();

    let t1 = Instant::now();
    let values = evaluate(backend, &batch);
    let eval_s = t1.elapsed().as_secs_f64();

    let t2 = Instant::now();
    let hist = histogram(q, &values);
    let zeros = count_zeros(&values);
    let reduce_s = t2.elapsed().as_secs_f64();

    let t3 = Instant::now();
    let mut line = format!("{q},{n},{m},{stream},{zeros}");
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
    /// First ChaCha20 stream index this cell consumes.
    pub seed_stream: u64,
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
        seed_stream,
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
        composite_rate: f64::NAN,
        eval_rate: f64::NAN,
        probe_matrix_s: f64::NAN,
        projected_rate: f64::NAN,
        projection_reference_n: 0,
        timed_stream_first: 0,
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
        seed_stream_first: seed_stream,
        cpu_mhz_mean: f64::NAN,
        cpu_temp_c: f64::NAN,
        gpu_temp_c: f64::NAN,
        order_index,
    };

    if let Support::Unsupported(reason) = support(backend, q, n) {
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

    let mut stream = seed_stream;
    let mut sampler = MatrixSampler::new(seed_root, q, n, stream);
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
                let _ = one_rep(q, n, 1, backend, &mut sampler, stream, &mut devnull);
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

    // Warm-up draws from a sub-range reserved for it, disjoint from the timed
    // sub-range. The number of warm-up repetitions depends on wall-clock, so if
    // warm-up and timed work shared one running counter the first timed stream
    // would depend on how fast the machine happened to be — and the recorded
    // seed would not regenerate the recorded sample. Reserving the ranges makes
    // the timed sequence a pure function of the cell's base stream.
    let warmup_base = seed_stream + WARMUP_STREAM_OFFSET;
    let mut warm_stream = warmup_base;
    let warm = Instant::now();
    loop {
        let mut s = MatrixSampler::new(seed_root, q, n, warm_stream);
        let _ = one_rep(q, n, m, backend, &mut s, warm_stream, &mut devnull);
        warm_stream += 1;
        if warm.elapsed().as_secs_f64() >= WARMUP_SECONDS {
            break;
        }
    }
    // Timed repetitions always start here, whatever the warm-up did.
    stream = seed_stream;
    result.timed_stream_first = stream + 1;

    let thermal = ThermalSample::probe();
    result.cpu_mhz_mean = thermal.cpu_mhz_mean;
    result.cpu_temp_c = thermal.cpu_temp_c;
    result.gpu_temp_c = thermal.gpu_temp_c;

    let mut rep_totals: Vec<f64> = Vec::new();
    let timed_start = Instant::now();
    loop {
        stream += 1;
        let mut s = MatrixSampler::new(seed_root, q, n, stream);
        let (times, zeros) = one_rep(q, n, m, backend, &mut s, stream, sink);
        rep_totals.push(times.total());
        result.gen_s += times.gen_s;
        result.eval_s += times.eval_s;
        result.reduce_s += times.reduce_s;
        result.store_s += times.store_s;
        result.matrices += m as u64;
        result.zeros += zeros;
        result.reps += 1;

        let elapsed = timed_start.elapsed().as_secs_f64();
        if (result.reps >= MIN_REPS && elapsed >= MIN_TIMED_SECONDS) || elapsed >= MAX_CELL_SECONDS
        {
            break;
        }
    }

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
        let mut s = MatrixSampler::new(seed_root, 3, 22, stream);
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
    /// First stream index this run drew from. Runs reserve disjoint ranges, so
    /// two runs at the same `(q, n)` produce independent samples.
    pub stream_first: u64,
}

/// CSV header for [`SustainedResult::to_csv_row`].
pub const SUSTAINED_CSV_HEADER: &str = "q,n,backend,batch_size,shards,matrices,zeros,wall_s,\
sustained_matrices_per_s,first_quarter_matrices_per_s,last_quarter_matrices_per_s,\
cpu_mhz_start,cpu_mhz_end,cpu_temp_start_c,cpu_temp_end_c,gpu_temp_end_c,seed_root,stream_first";

impl SustainedResult {
    #[must_use]
    pub fn to_csv_row(&self) -> String {
        format!(
            "{},{},{},{},{},{},{},{:.3},{:.4},{:.4},{:.4},{:.1},{:.1},{:.1},{:.1},{:.1},0x{:016x},{}",
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
            self.stream_first,
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
    /// Selects a reserved stream range, so two runs at the same `(q, n)` never
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
    let stream_base = SUSTAINED_STREAM_BASE + run_index * SUSTAINED_STREAMS_PER_RUN;
    let mut stream = stream_base;

    let start = Instant::now();
    while start.elapsed().as_secs_f64() < seconds {
        stream += 1;
        let mut s = MatrixSampler::new(seed_root, q, n, stream);
        let t = Instant::now();
        let (_, z) = one_rep(q, n, batch_size, backend, &mut s, stream, sink);
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
        stream_first: stream_base + 1,
    }
}

/// Deterministic in-place shuffle used to randomise cell execution order.
///
/// Fisher–Yates driven by a ChaCha20 stream so the order is recorded by its
/// seed and reproducible.
pub fn shuffle<T>(items: &mut [T], seed_root: u64) {
    let mut sampler = MatrixSampler::new(seed_root, 0xFFFF_FFFF, 0, 0xFFFF_FFFF);
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
