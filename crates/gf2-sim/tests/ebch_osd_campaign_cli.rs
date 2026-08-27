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
    workers_campaign_args(dir, max_samples, target_block_errors, "2")
}

fn workers_campaign_args(
    dir: &Path,
    max_samples: &str,
    target_block_errors: &str,
    workers: &str,
) -> Vec<String> {
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
        "--workers".to_owned(),
        workers.to_owned(),
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
    assert!(usage.contains("--workers N"), "usage: {usage}");
    assert!(usage.contains("ascending Hamming weight"), "usage: {usage}");
    assert!(usage.contains("source-undefined"), "usage: {usage}");
}

/// The worker count is invocation-local provenance, not campaign
/// configuration: the same cell evidence comes out at any worker count, and
/// the resolved count is always part of the recorded argument vector — the
/// default included, so the record reproduces the run.
#[test]
fn worker_count_is_recorded_provenance_and_leaves_cell_evidence_unchanged() {
    let defaulted_dir = TempDir::new("workers-default");
    let defaulted_args = vec![
        "--checkpoint".to_owned(),
        defaulted_dir
            .path()
            .join("checkpoint.json")
            .display()
            .to_string(),
        "--receipt".to_owned(),
        defaulted_dir
            .path()
            .join("receipt.json")
            .display()
            .to_string(),
        "--max-samples".to_owned(),
        "8".to_owned(),
        "--target-block-errors".to_owned(),
        "1000000".to_owned(),
        "--seed".to_owned(),
        "42".to_owned(),
    ];
    let defaulted = run(&defaulted_args);
    assert!(
        defaulted.status.success(),
        "defaulted worker count failed:\n{}",
        String::from_utf8_lossy(&defaulted.stderr)
    );
    let defaulted_receipt = receipt(&defaulted_dir.path().join("receipt.json"));
    let recorded = &defaulted_receipt.invocation_history[0]
        .provenance
        .invocation;
    let available = std::thread::available_parallelism()
        .expect("host reports available parallelism")
        .to_string();
    assert_eq!(
        &recorded[recorded.len() - 2..],
        ["--workers".to_owned(), available],
        "the defaulted worker count must appear in the recorded argument vector"
    );

    // The single-worker run is the reference the invariance contract is stated
    // against; the real eBCH/OSD evaluator must reproduce it at every count.
    let reference_dir = TempDir::new("workers-1");
    let reference_run = run(&workers_campaign_args(
        reference_dir.path(),
        "8",
        "1000000",
        "1",
    ));
    assert!(
        reference_run.status.success(),
        "single-worker run failed:\n{}",
        String::from_utf8_lossy(&reference_run.stderr)
    );
    let reference = receipt(&reference_dir.path().join("receipt.json"));
    assert_eq!(reference.cell_results[0].samples, 8);

    for workers in ["2", "8", "24"] {
        let dir = TempDir::new(&format!("workers-{workers}"));
        let output = run(&workers_campaign_args(dir.path(), "8", "1000000", workers));
        assert!(
            output.status.success(),
            "{workers}-worker run failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let observed = receipt(&dir.path().join("receipt.json"));
        assert_eq!(
            serde_json::to_vec(&observed.cell_results[0]).unwrap(),
            serde_json::to_vec(&reference.cell_results[0]).unwrap(),
            "{workers} workers must not change a cell's sampled evidence"
        );
        assert_eq!(
            observed.configuration_hash, reference.configuration_hash,
            "worker count must stay out of the campaign configuration identity"
        );
    }

    assert_eq!(
        serde_json::to_vec(&defaulted_receipt.cell_results[0]).unwrap(),
        serde_json::to_vec(&reference.cell_results[0]).unwrap(),
        "the defaulted worker count must produce the same cell evidence"
    );
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

    let uninterrupted_dir = TempDir::new("uninterrupted-prefix");
    let uninterrupted_output = run(&campaign_args(uninterrupted_dir.path(), "3", "1000000"));
    assert!(
        uninterrupted_output.status.success(),
        "uninterrupted bounded run failed:\n{}",
        String::from_utf8_lossy(&uninterrupted_output.stderr)
    );
    let uninterrupted = receipt(&uninterrupted_dir.path().join("receipt.json"));
    let mut resumed_cell = resumed.cell_results[1].clone();
    let mut uninterrupted_cell = uninterrupted.cell_results[0].clone();
    resumed_cell.invocation_index = None;
    uninterrupted_cell.invocation_index = None;
    assert_eq!(
        serde_json::to_vec(&resumed_cell).unwrap(),
        serde_json::to_vec(&uninterrupted_cell).unwrap(),
        "resuming must preserve the byte identity of cumulative cell evidence"
    );
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
