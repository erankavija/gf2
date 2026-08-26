//! Process-level contract tests for the pinned eBCH OSD campaign executable.
//!
//! The fixture deliberately uses tiny per-invocation sample bounds.  It checks
//! that the executable maps the pinned cells into the shared protocol, writes
//! both durable outputs, resumes the interrupted cell without replaying a
//! terminal cell, and rejects malformed command lines before doing campaign
//! work.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use gf2_sim::osd_campaign::{OsdCampaignReceipt, OsdCellTermination};

struct TempDir(PathBuf);

impl TempDir {
    fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "gf2-ebch-osd-cli-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos()
        ));
        fs::create_dir_all(&path).expect("create temporary campaign directory");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn binary_path() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_ebch_osd_awgn_campaign"))
}

fn campaign_args(dir: &Path, max_samples: &str, target_block_errors: &str) -> Vec<String> {
    vec![
        "--checkpoint".to_owned(),
        dir.join("checkpoint.json").display().to_string(),
        "--receipt".to_owned(),
        dir.join("receipt.json").display().to_string(),
        "--max-samples".to_owned(),
        max_samples.to_owned(),
        "--target-block-errors".to_owned(),
        target_block_errors.to_owned(),
        "--seed".to_owned(),
        "42".to_owned(),
    ]
}

fn run(args: &[String]) -> Output {
    Command::new(binary_path())
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("spawn ebch_osd_awgn_campaign")
}

fn receipt(path: &Path) -> OsdCampaignReceipt {
    serde_json::from_slice(&fs::read(path).expect("read campaign receipt"))
        .expect("parse versioned OSD receipt")
}

#[test]
fn usage_names_pinned_configuration_and_output_paths() {
    let output = run(&["--help".to_owned()]);
    assert!(output.status.success());
    let usage = String::from_utf8_lossy(&output.stdout);
    assert!(usage.contains("eBCH(128,64,22)"), "usage: {usage}");
    assert!(usage.contains("BI-AWGN/BPSK"), "usage: {usage}");
    assert!(usage.contains("BER vs Eb/N0"), "usage: {usage}");
    assert!(usage.contains("--checkpoint PATH"), "usage: {usage}");
    assert!(usage.contains("--receipt PATH"), "usage: {usage}");
    assert!(usage.contains("--target-block-errors N"), "usage: {usage}");
    assert!(usage.contains("ascending Hamming weight"), "usage: {usage}");
    assert!(usage.contains("source-undefined"), "usage: {usage}");
}

#[test]
fn campaign_maps_pinned_cells_and_resumes_into_the_same_receipt() {
    let dir = TempDir::new("resume");
    let first = run(&campaign_args(dir.path(), "1", "1000000"));
    assert!(
        first.status.success(),
        "first bounded run failed:\n{}",
        String::from_utf8_lossy(&first.stderr)
    );

    let checkpoint_path = dir.path().join("checkpoint.json");
    let receipt_path = dir.path().join("receipt.json");
    assert!(checkpoint_path.is_file());
    assert!(receipt_path.is_file());
    let first_receipt = receipt(&receipt_path);
    assert_eq!(first_receipt.cells.len(), 14);
    assert_eq!(first_receipt.cells[0].id.as_str(), "order-2-point-00");
    assert_eq!(first_receipt.cells[0].osd_order, 2);
    assert_eq!(first_receipt.cells[0].eb_n0_db, 1.55);
    assert_eq!(first_receipt.cells[7].id.as_str(), "order-1-point-00");
    assert_eq!(first_receipt.cells[7].osd_order, 1);
    assert_eq!(first_receipt.cell_results.len(), 1);
    assert!(matches!(
        first_receipt.cell_results[0].termination,
        OsdCellTermination::Interrupted
    ));
    assert_eq!(first_receipt.cell_results[0].samples, 1);
    assert_eq!(first_receipt.invocation_history.len(), 1);
    assert_eq!(
        &first_receipt.invocation_history[0].provenance.invocation[1..],
        campaign_args(dir.path(), "1", "1000000")
    );

    let second = run(&campaign_args(dir.path(), "2", "1000000"));
    assert!(
        second.status.success(),
        "resumed bounded run failed:\n{}",
        String::from_utf8_lossy(&second.stderr)
    );
    let resumed = receipt(&receipt_path);
    assert_eq!(resumed.cell_results.len(), 2);
    assert!(matches!(
        resumed.cell_results[0].termination,
        OsdCellTermination::Interrupted
    ));
    assert_eq!(resumed.cell_results[1].cell.id, resumed.cells[0].id);
    assert_eq!(resumed.cell_results[1].samples, 3);
    assert_eq!(resumed.cell_results[1].seed, resumed.cell_results[0].seed);
    assert_eq!(resumed.invocation_history.len(), 2);
    assert_eq!(
        &resumed.invocation_history[0].provenance.invocation[1..],
        campaign_args(dir.path(), "1", "1000000")
    );
    assert_eq!(
        &resumed.invocation_history[1].provenance.invocation[1..],
        campaign_args(dir.path(), "2", "1000000")
    );
    assert_eq!(resumed.cell_results[0].invocation_index, Some(0));
    assert_eq!(resumed.cell_results[1].invocation_index, Some(1));
}

#[test]
fn recorded_cpu_model_matches_runtime_observed_processor_identity() {
    let dir = TempDir::new("cpu-identity");
    let output = run(&campaign_args(dir.path(), "1", "1000000"));
    assert!(
        output.status.success(),
        "bounded campaign failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let receipt = receipt(&dir.path().join("receipt.json"));
    let cpuinfo = fs::read_to_string("/proc/cpuinfo").expect("Linux processor identity");
    let observed_model = cpuinfo
        .lines()
        .find_map(|line| line.strip_prefix("model name\t: "))
        .expect("/proc/cpuinfo model name");

    assert_eq!(receipt.provenance.runtime.cpu_model, observed_model);
    assert_ne!(
        receipt.provenance.runtime.cpu_model,
        format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
    );
    let physical_cores = receipt
        .provenance
        .runtime
        .cpu_physical_cores
        .expect("physical core count");
    let logical_threads = receipt
        .provenance
        .runtime
        .cpu_logical_threads
        .expect("logical thread count");
    assert!(physical_cores > 0);
    assert!(logical_threads >= physical_cores);
}

#[test]
fn invalid_input_returns_nonzero_without_emitting_a_receipt() {
    let dir = TempDir::new("invalid");
    let args = vec![
        "--checkpoint".to_owned(),
        dir.path().join("checkpoint.json").display().to_string(),
        "--receipt".to_owned(),
        dir.path().join("receipt.json").display().to_string(),
        "--max-samples".to_owned(),
        "0".to_owned(),
    ];
    let output = run(&args);
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("max-samples"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!dir.path().join("checkpoint.json").exists());
    assert!(!dir.path().join("receipt.json").exists());
}
