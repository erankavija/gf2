//! Synthetic arm for protocol smoke runs: XOR-folds a seeded buffer.
//!
//! It exists to prove the receipt pipeline end to end and measures nothing
//! that is claimed as a performance result. It reads the canonical child-v2
//! request on stdin and writes exactly one result line.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::hint::black_box;
use std::io;
use std::time::Duration;
use tuning_campaign_support::abtest::SplitMix64;
use tuning_campaign_support::abtest::{frame_ber_interval, wilson_interval_95};
use tuning_campaign_support::host::CpuAffinity;
use tuning_campaign_support::protocol::DecoderCell;
use tuning_campaign_support::receipt::{ArmQuality, DecoderArmSettings, IterationDistribution};
use tuning_campaign_support::timing::{execution_windows_fixed_or_calibrated, FIXTURE_BANKS};
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cold_calls: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    decoder: Option<DecoderCell>,
    windows: u32,
    window_target_ms: u32,
    cpus: Vec<u32>,
    workers_declared: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    words: usize,
    seed: u64,
}

#[derive(Serialize)]
struct Window {
    calls: u64,
    elapsed_ns: u64,
}

#[derive(Serialize)]
struct Result_ {
    schema: String,
    windows: Vec<Window>,
    cache_state_applied: String,
    workers_observed: u32,
    cpus_observed: Vec<u32>,
    selected_path: Option<String>,
    conversion: Option<Value>,
    quality: Option<ArmQuality>,
    calibrated: bool,
}

fn fold(buffer: &[u64]) -> u64 {
    buffer.iter().fold(0, |acc, word| acc ^ word.rotate_left(1))
}

fn main() {
    let sentinel = std::env::var(transport::FRESH_CASE_VAR).ok();
    let request: Request =
        match transport::read_guarded_case(sentinel.as_deref(), io::stdin().lock()) {
            Ok(request) => request,
            Err(error) => {
                eprintln!("ab-smoke-workload: {error}");
                std::process::exit(2);
            }
        };
    let case: Case = match serde_json::from_value(request.case.clone()) {
        Ok(case) => case,
        Err(error) => {
            eprintln!("ab-smoke-workload: case does not decode: {error}");
            std::process::exit(2);
        }
    };
    // Arms differ through their environment, never through the shared case.
    let passes: u32 = std::env::var("GF2_SMOKE_PASSES")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(1);
    let banks = if request.cache_state == "streaming" {
        FIXTURE_BANKS
    } else {
        1
    };
    let mut mixer = SplitMix64::new(case.seed);
    let buffers: Vec<Vec<u64>> = (0..banks)
        .map(|_| (0..case.words).map(|_| mixer.next_u64()).collect())
        .collect();
    if request.cache_state == "cold" && request.cold_calls.is_none() {
        eprintln!("cold cell requires frozen calls; pre-calibration is forbidden");
        std::process::exit(2);
    }
    let mut sink = 0u64;
    if request.cache_state == "warm" {
        sink ^= fold(black_box(&buffers[0]));
    }
    let mut body = |bank: usize| {
        let buffer = &buffers[bank % banks];
        for _ in 0..passes.max(1) {
            sink ^= fold(black_box(buffer));
        }
    };
    let samples = match execution_windows_fixed_or_calibrated(
        0,
        u64::from(request.windows),
        Duration::from_millis(u64::from(request.window_target_ms)),
        request.cold_calls,
        &mut body,
        |_| Ok(()),
    ) {
        Ok(samples) => samples,
        Err(error) => {
            eprintln!("ab-smoke-workload: timing failed: {error}");
            std::process::exit(1);
        }
    };
    black_box(sink);
    let cpus_observed = CpuAffinity::observe()
        .map(|affinity| affinity.cpus().to_vec())
        .unwrap_or_default();
    let result = Result_ {
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
        selected_path: Some("xor-fold-scalar".into()),
        conversion: None,
        quality: request.decoder.as_ref().map(synthetic_quality),
        calibrated: request.cold_calls.is_none(),
    };
    if let Err(error) = transport::write_result_line(io::stdout().lock(), &result) {
        eprintln!("ab-smoke-workload: {error}");
        std::process::exit(1);
    }
}

// Deterministic pipeline fixture, not a decoder performance/Monte Carlo result.
// Half of the independent frame slots carry one information-bit error.
fn synthetic_quality(decoder: &DecoderCell) -> ArmQuality {
    let frame_bit_errors: Vec<_> = (0..decoder.input.frames)
        .map(|i| u64::from(i % 2 == 0))
        .collect();
    let frame_errors = frame_bit_errors.iter().filter(|e| **e != 0).count() as u64;
    let bit_errors = frame_errors;
    let bits = decoder.input.frames * decoder.code.k;
    let fer = wilson_interval_95(frame_errors, decoder.input.frames).unwrap();
    let ber = frame_ber_interval(&frame_bit_errors, decoder.code.k, 0.95).unwrap();
    ArmQuality {
        frame_bit_errors,
        frames: decoder.input.frames,
        frame_errors,
        bits,
        bit_errors,
        fer: frame_errors as f64 / decoder.input.frames as f64,
        fer_interval: [fer.0, fer.1],
        ber: bit_errors as f64 / bits as f64,
        ber_interval: [ber.0, ber.1],
        interval_method: "frame-hoeffding-95+fer-wilson-95".into(),
        iterations: IterationDistribution {
            mean: 1.0,
            p50: 1,
            p90: 1,
            max: 1,
        },
        memory_bytes: 0,
        latency_ns_p50: 0,
        settings: DecoderArmSettings {
            precision: decoder.precision,
            schedule: decoder.schedule,
            normalization: decoder.normalization.clone(),
            iteration_cap: decoder.iteration_cap,
            stopping: decoder.stopping.clone(),
            batch_size: decoder.batching.batch_size,
        },
    }
}
