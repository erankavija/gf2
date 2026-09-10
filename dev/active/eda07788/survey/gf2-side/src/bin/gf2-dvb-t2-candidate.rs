//! Native child-v2 arm for gf2's DVB-T2 bit interleaver.
//!
//! The timed call is the whole consumer operation in gf2's native
//! representation: `DvbT2BitInterleaver::interleave` on a packed `BitVec`,
//! including the output `BitVec` allocation. `pack_ns` and `unpack_ns` are
//! zero because `BitVec` is that representation end to end; that zero is a
//! measured property of the arm, reported rather than omitted, so the
//! whole-consumer cell compares like with like against the external arm's
//! nonzero conversion cost.
//!
//! `setup_ns` is the wall time of one `DvbT2BitInterleaver::new` construction,
//! recorded once and not amortized over calls: the permutation table is built
//! per MODCOD and reused, matching real usage.

use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2BitInterleaver;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::hint::black_box;
use std::io;
use std::time::{Duration, Instant};
use survey_gf2_side::{bitvec_from_words, frame_bits, modcod_for_name, seeded_word_banks};
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
                eprintln!("gf2-dvb-t2-candidate: {error}");
                std::process::exit(2);
            }
        };
    let case: Case = match serde_json::from_value(request.case.clone()) {
        Ok(case) => case,
        Err(error) => {
            eprintln!("gf2-dvb-t2-candidate: case does not decode: {error}");
            std::process::exit(2);
        }
    };
    let bits = frame_bits(&case.modcod);
    let setup_start = Instant::now();
    let interleaver = DvbT2BitInterleaver::new(modcod_for_name(&case.modcod));
    let setup_ns = u64::try_from(setup_start.elapsed().as_nanos()).expect("setup time fits in u64");
    assert_eq!(
        interleaver.frame_bits(),
        bits,
        "gf2 frame length disagrees with the survey table"
    );
    let banks = if request.cache_state == "streaming" {
        FIXTURE_BANKS
    } else {
        1
    };
    let word_buffers = seeded_word_banks(case.seed, banks, bits);
    let buffers: Vec<_> = word_buffers
        .iter()
        .map(|words| bitvec_from_words(words, bits))
        .collect();
    if request.cache_state == "warm" {
        black_box(&buffers[0]);
    }

    let mut body = |bank: usize| {
        let output = interleaver.interleave(&buffers[bank % banks]);
        black_box(output);
    };
    let samples = match execution_windows_configured(
        0,
        u64::from(request.windows),
        Duration::from_millis(u64::from(request.window_target_ms)),
        &mut body,
        |_| Ok(()),
    ) {
        Ok(samples) => samples,
        Err(error) => {
            eprintln!("gf2-dvb-t2-candidate: timing failed: {error}");
            std::process::exit(1);
        }
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
        selected_path: Some(format!("gf2-dvb-t2-interleave-{}", case.modcod)),
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
        eprintln!("gf2-dvb-t2-candidate: {error}");
        std::process::exit(1);
    }
}
