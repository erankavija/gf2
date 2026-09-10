//! External child arm for AFF3CT's 5G NR LLR de-rate-matching.
//!
//! The timed call is the whole consumer operation in gf2's representation,
//! `Adapter::prepare_llrs`: convert the `&[Llr]` channel frame to AFF3CT's
//! `float` input and allocate a zeroed `N_LDPC` output, run AFF3CT's public
//! `Puncturer::depuncture`, then convert back to `Vec<Llr>` with AFF3CT's
//! +infinity fillers replaced by gf2's filler value. Every step is inside the
//! timed closure.
//!
//! Setup refuses a configuration unless AFF3CT's mother-code length equals
//! gf2's `full_n` and AFF3CT's filler range equals the filler positions gf2
//! writes, so a non-equivalent configuration is never timed.
//!
//! `setup_ns` is one `Puncturer_5G` construction. `unpack_ns` and `pack_ns`
//! are untimed diagnostics measured after the timing windows: the mean time of
//! the adapter's input stage and output stage, each run alone for the last
//! window's call count. They are not additive components of the timed call.

use gf2_coding::Llr;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::hint::black_box;
use std::io;
use std::time::{Duration, Instant};
use survey_nr_derate::aff3ct::{Adapter, Depuncturer};
use survey_nr_derate::{configuration_named, gf2_code, observed_fillers, seeded_llr_banks};
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
    eprintln!("aff3ct-nr-derate-arm: {message}");
    std::process::exit(code);
}

fn nanos(duration: Duration) -> u64 {
    u64::try_from(duration.as_nanos()).expect("duration fits in u64")
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

    // gf2 supplies the reference dimensions and filler value, untimed.
    let reference = gf2_code(configuration);
    let (filler_positions, filler) = observed_fillers(&reference);

    let setup_start = Instant::now();
    let depuncturer = match Depuncturer::new(configuration.target_k, configuration.target_n) {
        Ok(depuncturer) => depuncturer,
        Err(error) => fail(error, 2),
    };
    let setup_ns = nanos(setup_start.elapsed());
    if depuncturer.full_len() != reference.params().full_n
        || depuncturer.filler_range().collect::<Vec<_>>() != filler_positions
    {
        fail(
            format!(
                "{} is not operation-equivalent: AFF3CT derives {:?}",
                configuration.name,
                depuncturer.derived()
            ),
            2,
        );
    }
    let adapter = Adapter::new(depuncturer, filler.unwrap_or_else(Llr::zero));
    let buffers = seeded_llr_banks(case.seed, banks, configuration.target_n);

    let mut body = |bank: usize| {
        black_box(adapter.prepare_llrs(&buffers[bank % banks]));
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

    // Untimed stage diagnostics after the windows.
    let calls = samples.last().map_or(1, |sample| sample.calls.max(1));
    let start = Instant::now();
    for call in 0..calls {
        black_box(adapter.unpack(&buffers[call as usize % banks]));
    }
    let unpack_ns = nanos(start.elapsed()) / calls;
    let (_, raw) = {
        let (input, mut full) = adapter.unpack(&buffers[0]);
        adapter.depuncturer().depuncture(&input, &mut full);
        (input, full)
    };
    let start = Instant::now();
    for _ in 0..calls {
        black_box(adapter.pack(black_box(&raw)));
    }
    let pack_ns = nanos(start.elapsed()) / calls;

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
        selected_path: Some(format!(
            "aff3ct-puncturer-5g-depuncture-{}",
            configuration.name
        )),
        conversion: Some(ConversionCosts {
            setup_ns,
            pack_ns,
            unpack_ns,
            batch_fill_ns: 0,
            dispatch_ns: 0,
        }),
        quality: None,
    };
    if let Err(error) = transport::write_result_line(io::stdout().lock(), &result) {
        fail(error, 1);
    }
}
