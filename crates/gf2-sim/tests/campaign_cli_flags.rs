//! Process-level CLI contract of the `dvb_t2_awgn_campaign` binary: flag
//! rejection, the curve and calibration CSV schema, `tracing.jsonl` events,
//! and checkpoint resume.

use std::path::PathBuf;
use std::process::{Command, Stdio};

use gf2_core::test_scratch::{scratch, Scratch};

/// A campaign output directory that does not exist yet and is removed, with
/// the scratch root holding it, when the returned handle drops.
fn output_dir(label: &str) -> (Scratch, String) {
    let root = scratch(&format!("gf2-{label}"));
    let path = root
        .join("out")
        .to_str()
        .expect("scratch paths are UTF-8")
        .to_owned();
    (root, path)
}

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

#[cfg(not(feature = "hip"))]
#[test]
fn cli_gpu_on_default_build_emits_clear_error() {
    let (_scratch, out_dir) = output_dir("cli-gpu-default");
    let out = Command::new(binary_path())
        .args(base_args(&out_dir))
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

#[test]
fn cli_strict_gpu_without_gpu_is_rejected() {
    let (_scratch, out_dir) = output_dir("cli-strict-no-gpu");
    let out = Command::new(binary_path())
        .args(base_args(&out_dir))
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
    let (_scratch, out_dir) = output_dir("cli-reject-decoder");
    let out = Command::new(binary_path())
        .args(base_args(&out_dir))
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
    let (_scratch, out_dir) = output_dir("cli-reject-demap");
    let out = Command::new(binary_path())
        .args(base_args(&out_dir))
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
    let (_scratch, out_dir) = output_dir("cli-reject-nms");
    let out = Command::new(binary_path())
        .args(base_args(&out_dir))
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
    let (_scratch, out_dir) = output_dir("cli-reject-calib-range");
    let out = Command::new(binary_path())
        .args(base_args(&out_dir))
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

#[test]
#[ignore = "sim: full-codec subprocess run for end-to-end CSV-schema + tracing.jsonl acceptance"]
fn cli_minimal_valid_run_writes_curve_csv() {
    let (_out_dir_scratch, out_dir) = output_dir("cli-minimal-run");
    let out = Command::new(binary_path())
        .args(base_args(&out_dir))
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

#[test]
#[ignore = "sim: --calibrate subprocess run for calibration CSV-schema + tracing.jsonl acceptance"]
fn cli_calibrate_writes_calibration_csv() {
    let (_out_dir_scratch, out_dir) = output_dir("cli-calibrate");
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
            &out_dir,
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
    assert_eq!(rows.len(), 3, "default calibration bracket produces 3 rows");
    for row in &rows {
        assert_eq!(
            row.split(',').count(),
            7,
            "each calibration row has 7 columns"
        );
        assert_eq!(
            row.split(',').nth(3),
            Some("4"),
            "calibration frames column must equal --calibrate-frames"
        );
    }

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

#[test]
#[ignore = "sim: kill/resume campaign subprocess smoke for checkpoint byte-identity"]
fn cli_resume_byte_identical_to_uninterrupted() {
    use std::time::Duration;

    let (_ref_dir_scratch, ref_dir) = output_dir("cli-resume-ref");
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
            &ref_dir,
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

    let (_int_dir_scratch, int_dir) = output_dir("cli-resume-int");
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
            &int_dir,
            "--seed",
            "42",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn interrupted campaign");
    std::thread::sleep(Duration::from_millis(500));
    let pid = child.id();
    #[cfg(unix)]
    {
        let _ = std::process::Command::new("kill")
            .args(["-INT", &pid.to_string()])
            .status();
    }
    let _ = child.wait();

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
            &int_dir,
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

#[test]
#[ignore = "sim: multi-point checkpointed run asserting live snr_point_completed ordering"]
fn cli_snr_point_completed_emitted_live_during_sweep() {
    let (_out_dir_scratch, out_dir) = output_dir("cli-live-completed");
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
            &out_dir,
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

    let completed = event_line_indices(&jsonl, "snr_point_completed");
    assert_eq!(
        completed.len(),
        2,
        "two SNR points must each emit exactly one snr_point_completed; got {}",
        completed.len()
    );

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

#[test]
#[ignore = "sim: multi-SNR kill/resume campaign subprocess for checkpoint byte-identity"]
fn cli_multi_snr_resume_skips_completed_points() {
    use std::time::Duration;

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

    let (_ref_dir_scratch, ref_dir) = output_dir("cli-multi-resume-ref");
    let ref_status = Command::new(binary_path())
        .args(common(&ref_dir))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("spawn reference multi-point campaign");
    assert!(ref_status.success(), "reference run must succeed");
    let ref_csv = std::fs::read_to_string(format!("{ref_dir}/curve_1_2_16qam.csv"))
        .expect("reference curve CSV");
    let ref_rows = parse_det_rows(&ref_csv);
    assert_eq!(ref_rows.len(), 3, "three SNR points in the reference");

    let (_int_dir_scratch, int_dir) = output_dir("cli-multi-resume-int");
    let first_ckpt = format!("{int_dir}/checkpoints/snr_0000.json");
    let final_csv = format!("{int_dir}/curve_1_2_16qam.csv");
    let mut child = Command::new(binary_path())
        .args(common(&int_dir))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn interrupted multi-point campaign");

    // The 120 s bound is a liveness backstop sized for slow CI runners.
    let mut saw_first_ckpt = false;
    let mut sweep_finished = false;
    let deadline = std::time::Instant::now() + Duration::from_secs(120);
    while std::time::Instant::now() < deadline {
        if std::path::Path::new(&final_csv).exists() {
            sweep_finished = true;
            break;
        }
        if std::path::Path::new(&first_ckpt).exists() {
            saw_first_ckpt = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(
        !sweep_finished,
        "the sweep finished before it could be interrupted: the per-point work \
         is too fast — raise --max-frames / --target-errors"
    );
    assert!(
        saw_first_ckpt,
        "first SNR point's checkpoint ({first_ckpt}) did not appear within 120 s \
         while the sweep was still running — the checkpoint writer is stuck"
    );

    let ckpt_before = std::fs::read(&first_ckpt).expect("read snr_0000.json before interrupt");

    let pid = child.id();
    #[cfg(unix)]
    {
        let _ = std::process::Command::new("kill")
            .args(["-INT", &pid.to_string()])
            .status();
    }
    let _ = child.wait();

    assert!(
        std::path::Path::new(&first_ckpt).exists(),
        "completed point's checkpoint must survive the interrupt"
    );

    // The resume appends to this file; read the interrupted run's events first.
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

    let resume_status = Command::new(binary_path())
        .args(common(&int_dir))
        .arg("--resume")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("spawn resumed multi-point campaign");
    assert!(resume_status.success(), "resumed run must succeed");

    let ckpt_after = std::fs::read(&first_ckpt).expect("read snr_0000.json after resume");
    assert_eq!(
        ckpt_before, ckpt_after,
        "resume must NOT recompute the already-completed first SNR point \
         (its checkpoint file must be byte-identical across the restart)"
    );

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

    let res_csv = std::fs::read_to_string(&final_csv).expect("resumed curve CSV");
    let res_rows = parse_det_rows(&res_csv);
    assert_eq!(res_rows.len(), 3, "three SNR points after resume");
    assert_eq!(
        ref_rows, res_rows,
        "resumed multi-point run must be byte-identical to the reference on \
         es_n0_db/fer/frames/errors/mean_iters (§11 CPU-only contract)"
    );
}
