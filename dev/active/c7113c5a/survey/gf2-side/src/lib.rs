//! Shared arm mechanics for the polynomial-multiplication baseline survey
//! (jit:c7113c5a).
//!
//! Both arms of every cell run the identical fixture generation, bank
//! rotation, worker fan-out and conversion accounting from this module and
//! differ only in the [`Backend`] that performs the arithmetic, so a measured
//! ratio attributes to the kernel rather than to the harness. The child-v2
//! framing and the window protocol come from `tuning-campaign-support`; this
//! module adds no parallel timing or transport mechanism.
//!
//! # Operations
//!
//! [`Case::PolyMul`] is the unreduced product of two `words`-word polynomials
//! over GF(2), written into a `2 * words`-word destination. [`Case::ClmulBatch`]
//! is `count` independent unreduced 64x64 -> 128 products. [`Case::Gf2mDot`] is
//! the whole-consumer GF(2^8) dot product: `count` unreduced products, XOR
//! accumulation, and one Barrett reduction. Every operand and product uses the
//! canonical little-endian bit numbering, so bit `i` of word `j` is the
//! coefficient of `x^(64j + i)`.

use gf2_core::gf2m::barrett::BarrettReducer;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Barrier;
use std::time::{Duration, Instant};
use tuning_campaign_support::abtest::SplitMix64;
use tuning_campaign_support::host::CpuAffinity;
use tuning_campaign_support::timing::{execution_windows_configured, FIXTURE_BANKS};
use tuning_campaign_support::transport;

/// Field degree of the whole-consumer GF(2^m) dot-product cell.
pub const DOT_FIELD_DEGREE: usize = 8;
/// Defining polynomial of that field: `x^8 + x^4 + x^3 + x + 1`.
pub const DOT_FIELD_POLY: u64 = 0x11B;

/// One measured workload, forwarded verbatim by the runner to both arms.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, tag = "kind", rename_all = "kebab-case")]
pub enum Case {
    /// Unreduced product of two `words`-word GF(2) polynomials.
    PolyMul {
        words: usize,
        inner: usize,
        seed: u64,
    },
    /// `count` independent unreduced 64x64 -> 128 carry-less products.
    ClmulBatch {
        count: usize,
        inner: usize,
        seed: u64,
    },
    /// GF(2^8) dot product of two `count`-element vectors.
    Gf2mDot {
        count: usize,
        inner: usize,
        seed: u64,
    },
}

impl Case {
    /// Repetitions each worker performs inside one logical call.
    pub fn inner(&self) -> usize {
        match *self {
            Case::PolyMul { inner, .. }
            | Case::ClmulBatch { inner, .. }
            | Case::Gf2mDot { inner, .. } => inner.max(1),
        }
    }

    /// Seed of the fixture generator.
    pub fn seed(&self) -> u64 {
        match *self {
            Case::PolyMul { seed, .. }
            | Case::ClmulBatch { seed, .. }
            | Case::Gf2mDot { seed, .. } => seed,
        }
    }

    /// Words in one operand.
    pub fn operand_words(&self) -> usize {
        match *self {
            Case::PolyMul { words, .. } => words,
            Case::ClmulBatch { count, .. } => count,
            Case::Gf2mDot { count, .. } => count,
        }
    }

    /// Words in the destination one call writes.
    pub fn product_words(&self) -> usize {
        match *self {
            Case::PolyMul { words, .. } => 2 * words,
            Case::ClmulBatch { count, .. } => 2 * count,
            Case::Gf2mDot { .. } => 1,
        }
    }
}

/// Operand and destination buffers of one fixture bank.
pub struct Bank {
    pub a: Vec<u64>,
    pub b: Vec<u64>,
    pub out: Vec<u64>,
}

/// Generates the fixture banks of one worker.
///
/// The generator stream is a pure function of the case seed and the worker
/// index, so both arms of a pair see byte-identical operands and a resumed
/// session reproduces them.
pub fn banks(case: &Case, worker: usize, count: usize) -> Vec<Bank> {
    let operand = case.operand_words();
    let product = case.product_words();
    (0..count)
        .map(|bank| {
            let mut mixer = SplitMix64::new(
                case.seed()
                    ^ (worker as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
                    ^ (bank as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9),
            );
            Bank {
                a: (0..operand).map(|_| mixer.next_u64()).collect(),
                b: (0..operand).map(|_| mixer.next_u64()).collect(),
                out: vec![0; product],
            }
        })
        .collect()
}

/// Costs an arm reports outside its timed windows.
///
/// The protocol's five fields carry these meanings in this survey; a field an
/// arm does not probe for a cell is zero:
///
/// - `setup_ns`: one-time state the arm builds before its first call: gf2x's
///   pool handle (gf2x allocates the scratch itself inside a multiplication),
///   the gf2 field of the dot-product cell and the gf2 raw-batch product
///   buffer. The gf2 long-product arm builds none.
/// - `pack_ns`: converting the raw fixture words into the arm's own operand
///   representation, in the dot-product cell.
/// - `batch_fill_ns`: one complete call of the dot-product cell's operation.
/// - `dispatch_ns`: the runtime capability detection the gf2 arm performs;
///   gf2x fixes its code at build time.
/// - `unpack_ns`: for the 4-word and 9-word long products, the
///   [`wide_field::WideReducer`] reduction to GF(2^256) or GF(2^571) that
///   `Gf2mWide::mul_ref` applies, run on the arm's own unreduced product; for
///   the dot-product cell, the GF(2^8) Barrett reduction of the raw XOR
///   accumulator with the carry-less multiply the arm's own call passes (see
///   [`dot_reduction_total_ns`]); for the gf2 raw-batch cell, the copy of the
///   batch kernel's 128-bit products into the canonical two-word layout, which
///   its timed call also performs.
///
/// The two reductions are averaged over [`AMORTISED_PROBE_REPEATS`]
/// repetitions and rounded up, because a single reduction is at or below timer
/// resolution. Every other probe is one pass, taken in a fresh process before
/// the first timed window; all but the raw-batch copy, which follows one call,
/// include first-touch and cold-code effects.
#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct Conversion {
    pub setup_ns: u64,
    pub pack_ns: u64,
    pub unpack_ns: u64,
    pub batch_fill_ns: u64,
    pub dispatch_ns: u64,
}

/// One arm's arithmetic.
///
/// An instance is created per worker thread and owns whatever mutable scratch
/// its library needs, so no state is shared across workers.
pub trait Backend: Sized + Send {
    /// Runtime-observed identity of the code path this instance selected.
    fn selected_path(&self) -> String;

    /// Creates a worker-local instance for one case.
    fn create(case: &Case) -> Self;

    /// Performs one operation of the case on one bank.
    fn run(&mut self, case: &Case, bank: &mut Bank);

    /// Setup, packing, batch-fill, dispatch and unpacking costs this arm pays
    /// around the arithmetic, timed outside the measured windows.
    fn conversion(&mut self, case: &Case, bank: &mut Bank) -> Conversion;
}

/// Request the runner writes on this child's stdin.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Request {
    schema: String,
    cell_id: String,
    arm: String,
    role: String,
    pair: u32,
    case: Value,
    cache_state: String,
    windows: u32,
    window_target_ms: u32,
    cpus: Vec<u32>,
    workers_declared: u32,
}

#[derive(Serialize)]
struct Window {
    calls: u64,
    elapsed_ns: u64,
}

/// The one canonical result line this child writes.
#[derive(Serialize)]
struct ArmResult {
    schema: String,
    windows: Vec<Window>,
    cache_state_applied: String,
    workers_observed: u32,
    cpus_observed: Vec<u32>,
    selected_path: Option<String>,
    conversion: Option<Conversion>,
    quality: Option<Value>,
    /// Every execution calibrates its call count; the survey declares no
    /// fixed-call cold cell, and a request carrying `cold_calls` fails to
    /// decode rather than being measured under the wrong policy.
    calibrated: bool,
}

fn fail(message: impl std::fmt::Display) -> ! {
    eprintln!("poly arm: {message}");
    std::process::exit(2);
}

/// Runs one arm execution end to end under the canonical child-v2 contract.
///
/// Reads the guarded request, applies the declared cache-state policy, spawns
/// the declared worker count, measures the protocol's windows, and writes the
/// single result line. `label` names the arm in diagnostics.
pub fn run_arm<B: Backend + 'static>(label: &str) {
    let sentinel = std::env::var(transport::FRESH_CASE_VAR).ok();
    let request: Request =
        match transport::read_guarded_case(sentinel.as_deref(), io::stdin().lock()) {
            Ok(request) => request,
            Err(error) => fail(format!("{label}: {error}")),
        };
    let case: Case = match serde_json::from_value(request.case.clone()) {
        Ok(case) => case,
        Err(error) => fail(format!("{label}: case does not decode: {error}")),
    };
    let workers = request.workers_declared.max(1) as usize;
    let bank_count = if request.cache_state == "streaming" {
        FIXTURE_BANKS
    } else {
        1
    };

    // Conversion accounting runs on a private worker before the timed windows
    // so its cost never enters a measured window.
    let mut probe = B::create(&case);
    let mut probe_banks = banks(&case, usize::MAX, 1);
    let conversion = probe.conversion(&case, &mut probe_banks[0]);
    let selected_path = probe.selected_path();
    drop(probe);
    drop(probe_banks);

    let windows = u64::from(request.windows);
    let target = Duration::from_millis(u64::from(request.window_target_ms));
    let inner = case.inner();
    let warm = request.cache_state == "warm";

    let samples = if workers == 1 {
        let mut backend = B::create(&case);
        let mut worker_banks = banks(&case, 0, bank_count);
        if warm {
            backend.run(&case, &mut worker_banks[0]);
        }
        let mut body = |bank: usize| {
            let slot = &mut worker_banks[bank % bank_count];
            for _ in 0..inner {
                backend.run(&case, slot);
            }
        };
        execution_windows_configured(0, windows, target, &mut body, |_| Ok(()))
    } else {
        measure_multicore::<B>(&case, workers, bank_count, warm, windows, target, inner)
    };
    let samples = match samples {
        Ok(samples) => samples,
        Err(error) => fail(format!("{label}: timing failed: {error}")),
    };

    let cpus_observed = CpuAffinity::observe()
        .map(|affinity| affinity.cpus().to_vec())
        .unwrap_or_default();
    let result = ArmResult {
        schema: "zen3-benchmark-arm-result-v1".into(),
        windows: samples
            .iter()
            .map(|sample| Window {
                calls: sample.calls,
                elapsed_ns: sample.elapsed_ns,
            })
            .collect(),
        cache_state_applied: request.cache_state.clone(),
        workers_observed: workers as u32,
        cpus_observed,
        selected_path: Some(selected_path),
        conversion: Some(conversion),
        quality: None,
        calibrated: true,
    };
    if let Err(error) = transport::write_result_line(io::stdout().lock(), &result) {
        fail(format!("{label}: {error}"));
    }
}

/// Measures a multicore cell where one logical call is `inner` operations on
/// each of `workers` threads.
///
/// Persistent threads meet at a barrier, so a call costs one barrier round trip
/// rather than a thread spawn, and both arms pay the identical synchronisation.
#[allow(clippy::too_many_arguments)]
fn measure_multicore<B: Backend + 'static>(
    case: &Case,
    workers: usize,
    bank_count: usize,
    warm: bool,
    windows: u64,
    target: Duration,
    inner: usize,
) -> io::Result<Vec<tuning_campaign_support::timing::TimingSample>> {
    let ready = Barrier::new(workers + 1);
    let done = Barrier::new(workers + 1);
    let stop = AtomicBool::new(false);
    let bank = AtomicUsize::new(0);
    let mut states: Vec<(B, Vec<Bank>)> = (0..workers)
        .map(|worker| (B::create(case), banks(case, worker, bank_count)))
        .collect();

    std::thread::scope(|scope| {
        for (mut backend, mut worker_banks) in states.drain(..) {
            let ready = &ready;
            let done = &done;
            let stop = &stop;
            let bank = &bank;
            scope.spawn(move || {
                if warm {
                    backend.run(case, &mut worker_banks[0]);
                }
                loop {
                    ready.wait();
                    if stop.load(Ordering::Acquire) {
                        return;
                    }
                    let slot = &mut worker_banks[bank.load(Ordering::Acquire) % bank_count];
                    for _ in 0..inner {
                        backend.run(case, slot);
                    }
                    done.wait();
                }
            });
        }
        let mut body = |index: usize| {
            bank.store(index, Ordering::Release);
            ready.wait();
            done.wait();
        };
        let samples = execution_windows_configured(0, windows, target, &mut body, |_| Ok(()));
        stop.store(true, Ordering::Release);
        ready.wait();
        samples
    })
}

/// Times one closure once and returns its wall duration in nanoseconds.
pub fn probe_ns(mut body: impl FnMut()) -> u64 {
    let start = Instant::now();
    body();
    start.elapsed().as_nanos() as u64
}

/// Repetitions the amortised probe averages over.
///
/// One GF(2^m) Barrett reduction costs a few nanoseconds, which is at or below
/// the resolution of a single `Instant` interval, so the reduction probe reports
/// the mean over this many repetitions instead of one unresolvable reading.
pub const AMORTISED_PROBE_REPEATS: u64 = 4096;

/// Total wall nanoseconds of [`AMORTISED_PROBE_REPEATS`] runs of `body`.
pub fn amortised_probe_total_ns(mut body: impl FnMut()) -> u64 {
    let start = Instant::now();
    for _ in 0..AMORTISED_PROBE_REPEATS {
        body();
    }
    start.elapsed().as_nanos() as u64
}

/// Mean nanoseconds of one repetition of `body` over
/// [`AMORTISED_PROBE_REPEATS`] runs, rounded up so the whole-nanosecond figure
/// bounds the mean from above.
pub fn amortised_probe_ns(body: impl FnMut()) -> u64 {
    amortised_probe_total_ns(body).div_ceil(AMORTISED_PROBE_REPEATS)
}

/// The Barrett reducer `Gf2mField::new(DOT_FIELD_DEGREE, DOT_FIELD_POLY)`
/// builds for the dot-product cell's field, which both arms reduce with.
pub fn dot_reducer() -> BarrettReducer {
    BarrettReducer::new(u128::from(DOT_FIELD_POLY), DOT_FIELD_DEGREE as u32)
}

/// Total nanoseconds of [`AMORTISED_PROBE_REPEATS`] reductions of
/// `accumulator` by [`dot_reducer`] with the carry-less multiply `clmul`.
///
/// `accumulator` is the raw XOR of the cell's unreduced 128-bit products, the
/// value the dot product's single reduction receives. `reduce_with_clmul`
/// returns an input below `x^8` unchanged, so feeding it the reduced dot
/// product instead times only that early exit.
pub fn dot_reduction_total_ns(accumulator: u128, clmul: fn(u64, u64) -> u128) -> u64 {
    let reducer = dot_reducer();
    amortised_probe_total_ns(|| {
        std::hint::black_box(reducer.reduce_with_clmul(std::hint::black_box(accumulator), clmul));
    })
}

/// [`dot_reduction_total_ns`] per reduction, rounded up to whole nanoseconds.
pub fn dot_reduction_probe_ns(accumulator: u128, clmul: fn(u64, u64) -> u128) -> u64 {
    dot_reduction_total_ns(accumulator, clmul).div_ceil(AMORTISED_PROBE_REPEATS)
}

pub mod gf2_backend;
pub mod wide_field;
