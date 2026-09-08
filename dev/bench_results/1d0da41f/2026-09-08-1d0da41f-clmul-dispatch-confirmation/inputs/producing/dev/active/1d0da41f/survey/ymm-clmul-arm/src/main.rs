//! Benchmark arm for the YMM raw carry-less batch dispatch repair (jit:1d0da41f).
//!
//! The binary implements the canonical child-v2 request/result contract of
//! `tuning-campaign-support`: one request on stdin, one result line on stdout.
//! Two workloads are available.
//!
//! - `raw-batch` times one `clmul_batch` call over `elements` operand pairs
//!   through the published `Gf2mFns::clmul_batch_fn` pointer, so the kernel's
//!   own per-call dispatch check is inside the measurement. The metric kind is
//!   kernel-isolated.
//! - `fieldvec-dot` times one `FieldVec::<Gf2mElement>::simd_dot_product` call,
//!   the production consumer of that kernel. Packing into `u64` scratch, batch
//!   dispatch, XOR accumulation and the single Barrett reduction are all inside
//!   the measurement. The metric kind is whole-consumer.
//!
//! Baseline and candidate arms are the same source built against two different
//! source trees: the pre-change snapshot and the repaired worktree. They differ
//! only in the crates they link, never in this file's timing behaviour.

#[cfg(not(target_arch = "x86_64"))]
compile_error!("this arm measures an x86_64 raw carry-less batch dispatch decision");

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::hint::black_box;
use std::io;
use std::time::{Duration, Instant};

use gf2_core::field::FieldVec;
use gf2_core::gf2m::{Gf2mElement, Gf2mField};
use gf2_core::primitive_polys::PrimitivePolynomialDatabase;
use tuning_campaign_support::abtest::SplitMix64;
use tuning_campaign_support::host::CpuAffinity;
use tuning_campaign_support::timing::{execution_windows_configured, FIXTURE_BANKS};
use tuning_campaign_support::transport;

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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// `raw-batch` or `fieldvec-dot`.
    workload: String,
    /// Operand pairs per call.
    elements: usize,
    /// Operand width in bits: the GF(2^m) degree for `fieldvec-dot`, and the
    /// bit width the raw operands are masked to for `raw-batch`.
    degree: usize,
    /// Operand generator seed.
    seed: u64,
}

#[derive(Serialize)]
struct Window {
    calls: u64,
    elapsed_ns: u64,
}

/// Setup and conversion costs, in the field order the campaign result schema
/// declares. The runner re-encodes the result line and rejects any other
/// ordering as noncanonical.
#[derive(Serialize)]
struct Conversion {
    setup_ns: u64,
    pack_ns: u64,
    unpack_ns: u64,
    batch_fill_ns: u64,
    dispatch_ns: u64,
}

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
}

/// Reports the raw-batch lane this build selects at run time.
///
/// The repaired tree publishes the lane tag its own dispatch predicate
/// chose, so the arm reports the kernel's decision rather than restating it.
#[cfg(not(feature = "frozen-baseline"))]
fn selected_path() -> Option<String> {
    gf2_kernels_simd::gf2m::detect()
        .and_then(|fns| fns.clmul_batch_path)
        .map(str::to_owned)
}

/// Reports the raw-batch lane the pre-change build selects at run time.
///
/// The pre-change tree publishes no lane tag, so this arm evaluates the
/// frozen pre-change dispatch expression, `vpclmulqdq && avx512vl`, against
/// the same `std::arch` feature detection the pre-change kernel consults. The
/// expression is a copy of frozen code that cannot change, and the values it
/// reads are observed on this host at run time.
#[cfg(feature = "frozen-baseline")]
fn selected_path() -> Option<String> {
    use std::arch::is_x86_feature_detected;

    gf2_kernels_simd::gf2m::detect()?;
    Some(
        if is_x86_feature_detected!("vpclmulqdq") && is_x86_feature_detected!("avx512vl") {
            "avx2+vpclmulqdq-ymm".to_owned()
        } else {
            "pclmulqdq-scalar-xmm".to_owned()
        },
    )
}

/// Masks a generated word to `degree` bits; `degree == 64` leaves it whole.
fn mask_for(degree: usize) -> u64 {
    if degree >= 64 {
        u64::MAX
    } else {
        (1u64 << degree) - 1
    }
}

fn operands(case: &Case, bank: u64) -> (Vec<u64>, Vec<u64>) {
    let mask = mask_for(case.degree);
    let mut mixer = SplitMix64::new(case.seed ^ bank.wrapping_mul(0x9E37_79B9_7F4A_7C15));
    let a = (0..case.elements)
        .map(|_| mixer.next_u64() & mask)
        .collect();
    let b = (0..case.elements)
        .map(|_| mixer.next_u64() & mask)
        .collect();
    (a, b)
}

fn banks_for(cache_state: &str) -> usize {
    if cache_state == "streaming" {
        FIXTURE_BANKS
    } else {
        1
    }
}

/// Times one closure once and returns whole nanoseconds.
fn time_once(mut body: impl FnMut()) -> u64 {
    let start = Instant::now();
    body();
    u64::try_from(start.elapsed().as_nanos()).unwrap_or(u64::MAX)
}

/// Measures the per-call dispatch overhead of the published batch pointer.
///
/// A zero-length batch performs the kernel's runtime lane check and nothing
/// else, so the mean over many calls isolates dispatch from arithmetic.
fn dispatch_ns() -> u64 {
    const CALLS: u64 = 100_000;
    let Some(batch) = gf2_kernels_simd::gf2m::detect().and_then(|fns| fns.clmul_batch_fn) else {
        return 0;
    };
    let a: [u64; 0] = [];
    let b: [u64; 0] = [];
    let mut out: [u128; 0] = [];
    let start = Instant::now();
    for _ in 0..CALLS {
        batch(black_box(&a), black_box(&b), black_box(&mut out));
    }
    let elapsed = u64::try_from(start.elapsed().as_nanos()).unwrap_or(u64::MAX);
    elapsed / CALLS
}

struct Measured {
    windows: Vec<Window>,
    conversion: Conversion,
}

fn run_raw_batch(request: &Request, case: &Case) -> Result<Measured, String> {
    let banks = banks_for(&request.cache_state);
    let setup_ns = time_once(|| {
        let _ = operands(case, 0);
    });
    let fixtures: Vec<(Vec<u64>, Vec<u64>)> =
        (0..banks).map(|bank| operands(case, bank as u64)).collect();
    let mut outputs: Vec<Vec<u128>> = (0..banks).map(|_| vec![0u128; case.elements]).collect();
    let batch = gf2_kernels_simd::gf2m::detect()
        .and_then(|fns| fns.clmul_batch_fn)
        .ok_or("this host publishes no raw carry-less batch kernel")?;

    if request.cache_state == "warm" {
        let (a, b) = &fixtures[0];
        batch(black_box(a), black_box(b), black_box(&mut outputs[0]));
    }
    let mut body = |bank: usize| {
        let index = bank % banks;
        let (a, b) = &fixtures[index];
        batch(black_box(a), black_box(b), black_box(&mut outputs[index]));
    };
    let samples = execution_windows_configured(
        0,
        u64::from(request.windows),
        Duration::from_millis(u64::from(request.window_target_ms)),
        &mut body,
        |_| Ok(()),
    )
    .map_err(|error| format!("timing failed: {error}"))?;
    black_box(&outputs);

    Ok(Measured {
        windows: samples
            .iter()
            .map(|sample| Window {
                calls: sample.calls,
                elapsed_ns: sample.elapsed_ns,
            })
            .collect(),
        conversion: Conversion {
            setup_ns,
            // A raw batch consumes `u64` operands directly: it neither packs
            // nor unpacks a representation, and it fills no batch scratch.
            pack_ns: 0,
            unpack_ns: 0,
            batch_fill_ns: 0,
            dispatch_ns: dispatch_ns(),
        },
    })
}

fn field_for(degree: usize) -> Result<Gf2mField, String> {
    let poly = PrimitivePolynomialDatabase::standard(degree)
        .ok_or_else(|| format!("no standard primitive polynomial for degree {degree}"))?;
    Ok(Gf2mField::new(degree, poly))
}

fn run_fieldvec_dot(request: &Request, case: &Case) -> Result<Measured, String> {
    let banks = banks_for(&request.cache_state);
    let field = field_for(case.degree)?;
    let mut vectors: Vec<(FieldVec<Gf2mElement>, FieldVec<Gf2mElement>)> = Vec::new();
    let setup_ns = time_once(|| {
        vectors = (0..banks)
            .map(|bank| {
                let (a, b) = operands(case, bank as u64);
                let a: FieldVec<Gf2mElement> =
                    a.into_iter().map(|value| field.element(value)).collect();
                let b: FieldVec<Gf2mElement> =
                    b.into_iter().map(|value| field.element(value)).collect();
                (a, b)
            })
            .collect();
    });

    // The consumer's representation conversion: one pass extracting field
    // element values into `u64` scratch, the loop `simd_dot_product` runs
    // before each batch call.
    let mut scratch_a = vec![0u64; case.elements];
    let mut scratch_b = vec![0u64; case.elements];
    let pack_ns = time_once(|| {
        let (a, b) = &vectors[0];
        for i in 0..case.elements {
            scratch_a[i] = a[i].value();
            scratch_b[i] = b[i].value();
        }
    });
    black_box(&scratch_a);
    black_box(&scratch_b);

    // The consumer's output conversion: one Barrett reduction of the
    // accumulated 128-bit product into a field element.
    let mut sink = field.element(0);
    let unpack_ns = time_once(|| {
        let (a, b) = &vectors[0];
        sink = a.simd_dot_product(b);
    });

    if request.cache_state == "warm" {
        let (a, b) = &vectors[0];
        sink = a.simd_dot_product(b);
    }
    let mut accumulator = 0u64;
    let mut body = |bank: usize| {
        let (a, b) = &vectors[bank % banks];
        accumulator ^= black_box(a).simd_dot_product(black_box(b)).value();
    };
    let samples = execution_windows_configured(
        0,
        u64::from(request.windows),
        Duration::from_millis(u64::from(request.window_target_ms)),
        &mut body,
        |_| Ok(()),
    )
    .map_err(|error| format!("timing failed: {error}"))?;
    black_box(accumulator);
    black_box(sink.value());

    Ok(Measured {
        windows: samples
            .iter()
            .map(|sample| Window {
                calls: sample.calls,
                elapsed_ns: sample.elapsed_ns,
            })
            .collect(),
        conversion: Conversion {
            setup_ns,
            pack_ns,
            unpack_ns,
            // The consumer fills its chunk scratch in the same loop that
            // packs, so `pack_ns` carries that cost and no separate batch
            // fill exists to measure.
            batch_fill_ns: 0,
            dispatch_ns: dispatch_ns(),
        },
    })
}

fn main() {
    let sentinel = std::env::var(transport::FRESH_CASE_VAR).ok();
    let request: Request =
        match transport::read_guarded_case(sentinel.as_deref(), io::stdin().lock()) {
            Ok(request) => request,
            Err(error) => {
                eprintln!("ymm-clmul-arm: {error}");
                std::process::exit(2);
            }
        };
    let case: Case = match serde_json::from_value(request.case.clone()) {
        Ok(case) => case,
        Err(error) => {
            eprintln!("ymm-clmul-arm: case does not decode: {error}");
            std::process::exit(2);
        }
    };
    let measured = match case.workload.as_str() {
        "raw-batch" => run_raw_batch(&request, &case),
        "fieldvec-dot" => run_fieldvec_dot(&request, &case),
        other => Err(format!("unknown workload {other:?}")),
    };
    let measured = match measured {
        Ok(measured) => measured,
        Err(error) => {
            eprintln!("ymm-clmul-arm: {error}");
            std::process::exit(1);
        }
    };
    let cpus_observed = CpuAffinity::observe()
        .map(|affinity| affinity.cpus().to_vec())
        .unwrap_or_default();
    let result = ArmResult {
        schema: "zen3-benchmark-arm-result-v1".into(),
        windows: measured.windows,
        cache_state_applied: request.cache_state.clone(),
        workers_observed: 1,
        cpus_observed,
        selected_path: selected_path(),
        conversion: Some(measured.conversion),
        quality: None,
    };
    if let Err(error) = transport::write_result_line(io::stdout().lock(), &result) {
        eprintln!("ymm-clmul-arm: {error}");
        std::process::exit(1);
    }
}
