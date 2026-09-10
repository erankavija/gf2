//! External child-v2 arm for the operation-equivalent xdsopl PCTITL baseline.
//!
//! The timed call is the whole consumer operation in this arm's native
//! representation: unpack gf2's canonical packed frame into one `int32` per
//! bit, apply `PCTITL::fwd`, pack the result back. Both conversions are inside
//! the timed closure, so the reported latency includes them.
//!
//! `pack_ns` and `unpack_ns` are mean nanoseconds per timed call, computed by
//! dividing the conversion wall time accumulated during the measured windows
//! (the accumulators are reset to zero when `execution_windows_configured`
//! reports `CalibrationComplete`, so pre-calibration calls do not inflate the
//! average) by the total calls in all returned windows. `setup_ns` is zero
//! because the external PCTITL templates are stateless.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::cell::Cell;
use std::hint::black_box;
use std::io;
use std::time::{Duration, Instant};
use survey_gf2_side::{frame_bits, pack_bits, seeded_word_banks, unpack_words, xdsopl_forward};
use tuning_campaign_support::host::CpuAffinity;
use tuning_campaign_support::timing::{
    execution_windows_configured, TimingProgress, FIXTURE_BANKS,
};
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
    modcod: String,
    seed: u64,
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

fn main() {
    let sentinel = std::env::var(transport::FRESH_CASE_VAR).ok();
    let request: Request =
        match transport::read_guarded_case(sentinel.as_deref(), io::stdin().lock()) {
            Ok(request) => request,
            Err(error) => {
                eprintln!("xdsopl-dvb-t2-baseline: {error}");
                std::process::exit(2);
            }
        };
    let case: Case = match serde_json::from_value(request.case.clone()) {
        Ok(case) => case,
        Err(error) => {
            eprintln!("xdsopl-dvb-t2-baseline: case does not decode: {error}");
            std::process::exit(2);
        }
    };
    let bits = frame_bits(&case.modcod);
    let banks = if request.cache_state == "streaming" {
        FIXTURE_BANKS
    } else {
        1
    };
    let buffers = seeded_word_banks(case.seed, banks, bits);
    if request.cache_state == "warm" {
        black_box(&buffers[0]);
    }

    let pack_ns_total = Cell::new(0_u64);
    let unpack_ns_total = Cell::new(0_u64);
    let mut body = |bank: usize| {
        let packed = &buffers[bank % banks];
        let unpack_start = Instant::now();
        let input = unpack_words(packed, bits);
        unpack_ns_total.set(unpack_ns_total.get().saturating_add(
            u64::try_from(unpack_start.elapsed().as_nanos()).expect("unpack time fits in u64"),
        ));
        let mut output = vec![0_i32; bits];
        xdsopl_forward(&case.modcod, &input, &mut output);
        let pack_start = Instant::now();
        let packed_output = pack_bits(&output);
        pack_ns_total.set(pack_ns_total.get().saturating_add(
            u64::try_from(pack_start.elapsed().as_nanos()).expect("pack time fits in u64"),
        ));
        black_box(packed_output);
    };
    let samples = match execution_windows_configured(
        0,
        u64::from(request.windows),
        Duration::from_millis(u64::from(request.window_target_ms)),
        &mut body,
        |progress| {
            if let TimingProgress::CalibrationComplete { .. } = progress {
                // Discard conversion time accumulated during calibration calls
                // so the reported average covers only the measured windows.
                pack_ns_total.set(0);
                unpack_ns_total.set(0);
            }
            Ok(())
        },
    ) {
        Ok(samples) => samples,
        Err(error) => {
            eprintln!("xdsopl-dvb-t2-baseline: timing failed: {error}");
            std::process::exit(1);
        }
    };
    let total_calls: u64 = samples.iter().map(|sample| sample.calls).sum();
    assert!(total_calls > 0, "timing protocol returned no calls");
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
        cache_state_applied: request.cache_state,
        workers_observed: 1,
        cpus_observed,
        selected_path: Some(format!("xdsopl-pctitl-{}", case.modcod)),
        conversion: Some(ConversionCosts {
            setup_ns: 0,
            pack_ns: pack_ns_total.get() / total_calls,
            unpack_ns: unpack_ns_total.get() / total_calls,
            batch_fill_ns: 0,
            dispatch_ns: 0,
        }),
        quality: None,
    };
    if let Err(error) = transport::write_result_line(io::stdout().lock(), &result) {
        eprintln!("xdsopl-dvb-t2-baseline: {error}");
        std::process::exit(1);
    }
}
