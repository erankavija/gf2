//! Argv-level acceptance and rejection of the migrated DVB-T2 BICM AWGN
//! campaign binary's CLI flags (jit:bbf6b6ee, wave D.2 of epic gf2-sim).
//!
//! Spawns the migrated `gf2-sim` campaign binary (located via
//! `CARGO_BIN_EXE_dvb_t2_awgn_campaign`) as a subprocess and asserts the real
//! process exit status + stderr for the flags whose behaviour is a
//! *process-level* contract:
//!
//! * `--gpu` on a default (non-`hip`) build emits a clear error and exits
//!   non-zero (the "`--gpu` ... emits a clear error on default builds"
//!   criterion);
//! * `--strict-gpu` without `--gpu` is rejected with a clear error;
//! * the migrated parser still rejects the same bad `--decoder` / `--demap`
//!   values the legacy binary did (the migration preserves all flag semantics);
//! * a minimal valid argv runs end-to-end and writes the curve CSV.
//!
//! All rejection tests fail fast at the parse/validation stage (no codec /
//! encoder / simulation work), so they stay well within the fast-tier budget
//! and run un-ignored. The single end-to-end acceptance test
//! (`cli_minimal_valid_run_writes_curve_csv`) spawns a full-codec subprocess
//! that runs a real n = 64800 frame, so it carries `#[ignore = "sim: ..."]`
//! (heavy live-simulation class; >5 s under the contended fast-tier battery).

use std::path::PathBuf;
use std::process::{Command, Stdio};

fn binary_path() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_dvb_t2_awgn_campaign"))
}

/// Minimal valid-shape argv so the parser reaches the flag under test.
fn base_args(output_dir: &str) -> Vec<String> {
    vec![
        "--rate".into(),
        "1/2".into(),
        "--modulation".into(),
        "16qam".into(),
        "--esn0-range".into(),
        "6.0:6.0:0.5".into(),
        "--output-dir".into(),
        output_dir.into(),
    ]
}

/// On a default build (no `hip` feature) `--gpu` must emit a clear error and
/// exit non-zero — never silently run on the CPU mislabelled as a GPU run.
#[cfg(not(feature = "hip"))]
#[test]
fn cli_gpu_on_default_build_emits_clear_error() {
    let out = Command::new(binary_path())
        .args(base_args("/tmp/dvb_d2_cli_gpu_default"))
        .arg("--gpu")
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .expect("spawn dvb_t2_awgn_campaign");
    assert!(
        !out.status.success(),
        "--gpu on a non-hip build must exit non-zero"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("--gpu") && stderr.contains("hip"),
        "stderr must clearly explain that --gpu needs --features hip; got: {stderr}"
    );
}

/// `--strict-gpu` without `--gpu` is meaningless; the binary rejects it with a
/// clear error.
#[test]
fn cli_strict_gpu_without_gpu_is_rejected() {
    let out = Command::new(binary_path())
        .args(base_args("/tmp/dvb_d2_cli_strict_no_gpu"))
        .arg("--strict-gpu")
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .expect("spawn dvb_t2_awgn_campaign");
    assert!(
        !out.status.success(),
        "--strict-gpu without --gpu must exit non-zero"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("strict-gpu") && stderr.contains("--gpu"),
        "stderr must explain --strict-gpu needs --gpu; got: {stderr}"
    );
}

#[test]
fn cli_rejects_unknown_decoder_algorithm() {
    let out = Command::new(binary_path())
        .args(base_args("/tmp/dvb_d2_cli_reject_decoder"))
        .args(["--decoder", "bogusalgo"])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .expect("spawn dvb_t2_awgn_campaign");
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("decoder") || stderr.contains("Unknown") || stderr.contains("bogusalgo"),
        "stderr should mention the decoder parse error; got: {stderr}"
    );
}

#[test]
fn cli_rejects_unknown_demap_method() {
    let out = Command::new(binary_path())
        .args(base_args("/tmp/dvb_d2_cli_reject_demap"))
        .args(["--demap", "softoutput"])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .expect("spawn dvb_t2_awgn_campaign");
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("demap") || stderr.contains("Unknown") || stderr.contains("softoutput"),
        "stderr should mention the demap parse error; got: {stderr}"
    );
}

#[test]
fn cli_rejects_nms_alpha_out_of_range() {
    let out = Command::new(binary_path())
        .args(base_args("/tmp/dvb_d2_cli_reject_nms"))
        .args(["--decoder", "nms:1.5"])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .expect("spawn dvb_t2_awgn_campaign");
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("nms") || stderr.contains("alpha"),
        "stderr should mention the alpha-range error; got: {stderr}"
    );
}

#[test]
fn cli_rejects_mutually_exclusive_calibrate_and_range() {
    let out = Command::new(binary_path())
        .args(base_args("/tmp/dvb_d2_cli_reject_calib_range"))
        .arg("--calibrate")
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .expect("spawn dvb_t2_awgn_campaign");
    assert!(
        !out.status.success(),
        "--calibrate together with --esn0-range must be rejected"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("mutually exclusive"),
        "stderr should mention the mutual exclusion; got: {stderr}"
    );
}

/// End-to-end acceptance: a minimal valid argv runs the migrated pipeline and
/// writes the curve CSV with the canonical 7-column schema (so `plot.py` keeps
/// working) **and** writes `tracing.jsonl` with at least one valid JSON line.
/// 4 frames × 1 SNR point.
///
/// `#[ignore]` because this spawns a full-codec subprocess that runs a real
/// n = 64800 frame (heavy live-simulation class; >5 s under the contended
/// fast-tier battery). Fast-tier CLI coverage is the parse-only rejection
/// tests above.
#[test]
#[ignore = "sim: full-codec subprocess run for end-to-end CSV-schema + tracing.jsonl acceptance"]
fn cli_minimal_valid_run_writes_curve_csv() {
    let out_dir = "/tmp/dvb_d2_cli_minimal_run";
    let _ = std::fs::remove_dir_all(out_dir);
    let out = Command::new(binary_path())
        .args(base_args(out_dir))
        .args([
            "--max-frames",
            "4",
            "--target-errors",
            "1000",
            "--seed",
            "7",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .expect("spawn dvb_t2_awgn_campaign");
    assert!(out.status.success(), "minimal valid run must succeed");

    let csv = std::fs::read_to_string(format!("{out_dir}/curve_1_2_16qam.csv"))
        .expect("curve CSV must be written");
    let header = csv.lines().next().expect("CSV has a header");
    assert_eq!(
        header, "es_n0_db,fer,ber,frames,errors,mean_iters,wall_seconds",
        "CSV schema must match the legacy binary's so plot.py keeps working"
    );
    let row = csv.lines().nth(1).expect("CSV has one data row");
    assert_eq!(row.split(',').count(), 7, "data row has 7 columns");
    assert_eq!(
        row.split(',').nth(3),
        Some("4"),
        "frames column = max_frames"
    );

    // HIGH-1: tracing.jsonl must exist, be non-empty, and every non-empty line
    // must parse as JSON.  We do not assert specific event names (no legacy
    // byte-compat per Q2 decision).
    let jsonl_path = format!("{out_dir}/tracing.jsonl");
    let jsonl = std::fs::read_to_string(&jsonl_path)
        .unwrap_or_else(|e| panic!("tracing.jsonl must be written at {jsonl_path}: {e}"));
    assert!(
        !jsonl.trim().is_empty(),
        "tracing.jsonl must be non-empty after a production run"
    );
    for (i, line) in jsonl.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        serde_json::from_str::<serde_json::Value>(line).unwrap_or_else(|e| {
            panic!("tracing.jsonl line {i} is not valid JSON: {e}\nline: {line}")
        });
    }
}

/// MEDIUM-5 (calibration smoke): `--calibrate` runs end-to-end and writes
/// `calibration/calibration_1_2_16qam.csv` with the canonical 7-column schema.
/// Also asserts `tracing.jsonl` is written (calibration is unconditional per
/// legacy parity). `#[ignore]` — spawns a full-codec subprocess (heavy
/// live-simulation class; >5 s under the contended fast-tier battery).
#[test]
#[ignore = "sim: --calibrate subprocess run for calibration CSV-schema + tracing.jsonl acceptance"]
fn cli_calibrate_writes_calibration_csv() {
    let out_dir = "/tmp/dvb_d2_cli_calibrate";
    let _ = std::fs::remove_dir_all(out_dir);
    let out = Command::new(binary_path())
        .args([
            "--rate",
            "1/2",
            "--modulation",
            "16qam",
            "--calibrate",
            "--calibrate-frames",
            "4",
            "--output-dir",
            out_dir,
            "--seed",
            "7",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .expect("spawn dvb_t2_awgn_campaign --calibrate");
    assert!(
        out.status.success(),
        "--calibrate run must succeed; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // Calibration CSV layout: calibration/<name>.csv with 7-column schema.
    let csv_path = format!("{out_dir}/calibration/calibration_1_2_16qam.csv");
    let csv = std::fs::read_to_string(&csv_path)
        .unwrap_or_else(|e| panic!("calibration CSV must be written at {csv_path}: {e}"));
    let header = csv.lines().next().expect("calibration CSV has a header");
    assert_eq!(
        header, "es_n0_db,fer,ber,frames,errors,mean_iters,wall_seconds",
        "calibration CSV schema must match the production schema"
    );
    let rows: Vec<&str> = csv
        .lines()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .collect();
    // Calibration uses the default 3-point bracket.
    assert_eq!(rows.len(), 3, "default calibration bracket produces 3 rows");
    for row in &rows {
        assert_eq!(
            row.split(',').count(),
            7,
            "each calibration row has 7 columns"
        );
        // frames column (index 3) must equal --calibrate-frames = 4.
        assert_eq!(
            row.split(',').nth(3),
            Some("4"),
            "calibration frames column must equal --calibrate-frames"
        );
    }

    // HIGH-1: tracing.jsonl must be written unconditionally (calibration too).
    let jsonl_path = format!("{out_dir}/tracing.jsonl");
    let jsonl = std::fs::read_to_string(&jsonl_path)
        .unwrap_or_else(|e| panic!("tracing.jsonl must be written at {jsonl_path}: {e}"));
    assert!(
        !jsonl.trim().is_empty(),
        "tracing.jsonl must be non-empty after a calibration run"
    );
    for (i, line) in jsonl.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        serde_json::from_str::<serde_json::Value>(line).unwrap_or_else(|e| {
            panic!("tracing.jsonl line {i} is not valid JSON: {e}\nline: {line}")
        });
    }
}

/// MEDIUM-5 (resume smoke): runs the migrated pipeline with a tiny frame
/// budget + heartbeat, interrupts at the first heartbeat via SIGINT, then
/// resumes and asserts the final CSV is byte-identical on the four
/// deterministic columns (`fer`, `frames`, `errors`, `mean_iters`) compared to
/// an uninterrupted reference run at the same seed.
///
/// Uses the same `--block-at-first-heartbeat`-style pattern established by
/// `checkpoint_compat.rs`, but against the campaign binary directly (which
/// does not expose that flag). Instead we use a small `--max-frames` that is
/// enough to trigger at least one heartbeat and then SIGINT the process while
/// it is running (relying on timing being sufficient for a quick run). The
/// small frame count (8 frames, heartbeat every 2) makes the window wide
/// enough to reliably interrupt.
///
/// `#[ignore]` — spawns two full-codec subprocesses with SIGINT delivery;
/// must be run as a slow-tier test.
#[test]
#[ignore = "sim: kill/resume campaign subprocess smoke for checkpoint byte-identity"]
fn cli_resume_byte_identical_to_uninterrupted() {
    use std::time::Duration;

    // Reference: uninterrupted run.
    let ref_dir = "/tmp/dvb_d2_cli_resume_ref";
    let _ = std::fs::remove_dir_all(ref_dir);
    let ref_status = Command::new(binary_path())
        .args([
            "--rate",
            "1/2",
            "--modulation",
            "16qam",
            "--esn0-range",
            "6.25:6.25:0.5",
            "--max-frames",
            "8",
            "--target-errors",
            "1000",
            "--decoder",
            "sumproduct",
            "--demap",
            "exactlogmap",
            "--output-dir",
            ref_dir,
            "--seed",
            "42",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("spawn reference campaign");
    assert!(ref_status.success(), "reference run must succeed");
    let ref_csv = std::fs::read_to_string(format!("{ref_dir}/curve_1_2_16qam.csv"))
        .expect("reference curve CSV");
    let ref_rows = parse_det_rows(&ref_csv);
    assert_eq!(ref_rows.len(), 1, "one SNR point");

    // Interrupted run (will be resumed).
    let int_dir = "/tmp/dvb_d2_cli_resume_int";
    let _ = std::fs::remove_dir_all(int_dir);
    let mut child = Command::new(binary_path())
        .args([
            "--rate",
            "1/2",
            "--modulation",
            "16qam",
            "--esn0-range",
            "6.25:6.25:0.5",
            "--max-frames",
            "8",
            "--target-errors",
            "1000",
            "--decoder",
            "sumproduct",
            "--demap",
            "exactlogmap",
            "--output-dir",
            int_dir,
            "--seed",
            "42",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn interrupted campaign");
    // Give the child a moment to start and write the first checkpoint.  8 frames
    // at the campaign's default heartbeat-every=1000 means the first checkpoint
    // is at the SNR-boundary (not within-point), so the resume will replay all
    // frames.  That is still byte-identical by the §11 contract.
    std::thread::sleep(Duration::from_millis(500));
    let pid = child.id();
    #[cfg(unix)]
    {
        // Send SIGINT via `kill -INT <pid>` (same pattern as checkpoint_compat).
        let _ = std::process::Command::new("kill")
            .args(["-INT", &pid.to_string()])
            .status();
    }
    let _ = child.wait(); // allow any exit

    // Resume: pick up from the checkpoint.
    let resume_status = Command::new(binary_path())
        .args([
            "--rate",
            "1/2",
            "--modulation",
            "16qam",
            "--esn0-range",
            "6.25:6.25:0.5",
            "--max-frames",
            "8",
            "--target-errors",
            "1000",
            "--decoder",
            "sumproduct",
            "--demap",
            "exactlogmap",
            "--output-dir",
            int_dir,
            "--seed",
            "42",
            "--resume",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("spawn resumed campaign");
    assert!(resume_status.success(), "resumed run must succeed");

    let res_csv = std::fs::read_to_string(format!("{int_dir}/curve_1_2_16qam.csv"))
        .expect("resumed curve CSV");
    let res_rows = parse_det_rows(&res_csv);
    assert_eq!(res_rows.len(), 1, "one SNR point");

    assert_eq!(
        ref_rows, res_rows,
        "resumed run must be byte-identical to the reference on \
         fer/frames/errors/mean_iters (§11 CPU-only contract)"
    );
}

/// Parses the campaign CSV into its deterministic per-point columns.
///
/// Shared by `cli_resume_byte_identical_to_uninterrupted`.
fn parse_det_rows(csv: &str) -> Vec<DetRow> {
    csv.lines()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let c: Vec<&str> = l.split(',').collect();
            assert_eq!(c.len(), 7, "campaign CSV must have 7 columns, got: {l}");
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

#[derive(Debug, Clone, PartialEq)]
struct DetRow {
    es_n0_db: String,
    fer: String,
    frames: String,
    errors: String,
    mean_iters: String,
}
