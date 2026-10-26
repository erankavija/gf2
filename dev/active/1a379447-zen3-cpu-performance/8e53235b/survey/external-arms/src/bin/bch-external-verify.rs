//! Holds every external engine to gf2's codewords on every code class and
//! batch size of the integrated BCH comparison, untimed.
//!
//! Usage: bch-external-verify > RECORD.json
//!
//! A JSON array with one row per engine, code and batch: `equal`, or `not-served` with the
//! reason the engine declines the case, or `differs` with the first differing
//! codeword, which also makes the process exit 1. The record carries no clock
//! reading, so two runs of one build write the same bytes.

use std::collections::BTreeMap;

use bch_external_arms::{Engine, EngineKind, Refusal};
use consumer_profile_gf2_side::Case;
use serde_json::{json, Value};

/// The `fd9d5416` W1 batch sizes.
const BATCHES: [u64; 4] = [1, 16, 256, 4096];
/// Message seed of every verified case, a constant of this tool.
const SEED: u64 = 0x1358_d6ab;

fn main() {
    let codes: [(&str, &str, u64); 3] = [
        ("bch-encode-batch", "degree", 14),
        ("dvb-bch-encode", "n", 7200),
        ("dvb-bch-encode", "n", 32400),
    ];
    let mut rows = Vec::new();
    let mut differs = false;
    for kind in EngineKind::ALL {
        for (workload, key, value) in codes {
            for batch in BATCHES {
                let case = Case {
                    workload: workload.to_owned(),
                    size: BTreeMap::from([(key.to_owned(), value), ("batch".to_owned(), batch)]),
                    seed: SEED,
                };
                let outcome: Value = match Engine::prepare(kind, &case) {
                    Ok(engine) => {
                        json!({"outcome": "equal", "selected_path": engine.selected_path()})
                    }
                    Err(Refusal::Differs(reason)) => {
                        differs = true;
                        json!({"outcome": "differs", "reason": reason})
                    }
                    Err(Refusal::Declined(reason)) => {
                        json!({"outcome": "not-served", "reason": reason})
                    }
                };
                let mut row = json!({"engine": kind.name(), "case": case});
                row.as_object_mut().expect("a row is an object").extend(
                    outcome
                        .as_object()
                        .expect("an outcome is an object")
                        .clone(),
                );
                rows.push(row);
            }
        }
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&rows).expect("the record serializes")
    );
    std::process::exit(i32::from(differs));
}
