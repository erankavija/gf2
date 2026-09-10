//! Conforming child-v2 Rust arm for the production logical-buffer XOR.
//!
//! The timed body copies `src0` into the destination and then folds the
//! remaining sources in with `xor_inplace`, because the production API
//! accumulates (`dst ^= src`) while the comparator's `xor_gen` contract
//! writes a fresh destination from `sources` inputs. The copy is
//! deliberately inside every timed call so both arms pay the same fresh
//! destination write; `pack_ns` reports one separately measured copy so the
//! arrangement cost is visible on its own.

use gf2_core::kernels::ops::xor_inplace;
use gf2_core::kernels::select_backend_for_size;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::hint::black_box;
use std::io;
use std::time::Instant;
use survey_gf2_side_6fb89a3c::{splitmix_words, timed_windows};
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
    words: u64,
    seed: u64,
    alignment_bytes: u64,
    #[serde(default = "two")]
    sources: u64,
}

fn two() -> u64 {
    2
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
    eprintln!("gf2_logical_xor_arm: {message}");
    std::process::exit(2);
}

fn aligned_storage(words: &[u64], alignment: usize) -> (Vec<u64>, usize) {
    let extra_words = alignment / std::mem::size_of::<u64>();
    let mut storage = vec![0u64; words.len() + extra_words];
    let base = storage.as_ptr() as usize;
    let byte_offset = (alignment - (base % alignment)) % alignment;
    let start = byte_offset / std::mem::size_of::<u64>();
    storage[start..start + words.len()].copy_from_slice(words);
    assert_eq!((storage[start..].as_ptr() as usize) % alignment, 0);
    (storage, start)
}

/// One separately measured fresh-destination copy, averaged over many
/// repetitions so a few-nanosecond cost rounds to an observed value.
fn copy_probe_ns(dest: &mut [u64], src0: &[u64]) -> u64 {
    const REPS: u32 = 1_000_000;
    let started = Instant::now();
    for _ in 0..REPS {
        black_box(&mut *dest).copy_from_slice(black_box(src0));
    }
    let total = u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX);
    (total + u64::from(REPS) / 2) / u64::from(REPS)
}

fn main() {
    let sentinel = std::env::var(transport::FRESH_CASE_VAR).ok();
    let request: Request = transport::read_guarded_case(sentinel.as_deref(), io::stdin().lock())
        .unwrap_or_else(|error| fail(error));
    let case: Case =
        serde_json::from_value(request.case.clone()).unwrap_or_else(|error| fail(error));
    let words = usize::try_from(case.words).unwrap_or_else(|_| fail("words does not fit usize"));
    let alignment = usize::try_from(case.alignment_bytes)
        .unwrap_or_else(|_| fail("alignment does not fit usize"));
    if alignment != 32 {
        fail("the ISA-L operation-equivalent arm requires 32-byte alignment");
    }
    let sources = usize::try_from(case.sources).unwrap_or_else(|_| fail("sources does not fit"));
    if !(2..=8).contains(&sources) {
        fail("sources must lie in 2..=8");
    }
    let setup_started = Instant::now();
    let source_storage: Vec<(Vec<u64>, usize)> = (0..sources)
        .map(|s| {
            aligned_storage(
                &splitmix_words(words, case.seed.wrapping_add(s as u64)),
                alignment,
            )
        })
        .collect();
    let (mut dest_storage, dest_start) = aligned_storage(&vec![0u64; words], alignment);
    let setup_ns = u64::try_from(setup_started.elapsed().as_nanos()).unwrap_or(u64::MAX);
    let sources_view: Vec<&[u64]> = source_storage
        .iter()
        .map(|(storage, start)| &storage[*start..*start + words])
        .collect();
    let selected_path = format!(
        "gf2-xor_inplace ({}) sources={sources}",
        select_backend_for_size(words).name()
    );
    let dest = &mut dest_storage[dest_start..dest_start + words];
    let pack_ns = copy_probe_ns(dest, sources_view[0]);
    let samples = timed_windows(request.windows, request.window_target_ms, |_| {
        dest.copy_from_slice(sources_view[0]);
        for src in &sources_view[1..] {
            xor_inplace(black_box(&mut *dest), black_box(src));
        }
    })
    .unwrap_or_else(|error| fail(error));
    black_box(&dest);
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
            pack_ns,
            unpack_ns: 0,
            batch_fill_ns: 0,
            dispatch_ns: 0,
        }),
        quality: None,
    };
    transport::write_result_line(io::stdout().lock(), &result).unwrap_or_else(|error| fail(error));
}
