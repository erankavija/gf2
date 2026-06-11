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
//! encoder / simulation work), so they stay well within the fast-tier budget.
//! The single end-to-end acceptance test runs 4 frames at one SNR point.

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
/// working). 4 frames × 1 SNR point keeps this in the fast tier.
#[test]
fn cli_minimal_valid_run_writes_curve_csv() {
    let out_dir = "/tmp/dvb_d2_cli_minimal_run";
    let _ = std::fs::remove_dir_all(out_dir);
    let out = Command::new(binary_path())
        .args(base_args(out_dir))
        .args(["--max-frames", "4", "--target-errors", "1000", "--seed", "7"])
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
    assert_eq!(row.split(',').nth(3), Some("4"), "frames column = max_frames");
}
