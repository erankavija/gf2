use std::fs;
use std::path::Path;

use experiments_common::{RepositoryRoot, RngAlgorithm, RngIdentity, RunRecord};
use permanent_statistics::rare_event::runner::{execute_frozen_run, Configuration, RunStatus};
use permanent_statistics::sampler::FieldOrder;

fn configuration(q: FieldOrder) -> Configuration {
    Configuration {
        root_seed: 73,
        field_order: q,
        rows: 65,
        target_runs: 3,
        trajectories_per_run: 4,
        prefix_rows: 16,
        student_critical_numerator: 4_303,
        student_critical_denominator: 1_000,
    }
}

fn record(configuration: &Configuration) -> RunRecord {
    let root = RepositoryRoot::containing(Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
    RunRecord::observe(
        &root,
        configuration.digest().unwrap(),
        configuration.root_seed,
        RngIdentity {
            algorithm: RngAlgorithm::ChaCha20,
            version: "rand_chacha 0.9.0".into(),
        },
    )
    .unwrap()
}

#[test]
fn persisted_prefix_resume_matches_uninterrupted_results_in_each_field() {
    for q in [FieldOrder::F3, FieldOrder::F5, FieldOrder::F7] {
        let configuration = configuration(q);
        let record = record(&configuration);
        let uninterrupted = tempfile::tempdir().unwrap();
        let expected =
            execute_frozen_run(&configuration, &record, uninterrupted.path(), None).unwrap();
        assert_eq!(expected.status, RunStatus::Complete);
        let resumed = tempfile::tempdir().unwrap();
        let stopped = execute_frozen_run(&configuration, &record, resumed.path(), Some(2)).unwrap();
        assert_eq!(stopped.status, RunStatus::Incomplete);
        assert_eq!(stopped.completed_trajectories, 0);
        assert_eq!(stopped.completed_prefix, 32);
        assert!(stopped.summary.is_none());
        let log_prefix = fs::read(resumed.path().join("execution.jsonl")).unwrap();
        let actual = execute_frozen_run(&configuration, &record, resumed.path(), None).unwrap();
        assert_eq!(actual, expected);
        assert!(fs::read(resumed.path().join("execution.jsonl"))
            .unwrap()
            .starts_with(&log_prefix));
        assert_eq!(
            fs::read(resumed.path().join("result.json")).unwrap(),
            fs::read(uninterrupted.path().join("result.json")).unwrap()
        );
    }
}

#[test]
fn a_run_creates_its_missing_output_directories_and_resumes_there() {
    let mut configuration = configuration(FieldOrder::F3);
    configuration.rows = 3;
    configuration.prefix_rows = 2;
    let record = record(&configuration);
    let parent = tempfile::tempdir().unwrap();
    let nested = parent.path().join("a/b");
    let stopped = execute_frozen_run(&configuration, &record, &nested, Some(1)).unwrap();
    assert_eq!(stopped.status, RunStatus::Incomplete);
    let resumed = execute_frozen_run(&configuration, &record, &nested, None).unwrap();
    assert_eq!(resumed.status, RunStatus::Complete);
    assert!(resumed.completed_trajectories > stopped.completed_trajectories);
    assert!(nested.join("result.json").is_file());
}

#[test]
fn completed_trajectories_survive_repeated_bounded_runs() {
    let mut configuration = configuration(FieldOrder::F3);
    configuration.rows = 3;
    configuration.prefix_rows = 2;
    let record = record(&configuration);
    let directory = tempfile::tempdir().unwrap();
    let first = execute_frozen_run(&configuration, &record, directory.path(), Some(3)).unwrap();
    assert_eq!(first.completed_trajectories, 1);
    assert_eq!(first.completed_prefix, 2);
    let mut previous = first;
    while previous.status != RunStatus::Complete {
        let current =
            execute_frozen_run(&configuration, &record, directory.path(), Some(3)).unwrap();
        assert!(current.completed_trajectories > previous.completed_trajectories);
        previous = current;
    }
    assert_eq!(previous.completed_trajectories, 12);
    let repeated = execute_frozen_run(&configuration, &record, directory.path(), Some(0)).unwrap();
    assert_eq!(repeated, previous);
    let summary = repeated.summary.unwrap();
    assert_eq!(summary.sample_count, 12);
    assert_eq!(summary.run_count, 3);
    assert!(!summary.interval_lower.is_empty());
    assert!(!summary.interval_upper.is_empty());
    assert!(summary.final_weight_ess.0.parse::<u64>().unwrap() > 0);
}

#[test]
fn log_recovers_a_checkpoint_that_lags_or_is_absent() {
    let configuration = configuration(FieldOrder::F5);
    let record = record(&configuration);
    let directory = tempfile::tempdir().unwrap();
    execute_frozen_run(&configuration, &record, directory.path(), Some(1)).unwrap();
    let older = fs::read(directory.path().join("checkpoint.json")).unwrap();
    let expected = execute_frozen_run(&configuration, &record, directory.path(), Some(2)).unwrap();
    fs::write(directory.path().join("checkpoint.json"), older).unwrap();
    let recovered = execute_frozen_run(&configuration, &record, directory.path(), Some(0)).unwrap();
    assert_eq!(recovered, expected);
    fs::remove_file(directory.path().join("checkpoint.json")).unwrap();
    let recovered = execute_frozen_run(&configuration, &record, directory.path(), Some(0)).unwrap();
    assert_eq!(recovered, expected);
}

#[test]
fn an_interrupted_log_append_does_not_repeat_durable_work() {
    use std::io::Write;
    let configuration = configuration(FieldOrder::F3);
    let record = record(&configuration);
    let directory = tempfile::tempdir().unwrap();
    let expected = execute_frozen_run(&configuration, &record, directory.path(), Some(2)).unwrap();
    fs::OpenOptions::new()
        .append(true)
        .open(directory.path().join("execution.jsonl"))
        .unwrap()
        .write_all(b"\n{\"progress\":{\"progress\":")
        .unwrap();
    for _ in 0..2 {
        let actual =
            execute_frozen_run(&configuration, &record, directory.path(), Some(0)).unwrap();
        assert_eq!(actual, expected);
    }
}

#[test]
fn only_the_closed_run_identity_controls_resume() {
    let configuration = configuration(FieldOrder::F3);
    let original = record(&configuration);
    let directory = tempfile::tempdir().unwrap();
    execute_frozen_run(&configuration, &original, directory.path(), Some(1)).unwrap();
    let mut same_identity = original.clone();
    same_identity.source_revision = "0000000000000000000000000000000000000001".parse().unwrap();
    execute_frozen_run(&configuration, &same_identity, directory.path(), Some(1)).unwrap();
    let before = fs::read(directory.path().join("execution.jsonl")).unwrap();
    let mut changed = original;
    changed
        .identity
        .build
        .rustflags
        .push("-Ctarget-cpu=native".into());
    assert!(execute_frozen_run(&configuration, &changed, directory.path(), None).is_err());
    assert_eq!(
        before,
        fs::read(directory.path().join("execution.jsonl")).unwrap()
    );
}

#[test]
fn invalid_configuration_does_not_start_a_run() {
    let mut configuration = configuration(FieldOrder::F3);
    let record = record(&configuration);
    configuration.target_runs = 1;
    let parent = tempfile::tempdir().unwrap();
    let destination = parent.path().join("not-created");
    assert!(execute_frozen_run(&configuration, &record, &destination, None).is_err());
    assert!(!destination.exists());
}

#[test]
fn empty_matrices_have_unit_likelihood_and_full_ess() {
    let mut configuration = configuration(FieldOrder::F7);
    configuration.rows = 0;
    let record = record(&configuration);
    let directory = tempfile::tempdir().unwrap();
    let outcome = execute_frozen_run(&configuration, &record, directory.path(), None).unwrap();
    let summary = outcome.summary.unwrap();
    assert_eq!(summary.likelihood, ("1".into(), "1".into()));
    assert_eq!(summary.final_weight_ess, ("12".into(), "1".into()));
    assert_eq!(summary.mean_variance, ("0".into(), "1".into()));
}

#[test]
fn binary_reads_the_seed_and_run_allocation_and_resumes() {
    for (seed, runs) in [(193, 3), (947, 4)] {
        let mut configuration = configuration(FieldOrder::F3);
        configuration.root_seed = seed;
        configuration.target_runs = runs;
        configuration.rows = 3;
        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("configuration.json");
        fs::write(&input, serde_json::to_vec(&configuration).unwrap()).unwrap();
        let output = directory.path().join("output");
        let invoke = |budget: &str| {
            std::process::Command::new(env!("CARGO_BIN_EXE_permanent_rare_event"))
                .args([input.as_os_str(), output.as_os_str(), budget.as_ref()])
                .output()
                .unwrap()
        };
        let first = invoke("1");
        assert_eq!(
            first.status.code(),
            Some(10),
            "{}",
            String::from_utf8_lossy(&first.stderr)
        );
        let prefix = fs::read(output.join("execution.jsonl")).unwrap();
        let final_run = invoke("100");
        assert!(
            final_run.status.success(),
            "{}",
            String::from_utf8_lossy(&final_run.stderr)
        );
        assert!(fs::read(output.join("execution.jsonl"))
            .unwrap()
            .starts_with(&prefix));
        let result: serde_json::Value =
            serde_json::from_slice(&fs::read(output.join("result.json")).unwrap()).unwrap();
        assert_eq!(result["configuration"]["root_seed"], seed);
        assert_eq!(result["summary"]["run_count"], runs);
        assert_eq!(result["summary"]["sample_count"], u64::from(runs) * 4);
    }
}
