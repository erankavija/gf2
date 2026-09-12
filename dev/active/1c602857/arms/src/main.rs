//! Arm executable for the public wide carry-less product receipt
//! (jit:1c602857).
//!
//! One executable serves both arms of every cell; the `GF2_CLMUL_PATH`
//! environment variable selects which entry point it times, so the two arms
//! share one build, one fixture generator, one bank rotation and one timing
//! loop and differ only in the function under measurement.
//!
//! # Arms
//!
//! * `portable` — `clmul_wide_slice_portable`, the bit-by-bit schoolbook that
//!   reaches no capability dispatch. For the owned form it is the pre-change
//!   public path word for word: a zeroed `2N`-word array followed by the
//!   accumulating schoolbook, which is what `clmul_wide` executed before this
//!   issue routed it.
//! * `public` — `clmul_wide` and `clmul_wide_slice`, the public long-product
//!   API as this issue leaves it.
//!
//! # Forms
//!
//! [`Case::ClmulWide`] times the owned form, which writes a fresh `2N`-word
//! product. [`Case::ClmulWideAccumulate`] times the slice form, which XORs the
//! product into a live destination; on a dispatched width that form pays a
//! scratch product and one XOR pass, which the receipt separates from the
//! owned form rather than averaging away.
//!
//! Widths 4 and 9 have kernels; every other width runs the portable schoolbook
//! on both arms, so those cells measure the dispatch decision alone.
//!
//! Field reduction is outside every timed window: this is the unreduced
//! product, the operation the public API names.

use gf2_core::gf2m::wide::{clmul_wide, clmul_wide_slice, clmul_wide_slice_portable};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::hint::black_box;
use std::io;
use std::time::{Duration, Instant};
use tuning_campaign_support::abtest::SplitMix64;
use tuning_campaign_support::host::CpuAffinity;
use tuning_campaign_support::timing::{execution_windows_configured, TimingSample, FIXTURE_BANKS};
use tuning_campaign_support::transport;

/// The entry point an arm times.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ArmPath {
    /// `clmul_wide_slice_portable`: the schoolbook, whatever the host offers.
    Portable,
    /// `clmul_wide` / `clmul_wide_slice`: the public long-product API.
    Public,
}

impl ArmPath {
    fn from_env() -> Self {
        match std::env::var("GF2_CLMUL_PATH").as_deref() {
            Ok("portable") => ArmPath::Portable,
            Ok("public") => ArmPath::Public,
            other => fail(format!(
                "GF2_CLMUL_PATH must be `portable` or `public`, got {other:?}"
            )),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            ArmPath::Portable => "portable",
            ArmPath::Public => "public",
        }
    }
}

/// One measured workload, forwarded verbatim by the runner to both arms.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, tag = "kind", rename_all = "kebab-case")]
enum Case {
    /// Owned unreduced product of two `words`-word operands.
    ClmulWide {
        words: usize,
        inner: usize,
        seed: u64,
    },
    /// Unreduced product XOR-accumulated into a live `2 * words`-word buffer.
    ClmulWideAccumulate {
        words: usize,
        inner: usize,
        seed: u64,
    },
}

impl Case {
    fn words(&self) -> usize {
        match *self {
            Case::ClmulWide { words, .. } | Case::ClmulWideAccumulate { words, .. } => words,
        }
    }

    fn inner(&self) -> usize {
        match *self {
            Case::ClmulWide { inner, .. } | Case::ClmulWideAccumulate { inner, .. } => inner.max(1),
        }
    }

    fn seed(&self) -> u64 {
        match *self {
            Case::ClmulWide { seed, .. } | Case::ClmulWideAccumulate { seed, .. } => seed,
        }
    }

    fn accumulates(&self) -> bool {
        matches!(*self, Case::ClmulWideAccumulate { .. })
    }
}

/// Operand and destination buffers of one fixture bank.
struct Bank<const N: usize, const M: usize> {
    a: [u64; N],
    b: [u64; N],
    out: [u64; M],
}

/// Generates the fixture banks.
///
/// The stream is a pure function of the case seed and the bank index, so both
/// arms of a pair see byte-identical operands and a resumed session
/// reproduces them.
fn banks<const N: usize, const M: usize>(seed: u64, count: usize) -> Vec<Bank<N, M>> {
    (0..count)
        .map(|bank| {
            let mut mixer =
                SplitMix64::new(seed ^ (bank as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9));
            let mut a = [0u64; N];
            let mut b = [0u64; N];
            for word in a.iter_mut().chain(b.iter_mut()) {
                *word = mixer.next_u64();
            }
            Bank {
                a,
                b,
                out: [0u64; M],
            }
        })
        .collect()
}

/// Request the runner writes on this child's stdin.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Request {
    schema: String,
    cell_id: String,
    arm: String,
    /// `baseline` or `candidate`; the arm measures the same way either way.
    role: String,
    pair: u32,
    case: Value,
    cache_state: String,
    /// Absent for every cell this family declares; the field exists so the
    /// canonical round trip matches the runner's own request type, which skips
    /// it when it is absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cold_calls: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    decoder: Option<Value>,
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

#[derive(Serialize)]
struct Conversion {
    setup_ns: u64,
    pack_ns: u64,
    unpack_ns: u64,
    batch_fill_ns: u64,
    dispatch_ns: u64,
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
    /// Every execution calibrates its call count; the family declares no
    /// fixed-call cold cell.
    calibrated: bool,
}

fn fail(message: impl std::fmt::Display) -> ! {
    eprintln!("clmul-wide-arm: {message}");
    std::process::exit(2);
}

/// Runtime-observed kernel lane of width `N`, or the portable name when the
/// width has no kernel or the arm bypasses dispatch.
fn lane(words: usize, path: ArmPath) -> &'static str {
    if path == ArmPath::Portable {
        return "portable-scalar";
    }
    match words {
        4 => gf2_kernels_simd::gf2m_wide::detect()
            .map(|fns| fns.name)
            .unwrap_or("portable-scalar"),
        9 => gf2_kernels_simd::gf2m_wide::detect_571()
            .map(|fns| fns.name)
            .unwrap_or("portable-scalar"),
        _ => "portable-scalar",
    }
}

/// Repetitions the dispatch probe averages over.
///
/// One capability lookup costs a few nanoseconds, at or below the resolution
/// of a single `Instant` interval.
const DISPATCH_PROBE_REPEATS: u64 = 4096;

/// Mean nanoseconds of one runtime capability lookup, rounded up.
///
/// The public path pays this lookup inside every product on a width that has
/// a kernel; the portable path pays nothing, and reports zero.
fn dispatch_probe_ns(words: usize, path: ArmPath) -> u64 {
    if path == ArmPath::Portable {
        return 0;
    }
    let start = Instant::now();
    for _ in 0..DISPATCH_PROBE_REPEATS {
        match words {
            4 => {
                black_box(gf2_kernels_simd::gf2m_wide::detect());
            }
            9 => {
                black_box(gf2_kernels_simd::gf2m_wide::detect_571());
            }
            _ => {
                black_box(words);
            }
        }
    }
    (start.elapsed().as_nanos() as u64).div_ceil(DISPATCH_PROBE_REPEATS)
}

/// Measures one cell at width `N`, with `M == 2 * N`.
fn measure<const N: usize, const M: usize>(
    request: &Request,
    case: &Case,
    path: ArmPath,
) -> io::Result<Vec<TimingSample>> {
    assert_eq!(M, 2 * N, "the arm's width table is inconsistent");
    let bank_count = if request.cache_state == "streaming" {
        FIXTURE_BANKS
    } else {
        1
    };
    let mut fixtures = banks::<N, M>(case.seed(), bank_count);
    let inner = case.inner();
    let accumulates = case.accumulates();

    if request.cache_state == "warm" {
        let slot = &mut fixtures[0];
        black_box(clmul_wide::<N, M>(&slot.a, &slot.b));
        clmul_wide_slice_portable::<N>(&slot.a, &slot.b, &mut slot.out);
        slot.out = [0u64; M];
    }

    let mut body = |index: usize| {
        let slot = &mut fixtures[index % bank_count];
        match (accumulates, path) {
            (false, ArmPath::Public) => {
                for _ in 0..inner {
                    black_box(clmul_wide::<N, M>(black_box(&slot.a), black_box(&slot.b)));
                }
            }
            (false, ArmPath::Portable) => {
                for _ in 0..inner {
                    let mut product = [0u64; M];
                    clmul_wide_slice_portable::<N>(
                        black_box(&slot.a),
                        black_box(&slot.b),
                        &mut product,
                    );
                    black_box(product);
                }
            }
            (true, ArmPath::Public) => {
                for _ in 0..inner {
                    clmul_wide_slice::<N>(
                        black_box(&slot.a),
                        black_box(&slot.b),
                        black_box(&mut slot.out[..]),
                    );
                }
            }
            (true, ArmPath::Portable) => {
                for _ in 0..inner {
                    clmul_wide_slice_portable::<N>(
                        black_box(&slot.a),
                        black_box(&slot.b),
                        black_box(&mut slot.out[..]),
                    );
                }
            }
        }
    };

    execution_windows_configured(
        0,
        u64::from(request.windows),
        Duration::from_millis(u64::from(request.window_target_ms)),
        &mut body,
        |_| Ok(()),
    )
}

fn main() {
    let sentinel = std::env::var(transport::FRESH_CASE_VAR).ok();
    let request: Request =
        match transport::read_guarded_case(sentinel.as_deref(), io::stdin().lock()) {
            Ok(request) => request,
            Err(error) => fail(error),
        };
    let case: Case = match serde_json::from_value(request.case.clone()) {
        Ok(case) => case,
        Err(error) => fail(format!("case does not decode: {error}")),
    };
    if request.cold_calls.is_some() {
        fail("this family declares no fixed-call cold cell");
    }
    if request.decoder.is_some() {
        fail("this family declares no decoder cell");
    }
    if request.workers_declared != 1 {
        fail(format!(
            "this family declares single-core cells only, got {} workers",
            request.workers_declared
        ));
    }
    let path = ArmPath::from_env();

    // The dispatch probe runs before the timed windows, so its cost never
    // enters a measured window.
    let conversion = Conversion {
        setup_ns: 0,
        pack_ns: 0,
        unpack_ns: 0,
        batch_fill_ns: 0,
        dispatch_ns: dispatch_probe_ns(case.words(), path),
    };

    let samples = match case.words() {
        1 => measure::<1, 2>(&request, &case, path),
        2 => measure::<2, 4>(&request, &case, path),
        4 => measure::<4, 8>(&request, &case, path),
        9 => measure::<9, 18>(&request, &case, path),
        16 => measure::<16, 32>(&request, &case, path),
        other => fail(format!("no width table entry for {other} words")),
    };
    let samples = match samples {
        Ok(samples) => samples,
        Err(error) => fail(format!("timing failed: {error}")),
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
        workers_observed: 1,
        cpus_observed,
        selected_path: Some(format!(
            "{}:{}",
            path.as_str(),
            lane(case.words(), path)
        )),
        conversion: Some(conversion),
        quality: None,
        calibrated: true,
    };
    if let Err(error) = transport::write_result_line(io::stdout().lock(), &result) {
        fail(error);
    }
}
