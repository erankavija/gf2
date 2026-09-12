//! Native child arm for gf2's 5G NR rate-matched encode.
//!
//! The timed call is the whole consumer operation in gf2's own
//! representation: `<Nr5gRateMatchedCode as BlockEncoder>::encode` on a
//! `target_k`-bit `BitVec`, including its `target_n`-bit output allocation
//! and the mother-code encode and transmitted-position gather it fuses.
//! `pack_ns` and `unpack_ns` are zero because `BitVec` is that representation
//! end to end.
//!
//! `setup_ns` is the wall time of one `nr_5g_rate_matched` construction,
//! recorded once and not amortized over calls: the code is built per
//! configuration and reused, matching real usage.

#[path = "arm_common.rs"]
mod arm_common;

use arm_common::{ArmResult, Case, ConversionCosts, Request, Window};
use gf2_coding::traits::BlockEncoder;
use std::hint::black_box;
use std::io;
use std::time::{Duration, Instant};
use survey_nr_encode::{configuration_named, gf2_code, seeded_messages};
use tuning_campaign_support::host::CpuAffinity;
use tuning_campaign_support::timing::{execution_windows_configured, FIXTURE_BANKS};
use tuning_campaign_support::transport;

fn fail(message: impl std::fmt::Display, code: i32) -> ! {
    eprintln!("gf2-nr-encode-arm: {message}");
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
                "{} selects redundancy version {}, which gf2's rate-matched encoder does not \
                 implement",
                configuration.name, configuration.redundancy_version
            ),
            2,
        );
    }
    let setup_start = Instant::now();
    let code = gf2_code(configuration);
    let setup_ns = arm_common::nanos(setup_start.elapsed());
    let messages = seeded_messages(case.seed, banks, configuration.target_k);

    let mut body = |bank: usize| {
        black_box(code.encode(&messages[bank % banks]));
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
        selected_path: Some(format!("gf2-encode-rate-matched-{}", configuration.name)),
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
