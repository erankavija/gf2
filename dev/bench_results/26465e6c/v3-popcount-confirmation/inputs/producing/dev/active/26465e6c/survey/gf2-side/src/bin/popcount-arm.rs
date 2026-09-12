//! Protocol arm child for both survey families (jit:26465e6c).
//!
//! Reads one canonical `zen3-benchmark-arm-request-v1` on stdin, resolves the
//! arm named by `GF2_POPCOUNT_ARM` to one function pointer, builds the cell's
//! fixtures, times the pointer through `timing::execution_windows_fixed_or_calibrated`
//! and writes one result line. `--build-identity` prints the external compile
//! record embedded at build time.

use popcount_survey::external::BUILD_RECORD;
use popcount_survey::{AndArm, Fixture, Pattern, PopcountArm};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::hint::black_box;
use std::io;
use std::time::{Duration, Instant};
use tuning_campaign_support::host::CpuAffinity;
use tuning_campaign_support::protocol::DecoderCell;
use tuning_campaign_support::receipt::{ConversionCosts, WindowRecord};
use tuning_campaign_support::timing::{execution_windows_fixed_or_calibrated, FIXTURE_BANKS};
use tuning_campaign_support::transport;

const ARM_VAR: &str = "GF2_POPCOUNT_ARM";
const RESULT_SCHEMA: &str = "zen3-benchmark-arm-result-v1";
/// Target length of one conversion-cost probe.
const PROBE_TARGET: Duration = Duration::from_millis(5);
/// Upper bound on probe iterations.
const PROBE_MAX_CALLS: u64 = 1 << 24;

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
struct PopcountCase {
    op: String,
    words: usize,
    seed: u64,
    pattern: String,
    word_offset: usize,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AndCase {
    op: String,
    words: usize,
    seed_lhs: u64,
    seed_rhs: u64,
    pattern: String,
    word_offset: usize,
    whole_consumer: bool,
}

#[derive(Serialize)]
struct Output {
    schema: &'static str,
    windows: Vec<WindowRecord>,
    cache_state_applied: String,
    workers_observed: u32,
    cpus_observed: Vec<u32>,
    selected_path: Option<String>,
    conversion: Option<ConversionCosts>,
    quality: Option<Value>,
    calibrated: bool,
}

/// Fixture banks a cell rotates through: every bank for `streaming`, one
/// working set otherwise, as `ab-smoke-workload` does.
fn banks(cache_state: &str) -> Result<usize, String> {
    match cache_state {
        "streaming" => Ok(FIXTURE_BANKS),
        "warm" | "cold" => Ok(1),
        other => Err(format!("unknown cache_state {other:?}")),
    }
}

fn elapsed_ns(start: Instant) -> u64 {
    u64::try_from(start.elapsed().as_nanos()).unwrap_or(u64::MAX)
}

/// Mean nanoseconds of one `body` call, calibrated to `PROBE_TARGET`. Probes
/// run after the timed windows and never subtract from them.
fn probe_ns(mut body: impl FnMut()) -> u64 {
    let mut calls = 1_u64;
    loop {
        let start = Instant::now();
        for _ in 0..calls {
            body();
        }
        let elapsed = start.elapsed();
        if elapsed >= PROBE_TARGET || calls >= PROBE_MAX_CALLS {
            return u64::try_from(elapsed.as_nanos() / u128::from(calls)).unwrap_or(u64::MAX);
        }
        calls *= 2;
    }
}

fn time(request: &Request, body: &mut impl FnMut(usize)) -> Result<Vec<WindowRecord>, String> {
    let samples = execution_windows_fixed_or_calibrated(
        0,
        u64::from(request.windows),
        Duration::from_millis(u64::from(request.window_target_ms)),
        request.cold_calls,
        body,
        |_| Ok(()),
    )
    .map_err(|error| format!("timing failed: {error}"))?;
    Ok(samples
        .into_iter()
        .map(|sample| WindowRecord {
            calls: sample.calls,
            elapsed_ns: sample.elapsed_ns,
        })
        .collect())
}

fn run_popcount(
    request: &Request,
    arm_name: &str,
) -> Result<(Vec<WindowRecord>, String, Option<ConversionCosts>), String> {
    let case: PopcountCase = serde_json::from_value(request.case.clone())
        .map_err(|error| format!("popcount case does not decode: {error}"))?;
    if case.op != "popcount" {
        return Err(format!("case op {:?} is not popcount", case.op));
    }
    let arm = PopcountArm::parse(arm_name)
        .ok_or_else(|| format!("{arm_name:?} is not a popcount arm"))?;
    let op = arm.resolve()?;
    let pattern = Pattern::parse(&case.pattern)?;
    let banks = banks(&request.cache_state)?;
    let fixtures = (0..banks)
        .map(|_| Fixture::new(case.words, case.seed, pattern, case.word_offset))
        .collect::<Result<Vec<_>, _>>()?;
    for fixture in &fixtures {
        if !arm.accepts_misalignment(fixture.misalignment_bytes()) {
            return Err(format!(
                "{arm_name} rejects a window {} bytes past a vector boundary",
                fixture.misalignment_bytes()
            ));
        }
    }
    let slots: [&[u64]; FIXTURE_BANKS] = std::array::from_fn(|bank| fixtures[bank % banks].words());
    let mut sink = 0_u64;
    if request.cache_state == "warm" {
        // SAFETY: every slot passed the arm's alignment check above, and `op`
        // was resolved on this host.
        sink ^= unsafe { op(slots[0]) };
    }
    let mut body = |bank: usize| {
        // SAFETY: as above; `bank` is already reduced below FIXTURE_BANKS.
        sink ^= unsafe { op(black_box(slots[bank])) };
    };
    let windows = time(request, &mut body)?;
    black_box(sink);
    Ok((windows, arm.selected_path(case.words), None))
}

fn run_and(
    request: &Request,
    arm_name: &str,
) -> Result<(Vec<WindowRecord>, String, Option<ConversionCosts>), String> {
    let case: AndCase = serde_json::from_value(request.case.clone())
        .map_err(|error| format!("and case does not decode: {error}"))?;
    if case.op != "and_popcnt" {
        return Err(format!("case op {:?} is not and_popcnt", case.op));
    }
    let arm = AndArm::parse(arm_name).ok_or_else(|| format!("{arm_name:?} is not an AND arm"))?;
    let setup_start = Instant::now();
    let op = arm.resolve()?;
    let pattern = Pattern::parse(&case.pattern)?;
    let banks = banks(&request.cache_state)?;
    let fixtures = (0..banks)
        .map(|_| {
            Ok((
                Fixture::new(case.words, case.seed_lhs, pattern, case.word_offset)?,
                Fixture::new(case.words, case.seed_rhs, pattern, case.word_offset)?,
            ))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let setup_ns = elapsed_ns(setup_start);
    let slots: [(&[u64], &[u64]); FIXTURE_BANKS] = std::array::from_fn(|bank| {
        let (lhs, rhs) = &fixtures[bank % banks];
        (lhs.words(), rhs.words())
    });
    let mut sink = 0_u64;
    if request.cache_state == "warm" {
        sink ^= op(slots[0].0, slots[0].1);
    }
    let mut body = |bank: usize| {
        let (lhs, rhs) = black_box(slots[bank]);
        sink ^= op(lhs, rhs);
    };
    let windows = time(request, &mut body)?;
    black_box(sink);
    let conversion = case.whole_consumer.then(|| {
        let (lhs, _) = slots[0];
        ConversionCosts {
            setup_ns,
            pack_ns: match arm {
                AndArm::TwoPass => probe_ns(|| {
                    black_box(black_box(lhs).to_vec());
                }),
                AndArm::Fused | AndArm::ScalarControl => 0,
            },
            unpack_ns: 0,
            batch_fill_ns: 0,
            dispatch_ns: match arm {
                AndArm::TwoPass => probe_ns(|| {
                    for _ in 0..2 {
                        black_box(popcount_survey::arms::dispatch_route(black_box(lhs.len())));
                    }
                }),
                AndArm::Fused | AndArm::ScalarControl => 0,
            },
        }
    });
    Ok((windows, arm.selected_path(case.words), conversion))
}

fn run(request: Request, arm_name: &str) -> Result<Output, String> {
    if request.workers_declared != 1 {
        return Err(format!(
            "every survey arm is single-worker; the cell declares {}",
            request.workers_declared
        ));
    }
    if request.cache_state == "cold" && request.cold_calls.is_none() {
        return Err("a cold cell requires frozen cold_calls".to_owned());
    }
    let op = request
        .case
        .get("op")
        .and_then(Value::as_str)
        .ok_or_else(|| "case lacks op".to_owned())?;
    let (windows, selected_path, conversion) = match op {
        "popcount" => run_popcount(&request, arm_name)?,
        "and_popcnt" => run_and(&request, arm_name)?,
        other => return Err(format!("unknown case op {other:?}")),
    };
    let cpus_observed = CpuAffinity::observe()
        .map(|affinity| affinity.cpus().to_vec())
        .map_err(|error| format!("cannot observe affinity: {error}"))?;
    Ok(Output {
        schema: RESULT_SCHEMA,
        windows,
        cache_state_applied: request.cache_state.clone(),
        workers_observed: 1,
        cpus_observed,
        selected_path: Some(selected_path),
        conversion,
        quality: None,
        calibrated: request.cold_calls.is_none(),
    })
}

fn main() {
    if std::env::args().nth(1).as_deref() == Some("--build-identity") {
        print!("{BUILD_RECORD}");
        return;
    }
    let arm = match std::env::var(ARM_VAR) {
        Ok(value) => value,
        Err(_) => {
            eprintln!("popcount-arm: {ARM_VAR} is absent");
            std::process::exit(2);
        }
    };
    let sentinel = std::env::var(transport::FRESH_CASE_VAR).ok();
    let request: Request =
        match transport::read_guarded_case(sentinel.as_deref(), io::stdin().lock()) {
            Ok(request) => request,
            Err(error) => {
                eprintln!("popcount-arm: {error}");
                std::process::exit(2);
            }
        };
    match run(request, &arm) {
        Ok(output) => {
            if let Err(error) = transport::write_result_line(io::stdout().lock(), &output) {
                eprintln!("popcount-arm: cannot write result: {error}");
                std::process::exit(1);
            }
        }
        Err(error) => {
            eprintln!("popcount-arm: {error}");
            std::process::exit(if error.starts_with("timing failed") {
                1
            } else {
                2
            });
        }
    }
}
