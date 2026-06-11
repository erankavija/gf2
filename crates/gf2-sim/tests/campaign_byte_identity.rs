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
//! Two legs, **both `#[ignore]`** because each spawns the migrated binary as a
//! subprocess that builds a full DVB-T2 codec (`DvbT2Concat` + LDPC encoder
//! cache) and runs at least one n = 64800 frame — far over the 5 s fast-tier
//! budget once the full workspace battery runs it 24-wide (the per-test wall is
//! ~1–2 s in isolation but exceeds 5 s under contention, the same heavy
//! live-simulation class as the sibling `executor_oom_fallback_run` /
//! `hybrid_resume` / `preset_vs_graph_byte_identity` suites). They run on the
//! slow tier; the fast-tier coverage of the binary's run-path plumbing is the
//! in-binary `point_to_csv_row` / `*_wires_to_config` unit tests plus the
//! parse-only CLI rejection tests in `campaign_cli_flags.rs`.
//!
//! - `byte_identical_two_runs_smoke` — `#[ignore = "sim: ..."]` (8 frames ×
//!   1 SNR point), proves the two-run determinism plumbing.
//! - `byte_identical_two_runs_waterfall` — `#[ignore = "sim: ..."]`
//!   (200 frames at the r1/2 16-QAM waterfall Es/N0 = 6.0 dB), where the run is
//!   **non-vacuous**: `0 < errored_frames < frames` is asserted, so the verdict
//!   boundary the determinism contract is about is genuinely exercised. (6.0 dB
//!   is the measured knee for this exact 200-frame SumProduct/ExactLogMap/seed-42
//!   setup — ~52/200 frame errors; 6.25 dB converges everything and would be
//!   vacuous, while ≤ 5.75 dB errors every frame.)

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
            assert_eq!(
                c.len(),
                7,
                "campaign CSV must have 7 columns, got line: {l}"
            );
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
fn run_campaign(
    out_dir: &str,
    esn0_range: &str,
    max_frames: &str,
    target_errors: &str,
) -> Vec<DetRow> {
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

/// Smoke: two runs at the same seed produce byte-identical
/// `fer`/`frames`/`errors`/`mean_iters`. 8 frames × 1 SNR point. `#[ignore]`
/// because each run spawns a full-codec subprocess (heavy live-simulation
/// class; >5 s under the contended fast-tier battery).
#[test]
#[ignore = "sim: two full-codec subprocess runs for binary two-run byte-identity"]
fn byte_identical_two_runs_smoke() {
    let a = run_campaign("/tmp/dvb_d2_byteid_smoke_a", "6.25:6.25:0.5", "8", "1000");
    let b = run_campaign("/tmp/dvb_d2_byteid_smoke_b", "6.25:6.25:0.5", "8", "1000");
    assert_eq!(a.len(), 1, "one SNR point");
    assert_eq!(
        a, b,
        "two runs at seed 42 must be byte-identical on fer/frames/errors/mean_iters"
    );
    assert_eq!(a[0].frames, "8", "max_frames honoured");
}

/// Slow-tier non-vacuous waterfall leg: 200 frames at the r1/2 16-QAM waterfall
/// Es/N0 = 6.0 dB. Asserts `0 < errors < frames` (a genuine mix of
/// decode-success and decode-fail frames), then two-run byte-identity on the
/// four deterministic columns — so the §11 verdict boundary is exercised.
#[test]
#[ignore = "sim: 200-frame n=64800 DVB-T2 BICM waterfall two-run byte-identity"]
fn byte_identical_two_runs_waterfall() {
    let a = run_campaign(
        "/tmp/dvb_d2_byteid_wf_a",
        "6.0:6.0:0.5",
        "200",
        "100000", // never reached; runs the full 200 frames
    );
    let b = run_campaign("/tmp/dvb_d2_byteid_wf_b", "6.0:6.0:0.5", "200", "100000");
    assert_eq!(a.len(), 1);

    // Non-vacuity: a genuine waterfall mix of errored and clean frames.
    let frames: u64 = a[0].frames.parse().unwrap();
    let errors: u64 = a[0].errors.parse().unwrap();
    assert_eq!(frames, 200, "the full frame budget ran");
    assert!(
        0 < errors && errors < frames,
        "6.0 dB r1/2 16-QAM must be a non-vacuous waterfall: 0 < errors ({errors}) < frames ({frames})"
    );

    assert_eq!(
        a, b,
        "two 200-frame runs at seed 42 must be byte-identical on fer/frames/errors/mean_iters"
    );
}
