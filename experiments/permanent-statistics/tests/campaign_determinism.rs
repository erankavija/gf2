mod common;
use permanent_statistics::campaign::{
    backend_supports_cell, dataset,
    runner::{run_cell, Invocation, OnUnavailable, RunStatus},
    Backend, CampaignConfig,
};
use std::{fs, io::Write, path::Path};

fn contract(config: &CampaignConfig, invocation: &Invocation, expected: &Path) {
    let record = common::record(config);
    let actual = tempfile::tempdir().unwrap();
    common::finish(config, &record, actual.path(), invocation);
    assert_eq!(
        fs::read(actual.path().join("summary.csv")).unwrap(),
        fs::read(expected.join("summary.csv")).unwrap()
    );
    for cell in config.cells() {
        assert_eq!(
            fs::read(actual.path().join(dataset::shard_path(cell.q, cell.n))).unwrap(),
            fs::read(expected.join(dataset::shard_path(cell.q, cell.n))).unwrap()
        );
    }
}

#[test]
fn shared_worker_resume_and_backend_contract() {
    let reference = tempfile::tempdir().unwrap();
    let baseline = common::config();
    common::finish(
        &baseline,
        &common::record(&baseline),
        reference.path(),
        &Invocation::default(),
    );
    for backend in common::cpu_backends() {
        let mut config = common::config();
        for group in &mut config.cell_groups {
            if backend_supports_cell(backend, group.q, group.n_last) {
                group.backend = backend;
            }
        }
        let expected = tempfile::tempdir().unwrap();
        common::finish(
            &config,
            &common::record(&config),
            expected.path(),
            &Invocation::default(),
        );
        assert_eq!(
            fs::read(expected.path().join("summary.csv")).unwrap(),
            fs::read(reference.path().join("summary.csv")).unwrap()
        );
        for cell in config.cells() {
            let mut actual = common::shards(expected.path(), cell.q, cell.n);
            let baseline = common::shards(reference.path(), cell.q, cell.n);
            for shard in &mut actual {
                shard.executed_backend = Backend::BatchParallel;
            }
            assert_eq!(actual, baseline);
        }
        for workers in [1, 2, 4] {
            for max_shards in [None, Some(1)] {
                contract(
                    &config,
                    &Invocation {
                        workers,
                        max_shards,
                        ..Invocation::default()
                    },
                    expected.path(),
                );
            }
        }
    }
}

#[test]
fn accepted_log_survives_a_kill_before_checkpoint_and_a_torn_append() {
    let config = common::config();
    let record = common::record(&config);
    let expected = tempfile::tempdir().unwrap();
    common::finish(&config, &record, expected.path(), &Invocation::default());
    let actual = tempfile::tempdir().unwrap();
    assert_eq!(
        run_cell(
            &config,
            &record,
            actual.path(),
            3,
            2,
            &Invocation {
                max_shards: Some(1),
                ..Invocation::default()
            }
        )
        .unwrap(),
        RunStatus::Incomplete
    );
    fs::remove_file(actual.path().join("checkpoint.json")).unwrap();
    fs::OpenOptions::new()
        .append(true)
        .open(actual.path().join("execution.jsonl"))
        .unwrap()
        .write_all(b"\n{\"shard_accepted\":")
        .unwrap();
    common::finish(
        &config,
        &record,
        actual.path(),
        &Invocation {
            workers: 4,
            max_shards: Some(1),
            ..Invocation::default()
        },
    );
    for cell in config.cells() {
        assert_eq!(
            common::shards(actual.path(), cell.q, cell.n),
            common::shards(expected.path(), cell.q, cell.n)
        );
    }
    assert_eq!(
        fs::read(actual.path().join("summary.csv")).unwrap(),
        fs::read(expected.path().join("summary.csv")).unwrap()
    );
}

#[test]
fn explicit_accelerator_fallback_shares_the_seeded_contract() {
    if permanent_statistics::campaign::backend_available(Backend::Accelerator) {
        return;
    }
    let expected = tempfile::tempdir().unwrap();
    let cpu = common::config();
    common::finish(
        &cpu,
        &common::record(&cpu),
        expected.path(),
        &Invocation::default(),
    );
    let mut config = cpu;
    for group in &mut config.cell_groups {
        group.backend = Backend::Accelerator;
        group.accelerator_per_matrix_us = Some(1.0);
    }
    let invocation = Invocation {
        workers: 4,
        max_shards: Some(1),
        on_unavailable: OnUnavailable::Fallback(Backend::BatchParallel),
        ..Invocation::default()
    };
    contract(&config, &invocation, expected.path());
}

#[cfg(feature = "hip")]
#[test]
fn real_device_shares_the_seeded_contract_when_present() {
    if !gf2_algebra::gpu::has_usable_device() {
        return;
    }
    let cpu = common::config();
    let expected = tempfile::tempdir().unwrap();
    common::finish(
        &cpu,
        &common::record(&cpu),
        expected.path(),
        &Invocation::default(),
    );
    let mut config = cpu;
    for group in &mut config.cell_groups {
        group.backend = Backend::Accelerator;
        group.accelerator_per_matrix_us = Some(1.0);
    }
    let actual = tempfile::tempdir().unwrap();
    common::finish(
        &config,
        &common::record(&config),
        actual.path(),
        &Invocation::default(),
    );
    for workers in [1, 2, 4] {
        for max_shards in [None, Some(1)] {
            contract(
                &config,
                &Invocation {
                    workers,
                    max_shards,
                    ..Invocation::default()
                },
                actual.path(),
            );
        }
    }
    for cell in config.cells() {
        let mut shards = common::shards(actual.path(), cell.q, cell.n);
        assert!(shards
            .iter()
            .all(|r| r.executed_backend == Backend::Accelerator));
        for shard in &mut shards {
            shard.executed_backend = Backend::BatchParallel;
        }
        assert_eq!(shards, common::shards(expected.path(), cell.q, cell.n));
    }
    assert_eq!(
        fs::read(actual.path().join("summary.csv")).unwrap(),
        fs::read(expected.path().join("summary.csv")).unwrap()
    );
}
