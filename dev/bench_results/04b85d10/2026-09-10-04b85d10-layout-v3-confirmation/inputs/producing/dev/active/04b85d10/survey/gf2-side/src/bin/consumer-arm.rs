//! Protocol arm for the bit-storage consumer profile (jit:04b85d10).
//!
//! It reads the canonical child-v2 request on stdin and writes exactly one
//! result line, so the Zen 3 benchmark runner drives it like any other arm.
//! The workload and its sizes arrive in the shared case; the route this arm
//! takes arrives in `GF2_CONSUMER_PATH`, so both children of a pair read
//! byte-identical input and differ only in the production route they call.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::hint::black_box;
use std::io;
use std::time::Duration;

use consumer_profile_gf2_side::{bank_count, prepare, ArmPath, Case, Conversion};
use tuning_campaign_support::host::CpuAffinity;
use tuning_campaign_support::timing::{execution_windows_configured, FIXTURE_BANKS};
use tuning_campaign_support::transport;

/// Environment naming the production route this arm takes.
const PATH_VAR: &str = "GF2_CONSUMER_PATH";

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

fn fail(message: impl AsRef<str>) -> ! {
    eprintln!("consumer-arm: {}", message.as_ref());
    std::process::exit(2);
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
    let path = match std::env::var(PATH_VAR) {
        Ok(value) => match ArmPath::parse(&value) {
            Ok(path) => path,
            Err(error) => fail(error),
        },
        Err(_) => fail(format!("{PATH_VAR} is absent")),
    };

    let mut prepared = match prepare(&case, path, &request.cache_state, FIXTURE_BANKS) {
        Ok(prepared) => prepared,
        Err(error) => fail(error),
    };
    let banks = bank_count(&request.cache_state, FIXTURE_BANKS);

    // A warm cell touches its whole working set once before calibration; a
    // cold cell enters calibration on untouched buffers; a streaming cell
    // rotates through the banks and warms none of them.
    if request.cache_state == "warm" {
        prepared.run(0);
    }

    let mut body = |bank: usize| {
        prepared.run(bank % banks);
    };
    let samples = match execution_windows_configured(
        0,
        u64::from(request.windows),
        Duration::from_millis(u64::from(request.window_target_ms)),
        &mut body,
        |_| Ok(()),
    ) {
        Ok(samples) => samples,
        Err(error) => fail(format!("timing failed: {error}")),
    };
    black_box(prepared.sink());

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
        workers_observed: prepared.workers_observed,
        cpus_observed,
        selected_path: Some(prepared.selected_path.clone()),
        conversion: prepared.conversion,
        quality: None,
    };
    if let Err(error) = transport::write_result_line(io::stdout().lock(), &result) {
        fail(error.to_string());
    }
}
