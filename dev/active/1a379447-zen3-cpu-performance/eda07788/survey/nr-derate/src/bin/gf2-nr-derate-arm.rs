//! Native child arm for gf2's 5G NR LLR de-rate-matching.
//!
//! The timed call is the whole consumer operation in gf2's native
//! representation: `Nr5gRateMatchedCode::prepare_llrs` on a `&[Llr]` channel
//! frame, including its `full_n` output allocation. `pack_ns` and `unpack_ns`
//! are zero because `Llr` vectors are that representation end to end.
//!
//! `setup_ns` is the wall time of one `nr_5g_rate_matched` construction,
//! recorded once and not amortized over calls: the code is built per
//! configuration and reused, matching real usage.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::hint::black_box;
use std::io;
use std::time::{Duration, Instant};
use survey_nr_derate::{configuration_named, gf2_code, seeded_llr_banks};
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
    configuration: String,
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

fn fail(message: impl std::fmt::Display, code: i32) -> ! {
    eprintln!("gf2-nr-derate-arm: {message}");
    std::process::exit(code);
}

fn main() {
    let sentinel = std::env::var(transport::FRESH_CASE_VAR).ok();
    let request: Request =
        match transport::read_guarded_case(sentinel.as_deref(), io::stdin().lock()) {
            Ok(request) => request,
            Err(error) => fail(error, 2),
        };
    let case: Case = match serde_json::from_value(request.case.clone()) {
        Ok(case) => case,
        Err(error) => fail(format!("case does not decode: {error}"), 2),
    };
    let banks = match request.cache_state.as_str() {
        "warm" => 1,
        "streaming" => FIXTURE_BANKS,
        other => fail(format!("unsupported cache state {other:?}"), 2),
    };
    let configuration = configuration_named(&case.configuration);
    let setup_start = Instant::now();
    let code = gf2_code(configuration);
    let setup_ns = u64::try_from(setup_start.elapsed().as_nanos()).expect("setup time fits in u64");
    let buffers = seeded_llr_banks(case.seed, banks, configuration.target_n);

    let mut body = |bank: usize| {
        black_box(code.prepare_llrs(&buffers[bank % banks]));
    };
    if banks == 1 {
        // The warm policy's untimed pass over the working set.
        body(0);
    }
    let samples = match execution_windows_configured(
        0,
        u64::from(request.windows),
        Duration::from_millis(u64::from(request.window_target_ms)),
        &mut body,
        |_| Ok(()),
    ) {
        Ok(samples) => samples,
        Err(error) => fail(format!("timing failed: {error}"), 1),
    };
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
        selected_path: Some(format!("gf2-prepare-llrs-{}", configuration.name)),
        conversion: Some(ConversionCosts {
            setup_ns,
            pack_ns: 0,
            unpack_ns: 0,
            batch_fill_ns: 0,
            dispatch_ns: 0,
        }),
        quality: None,
    };
    if let Err(error) = transport::write_result_line(io::stdout().lock(), &result) {
        fail(error, 1);
    }
}
