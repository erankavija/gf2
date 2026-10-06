//! Profile driver for one measured path of the study (jit:53c5a8c0).
//!
//! A perf counting or sampling session needs a process whose whole run is one
//! path executing a known number of calls, which the child-v2 arms are not:
//! they calibrate their own call count, rotate banks and time windows, so a
//! counter read over one of them attributes to the framing as much as to the
//! path. This binary runs the identical [`Backend`] on the identical fixtures
//! and nothing else: create the backend, warm the bank, then issue exactly
//! `--calls` logical calls of the cell's operation inside one loop. One logical
//! call is the case's `inner` operations, which is what a logical call is in
//! the arms' own window loop, so a per-call counter figure means the same thing
//! in a profile record as in a receipt.
//!
//! It produces no timing and no ratio. The counters are perf's, the call count
//! is the argument, and the only thing this process reports on stdout is the
//! runtime-observed path identity and the counts of what it did, so a profile
//! record can be attributed without the profiler having to parse the arm.
//!
//! A counting session covers the whole process, so the record carries the
//! warm-up count, the measured count and the total operation count, and the
//! summary divides by the total rather than by the measured part. A sampling
//! session needs no such division.
//!
//! Usage:
//!   profile-arm --case '<case json>' --calls <n> [--warmup <n>] [--label <s>]
//!
//! `GF2_CROSSOVER_PATH` selects the per-element or batched entry point for the
//! crossover cases, exactly as it does for `crossover-arm`.
//!
//! Arm selection: `--arm gf2` uses the gf2 backend; `--arm gf2x` uses the
//! pinned gf2x build. Both live in this binary so one profile session shape
//! serves both sides of a comparator-gap cell; the gf2x binding makes this
//! executable's digest distinct from `crossover-arm`'s, which is why the
//! crossover family's own arms do not live here.

#[path = "../gf2x_backend.rs"]
mod gf2x_backend;

use clmul_crossover_arms::gf2_backend::Gf2Backend;
use clmul_crossover_arms::{banks, Backend, Bank, Case};

fn argument(name: &str) -> Option<String> {
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        if flag == name {
            return args.next();
        }
    }
    None
}

fn required(name: &str) -> String {
    argument(name).unwrap_or_else(|| panic!("missing required argument {name}"))
}

/// Issues `warmup + calls` logical calls of `case` through `B`, reporting the
/// path `B` selected.
fn drive<B: Backend>(case: &Case, warmup: u64, calls: u64) -> String {
    let inner = case.inner();
    let mut backend = B::create(case);
    let mut bank: Vec<Bank> = banks(case, 0, 1);
    let bank = &mut bank[0];
    let selected = backend.selected_path();
    for _ in 0..(warmup + calls) {
        for _ in 0..inner {
            backend.run(case, bank);
        }
    }
    std::hint::black_box(&bank.out);
    selected
}

fn main() {
    let case: Case = serde_json::from_str(&required("--case")).expect("--case is a study case");
    let calls: u64 = required("--calls").parse().expect("--calls is a count");
    let warmup: u64 = argument("--warmup")
        .map(|value| value.parse().expect("--warmup is a count"))
        .unwrap_or(0);
    let arm = argument("--arm").unwrap_or_else(|| "gf2".to_owned());
    let label = argument("--label").unwrap_or_else(|| "unlabelled".to_owned());

    let selected = match arm.as_str() {
        "gf2" => drive::<Gf2Backend>(&case, warmup, calls),
        "gf2x" => drive::<gf2x_backend::Gf2xBackend>(&case, warmup, calls),
        other => panic!("--arm is `gf2` or `gf2x`, got {other:?}"),
    };

    let record = serde_json::json!({
        "schema": "clmul-crossover-profile-run-v1",
        "label": label,
        "arm": arm,
        "case": case,
        "inner_per_call": case.inner(),
        "warmup_calls": warmup,
        "measured_calls": calls,
        "total_operations": (warmup + calls) * case.inner() as u64,
        "selected_path": selected,
    });
    println!("{record}");
}
