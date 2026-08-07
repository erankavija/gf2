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
//! Every cell first runs an untimed warm-up of at least [`WARMUP_SECONDS`], to
//! let boost clocks and the GPU settle. It then repeats until it has both at
//! least [`MIN_REPS`] repetitions and at least [`MIN_TIMED_SECONDS`] of timed
//! work, stopping early only at [`MAX_CELL_SECONDS`].
//!
//! Before that, every cell runs a single-matrix **probe**, which sizes the
//! batch for the adaptive backends and decides whether the cell is affordable
//! at all. A probe exceeding [`CENSOR_MATRIX_SECONDS`] censors the cell.
//!
//! Three outcomes are therefore distinguishable in the CSV: `measured`,
//! `unsupported` (a kernel bound forbids the cell), and `censored` (the cell
//! was not attempted at its batch size because the probe was too slow).
//!
//! # Why a censored cell reports no rate
//!
//! A censored cell records its probe time in `probe_matrix_s` and leaves
//! `composite_matrices_per_s` empty. The probe's reciprocal is deliberately
//! *not* published as a throughput bound: the GPU backend parallelises across
//! the batch, one block per matrix, so a batch of one occupies a single compute
//! unit and runs roughly two orders of magnitude below the device's batched
//! rate. Treating `1 / probe` as an upper bound would therefore be false for
//! exactly the backend most likely to be censored. The probe time is reported
//! as what it is — the cost of one matrix — and the cell is marked as carrying
//! no rate measurement.

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
/// Per-matrix cost above which a cell is censored instead of measured.
pub const CENSOR_MATRIX_SECONDS: f64 = 150.0;
/// Compute units on the GPU, used to project a batch's wall-clock from a
/// single-matrix probe: the kernel runs one block per matrix, so at most this
/// many matrices are resident at once. Read from `rocminfo` on the benchmark
/// host (AMD Radeon RX 6950 XT, gfx1030).
pub const GPU_COMPUTE_UNITS: usize = 80;
/// Batch wall-clock each cell's `M` is calibrated to hit.
pub const TARGET_REP_SECONDS: f64 = 2.0;
/// Ceiling on `M`, to bound a cell's resident matrix memory.
pub const MAX_BATCH: usize = 65_536;
/// Physical core the single-thread cells are pinned to.
pub const PINNED_CORE: usize = 0;

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
    /// Wall-clock of the single-matrix probe that sized the batch and decided
    /// affordability. Reported for every attempted cell, and the sole
    /// quantitative evidence a censored cell carries.
    pub probe_matrix_s: f64,
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
probe_matrix_s,rep_min_s,rep_max_s,rep_sd_s,threads,pinned_core,seed_root,seed_stream_first,\
cpu_mhz_mean,cpu_temp_c,gpu_temp_c,order_index,note";

impl CellResult {
    #[must_use]
    pub fn to_csv_row(&self) -> String {
        let mut s = String::new();
        let _ = write!(
            s,
            "{},{},{},{},{},{},{},{},{:.6},{:.6},{:.6},{:.6},{:.6},{:.4},{:.4},\
{:.6},{:.6},{:.6},{:.6},{},{},0x{:016x},{},{:.1},{:.1},{:.1},{},{}",
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

/// Build `m` packed matrices of order `n` over `F_q` from `sampler`.
fn generate(q: u64, n: usize, m: usize, sampler: &mut MatrixSampler) -> Batch {
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
    let batch = generate(q, n, m, sampler);
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

/// Project a cell's single-matrix cost from a measured probe at another `n` on
/// the same `(q, backend)`, using Ryser's exact `n * 2^n` work model.
///
/// Used only to decline a cell before paying for its probe. The projection and
/// its basis are recorded in the row's note, and it is labelled an estimate.
#[must_use]
pub fn project_probe(reference_n: usize, reference_probe_s: f64, target_n: usize) -> f64 {
    reference_probe_s * ryser_work(target_n) / ryser_work(reference_n)
}

/// The parallel width a backend applies to a batch: how many matrices are in
/// flight at once. One for a pinned single thread, the pool size for rayon
/// across matrices, and the compute-unit count for the GPU's one-block-per-
/// matrix kernels.
#[must_use]
pub fn parallel_width(backend: Backend) -> usize {
    match backend {
        Backend::Scalar | Backend::Avx2 => 1,
        Backend::Rayon | Backend::RayonAvx2 => rayon::current_num_threads(),
        // The intra-matrix path parallelises inside one matrix, so matrices are
        // still consumed one at a time.
        Backend::RayonIntra => 1,
        Backend::Gpu => GPU_COMPUTE_UNITS,
    }
}

/// Measured single-matrix probes, keyed by `(q, backend, n)`.
///
/// Serves two purposes: a cell whose exact `(q, backend, n)` was already probed
/// reuses that number instead of re-measuring it — which matters because the
/// two GPU batch-size variants of one `(q, n)` share a probe, and at
/// `q = 7, n = 28` that probe costs about 42 minutes — and a cell with no exact
/// match projects from the nearest measured `n` on the same `(q, backend)`.
pub type ProbeCache = std::collections::HashMap<(u64, &'static str, usize), f64>;

/// The measured probe at exactly `(q, backend, n)`, if one exists.
#[must_use]
pub fn exact_probe(probes: &ProbeCache, q: u64, backend: Backend, n: usize) -> Option<f64> {
    probes.get(&(q, backend.name(), n)).copied()
}

/// The measured probe on the same `(q, backend)` at the largest other `n`.
#[must_use]
pub fn projection_reference(
    probes: &ProbeCache,
    q: u64,
    backend: Backend,
    n: usize,
) -> Option<(usize, f64)> {
    probes
        .iter()
        .filter(|((pq, pb, pn), _)| *pq == q && *pb == backend.name() && *pn != n)
        .max_by_key(|((_, _, pn), _)| *pn)
        .map(|((_, _, pn), probe)| (*pn, *probe))
}

/// Run one cell of the grid end to end.
///
/// `sink` receives the shard records written during the timed `store` phase;
/// pass a handle to a scratch file so the measured cost is a real filesystem
/// write rather than a discard.
///
/// `probes` accumulates measured probe costs and is consulted to decline a cell
/// whose projected cost is hopeless before running its own probe.
pub fn run_cell(
    spec: &CellSpec,
    sink: &mut dyn std::io::Write,
    probes: &mut ProbeCache,
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

    // Decline before probing when a measured probe at another n on the same
    // (q, backend) already projects past the threshold. At q=7, n=28 on the GPU
    // the probe alone costs about 42 minutes, so paying it to learn what the
    // n=20 probe already implies is not affordable.
    if exact_probe(probes, q, backend, n).is_none() {
        if let Some((ref_n, ref_probe)) = projection_reference(probes, q, backend, n) {
            let projected = project_probe(ref_n, ref_probe, n);
            if projected > CENSOR_MATRIX_SECONDS {
                result.outcome = Outcome::Censored;
                result.note = format!(
                    "declined without probing: the measured probe at n={ref_n} ({ref_probe:.3} s) \
projects to {projected:.1} s at n={n} under Ryser's n*2^n work model, above the \
{CENSOR_MATRIX_SECONDS:.0} s threshold; projection is an ESTIMATE and the cell carries no rate"
                );
                return result;
            }
        }
    }

    // Probe: one matrix, to size the batch for the adaptive backends and to
    // decide whether the cell is affordable at its batch size. A probe already
    // measured for this exact (q, backend, n) is reused: the two GPU batch-size
    // variants of one (q, n) would otherwise pay for it twice.
    let per_matrix_s = match exact_probe(probes, q, backend, n) {
        Some(cached) => {
            result.note =
                "probe reused from an earlier cell at the same (q, n, backend)".to_string();
            cached
        }
        None => {
            let cal = Instant::now();
            let _ = one_rep(q, n, 1, backend, &mut sampler, stream, &mut devnull);
            let measured = cal.elapsed().as_secs_f64();
            probes.insert((q, backend.name(), n), measured);
            measured
        }
    };
    result.probe_matrix_s = per_matrix_s;

    if per_matrix_s > CENSOR_MATRIX_SECONDS {
        result.outcome = Outcome::Censored;
        result.batch_size = spec.batch_size.unwrap_or(0);
        result.note = format!(
            "probe of one matrix took {per_matrix_s:.1} s, above the \
{CENSOR_MATRIX_SECONDS:.0} s threshold; cell not attempted at its batch size and carries \
no rate (1/probe is not a bound: the GPU runs one block per matrix, so a batch of one \
occupies a single compute unit)"
        );
        return result;
    }

    // Decline a fixed-batch cell whose projected repetition would blow the
    // per-cell cap: the cap is only checked after a repetition completes, so a
    // single oversized repetition would otherwise run to the end regardless.
    if let Some(m) = spec.batch_size {
        let width = parallel_width(backend);
        let projected_rep_s = per_matrix_s * m as f64 / width as f64;
        if projected_rep_s > MAX_CELL_SECONDS {
            result.outcome = Outcome::Censored;
            result.batch_size = m;
            result.note = format!(
                "probe {per_matrix_s:.1} s projects a {projected_rep_s:.0} s repetition at \
M={m} over {width} concurrent units, above the {MAX_CELL_SECONDS:.0} s per-cell cap; \
cell not attempted and carries no rate (projection is an ESTIMATE)"
            );
            return result;
        }
    }

    let m = spec.batch_size.unwrap_or_else(|| {
        let target = (TARGET_REP_SECONDS / per_matrix_s.max(1e-9)).ceil() as usize;
        let floor = if backend.is_multithreaded() {
            rayon::current_num_threads()
        } else {
            1
        };
        target.clamp(floor, MAX_BATCH)
    });
    result.batch_size = m;

    // Warm-up: untimed, same work, at least WARMUP_SECONDS.
    let warm = Instant::now();
    loop {
        stream += 1;
        let mut s = MatrixSampler::new(seed_root, q, n, stream);
        let _ = one_rep(q, n, m, backend, &mut s, stream, &mut devnull);
        if warm.elapsed().as_secs_f64() >= WARMUP_SECONDS {
            break;
        }
    }

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
        let batch = generate(3, 22, rayon::current_num_threads(), &mut s);
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
}

/// CSV header for [`SustainedResult::to_csv_row`].
pub const SUSTAINED_CSV_HEADER: &str = "q,n,backend,batch_size,shards,matrices,zeros,wall_s,\
sustained_matrices_per_s,first_quarter_matrices_per_s,last_quarter_matrices_per_s,\
cpu_mhz_start,cpu_mhz_end,cpu_temp_start_c,cpu_temp_end_c,gpu_temp_end_c,seed_root";

impl SustainedResult {
    #[must_use]
    pub fn to_csv_row(&self) -> String {
        format!(
            "{},{},{},{},{},{},{},{:.3},{:.4},{:.4},{:.4},{:.1},{:.1},{:.1},{:.1},{:.1},0x{:016x}",
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
        )
    }
}

/// Run the composite hot path continuously for `seconds`.
pub fn run_sustained(
    q: u64,
    n: usize,
    backend: Backend,
    batch_size: usize,
    seconds: f64,
    seed_root: u64,
    sink: &mut dyn std::io::Write,
) -> SustainedResult {
    pin_thread(if backend.is_multithreaded() {
        None
    } else {
        Some(PINNED_CORE)
    });

    let start_thermal = ThermalSample::probe();
    let mut shard_times: Vec<f64> = Vec::new();
    let mut matrices = 0u64;
    let mut zeros = 0u64;
    let mut stream = 1_000_000u64;

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

    let quarter = (shard_times.len() / 4).max(1);
    let first: f64 = shard_times.iter().take(quarter).sum();
    let last: f64 = shard_times.iter().rev().take(quarter).sum();

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
        first_quarter_rate: (quarter * batch_size) as f64 / first,
        last_quarter_rate: (quarter * batch_size) as f64 / last,
        cpu_mhz_start: start_thermal.cpu_mhz_mean,
        cpu_mhz_end: end_thermal.cpu_mhz_mean,
        cpu_temp_start_c: start_thermal.cpu_temp_c,
        cpu_temp_end_c: end_thermal.cpu_temp_c,
        gpu_temp_end_c: end_thermal.gpu_temp_c,
        seed_root,
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
