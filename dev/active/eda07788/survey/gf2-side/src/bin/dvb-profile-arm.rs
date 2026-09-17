//! Protocol-v4 campaign arm for the DVB-T2 interleaver profile (jit:9fb40c83).
//!
//! The executable serves three frozen arms selected by environment: two
//! identical direct-gf2 arms form the isolated null control, while the real
//! `gf2-sim::stages::BitInterleave` boundary is compared with the equivalent
//! xdsopl adapter at the same one-frame `BitPackedBatch` boundary.

use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2BitInterleaver;
use gf2_sim::batch::BitPackedBatch;
use gf2_sim::stages::BitInterleave;
use gf2_sim::Stage;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::cell::Cell;
use std::hint::black_box;
use std::io;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;
use survey_gf2_side::{
    bitvec_from_words, frame_bits, modcod_for_name, pack_bits, seeded_word_banks, timed_windows,
    unpack_words, xdsopl_forward_mut, CachePolicy,
};
use tuning_campaign_support::host::CpuAffinity;
use tuning_campaign_support::timing::TimingProgress;
use tuning_campaign_support::transport;

/// Mirror of the runner's arm request.
///
/// `transport::decode_case` accepts only the exact bytes this type re-encodes,
/// so every field spelling, order and omission rule matches the runner's own
/// request type: the runner omits `cold_calls` and `decoder` for a cell that
/// declares neither, and a mirror that spells them `null` rejects the request.
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

#[derive(Clone, Copy)]
enum Route {
    Gf2Direct,
    Gf2Stage,
    XdsoplStage,
}

impl Route {
    fn from_environment() -> Result<Self, String> {
        match std::env::var("GF2_DVB_PROFILE_ROUTE").as_deref() {
            Ok("gf2-direct-a" | "gf2-direct-b") => Ok(Self::Gf2Direct),
            Ok("gf2-stage") => Ok(Self::Gf2Stage),
            Ok("xdsopl-stage") => Ok(Self::XdsoplStage),
            Ok(other) => Err(format!("unknown GF2_DVB_PROFILE_ROUTE {other:?}")),
            Err(error) => Err(format!("GF2_DVB_PROFILE_ROUTE is unavailable: {error}")),
        }
    }

    fn selected_path(self, modcod: &str) -> String {
        match self {
            Self::Gf2Direct => format!(
                "gf2-coding/DvbT2BitInterleaver::interleave/scalar-bit-scatter/{modcod}"
            ),
            Self::Gf2Stage => format!(
                "gf2-sim/BitInterleave::process/DvbT2BitInterleaver::interleave/scalar-bit-scatter/{modcod}"
            ),
            Self::XdsoplStage => format!(
                "profile-adapter/BitPackedBatch/unpack-copy/xdsopl-PCTITL/pack/{modcod}"
            ),
        }
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("dvb-profile-arm: {error}");
        std::process::exit(2);
    }
}

fn run() -> Result<(), String> {
    let sentinel = std::env::var(transport::FRESH_CASE_VAR).ok();
    require_benchmark_window(sentinel.as_deref())?;
    let request: Request = transport::read_guarded_case(sentinel.as_deref(), io::stdin().lock())
        .map_err(|error| error.to_string())?;
    if request.workers_declared != 1 || request.cold_calls.is_some() || request.decoder.is_some() {
        return Err("the profile implements one-worker non-decoder warm/streaming cells".into());
    }
    let case: Case = serde_json::from_value(request.case.clone())
        .map_err(|error| format!("case does not decode: {error}"))?;
    let route = Route::from_environment()?;
    let policy =
        CachePolicy::from_request(&request.cache_state).map_err(|error| error.to_string())?;
    let bits = frame_bits(&case.modcod);
    let words = seeded_word_banks(case.seed, policy.banks(), bits);

    let pack_ns = Rc::new(Cell::new(0_u64));
    let unpack_ns = Rc::new(Cell::new(0_u64));
    let batch_fill_ns = Rc::new(Cell::new(0_u64));
    let setup_start = Instant::now();
    let mut body: Box<dyn FnMut(usize)> = match route {
        Route::Gf2Direct => {
            let interleaver = DvbT2BitInterleaver::new(modcod_for_name(&case.modcod));
            let buffers: Vec<_> = words
                .iter()
                .map(|bank| bitvec_from_words(bank, bits))
                .collect();
            Box::new(move |bank| {
                black_box(interleaver.interleave(&buffers[bank]));
            })
        }
        Route::Gf2Stage => {
            let interleaver = Arc::new(DvbT2BitInterleaver::new(modcod_for_name(&case.modcod)));
            let stage = BitInterleave::new(interleaver);
            let batches: Vec<_> = words
                .iter()
                .map(|bank| BitPackedBatch::new(vec![bitvec_from_words(bank, bits)]))
                .collect();
            Box::new(move |bank| {
                black_box(
                    stage
                        .process(&batches[bank], &mut ())
                        .expect("stage succeeds"),
                );
            })
        }
        Route::XdsoplStage => {
            let modcod = case.modcod.clone();
            let body_pack_ns = Rc::clone(&pack_ns);
            let body_unpack_ns = Rc::clone(&unpack_ns);
            let body_batch_fill_ns = Rc::clone(&batch_fill_ns);
            Box::new(move |bank| {
                let started = Instant::now();
                let unpacked = unpack_words(&words[bank], bits);
                body_unpack_ns.set(body_unpack_ns.get().saturating_add(elapsed_ns(started)));

                let started = Instant::now();
                let mut mutable_input = unpacked.clone();
                let mut output = vec![0_i32; bits];
                body_batch_fill_ns
                    .set(body_batch_fill_ns.get().saturating_add(elapsed_ns(started)));
                xdsopl_forward_mut(&modcod, &mut mutable_input, &mut output);

                let started = Instant::now();
                let packed = pack_bits(&output);
                body_pack_ns.set(body_pack_ns.get().saturating_add(elapsed_ns(started)));
                black_box(BitPackedBatch::new(vec![bitvec_from_words(&packed, bits)]));
            })
        }
    };
    let setup_ns = elapsed_ns(setup_start);

    let samples = timed_windows(
        policy,
        request.windows,
        request.window_target_ms,
        &mut body,
        |progress| {
            if let TimingProgress::CalibrationComplete { .. } = progress {
                pack_ns.set(0);
                unpack_ns.set(0);
                batch_fill_ns.set(0);
            }
            Ok(())
        },
    )
    .map_err(|error| format!("timing failed: {error}"))?;
    let total_calls: u64 = samples.iter().map(|sample| sample.calls).sum();
    if total_calls == 0 {
        return Err("timing protocol returned no calls".into());
    }
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
        cache_state_applied: policy.name().into(),
        workers_observed: 1,
        cpus_observed,
        selected_path: Some(route.selected_path(&case.modcod)),
        conversion: Some(ConversionCosts {
            setup_ns,
            pack_ns: pack_ns.get() / total_calls,
            unpack_ns: unpack_ns.get() / total_calls,
            batch_fill_ns: batch_fill_ns.get() / total_calls,
            dispatch_ns: 0,
        }),
        quality: None,
    };
    transport::write_result_line(io::stdout().lock(), &result).map_err(|error| error.to_string())
}

/// Fails a hand invocation closed and leaves window policy to the campaign.
///
/// `benchmark-ab-runner` clears the child environment and installs only the
/// plan's per-arm variables plus the child-v2 sentinel, so a campaign child
/// never observes the window variables its launcher exported. A child that
/// carries the sentinel therefore defers to the two layers that do enforce the
/// window, `survey/run-dvb-campaign.sh` and `dev/scripts/ccx1-bench-flock.sh`,
/// and to the untimed runner smoke the worker brief requires outside one.
/// Every other invocation requires the window variables and exits before it
/// reads a request.
fn require_benchmark_window(sentinel: Option<&str>) -> Result<(), String> {
    if sentinel == Some(transport::FRESH_CASE_VALUE) {
        return Ok(());
    }
    if std::env::var("GF2_BENCH_WINDOW").as_deref() != Ok("1")
        || std::env::var("GF2_BENCH").as_deref() != Ok("1")
    {
        return Err("timed DVB profiling requires GF2_BENCH_WINDOW=1 and GF2_BENCH=1".into());
    }
    Ok(())
}

fn elapsed_ns(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX)
}
