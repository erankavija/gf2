//! Conforming child-v2 Rust arm for raw and whole-consumer transposes.
//!
//! The local wire structs intentionally mirror the private structs in
//! `benchmark-ab-runner.rs`; the transport helper remains the single source
//! for child-v2 framing.

use gf2_kernels_simd::transpose;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::hint::black_box;
use std::io;
use survey_gf2_side_6fb89a3c::{seeded_matrix, splitmix_words, timed_windows};
use tuning_campaign_support::host::CpuAffinity;
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
struct FixedCase {
    n: u64,
    seed: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TiledCase {
    rows: u64,
    cols: u64,
    seed: u64,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Case {
    Fixed(FixedCase),
    Tiled(TiledCase),
}

#[derive(Serialize)]
struct Window {
    calls: u64,
    elapsed_ns: u64,
}

#[derive(Serialize)]
struct ConversionCosts {
    setup_ns: u64,
    pack_ns: u64,
    unpack_ns: u64,
    batch_fill_ns: u64,
    dispatch_ns: u64,
}

#[derive(Serialize)]
struct Result_ {
    schema: String,
    windows: Vec<Window>,
    cache_state_applied: String,
    workers_observed: u32,
    cpus_observed: Vec<u32>,
    selected_path: Option<String>,
    conversion: Option<ConversionCosts>,
    quality: Option<Value>,
}

fn fail(message: impl std::fmt::Display) -> ! {
    eprintln!("gf2_transpose_arm: {message}");
    std::process::exit(2);
}

fn windows(samples: Vec<tuning_campaign_support::timing::TimingSample>) -> Vec<Window> {
    samples
        .into_iter()
        .map(|sample| Window {
            calls: sample.calls,
            elapsed_ns: sample.elapsed_ns,
        })
        .collect()
}

fn main() {
    let sentinel = std::env::var(transport::FRESH_CASE_VAR).ok();
    let request: Request = transport::read_guarded_case(sentinel.as_deref(), io::stdin().lock())
        .unwrap_or_else(|error| fail(error));
    let case: Case =
        serde_json::from_value(request.case.clone()).unwrap_or_else(|error| fail(error));

    let (selected_path, conversion, samples) = match case {
        Case::Fixed(case) => {
            if case.n != 64 {
                fail("fixed transpose requires n=64");
            }
            let input: [u64; 64] = splitmix_words(64, case.seed)
                .try_into()
                .expect("fixed transpose input has 64 words");
            let fns = transpose::detect().unwrap_or(transpose::TransposeFns {
                transpose_64x64: transpose::transpose_64x64_scalar,
                name: "scalar-bit-twiddle",
            });
            let mut output = [0u64; 64];
            let samples = timed_windows(request.windows, request.window_target_ms, |_| {
                (fns.transpose_64x64)(black_box(&input), black_box(&mut output));
            })
            .unwrap_or_else(|error| fail(error));
            black_box(output);
            (fns.name.to_owned(), None, samples)
        }
        Case::Tiled(case) => {
            let rows =
                usize::try_from(case.rows).unwrap_or_else(|_| fail("rows does not fit usize"));
            let cols =
                usize::try_from(case.cols).unwrap_or_else(|_| fail("cols does not fit usize"));
            // The construction/fill belongs to setup/conversion and is done
            // once per child execution, outside the calibrated loop.
            let started = std::time::Instant::now();
            let matrix = seeded_matrix(rows, cols, case.seed);
            let setup_ns = u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX);
            let samples = timed_windows(request.windows, request.window_target_ms, |_| {
                black_box(matrix.transpose());
            })
            .unwrap_or_else(|error| fail(error));
            (
                "BitMatrix::transpose".to_owned(),
                Some(ConversionCosts {
                    setup_ns,
                    pack_ns: 0,
                    unpack_ns: 0,
                    batch_fill_ns: 0,
                    dispatch_ns: 0,
                }),
                samples,
            )
        }
    };

    let cpus_observed = CpuAffinity::observe()
        .map(|affinity| affinity.cpus().to_vec())
        .unwrap_or_default();
    let result = Result_ {
        schema: "zen3-benchmark-arm-result-v1".into(),
        windows: windows(samples),
        cache_state_applied: request.cache_state,
        workers_observed: 1,
        cpus_observed,
        selected_path: Some(selected_path),
        conversion,
        quality: None,
    };
    transport::write_result_line(io::stdout().lock(), &result).unwrap_or_else(|error| fail(error));
}
