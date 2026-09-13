//! External child arm for srsRAN's 5G NR rate-matched encode.
//!
//! The timed call is the whole consumer operation in gf2's representation,
//! `Adapter::encode`: convert the `target_k`-bit `BitVec` into srsRAN's
//! packed `bit_buffer` layout over `K_LDPC` bits with the filler positions
//! zero, allocate the packed output, run srsRAN's public
//! `ldpc_encoder::encode` and `ldpc_rate_matcher::rate_match`, then convert
//! the packed result back to a `BitVec`. Every step is inside the timed
//! closure.
//!
//! Setup refuses a configuration unless srsRAN's mother dimensions equal
//! gf2's and the configuration selects redundancy version 0, so a
//! non-equivalent configuration is never timed.
//!
//! `setup_ns` is one encoder and rate-matcher construction. `unpack_ns` and
//! `pack_ns` are untimed diagnostics measured after the timing windows: the
//! mean time of the adapter's input stage and output stage, each run alone
//! for the last window's call count. They are not additive components of the
//! timed call.

#[path = "arm_common.rs"]
mod arm_common;

use arm_common::{ArmResult, Case, ConversionCosts, Request, Window};
use std::hint::black_box;
use std::io;
use std::time::{Duration, Instant};
use survey_nr_encode::srsran::{Adapter, Comparator};
use survey_nr_encode::{configuration_named, gf2_code, seeded_messages};
use tuning_campaign_support::host::CpuAffinity;
use tuning_campaign_support::timing::{execution_windows_configured, FIXTURE_BANKS};
use tuning_campaign_support::transport;

/// The modulation order the comparison fixes; TS 38.212 Section 5.4.2.2's
/// interleaving is the identity at one bit per symbol.
const MODULATION_ORDER: u8 = 1;

fn fail(message: impl std::fmt::Display, code: i32) -> ! {
    eprintln!("srsran-nr-encode-arm: {message}");
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
    if configuration.redundancy_version != 0 {
        fail(
            format!(
                "{} selects redundancy version {}, which gf2's encoder does not implement; the \
                 configuration is recorded, not timed",
                configuration.name, configuration.redundancy_version
            ),
            2,
        );
    }

    // gf2 supplies the reference dimensions, untimed.
    let reference = gf2_code(configuration);
    let params = reference.params().clone();
    let Some(dims) = survey_nr_encode::srsran::dimensions(params.base_graph, params.lifting_factor)
    else {
        fail(
            format!(
                "srsRAN carries no graph for BG{} Z={}",
                params.base_graph, params.lifting_factor
            ),
            2,
        );
    };
    if usize::try_from(dims.k_ldpc).unwrap_or(0) != params.full_k
        || usize::try_from(dims.n_full).unwrap_or(0) != params.full_n
    {
        fail(
            format!(
                "{} is not operation-equivalent: srsRAN derives K_LDPC={} N={}, gf2 uses {} and {}",
                configuration.name, dims.k_ldpc, dims.n_full, params.full_k, params.full_n
            ),
            2,
        );
    }

    let setup_start = Instant::now();
    let comparator = match Comparator::new(None) {
        Ok(comparator) => comparator,
        Err(error) => fail(error, 2),
    };
    let backend = comparator.backend().name().to_string();
    let adapter = Adapter::new(
        comparator,
        &params,
        configuration.redundancy_version,
        MODULATION_ORDER,
    );
    let setup_ns = arm_common::nanos(setup_start.elapsed());
    let messages = seeded_messages(case.seed, banks, configuration.target_k);

    let mut body = |bank: usize| {
        black_box(adapter.encode(&messages[bank % banks]));
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
        black_box(adapter.unpack(&messages[call as usize % banks]));
    }
    let unpack_ns = arm_common::nanos(start.elapsed()) / calls;
    let packed = {
        let (input, mut output) = adapter.unpack(&messages[0]);
        adapter
            .comparator()
            .encode_rate_match(adapter.request(), &input, &mut output)
            .unwrap_or_else(|status| fail(format!("srsRAN rejected the request: {status}"), 1));
        output
    };
    let start = Instant::now();
    for _ in 0..calls {
        black_box(adapter.pack(black_box(&packed)));
    }
    let pack_ns = arm_common::nanos(start.elapsed()) / calls;

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
        cache_state_applied: request.cache_state,
        workers_observed: 1,
        cpus_observed,
        selected_path: Some(format!("srsran-{backend}-{}", configuration.name)),
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
