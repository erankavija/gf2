//! Within-new-pipeline byte-identity for the migrated DVB-T2 BICM AWGN
//! campaign binary (jit:bbf6b6ee, wave D.2 of epic gf2-sim `f9717e7e`).
//!
//! Spawns the migrated `gf2-sim` campaign binary
//! (`crates/gf2-sim/src/bin/dvb_t2_awgn_campaign.rs`, located via
//! `CARGO_BIN_EXE_dvb_t2_awgn_campaign`) **twice** at the same seed and config,
//! and asserts the four deterministic CSV columns `fer` / `frames` / `errors` /
//! `mean_iters` are **byte-identical** across the two runs (the §11 CPU-only
//! contract). The two excluded columns — `ber` (non-associative f32 reduction)
//! and `wall_seconds` (run-duration-dependent) — are NOT compared.
//!
//! Per `ec530af9` §3/§12 (user-approved Q2 on 2026-06-07), the legacy
//! `simulation.rs` path is **not** a byte-identity comparison target; only the
//! new pipeline vs itself.
//!
//! Two legs:
//! - `byte_identical_two_runs_smoke` — fast tier (8 frames × 1 SNR point), proves
//!   the two-run determinism plumbing without the 5 s budget risk.
//! - `byte_identical_two_runs_waterfall` — `#[ignore = "sim: ..."]` slow tier
//!   (200 frames at the r1/2 16-QAM waterfall Es/N0 = 6.25 dB), where the run is
//!   **non-vacuous**: `0 < errored_frames < frames` is asserted, so the verdict
//!   boundary the determinism contract is about is genuinely exercised.

use std::path::PathBuf;
use std::process::{Command, Stdio};

/// Path to the migrated campaign binary cargo built for this test.
fn binary_path() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_dvb_t2_awgn_campaign"))
}

/// One CSV row's deterministic columns: `(fer, frames, errors, mean_iters)`.
/// `es_n0_db` is the key; `ber` and `wall_seconds` are dropped (§11-excluded).
#[derive(Debug, Clone, PartialEq)]
struct DetRow {
    es_n0_db: String,
    fer: String,
    frames: String,
    errors: String,
    mean_iters: String,
}

/// Parses the campaign CSV into its deterministic per-point columns.
///
/// Columns are `es_n0_db,fer,ber,frames,errors,mean_iters,wall_seconds`; this
/// keeps `es_n0_db` (col 0), `fer` (1), `frames` (3), `errors` (4),
/// `mean_iters` (5), and drops `ber` (2) and `wall_seconds` (6).
fn parse_det_rows(csv: &str) -> Vec<DetRow> {
    csv.lines()
        .skip(1) // header
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let c: Vec<&str> = l.split(',').collect();
            assert_eq!(c.len(), 7, "campaign CSV must have 7 columns, got line: {l}");
            DetRow {
                es_n0_db: c[0].to_string(),
                fer: c[1].to_string(),
                frames: c[3].to_string(),
                errors: c[4].to_string(),
                mean_iters: c[5].to_string(),
            }
        })
        .collect()
}

/// Runs the migrated campaign once, returning the parsed curve CSV rows.
fn run_campaign(out_dir: &str, esn0_range: &str, max_frames: &str, target_errors: &str) -> Vec<DetRow> {
    let _ = std::fs::remove_dir_all(out_dir);
    let bin = binary_path();
    let status = Command::new(&bin)
        .args([
            "--rate",
            "1/2",
            "--modulation",
            "16qam",
            "--esn0-range",
            esn0_range,
            "--max-frames",
            max_frames,
            "--target-errors",
            target_errors,
            "--decoder",
            "sumproduct",
            "--demap",
            "exactlogmap",
            "--output-dir",
            out_dir,
            "--seed",
            "42",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("spawn dvb_t2_awgn_campaign");
    assert!(status.success(), "campaign run must succeed");
    let csv = std::fs::read_to_string(format!("{out_dir}/curve_1_2_16qam.csv"))
        .expect("campaign curve CSV must be written");
    parse_det_rows(&csv)
}

/// Fast smoke: two runs at the same seed produce byte-identical
/// `fer`/`frames`/`errors`/`mean_iters`. 8 frames × 1 SNR point keeps this well
/// under the 5 s fast-tier budget.
#[test]
fn byte_identical_two_runs_smoke() {
    let a = run_campaign(
        "/tmp/dvb_d2_byteid_smoke_a",
        "6.25:6.25:0.5",
        "8",
        "1000",
    );
    let b = run_campaign(
        "/tmp/dvb_d2_byteid_smoke_b",
        "6.25:6.25:0.5",
        "8",
        "1000",
    );
    assert_eq!(a.len(), 1, "one SNR point");
    assert_eq!(
        a, b,
        "two runs at seed 42 must be byte-identical on fer/frames/errors/mean_iters"
    );
    assert_eq!(a[0].frames, "8", "max_frames honoured");
}

/// Slow-tier non-vacuous waterfall leg: 200 frames at the r1/2 16-QAM waterfall
/// Es/N0 = 6.25 dB. Asserts `0 < errors < frames` (a genuine mix of
/// decode-success and decode-fail frames), then two-run byte-identity on the
/// four deterministic columns — so the §11 verdict boundary is exercised.
#[test]
#[ignore = "sim: 200-frame n=64800 DVB-T2 BICM waterfall two-run byte-identity"]
fn byte_identical_two_runs_waterfall() {
    let a = run_campaign(
        "/tmp/dvb_d2_byteid_wf_a",
        "6.25:6.25:0.5",
        "200",
        "100000", // never reached; runs the full 200 frames
    );
    let b = run_campaign(
        "/tmp/dvb_d2_byteid_wf_b",
        "6.25:6.25:0.5",
        "200",
        "100000",
    );
    assert_eq!(a.len(), 1);

    // Non-vacuity: a genuine waterfall mix of errored and clean frames.
    let frames: u64 = a[0].frames.parse().unwrap();
    let errors: u64 = a[0].errors.parse().unwrap();
    assert_eq!(frames, 200, "the full frame budget ran");
    assert!(
        0 < errors && errors < frames,
        "6.25 dB r1/2 16-QAM must be a non-vacuous waterfall: 0 < errors ({errors}) < frames ({frames})"
    );

    assert_eq!(
        a, b,
        "two 200-frame runs at seed 42 must be byte-identical on fer/frames/errors/mean_iters"
    );
}
