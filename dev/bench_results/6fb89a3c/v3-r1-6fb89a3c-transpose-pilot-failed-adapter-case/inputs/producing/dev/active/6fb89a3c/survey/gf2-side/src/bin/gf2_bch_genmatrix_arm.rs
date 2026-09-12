//! Conforming child-v2 Rust BCH generator-matrix arm and generator dumper.

use gf2_coding::test_support::bch_generator_matrix_by_encoding;
use gf2_core::BitMatrix;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs::File;
use std::hint::black_box;
use std::io::{self, BufWriter, Write};
use std::time::Instant;
use survey_gf2_side_6fb89a3c::{build_bch, generator_bits, timed_windows, BCH_ROWS};
use tuning_campaign_support::host::CpuAffinity;
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
    code: String,
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

fn fail(message: impl std::fmt::Display) -> ! {
    eprintln!("gf2_bch_genmatrix_arm: {message}");
    std::process::exit(2);
}

fn dump_generators(path: &str) -> Result<(), String> {
    let file = File::create(path).map_err(|error| format!("cannot create {path}: {error}"))?;
    let mut writer = BufWriter::new(file);
    for row in BCH_ROWS {
        let code = build_bch(row.name)?;
        let degree = code
            .generator()
            .degree()
            .ok_or_else(|| format!("{} has no generator degree", row.name))?;
        writeln!(
            writer,
            "{} {} {} {} {}",
            row.name,
            code.n(),
            code.k(),
            degree,
            generator_bits(&code)
        )
        .map_err(|error| format!("cannot write {path}: {error}"))?;
    }
    writer
        .flush()
        .map_err(|error| format!("cannot flush {path}: {error}"))
}

fn main() {
    let mut args = std::env::args();
    let _program = args.next();
    if args.next().as_deref() == Some("dump-generators") {
        let path = args
            .next()
            .unwrap_or_else(|| fail("usage: dump-generators <path>"));
        if args.next().is_some() {
            fail("usage: dump-generators <path>");
        }
        dump_generators(&path).unwrap_or_else(|error| fail(error));
        return;
    }

    let sentinel = std::env::var(transport::FRESH_CASE_VAR).ok();
    let request: Request = transport::read_guarded_case(sentinel.as_deref(), io::stdin().lock())
        .unwrap_or_else(|error| fail(error));
    let case: Case =
        serde_json::from_value(request.case.clone()).unwrap_or_else(|error| fail(error));
    let _seed = case.seed;
    let started = Instant::now();
    let code = build_bch(&case.code).unwrap_or_else(|error| fail(error));
    let setup_ns = u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX);
    let selected_path = format!("bch_generator_matrix_by_encoding/{}", case.code);
    let samples = timed_windows(request.windows, request.window_target_ms, |_| {
        // Fresh allocation is intentionally inside every timed call, matching
        // M4RI's fresh matrix per call so no overwrite artifact can leak from
        // one call to the next.  Allocation dominance is reported as an open
        // comparison concern rather than hidden by methodology changes.
        let mut out = BitMatrix::zeros(code.k(), code.n());
        bch_generator_matrix_by_encoding(&code, &mut out).expect("generator matrix shape");
        black_box(out);
    })
    .unwrap_or_else(|error| fail(error));
    let cpus_observed = CpuAffinity::observe()
        .map(|affinity| affinity.cpus().to_vec())
        .unwrap_or_default();
    let result = Result_ {
        schema: "zen3-benchmark-arm-result-v1".into(),
        windows: samples
            .into_iter()
            .map(|sample| Window {
                calls: sample.calls,
                elapsed_ns: sample.elapsed_ns,
            })
            .collect(),
        cache_state_applied: request.cache_state,
        workers_observed: 1,
        cpus_observed,
        selected_path: Some(selected_path),
        conversion: Some(ConversionCosts {
            setup_ns,
            pack_ns: 0,
            unpack_ns: 0,
            batch_fill_ns: 0,
            dispatch_ns: 0,
        }),
        quality: None,
    };
    transport::write_result_line(io::stdout().lock(), &result).unwrap_or_else(|error| fail(error));
}
