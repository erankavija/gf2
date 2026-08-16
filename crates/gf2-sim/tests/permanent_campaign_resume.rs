//! Checkpointed permanent-campaign driver contract tests.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use gf2_sim::checkpoint::{CheckpointReader, CheckpointWriter};
use gf2_sim::permanent_campaign::driver::{
    campaign_config_hash, campaign_configuration, run_field_checkpointed,
    run_field_checkpointed_with_evaluator, CampaignCheckpoint, CampaignDriverError,
};
use gf2_sim::permanent_campaign::schedule::evaluate_work_item;
use gf2_sim::permanent_campaign::schema::{
    read_field_summary, ArtifactIdentity, Availability, Backend, CampaignManifest, CellSpec,
    DeterminantPlan, FieldSummary, GitRevision, Provenance, RngAlgorithm, ShardSpec, StreamPurpose,
    SCHEMA_VERSION,
};
use gf2_sim::snr_checkpoint::{clear_interrupt, request_interrupt};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn temp_root(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "gf2-campaign-resume-{label}-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(root.join("campaign-resume-test")).unwrap();
    root.join("campaign-resume-test")
}

fn manifest(root_seed: u64) -> CampaignManifest {
    CampaignManifest {
        schema_version: SCHEMA_VERSION,
        campaign_id: "campaign-resume-test".parse().unwrap(),
        root_seed,
        stream_purposes: vec![StreamPurpose {
            name: "campaign-cells".parse().unwrap(),
            tag: 3,
        }],
        cells: vec![CellSpec {
            q: 3,
            n: 2,
            matrix_count: 4,
            shard_size: 2,
            shards: vec![
                ShardSpec {
                    shard_id: 0,
                    stream_index: 10,
                },
                ShardSpec {
                    shard_id: 1,
                    stream_index: 11,
                },
            ],
            backend: Backend::GenericRyser,
            backend_receipt: ArtifactIdentity {
                path: "test-receipt.json".parse().unwrap(),
                sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                    .parse()
                    .unwrap(),
            },
            determinant_companion: DeterminantPlan::NotEvaluated,
        }],
        provenance: Provenance {
            git_revision: "95ccd9776376b2b060e0dd40785e2effae29e766"
                .parse::<GitRevision>()
                .unwrap(),
            compiler_version: "test".to_owned(),
            rng_algorithm: RngAlgorithm::ChaCha20,
            rng_version: "rand_chacha test".to_owned(),
            invocation: vec!["permanent_campaign".to_owned()],
            accelerator_runtime: Availability::NotPresent,
            cpu_model: "test".to_owned(),
            gpu_model: Availability::NotPresent,
        },
    }
}

fn checkpoint(root: &Path) -> PathBuf {
    root.join("campaign.checkpoint.json")
}

fn dataset_bytes(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    fn visit(root: &Path, path: &Path, output: &mut Vec<(PathBuf, Vec<u8>)>) {
        for entry in fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.file_name().and_then(|name| name.to_str()) == Some("campaign.checkpoint.json") {
                continue;
            }
            if path.is_dir() {
                visit(root, &path, output);
            } else {
                output.push((
                    path.strip_prefix(root).unwrap().to_owned(),
                    fs::read(path).unwrap(),
                ));
            }
        }
    }
    let mut output = Vec::new();
    visit(root, root, &mut output);
    output.sort_by(|left, right| left.0.cmp(&right.0));
    output
}

#[test]
fn interrupted_and_resumed_run_matches_uninterrupted() {
    clear_interrupt();
    let manifest = manifest(0x0005_1498);
    let interrupted_root = temp_root("interrupted");
    let interrupted_checkpoint = checkpoint(&interrupted_root);
    let mut evaluations = 0;
    let interrupted = run_field_checkpointed_with_evaluator(
        &interrupted_root,
        &manifest,
        3,
        &interrupted_checkpoint,
        1,
        |item| {
            evaluations += 1;
            let result = evaluate_work_item(&manifest, item).map_err(|error| error.to_string());
            if evaluations == 1 {
                request_interrupt();
            }
            result
        },
    );
    assert!(matches!(interrupted, Err(CampaignDriverError::Interrupted)));
    clear_interrupt();
    run_field_checkpointed(&interrupted_root, &manifest, 3, &interrupted_checkpoint, 2).unwrap();

    let uninterrupted_root = temp_root("uninterrupted");
    run_field_checkpointed(
        &uninterrupted_root,
        &manifest,
        3,
        &checkpoint(&uninterrupted_root),
        1,
    )
    .unwrap();
    assert_eq!(
        dataset_bytes(&interrupted_root),
        dataset_bytes(&uninterrupted_root)
    );
}

#[test]
fn resume_across_checkpoint_boundary_and_worker_counts_is_identical() {
    clear_interrupt();
    let manifest = manifest(0xABCD);
    let resumed_root = temp_root("workers-resumed");
    let resumed_checkpoint = checkpoint(&resumed_root);
    request_interrupt();
    assert!(matches!(
        run_field_checkpointed(&resumed_root, &manifest, 3, &resumed_checkpoint, 1,),
        Err(CampaignDriverError::Interrupted)
    ));
    clear_interrupt();
    run_field_checkpointed(&resumed_root, &manifest, 3, &resumed_checkpoint, 4).unwrap();

    let reference_root = temp_root("workers-reference");
    run_field_checkpointed(
        &reference_root,
        &manifest,
        3,
        &checkpoint(&reference_root),
        4,
    )
    .unwrap();
    assert_eq!(dataset_bytes(&resumed_root), dataset_bytes(&reference_root));
}

#[test]
fn changed_configuration_refuses_resume_naming_the_disagreement() {
    clear_interrupt();
    let root = temp_root("config");
    let path = checkpoint(&root);
    let original = manifest(7);
    request_interrupt();
    assert!(matches!(
        run_field_checkpointed(&root, &original, 3, &path, 1),
        Err(CampaignDriverError::Interrupted)
    ));
    clear_interrupt();
    let changed = manifest(8);
    let error = run_field_checkpointed(&root, &changed, 3, &path, 1).unwrap_err();
    assert!(error.to_string().contains("root_seed"), "error: {error}");
}

#[test]
fn changed_campaign_id_refuses_resume_naming_campaign_id() {
    clear_interrupt();
    let root = temp_root("config-campaign-id");
    let path = checkpoint(&root);
    let original = manifest(7);
    request_interrupt();
    assert!(matches!(
        run_field_checkpointed(&root, &original, 3, &path, 1),
        Err(CampaignDriverError::Interrupted)
    ));
    clear_interrupt();
    let mut changed = manifest(7);
    changed.campaign_id = "campaign-resume-other".parse().unwrap();
    let error = run_field_checkpointed(&root, &changed, 3, &path, 1).unwrap_err();
    assert!(error.to_string().contains("campaign_id"), "error: {error}");
}

#[test]
fn crash_window_shard_is_adopted_without_re_emission() {
    clear_interrupt();
    let manifest = manifest(19);
    let root = temp_root("adopt");
    let path = checkpoint(&root);
    let mut evaluations = 0;
    let interrupted =
        run_field_checkpointed_with_evaluator(&root, &manifest, 3, &path, 1, |item| {
            evaluations += 1;
            let result = evaluate_work_item(&manifest, item).map_err(|error| error.to_string());
            if evaluations == 1 {
                request_interrupt();
            }
            result
        });
    assert!(matches!(interrupted, Err(CampaignDriverError::Interrupted)));
    fs::remove_file(&path).unwrap();
    clear_interrupt();

    let mut resumed_evaluations = 0;
    run_field_checkpointed_with_evaluator(&root, &manifest, 3, &path, 1, |item| {
        resumed_evaluations += 1;
        evaluate_work_item(&manifest, item).map_err(|error| error.to_string())
    })
    .unwrap();

    let reference_root = temp_root("adopt-reference");
    run_field_checkpointed(
        &reference_root,
        &manifest,
        3,
        &checkpoint(&reference_root),
        1,
    )
    .unwrap();
    assert_eq!(dataset_bytes(&root), dataset_bytes(&reference_root));
    assert_eq!(resumed_evaluations, 2);
}

#[test]
fn corrupted_crash_window_shard_is_refused_with_path_and_mismatch() {
    clear_interrupt();
    let manifest = manifest(23);
    let root = temp_root("adopt-corrupt");
    let path = checkpoint(&root);
    let mut evaluations = 0;
    let interrupted =
        run_field_checkpointed_with_evaluator(&root, &manifest, 3, &path, 1, |item| {
            evaluations += 1;
            let result = evaluate_work_item(&manifest, item).map_err(|error| error.to_string());
            if evaluations == 1 {
                request_interrupt();
            }
            result
        });
    assert!(matches!(interrupted, Err(CampaignDriverError::Interrupted)));
    fs::remove_file(&path).unwrap();
    let shard_path = root.join("shards/q3/n02/shard-000000.json");
    fs::write(&shard_path, b"tampered").unwrap();
    clear_interrupt();

    let error = run_field_checkpointed(&root, &manifest, 3, &path, 1).unwrap_err();
    let message = error.to_string();
    assert!(
        message.contains(shard_path.to_string_lossy().as_ref()),
        "error: {message}"
    );
    assert!(message.contains("mismatch"), "error: {message}");
}

#[test]
fn failing_work_item_is_quarantined_and_visible() {
    clear_interrupt();
    let root = temp_root("quarantine");
    let manifest = manifest(11);
    let path = checkpoint(&root);
    let run = run_field_checkpointed_with_evaluator(&root, &manifest, 3, &path, 1, |item| {
        if item.shard_id == 0 {
            Err("injected evaluation failure".to_owned())
        } else {
            evaluate_work_item(&manifest, item).map_err(|error| error.to_string())
        }
    })
    .unwrap();
    assert_eq!(run.shards().len(), 1);
    let summary = read_field_summary(&root, 3).unwrap();
    assert_eq!(summary.quarantined.len(), 1);
    assert_eq!(summary.quarantined[0].shard_id, 0);
    assert_eq!(summary.quarantined[0].error, "injected evaluation failure");
    assert!(root.join("shards/q3/n02/shard-000001.json").is_file());
}

#[test]
fn signal_during_checkpoint_write_never_corrupts_resume() {
    clear_interrupt();
    let root = temp_root("fsync");
    let manifest = manifest(13);
    let path = checkpoint(&root);
    let payload = CampaignCheckpoint::new(campaign_configuration(&manifest, 3));
    let writer = CheckpointWriter::<CampaignCheckpoint, _>::for_payload(
        &path,
        campaign_config_hash(&manifest, 3),
    )
    .unwrap();
    writer
        .write_payload_with_fsync_hook(&payload, request_interrupt)
        .unwrap();
    let reader = CheckpointReader::<CampaignCheckpoint, _>::for_payload(
        &path,
        campaign_config_hash(&manifest, 3),
    );
    assert_eq!(reader.load_payload().unwrap(), Some(payload));
    clear_interrupt();
}

#[test]
fn resume_does_not_repeat_completed_work() {
    clear_interrupt();
    let root = temp_root("no-repeat");
    let manifest = manifest(17);
    let path = checkpoint(&root);
    let mut first_calls = 0;
    let _ = run_field_checkpointed_with_evaluator(&root, &manifest, 3, &path, 1, |item| {
        first_calls += 1;
        let result = evaluate_work_item(&manifest, item).map_err(|error| error.to_string());
        if first_calls == 1 {
            request_interrupt();
        }
        result
    });
    clear_interrupt();
    let mut resumed_ids = BTreeSet::new();
    run_field_checkpointed_with_evaluator(&root, &manifest, 3, &path, 1, |item| {
        assert_ne!(
            item.shard_id, 0,
            "completed shard must not be evaluated again"
        );
        resumed_ids.insert(item.shard_id);
        evaluate_work_item(&manifest, item).map_err(|error| error.to_string())
    })
    .unwrap();
    assert_eq!(resumed_ids, BTreeSet::from([1]));
}

#[allow(dead_code)]
fn _field_summary_is_used(_: FieldSummary) {}
