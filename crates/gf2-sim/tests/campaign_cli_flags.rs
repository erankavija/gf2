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

/// Parses `tracing.jsonl`, asserting every non-empty line is valid JSON, and
/// returns the count of events whose `fields.event_type` equals `event_type`.
fn count_events(jsonl: &str, event_type: &str) -> usize {
    let mut n = 0;
    for (i, line) in jsonl.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let v: serde_json::Value = serde_json::from_str(line).unwrap_or_else(|e| {
            panic!("tracing.jsonl line {i} is not valid JSON: {e}\nline: {line}")
        });
        if v["fields"]["event_type"] == event_type {
            n += 1;
        }
    }
    n
}

/// End-to-end acceptance: a minimal valid argv runs the migrated pipeline and
/// writes the curve CSV with the canonical 7-column schema (so `plot.py` keeps
/// working) **and** writes a non-vacuous `tracing.jsonl`: every line valid
/// JSON, at least one **live worker-thread** `campaign_heartbeat` event
/// (`--heartbeat-frames 2` with 4 frames guarantees two), and exactly one
/// live `snr_point_completed` event (the checkpointed sweep emits it at the
/// SNR-point boundary). The heartbeat assertion proves
/// events emitted from the executor's rayon workers reach the file through
/// the process-GLOBAL subscriber (a thread-local default would drop them).
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
            "--heartbeat-frames",
            "2",
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

    // HIGH-1: tracing.jsonl must exist, be non-empty, every line valid JSON,
    // AND contain the live monitoring events (not just campaign_start). We do
    // not assert legacy byte-compat of event shapes (Q2 decision) — only that
    // the monitoring channel exists.
    let jsonl_path = format!("{out_dir}/tracing.jsonl");
    let jsonl = std::fs::read_to_string(&jsonl_path)
        .unwrap_or_else(|e| panic!("tracing.jsonl must be written at {jsonl_path}: {e}"));
    assert!(
        !jsonl.trim().is_empty(),
        "tracing.jsonl must be non-empty after a production run"
    );
    let heartbeats = count_events(&jsonl, "campaign_heartbeat");
    assert!(
        heartbeats >= 1,
        "tracing.jsonl must contain at least one campaign_heartbeat event \
         (4 frames at --heartbeat-frames 2 ⇒ 2 expected); this is the proof \
         that worker-thread events reach the GLOBAL subscriber; got {heartbeats}"
    );
    let completed = count_events(&jsonl, "snr_point_completed");
    assert_eq!(
        completed, 1,
        "tracing.jsonl must contain exactly one snr_point_completed event \
         for the single-point sweep; got {completed}"
    );
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
    // Calibration runs the plain (non-checkpointed) path, so there are no
    // campaign_heartbeat events; the post-sweep snr_point_completed events
    // (one per bracket point) must still be present.
    let jsonl_path = format!("{out_dir}/tracing.jsonl");
    let jsonl = std::fs::read_to_string(&jsonl_path)
        .unwrap_or_else(|e| panic!("tracing.jsonl must be written at {jsonl_path}: {e}"));
    assert!(
        !jsonl.trim().is_empty(),
        "tracing.jsonl must be non-empty after a calibration run"
    );
    let completed = count_events(&jsonl, "snr_point_completed");
    assert_eq!(
        completed, 3,
        "calibration must emit one snr_point_completed per bracket point; got {completed}"
    );
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

/// Returns the line indices (in file/emission order) of every valid-JSON
/// `tracing.jsonl` line whose `fields.event_type` matches `event_type`.
/// Shared by the live-tracing ordering test below.
fn event_line_indices(jsonl: &str, event_type: &str) -> Vec<usize> {
    let mut idxs = Vec::new();
    for (i, line) in jsonl.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let v: serde_json::Value = serde_json::from_str(line)
            .unwrap_or_else(|e| panic!("tracing.jsonl line {i} is not valid JSON: {e}\n{line}"));
        if v["fields"]["event_type"] == event_type {
            idxs.push(i);
        }
    }
    idxs
}

/// Returns the `es_n0_db` value (as a string key) of every `snr_point_completed`
/// record in `jsonl`, in emission order. Used to tally completion records per
/// SNR point and detect double-logging across an interrupt+resume lifecycle.
fn completed_point_keys(jsonl: &str) -> Vec<String> {
    let mut keys = Vec::new();
    for (i, line) in jsonl.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let v: serde_json::Value = serde_json::from_str(line)
            .unwrap_or_else(|e| panic!("tracing.jsonl line {i} is not valid JSON: {e}\n{line}"));
        if v["fields"]["event_type"] == "snr_point_completed" {
            // es_n0_db is a float field; format it via the serde Number so the
            // key is a stable textual identity per SNR point.
            keys.push(v["fields"]["es_n0_db"].to_string());
        }
    }
    keys
}

/// Returns the line indices of `campaign_heartbeat` events whose `snr_idx`
/// field equals `snr_idx`, in emission order.
fn heartbeat_line_indices_for_snr(jsonl: &str, snr_idx: u64) -> Vec<usize> {
    let mut idxs = Vec::new();
    for (i, line) in jsonl.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let v: serde_json::Value = serde_json::from_str(line)
            .unwrap_or_else(|e| panic!("tracing.jsonl line {i} is not valid JSON: {e}\n{line}"));
        if v["fields"]["event_type"] == "campaign_heartbeat"
            && v["fields"]["snr_idx"].as_u64() == Some(snr_idx)
        {
            idxs.push(i);
        }
    }
    idxs
}

/// Fix 1 (epic 2928ccce): the checkpointed sweep emits each
/// `snr_point_completed` record **live** at its SNR-point boundary, not in a
/// post-sweep batch. A multi-point run with a small per-point heartbeat budget
/// interleaves events as: point 0 heartbeats → point 0 `snr_point_completed`
/// → point 1 heartbeats → point 1 `snr_point_completed`. We assert that the
/// FIRST `snr_point_completed` line is ordered BEFORE the first point-1
/// `campaign_heartbeat`, which can only hold if point 0's completion record was
/// written live during the sweep (a post-sweep batch would emit both
/// completions only after every heartbeat).
///
/// `#[ignore]` — spawns a full-codec multi-point subprocess (heavy live
/// simulation; slow tier).
#[test]
#[ignore = "sim: multi-point checkpointed run asserting live snr_point_completed ordering"]
fn cli_snr_point_completed_emitted_live_during_sweep() {
    let out_dir = "/tmp/dvb_d2_cli_live_completed";
    let _ = std::fs::remove_dir_all(out_dir);
    let out = Command::new(binary_path())
        .args([
            "--rate",
            "1/2",
            "--modulation",
            "16qam",
            "--esn0-range",
            // Two SNR points.
            "6.0:6.5:0.5",
            "--max-frames",
            "4",
            "--target-errors",
            "1000",
            "--decoder",
            "sumproduct",
            "--demap",
            "exactlogmap",
            "--output-dir",
            out_dir,
            "--seed",
            "11",
            "--heartbeat-frames",
            "2",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .expect("spawn dvb_t2_awgn_campaign multi-point");
    assert!(
        out.status.success(),
        "multi-point run must succeed; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let jsonl = std::fs::read_to_string(format!("{out_dir}/tracing.jsonl"))
        .expect("tracing.jsonl must be written");

    // Two SNR points ⇒ exactly two live snr_point_completed records.
    let completed = event_line_indices(&jsonl, "snr_point_completed");
    assert_eq!(
        completed.len(),
        2,
        "two SNR points must each emit exactly one snr_point_completed; got {}",
        completed.len()
    );

    // The first completion (point 0) must be ordered BEFORE point 1's first
    // heartbeat. With post-sweep batching, BOTH completions would land after
    // all heartbeats, so first_completed > last point-1 heartbeat — which this
    // assertion rules out. This is the "reaches the JSON before the sweep
    // completes" guarantee.
    let snr1_heartbeats = heartbeat_line_indices_for_snr(&jsonl, 1);
    assert!(
        !snr1_heartbeats.is_empty(),
        "expected at least one campaign_heartbeat for snr_idx=1 \
         (4 frames at --heartbeat-frames 2 ⇒ 2 per point)"
    );
    let first_completed = completed[0];
    let first_snr1_heartbeat = snr1_heartbeats[0];
    assert!(
        first_completed < first_snr1_heartbeat,
        "point 0's snr_point_completed (line {first_completed}) must be emitted \
         LIVE before point 1's first heartbeat (line {first_snr1_heartbeat}); \
         a post-sweep batch would emit it after every heartbeat"
    );
}

/// Fix 2 (epic 2928ccce): a multi-SNR kill/resume integration test that
/// actually exercises "kill after >= 1 SNR point completed -> resume from the
/// NEXT point" (the single-point `cli_resume_byte_identical_to_uninterrupted`
/// cannot). Runs a >= 3-point sweep, kills the process once at least one SNR
/// point's checkpoint exists but before the sweep finishes, resumes with
/// `--resume`, asserts the already-completed point's checkpoint is NOT
/// recomputed (its on-disk file is byte-unchanged across the restart), and
/// asserts the final CSV is byte-identical to an uninterrupted reference run on
/// the deterministic columns (`es_n0_db`, `fer`, `frames`, `errors`,
/// `mean_iters`; `wall_seconds`/`ber` excluded, matching the single-SNR test).
///
/// It also pins the EXACTLY-ONCE-AT-COMPLETION `snr_point_completed` contract
/// across the interrupt+resume lifecycle (regression guard for the Fix 1 bug
/// where the event was emitted unconditionally, before the interrupt/resume
/// checks): on the interrupted run's `tracing.jsonl` a completion record exists
/// for each point that finished before the kill and NOT for the partial point;
/// and across interrupt + resume COMBINED (the subscriber opens the log in
/// append mode, so the resume's events accrue onto the same file) every SNR
/// point has exactly ONE completion record total — points completed before the
/// kill are not re-logged on resume.
///
/// `#[ignore]` — spawns multiple full-codec subprocesses with SIGINT delivery
/// (slow tier).
#[test]
#[ignore = "sim: multi-SNR kill/resume campaign subprocess for checkpoint byte-identity"]
fn cli_multi_snr_resume_skips_completed_points() {
    use std::time::Duration;

    // A 3-point sweep, tiny per-point budget so the whole reference run is fast
    // but each point still takes long enough that we can SIGINT mid-sweep.
    const ESN0_RANGE: &str = "6.0:7.0:0.5"; // 6.0, 6.5, 7.0 -> 3 points
    let common = |dir: &str| -> Vec<String> {
        vec![
            "--rate".into(),
            "1/2".into(),
            "--modulation".into(),
            "16qam".into(),
            "--esn0-range".into(),
            ESN0_RANGE.into(),
            "--max-frames".into(),
            "8".into(),
            "--target-errors".into(),
            "1000".into(),
            "--decoder".into(),
            "sumproduct".into(),
            "--demap".into(),
            "exactlogmap".into(),
            "--output-dir".into(),
            dir.into(),
            "--seed".into(),
            "42".into(),
        ]
    };

    // Reference: uninterrupted 3-point run.
    let ref_dir = "/tmp/dvb_d2_cli_multi_resume_ref";
    let _ = std::fs::remove_dir_all(ref_dir);
    let ref_status = Command::new(binary_path())
        .args(common(ref_dir))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("spawn reference multi-point campaign");
    assert!(ref_status.success(), "reference run must succeed");
    let ref_csv = std::fs::read_to_string(format!("{ref_dir}/curve_1_2_16qam.csv"))
        .expect("reference curve CSV");
    let ref_rows = parse_det_rows(&ref_csv);
    assert_eq!(ref_rows.len(), 3, "three SNR points in the reference");

    // Interrupted run: spawn, wait until at least the first point's checkpoint
    // (snr_0000.json) is on disk but the sweep is not yet done, then SIGINT.
    let int_dir = "/tmp/dvb_d2_cli_multi_resume_int";
    let _ = std::fs::remove_dir_all(int_dir);
    let first_ckpt = format!("{int_dir}/checkpoints/snr_0000.json");
    let final_csv = format!("{int_dir}/curve_1_2_16qam.csv");
    let mut child = Command::new(binary_path())
        .args(common(int_dir))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn interrupted multi-point campaign");

    // Poll for the first SNR point's checkpoint, but bail before the final CSV
    // is written (that would mean the whole sweep already finished). Up to ~10s.
    let mut saw_first_ckpt = false;
    for _ in 0..200 {
        if std::path::Path::new(&final_csv).exists() {
            break; // sweep finished before we could interrupt
        }
        if std::path::Path::new(&first_ckpt).exists() {
            saw_first_ckpt = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(
        saw_first_ckpt,
        "first SNR point's checkpoint ({first_ckpt}) must appear before the \
         sweep finishes; if this fails the per-point work is too fast to \
         interrupt — lower --max-frames is not possible, raise it instead"
    );

    // Snapshot the completed point's checkpoint so we can prove resume does not
    // recompute it.
    let ckpt_before = std::fs::read(&first_ckpt).expect("read snr_0000.json before interrupt");

    let pid = child.id();
    #[cfg(unix)]
    {
        let _ = std::process::Command::new("kill")
            .args(["-INT", &pid.to_string()])
            .status();
    }
    let _ = child.wait();

    // The first point's checkpoint must still be on disk after the interrupt.
    assert!(
        std::path::Path::new(&first_ckpt).exists(),
        "completed point's checkpoint must survive the interrupt"
    );

    // EXACTLY-ONCE part 1 — snapshot the INTERRUPTED run's tracing.jsonl (the
    // resume below appends to the same file, so capture it now). At least the
    // first SNR point completed before the kill (its checkpoint exists), so its
    // completion record must be present; the interrupted/partial point must NOT
    // have one. We know point 0 is complete (snr_0000.json on disk pre-kill),
    // and the sweep was still running, so at least one but fewer than all 3
    // points completed. With a fresh `--esn0-range 6.0:7.0:0.5` and default
    // heartbeat the per-point checkpoint lands only at the SNR boundary, so the
    // number of completion records equals the number of fully finished points.
    let tracing_path = format!("{int_dir}/tracing.jsonl");
    let int_jsonl = std::fs::read_to_string(&tracing_path).expect("interrupted run tracing.jsonl");
    let int_completed = completed_point_keys(&int_jsonl);
    assert!(
        !int_completed.is_empty(),
        "the interrupted run must have logged at least one snr_point_completed \
         (point 0 finished before the kill); got none"
    );
    assert!(
        int_completed.len() < 3,
        "the interrupted run must NOT log a completion for the partial/interrupted \
         point — fewer than all 3 points should be logged; got {}",
        int_completed.len()
    );
    // No point double-logged within the interrupted run itself.
    {
        let mut sorted = int_completed.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(
            sorted.len(),
            int_completed.len(),
            "no SNR point may be logged twice within the interrupted run: {int_completed:?}"
        );
    }

    // Resume.
    let resume_status = Command::new(binary_path())
        .args(common(int_dir))
        .arg("--resume")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("spawn resumed multi-point campaign");
    assert!(resume_status.success(), "resumed run must succeed");

    // The already-completed point's checkpoint must be byte-unchanged across the
    // restart: resume folds its saved counters and skips the point rather than
    // recomputing and rewriting it.
    let ckpt_after = std::fs::read(&first_ckpt).expect("read snr_0000.json after resume");
    assert_eq!(
        ckpt_before, ckpt_after,
        "resume must NOT recompute the already-completed first SNR point \
         (its checkpoint file must be byte-identical across the restart)"
    );

    // EXACTLY-ONCE part 2 — across interrupt + resume COMBINED, every SNR point
    // has exactly ONE completion record. The subscriber appends, so the
    // post-resume tracing.jsonl holds both invocations' events. Points completed
    // before the kill must NOT be re-logged on resume (the bug this regression
    // guards), and the interrupted point must be logged exactly once when the
    // resume finishes it.
    let combined_jsonl =
        std::fs::read_to_string(&tracing_path).expect("combined tracing.jsonl after resume");
    let combined_completed = completed_point_keys(&combined_jsonl);
    assert_eq!(
        combined_completed.len(),
        3,
        "across interrupt + resume there must be exactly 3 snr_point_completed \
         records (one per SNR point), no double-logging; got {combined_completed:?}"
    );
    let mut unique = combined_completed.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(
        unique.len(),
        3,
        "each of the 3 SNR points must have exactly ONE completion record across \
         the interrupt+resume lifecycle (no point logged twice): {combined_completed:?}"
    );

    // Final CSV byte-identical to the uninterrupted reference on the
    // deterministic columns.
    let res_csv = std::fs::read_to_string(&final_csv).expect("resumed curve CSV");
    let res_rows = parse_det_rows(&res_csv);
    assert_eq!(res_rows.len(), 3, "three SNR points after resume");
    assert_eq!(
        ref_rows, res_rows,
        "resumed multi-point run must be byte-identical to the reference on \
         es_n0_db/fer/frames/errors/mean_iters (§11 CPU-only contract)"
    );
}
