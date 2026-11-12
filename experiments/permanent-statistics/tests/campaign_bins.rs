mod common;
use std::{fs, path::Path, process::Command};
fn campaign(config: &Path, output: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_permanent_campaign"))
        .arg("--config")
        .arg(config)
        .arg("--output")
        .arg(output)
        .args(args)
        .output()
        .unwrap()
}
#[test]
fn validation_check_parses_and_reports_without_touching_a_stream() {
    let validation = || Command::new(env!("CARGO_BIN_EXE_permanent_validation"));
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config.json");
    let parameters = common::config();
    fs::write(&config, serde_json::to_vec(&parameters).unwrap()).unwrap();
    let report = validation()
        .current_dir(dir.path())
        .arg("--check")
        .arg(&config)
        .output()
        .unwrap();
    assert_eq!(report.status.code(), Some(0));
    let stdout = String::from_utf8(report.stdout).unwrap();
    assert!(stdout.contains(&parameters.digest().unwrap().to_string()));
    assert!(stdout.contains(&parameters.campaign_id));
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);

    let missing = dir.path().join("missing.json");
    let refused = validation().arg("--check").arg(&missing).output().unwrap();
    assert_eq!(refused.status.code(), Some(1));
    let mixed = validation()
        .arg("--check")
        .arg(&config)
        .args(["--workers", "1"])
        .output()
        .unwrap();
    assert_eq!(mixed.status.code(), Some(2));
}
#[test]
fn campaign_and_dataset_exit_codes_follow_bounded_progress() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config.json");
    fs::write(&config, serde_json::to_vec(&common::config()).unwrap()).unwrap();
    let output = dir.path().join("dataset");
    assert_eq!(
        campaign(
            &config,
            &output,
            &["--q", "3", "--n", "2", "--max-shards", "1"]
        )
        .status
        .code(),
        Some(10)
    );
    for q in ["3", "5", "7"] {
        let result = campaign(&config, &output, &["--q", q, "--n", "2", "--workers", "4"]);
        assert_eq!(
            result.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    for command in ["finalize", "verify"] {
        let result = Command::new(env!("CARGO_BIN_EXE_permanent_dataset"))
            .arg(command)
            .arg(&output)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    assert_eq!(
        campaign(&config, &output, &["--q", "3", "--n", "99"])
            .status
            .code(),
        Some(2)
    );
    assert_eq!(
        campaign(&config, &output, &["--q", "4", "--n", "2"])
            .status
            .code(),
        Some(2)
    );
    assert_eq!(
        campaign(
            &config,
            &output,
            &["--q", "3", "--n", "2", "--on-unavailable", "auto"]
        )
        .status
        .code(),
        Some(2)
    );
}
#[test]
fn default_unavailable_refusal_writes_nothing_and_halt_retains_zero_evidence() {
    use permanent_statistics::campaign::{backend_available, Backend};
    if backend_available(Backend::Accelerator) {
        return;
    }
    let mut configuration = common::config();
    for g in &mut configuration.cell_groups {
        g.backend = Backend::Accelerator;
        g.accelerator_per_matrix_us = Some(1.0);
    }
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config.json");
    fs::write(&config, serde_json::to_vec(&configuration).unwrap()).unwrap();
    let output = dir.path().join("dataset");
    assert_eq!(
        campaign(&config, &output, &["--q", "3", "--n", "2"])
            .status
            .code(),
        Some(1)
    );
    assert!(!output.exists());
    assert_eq!(
        campaign(
            &config,
            &output,
            &["--q", "3", "--n", "2", "--on-unavailable", "halt"]
        )
        .status
        .code(),
        Some(0)
    );
    permanent_statistics::campaign::dataset::finalize(&output).unwrap();
    permanent_statistics::campaign::dataset::verify(&output).unwrap();
    for cell in configuration.cells() {
        assert!(common::shards(&output, cell.q, cell.n).is_empty());
    }
    assert!(fs::read_to_string(output.join("summary.csv"))
        .unwrap()
        .lines()
        .skip(1)
        .all(|l| l.contains("halted,backend_unavailable")));
}
#[test]
fn validation_exit_codes_adopt_terminal_anchors() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config.json");
    let output = dir.path().join("validation");
    fs::write(&config, serde_json::to_vec(&common::config()).unwrap()).unwrap();
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_permanent_validation"))
            .arg("--config")
            .arg(&config)
            .arg("--output")
            .arg(&output)
            .args(args)
            .output()
            .unwrap()
    };
    let bounded = run(&["--max-anchors", "1"]);
    assert_eq!(bounded.status.code(), Some(10));
    let executable = experiments_common::Sha256Digest::of_file(Path::new(env!(
        "CARGO_BIN_EXE_permanent_validation"
    )))
    .unwrap();
    assert!(
        String::from_utf8(bounded.stdout)
            .unwrap()
            .starts_with(&format!("sha256 {executable} ")),
        "a validation run prints its executable digest first"
    );
    assert_eq!(run(&[]).status.code(), Some(0));
    assert_eq!(run(&["--workers", "0"]).status.code(), Some(2));
}
