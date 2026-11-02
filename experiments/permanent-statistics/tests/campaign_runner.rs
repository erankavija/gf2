mod common;
use permanent_statistics::campaign::{
    assess_completed_cell,
    dataset::{self, AcceptanceVerdict, Manifest},
    runner::{run_cell, Invocation, RunStatus},
    AcceptancePlan, CampaignConfig, CampaignError,
};
use std::{fs, fs::OpenOptions, io::Write};

fn append(path: &std::path::Path, value: serde_json::Value) {
    let mut file = OpenOptions::new()
        .append(true)
        .open(path.join("execution.jsonl"))
        .unwrap();
    writeln!(file, "\n{value}").unwrap();
    file.sync_all().unwrap();
}
#[test]
fn one_predraw_budget_and_inclusive_log_scale_decisions() {
    let mut config = common::config();
    config.cell_groups[0].matrix_count = 1_000_000;
    let manifest = Manifest::from_config(&config).unwrap();
    let mut plan = AcceptancePlan::for_manifest(&manifest).unwrap();
    assert_eq!(
        plan.permanent_level * manifest.cells.len() as f64,
        config.acceptance.permanent_family_error
    );
    assert_eq!(
        plan.determinant_level * manifest.cells.len() as f64,
        config.acceptance.determinant_family_error
    );
    let cell = &config.cells()[0];
    let row = assess_completed_cell(&plan, cell, cell.matrix_count, 0, Some(0)).unwrap();
    assert!(row.permanent_log_p_value.unwrap() < -1000.0);
    assert_eq!(row.permanent_verdict, Some(AcceptanceVerdict::Rejected));
    let cell = &common::config().cells()[0];
    let test = gf2_stats::binomial::lower_tail_test(30, cell.matrix_count, 1.0 / 3.0);
    plan.permanent_level = test.log_p_value().exp();
    // Use the test's own threshold comparator to retain inclusive log semantics.
    assert_eq!(
        assess_completed_cell(&plan, cell, cell.matrix_count, 30, Some(45))
            .unwrap()
            .permanent_verdict,
        Some(if test.rejects_at(plan.permanent_level) {
            AcceptanceVerdict::Rejected
        } else {
            AcceptanceVerdict::Accepted
        })
    );
}
#[test]
fn first_cell_and_serial_admission_refuse_before_drawing() {
    let config = common::config();
    let record = common::record(&config);
    let dir = tempfile::tempdir().unwrap();
    assert!(run_cell(&config, &record, dir.path(), 5, 2, &Invocation::default()).is_err());
    assert!(common::events(dir.path())
        .iter()
        .all(|e| e.get("shard_accepted").is_none()));
    run_cell(
        &config,
        &record,
        dir.path(),
        3,
        2,
        &Invocation {
            max_shards: Some(1),
            ..Invocation::default()
        },
    )
    .unwrap();
    assert!(run_cell(&config, &record, dir.path(), 7, 2, &Invocation::default()).is_err());
}
#[test]
fn only_closed_identity_changes_refuse_resume() {
    let config = common::config();
    let record = common::record(&config);
    let dir = tempfile::tempdir().unwrap();
    let invocation = Invocation {
        max_shards: Some(1),
        ..Invocation::default()
    };
    run_cell(&config, &record, dir.path(), 3, 2, &invocation).unwrap();
    let mut changed = record.clone();
    changed.source_revision = "2222222222222222222222222222222222222222".parse().unwrap();
    assert_eq!(
        run_cell(&config, &changed, dir.path(), 3, 2, &invocation).unwrap(),
        RunStatus::Incomplete
    );
    let original = serde_json::to_value(&record).unwrap();
    for (name, value) in [
        (
            "executable_sha256",
            serde_json::json!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
        ),
        (
            "configuration_sha256",
            serde_json::json!("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
        ),
        ("seed", serde_json::json!(999)),
        ("toolchain_version", serde_json::json!("different")),
        (
            "rng",
            serde_json::json!({"algorithm":"cha_cha20","version":"different"}),
        ),
        (
            "build",
            serde_json::json!({"profile":"release","opt_level":"3","lto":"false","codegen_units":16,"target":"test","rustflags":[]}),
        ),
        (
            "host",
            serde_json::json!({"cpu":{"model":"other CPU","physical_cores":4,"logical_threads":4},"accelerator":{"runtime":null,"model":null}}),
        ),
    ] {
        let mut changed = original.clone();
        changed["identity"][name] = value;
        let changed = serde_json::from_value(changed).unwrap();
        let before = common::events(dir.path())
            .iter()
            .filter(|e| e.get("shard_accepted").is_some())
            .count();
        assert!(
            run_cell(&config, &changed, dir.path(), 3, 2, &invocation).is_err(),
            "{name}"
        );
        assert_eq!(
            common::events(dir.path())
                .iter()
                .filter(|e| e.get("shard_accepted").is_some())
                .count(),
            before
        );
    }
}
#[test]
fn reported_failures_exhaust_retry_but_interruption_does_not() {
    let config = common::config();
    let record = common::record(&config);
    let dir = tempfile::tempdir().unwrap();
    run_cell(
        &config,
        &record,
        dir.path(),
        3,
        2,
        &Invocation {
            max_shards: Some(1),
            ..Invocation::default()
        },
    )
    .unwrap();
    // One accepted stream followed by an interrupted draw has no failure event.
    fs::remove_file(dir.path().join("checkpoint.json")).unwrap();
    append(
        dir.path(),
        serde_json::json!({"shard_failed":{"q":3,"n":2,"shard_id":1,"attempt":1,"error":"dispatch failed"}}),
    );
    append(
        dir.path(),
        serde_json::json!({"shard_failed":{"q":3,"n":2,"shard_id":1,"attempt":2,"error":"dispatch failed again"}}),
    );
    assert_eq!(
        run_cell(&config, &record, dir.path(), 3, 2, &Invocation::default()).unwrap(),
        RunStatus::Terminal
    );
    dataset::finalize(dir.path()).unwrap();
    dataset::verify(dir.path()).unwrap();
    let summary = fs::read_to_string(dir.path().join("summary.csv")).unwrap();
    let rows: Vec<_> = summary
        .lines()
        .skip(1)
        .map(|l| dataset::parse_summary_row(&l.split(',').collect::<Vec<_>>()).unwrap())
        .collect();
    assert_eq!(rows[0].matrix_count, 40);
    assert!(rows.iter().all(
        |r| r.halt_reason == Some(dataset::HaltReason::ExecutionFailure)
            && r.permanent_estimate.is_none()
    ));
    assert_eq!(common::shards(dir.path(), 3, 2).len(), 1);
    assert!(run_cell(&config, &record, dir.path(), 5, 2, &Invocation::default()).is_err());
}
#[test]
fn checkpoint_without_log_progress_refuses_and_lock_contention_draws_nothing() {
    let config = common::config();
    let record = common::record(&config);
    let dir = tempfile::tempdir().unwrap();
    run_cell(
        &config,
        &record,
        dir.path(),
        3,
        2,
        &Invocation {
            max_shards: Some(1),
            ..Invocation::default()
        },
    )
    .unwrap();
    let path = dir.path().join("execution.jsonl");
    let bytes = fs::read(&path).unwrap();
    let lock = OpenOptions::new().append(true).open(&path).unwrap();
    lock.try_lock().unwrap();
    assert!(matches!(
        run_cell(&config, &record, dir.path(), 3, 2, &Invocation::default()),
        Err(CampaignError::Io(_))
    ));
    assert_eq!(fs::read(&path).unwrap(), bytes);
    drop(lock);
    fs::write(&path, []).unwrap();
    assert!(run_cell(&config, &record, dir.path(), 3, 2, &Invocation::default()).is_err());
}
#[test]
fn invalid_config_and_selectors_refuse_without_sampling() {
    let mut config = common::config();
    let record = common::record(&config);
    let dir = tempfile::tempdir().unwrap();
    assert!(run_cell(&config, &record, dir.path(), 3, 99, &Invocation::default()).is_err());
    config.cell_groups[0].shard_size = 0;
    assert!(config.validate().is_err());
    let mut value = serde_json::to_value(common::config()).unwrap();
    value["unexpected"] = true.into();
    assert!(serde_json::from_value::<CampaignConfig>(value).is_err());
}

#[test]
fn a_logged_rejection_survives_interruption_before_halt_propagation() {
    let mut config = common::config();
    config.cell_groups[0].matrix_count = 1_000_000;
    config.cell_groups[0].shard_size = 1_000_000;
    let record = common::record(&config);
    let dir = tempfile::tempdir().unwrap();
    let manifest = Manifest::from_config(&config).unwrap();
    fs::write(
        dir.path().join("manifest.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    fs::write(dir.path().join("execution.jsonl"), []).unwrap();
    append(
        dir.path(),
        serde_json::json!({"start":{"q":3,"n":2,"record":record,"workers":1,"argv":[],"on_unavailable":"refuse"}}),
    );
    let shard = dataset::ShardRecord {
        shard_id: 0,
        stream_index: 0,
        matrix_count: 1_000_000,
        permanent_zero_count: 0,
        permanent_histogram: vec![0, 1_000_000, 0],
        determinant_zero_count: Some(407_407),
        executed_backend: config.cell_groups[0].backend,
    };
    append(
        dir.path(),
        serde_json::json!({"shard_accepted":{"q":3,"n":2,"shard":shard}}),
    );
    let row = assess_completed_cell(
        &AcceptancePlan::for_manifest(&manifest).unwrap(),
        &config.cells()[0],
        1_000_000,
        0,
        Some(407_407),
    )
    .unwrap();
    assert_eq!(row.permanent_verdict, Some(AcceptanceVerdict::Rejected));
    append(
        dir.path(),
        serde_json::json!({"cell_terminal":{"q":3,"n":2,"row":row}}),
    );
    assert!(run_cell(&config, &record, dir.path(), 5, 2, &Invocation::default()).is_err());
    dataset::finalize(dir.path()).unwrap();
    dataset::verify(dir.path()).unwrap();
    let summary = fs::read_to_string(dir.path().join("summary.csv")).unwrap();
    assert!(summary
        .lines()
        .skip(2)
        .all(|l| l.contains("halted,acceptance_failure")));
    assert!(summary.lines().nth(1).unwrap().contains("rejected"));
}

#[test]
fn backend_unavailable_failure_halts_even_before_the_halt_event_is_synced() {
    let config = common::config();
    let record = common::record(&config);
    let mut completed = Vec::new();
    for halt_logged in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        run_cell(
            &config,
            &record,
            directory.path(),
            3,
            2,
            &Invocation {
                max_shards: Some(1),
                ..Invocation::default()
            },
        )
        .unwrap();
        append(
            directory.path(),
            serde_json::json!({"shard_failed":{
                "q":3,"n":2,"shard_id":1,"attempt":1,"error":"backend unavailable","halt_reason":"backend_unavailable"
            }}),
        );
        if halt_logged {
            append(
                directory.path(),
                serde_json::json!({"campaign_halted":{
                    "q":3,"n":2,"reason":"backend_unavailable"
                }}),
            );
        }
        assert!(run_cell(
            &config,
            &record,
            directory.path(),
            3,
            2,
            &Invocation::default()
        )
        .is_err());
        dataset::finalize(directory.path()).unwrap();
        dataset::verify(directory.path()).unwrap();
        assert_eq!(common::shards(directory.path(), 3, 2).len(), 1);
        for cell in config.cells().iter().skip(1) {
            assert!(common::shards(directory.path(), cell.q, cell.n).is_empty());
        }
        let summary = fs::read_to_string(directory.path().join("summary.csv")).unwrap();
        assert!(summary
            .lines()
            .skip(1)
            .all(|line| line.contains("halted,backend_unavailable")));
        completed.push(summary);
    }
    assert_eq!(completed[0], completed[1]);
}

#[test]
fn determinant_family_counts_only_companions_and_permanent_counts_all_cells() {
    let mut config = common::config();
    config.cell_groups.truncate(2);
    config.cell_groups[0].matrix_count = 12;
    config.cell_groups[0].shard_size = 12;
    config.cell_groups[1].determinant_companion = false;
    let manifest = Manifest::from_config(&config).unwrap();
    let plan = AcceptancePlan::for_manifest(&manifest).unwrap();
    let cell = &config.cells()[0];
    let row = assess_completed_cell(&plan, cell, 12, 4, Some(9)).unwrap();
    let p_value = row.determinant_log_p_value.unwrap().exp();
    assert!((p_value - 0.01934).abs() < 0.00001);
    assert_eq!(row.determinant_verdict, Some(AcceptanceVerdict::Rejected));
    assert_eq!(
        plan.permanent_level * 2.0,
        config.acceptance.permanent_family_error
    );
    assert_eq!(
        plan.determinant_level,
        config.acceptance.determinant_family_error
    );

    config.cell_groups[0].determinant_companion = false;
    let plan = AcceptancePlan::for_manifest(&Manifest::from_config(&config).unwrap()).unwrap();
    let row = assess_completed_cell(&plan, &config.cells()[0], 12, 4, None).unwrap();
    assert!(row.determinant_verdict.is_none());
    assert!(plan.determinant_level.is_finite());
}
