//! gf2-side arm of the transpose-lane-versus-external receipts (jit:1d4fd63d).
//!
//! The external arms of these cells are 6fb89a3c's pinned `m4ri_transpose_arm`
//! and `bitshuffle_transpose_arm`, which decode the fixed case `{n: 64, seed}`
//! as one 64×64 bit-block transpose per timed call into a preallocated output.
//! This arm answers the same case with one lane of
//! `gf2_kernels_simd::transpose`, so a cell measures the lane against the
//! comparator and nothing else.
//!
//! # Equivalent bit mapping
//!
//! Every arm of a cell reads the same fixture: 64 words drawn from SplitMix64
//! [Steele2014] seeded at the case's `seed`, word `r` carrying matrix row `r`
//! with bit `c` the entry $(r, c)$. Every arm writes 64 words with bit `r` of
//! word `c` equal to that entry. `--dump` prints the transposed matrix one row
//! per line with bit 0 first, which is the form the C arms' `--dump-check`
//! prints, so `survey/verify-bit-mapping.py` compares all of them against
//! naive bit arithmetic before any timing.
//!
//! # Arms
//!
//! `GF2_TRANSPOSE_LANE` names the lane, or `production` for the kernel
//! `gf2_kernels_simd::transpose::detect` publishes on this host.

use gf2_kernels_simd::transpose::{self, Transpose64x64Fn, TransposeLane};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::hint::black_box;
use std::io::{self, Write};
use std::time::Duration;
use tuning_campaign_support::abtest::SplitMix64;
use tuning_campaign_support::host::CpuAffinity;
use tuning_campaign_support::timing::{execution_windows_configured, TimingSample, FIXTURE_BANKS};
use tuning_campaign_support::transport;

/// The fixed kernel case both external arms decode.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FixedCase {
    n: u64,
    seed: u64,
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
    calibrated: bool,
}

fn fail(message: impl std::fmt::Display) -> ! {
    eprintln!("transpose-lane-external-arm: {message}");
    std::process::exit(2);
}

/// The lane this arm runs and the tag its receipt carries.
fn resolve_lane() -> (Transpose64x64Fn, String) {
    let requested = std::env::var("GF2_TRANSPOSE_LANE")
        .unwrap_or_else(|_| fail("GF2_TRANSPOSE_LANE names the lane this arm runs"));
    if requested == "production" {
        let fns = transpose::detect()
            .unwrap_or_else(|| fail("no transpose lane is available on this host"));
        return (fns.transpose_64x64, format!("production/{}", fns.name));
    }
    let named = TransposeLane::from_name(&requested)
        .unwrap_or_else(|| fail(format!("{requested:?} names no transpose lane")));
    let kernel = transpose::lane(named)
        .unwrap_or_else(|| fail(format!("this host cannot run the {} lane", named.name())));
    (kernel, format!("pinned/{}", named.name()))
}

/// 64 words drawn from SplitMix64 at `seed`, the fixture every arm reads.
fn fixture(seed: u64) -> [u64; 64] {
    let mut mixer = SplitMix64::new(seed);
    let mut block = [0u64; 64];
    for word in block.iter_mut() {
        *word = mixer.next_u64();
    }
    block
}

/// Writes the transposed block one matrix row per line, bit 0 first.
fn dump(seed: u64) -> io::Result<()> {
    let (kernel, _) = resolve_lane();
    let input = fixture(seed);
    let mut output = [0u64; 64];
    kernel(&input, &mut output);
    let stdout = io::stdout();
    let mut writer = io::BufWriter::new(stdout.lock());
    for word in output {
        for bit in 0..64 {
            writer.write_all(if word & (1u64 << bit) != 0 { b"1" } else { b"0" })?;
        }
        writer.write_all(b"\n")?;
    }
    writer.flush()
}

fn main() {
    let mut arguments = std::env::args().skip(1);
    if let Some(first) = arguments.next() {
        if first == "--dump" {
            let seed = arguments
                .next()
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or_else(|| fail("--dump takes one unsigned seed"));
            if let Err(error) = dump(seed) {
                fail(error);
            }
            return;
        }
        fail(format!("unknown argument {first:?}; this arm takes --dump <seed>"));
    }

    let sentinel = std::env::var(transport::FRESH_CASE_VAR).ok();
    let request: Request = transport::read_guarded_case(sentinel.as_deref(), io::stdin().lock())
        .unwrap_or_else(|error| fail(error));
    let case: FixedCase = serde_json::from_value(request.case.clone())
        .unwrap_or_else(|error| fail(format!("case does not decode: {error}")));
    if case.n != 64 {
        fail("the fixed transpose case is 64 rows of 64 columns");
    }
    if request.cold_calls.is_some() {
        fail("this family declares no fixed-call cold cell");
    }
    if request.decoder.is_some() {
        fail("this family declares no decoder cell");
    }
    if request.workers_declared != 1 {
        fail(format!(
            "this family declares one worker per cell, got {}",
            request.workers_declared
        ));
    }
    if request.cache_state != "warm" {
        fail(format!(
            "this family declares warm cells only, got {:?}",
            request.cache_state
        ));
    }

    let (kernel, selected_path) = resolve_lane();
    let input = fixture(case.seed);
    let mut output = [0u64; 64];
    let mut body = |_bank: usize| {
        kernel(black_box(&input), black_box(&mut output));
    };

    // The declared warm policy: one untimed pass over the working set before
    // calibration. The working set is one block, so one call is the pass.
    body(0);

    let samples: Vec<TimingSample> = execution_windows_configured(
        0,
        u64::from(request.windows),
        Duration::from_millis(u64::from(request.window_target_ms)),
        &mut |bank| body(bank % FIXTURE_BANKS),
        |_| Ok(()),
    )
    .unwrap_or_else(|error| fail(format!("timing failed: {error}")));
    black_box(output);

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
        // The block is one 512-byte fixture the child builds before the warm
        // pass; the lane is resolved once, outside every window.
        conversion: Some(Conversion {
            setup_ns: 0,
            pack_ns: 0,
            unpack_ns: 0,
            batch_fill_ns: 0,
            dispatch_ns: 0,
        }),
        selected_path: Some(selected_path),
        quality: None,
        calibrated: true,
    };
    if let Err(error) = transport::write_result_line(io::stdout().lock(), &result) {
        fail(error);
    }
}
