use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use gf2_sim::permanent_campaign::acceptance::{
    assess_completed_cell, AcceptanceFamily, AcceptancePlan,
};
use gf2_sim::permanent_campaign::coordinator::{
    coordinator_lock_path, coordinator_receipt_path, emit_field_sidecar,
    execute_scheduled_cell_with_evaluator, ArmInvocation, CampaignCoordinator, CampaignHaltCause,
    CampaignHaltState, CellExecutionState, ExactCellScope, FieldExecutionState,
    FieldInterpretation, LiteratureSearchClaim, ShardAttemptState,
};
use gf2_sim::permanent_campaign::schedule::ScheduleError;
use gf2_sim::permanent_campaign::schema::{
    field_summary_file, read_field_summary, shard_record_file, AcceptanceVerdict,
    ArtifactIdentity, Availability, Backend, CampaignManifest, CellSpec, CellTerminalState,
    DeterminantCount, DeterminantPlan, GitRevision, HaltReason, Provenance, RngAlgorithm,
    ShardRecord, ShardSpec, StreamAddress, StreamPurpose, SCHEMA_VERSION,
};
use sha2::{Digest, Sha256};

static FIXTURE_ID: AtomicU64 = AtomicU64::new(0);

fn artifact(path: &str, byte: char) -> ArtifactIdentity {
    ArtifactIdentity {
        path: path.parse().unwrap(),
        sha256: std::iter::repeat_n(byte, 64)
            .collect::<String>()
            .parse()
            .unwrap(),
    }
}

fn cell(q: u8, n: u16, determinant_companion: DeterminantPlan) -> CellSpec {
    CellSpec {
        q,
        n,
        matrix_count: 100,
        shard_size: 100,
        shards: vec![ShardSpec {
            shard_id: 0,
            stream_index: (u64::from(q) << 32) | u64::from(n),
        }],
        backend: Backend::GenericRyser,
        backend_receipt: artifact("dev/benchmarks/permanent/backend.json", 'a'),
        determinant_companion,
    }
}

fn manifest() -> CampaignManifest {
    let mut q5 = cell(5, 4, DeterminantPlan::Evaluate);
    q5.shard_size = 50;
    q5.shards.push(ShardSpec {
        shard_id: 1,
        stream_index: (5_u64 << 32) | 5,
    });
    CampaignManifest {
        schema_version: SCHEMA_VERSION,
        campaign_id: "coordinator-fixture".parse().unwrap(),
        root_seed: 77,
        stream_purposes: vec![StreamPurpose {
            name: "campaign-cells".parse().unwrap(),
            tag: 3,
        }],
        cells: vec![
            cell(7, 4, DeterminantPlan::NotEvaluated),
            cell(7, 20, DeterminantPlan::NotEvaluated),
            q5,
            cell(3, 4, DeterminantPlan::Evaluate),
        ],
        provenance: Provenance {
            git_revision: "95ccd9776376b2b060e0dd40785e2effae29e766"
                .parse::<GitRevision>()
                .unwrap(),
            binary_sha256: Some("b".repeat(64).parse().unwrap()),
            deps_source_revision: Some(
                "95ccd9776376b2b060e0dd40785e2effae29e766"
                    .parse::<GitRevision>()
                    .unwrap(),
            ),
            deps_source_dirty: Some(false),
            compiler_version: "rustc 1.95.0".to_owned(),
            rng_algorithm: RngAlgorithm::ChaCha20,
            rng_version: "rand_chacha 0.9.0".to_owned(),
            invocation: vec![
                "permanent_campaign".to_owned(),
                "--manifest".to_owned(),
                "dev/fixture/coordinator-fixture".to_owned(),
                "--output".to_owned(),
                "dev/fixture/coordinator-fixture".to_owned(),
                "--q".to_owned(),
                "7".to_owned(),
                "--n".to_owned(),
                "20".to_owned(),
                "--workers".to_owned(),
                "1".to_owned(),
            ],
            accelerator_runtime: Availability::NotPresent,
            cpu_model: "fixture".to_owned(),
            cpu_physical_cores: Some(1),
            cpu_logical_threads: Some(1),
            gpu_model: Availability::NotPresent,
        },
    }
}

fn arm(q: u8, n: u16) -> ArmInvocation {
    ArmInvocation {
        scope: ExactCellScope { q, n },
        argv: vec![
            "permanent_campaign".to_owned(),
            "--manifest".to_owned(),
            "dev/fixture/coordinator-fixture".to_owned(),
            "--output".to_owned(),
            "dev/fixture/coordinator-fixture".to_owned(),
            "--q".to_owned(),
            q.to_string(),
            "--n".to_owned(),
            n.to_string(),
            "--workers".to_owned(),
            "1".to_owned(),
        ],
        worker_count: 1,
        executable_sha256: "b".repeat(64).parse().unwrap(),
    }
}

fn fixture() -> (PathBuf, CampaignManifest, CampaignCoordinator) {
    let manifest = manifest();
    let id = FIXTURE_ID.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "gf2-coordinator-fixture-{}-{id}",
        std::process::id()
    ));
    let campaign_root = root.join(manifest.campaign_id.to_string());
    fs::create_dir_all(&campaign_root).unwrap();
    let bytes = serde_json::to_vec_pretty(&manifest).unwrap();
    fs::write(campaign_root.join("manifest.json"), &bytes).unwrap();
    fs::write(root.join("protocol.md"), b"fixture frozen protocol\n").unwrap();
    let coordinator = CampaignCoordinator::new(&campaign_root).unwrap();
    (root, manifest, coordinator)
}

fn write_shard(
    campaign_root: &Path,
    manifest: &CampaignManifest,
    q: u8,
    n: u16,
    shard_id: u64,
    permanent_zeros: u64,
    determinant_zeros: Option<u64>,
) -> Vec<u8> {
    let cell = manifest
        .cells
        .iter()
        .find(|cell| (cell.q, cell.n) == (q, n))
        .unwrap();
    let ordinal = cell
        .shards
        .iter()
        .position(|shard| shard.shard_id == shard_id)
        .unwrap() as u64;
    let shard = &cell.shards[ordinal as usize];
    let matrix_count = cell
        .matrix_count
        .saturating_sub(ordinal * cell.shard_size)
        .min(cell.shard_size);
    let determinant = match determinant_zeros {
        Some(zero_count) => DeterminantCount::Evaluated {
            sample_count: matrix_count,
            zero_count,
        },
        None => DeterminantCount::NotEvaluated,
    };
    let mut histogram = vec![0; usize::from(q)];
    histogram[0] = permanent_zeros;
    histogram[1] = matrix_count - permanent_zeros;
    let record = ShardRecord {
        schema_version: SCHEMA_VERSION,
        shard_id,
        stream_address: StreamAddress {
            root_seed: manifest.root_seed,
            q,
            n,
            purpose_tag: 3,
            stream_index: shard.stream_index,
        },
        matrix_count,
        permanent_zero_count: permanent_zeros,
        permanent_histogram: histogram,
        determinant,
    };
    let bytes = serde_json::to_vec_pretty(&record).unwrap();
    let path = campaign_root.join(shard_record_file(q, n, shard_id));
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, &bytes).unwrap();
    bytes
}

fn accept_cell(
    coordinator: &mut CampaignCoordinator,
    root: &Path,
    manifest: &CampaignManifest,
    q: u8,
    n: u16,
    permanent_zeros: u64,
    determinant_zeros: Option<u64>,
) {
    coordinator.authorize_arm(arm(q, n)).unwrap();
    coordinator.persist(root).unwrap();
    coordinator.authorize_attempt(q, n, 0).unwrap();
    coordinator.persist(root).unwrap();
    write_shard(root, manifest, q, n, 0, permanent_zeros, determinant_zeros);
    coordinator.record_accepted(root, q, n, 0).unwrap();
    coordinator
        .record_completed(
            q,
            n,
            100,
            permanent_zeros,
            match determinant_zeros {
                Some(zero_count) => DeterminantCount::Evaluated {
                    sample_count: 100,
                    zero_count,
                },
                None => DeterminantCount::NotEvaluated,
            },
        )
        .unwrap();
}

fn write_receipt(
    campaign_root: &Path,
    campaign_id: &gf2_sim::permanent_campaign::schema::CampaignId,
    receipt: &gf2_sim::permanent_campaign::coordinator::CampaignCoordinatorReceipt,
) {
    let path = coordinator_receipt_path(campaign_root, campaign_id);
    fs::write(path, serde_json::to_vec_pretty(receipt).unwrap()).unwrap();
}

#[test]
fn acceptance_uses_one_budget_exact_log_tails_and_finite_n_determinants() {
    let campaign = manifest();
    let plan = AcceptancePlan::for_manifest(&campaign).unwrap();
    assert_eq!(plan.global_error(), 0.05);
    assert_eq!(plan.family_budget(AcceptanceFamily::PermanentFloor), 0.025);
    assert_eq!(plan.family_budget(AcceptanceFamily::Determinant), 0.025);
    assert_eq!(plan.family_test_count(AcceptanceFamily::PermanentFloor), 4);
    assert_eq!(plan.family_test_count(AcceptanceFamily::Determinant), 2);

    let mut underflow_cell = campaign.cells[1].clone();
    underflow_cell.matrix_count = 1_000_000;
    let underflow = assess_completed_cell(
        &plan,
        &underflow_cell,
        1_000_000,
        0,
        DeterminantCount::NotEvaluated,
    )
    .unwrap();
    assert!(underflow.permanent.test.log_p_value < -745.0);
    assert_eq!(
        underflow.permanent.test.level,
        0.025 / plan.family_test_count(AcceptanceFamily::PermanentFloor) as f64
    );
    assert_eq!(
        underflow.permanent.test.verdict,
        AcceptanceVerdict::Rejected
    );
    assert!(
        underflow.determinant.is_none(),
        "not_evaluated is a recorded skip"
    );

    let q3 = campaign.cells.iter().find(|cell| cell.q == 3).unwrap();
    let assessed = assess_completed_cell(
        &plan,
        q3,
        100,
        33,
        DeterminantCount::Evaluated {
            sample_count: 100,
            zero_count: 44,
        },
    )
    .unwrap();
    let determinant = assessed.determinant.unwrap();
    assert_eq!(determinant.test.verdict, AcceptanceVerdict::Accepted);
    let finite_n = 1.0
        - (1..=q3.n).fold(1.0, |product, i| {
            product * (1.0 - f64::from(q3.q).powi(-i32::from(i)))
        });
    assert_eq!(determinant.test.null_probability, finite_n);
    assert_eq!(
        determinant.test.level,
        0.025 / plan.family_test_count(AcceptanceFamily::Determinant) as f64
    );
    assert!((finite_n - (1.0 - 0.56_f64)).abs() > 0.001);
    assert!(assess_completed_cell(&plan, q3, 100, 33, DeterminantCount::NotEvaluated).is_err());
}

#[test]
fn coordinator_enforces_first_cell_retry_and_contradiction_preservation() {
    let (root, campaign, mut coordinator) = fixture();
    let campaign_root = root.join(campaign.campaign_id.to_string());
    for &(q, n) in &[(7, 4), (5, 4), (3, 4)] {
        assert!(coordinator.authorize_arm(arm(q, n)).is_err());
    }
    coordinator.authorize_arm(arm(7, 20)).unwrap();
    for &(q, n) in &[(7, 4), (5, 4), (3, 4)] {
        assert!(coordinator.authorize_arm(arm(q, n)).is_err());
    }
    let mut duplicate_selector = arm(7, 20);
    duplicate_selector
        .argv
        .extend(["--q".to_owned(), "7".to_owned()]);
    assert!(CampaignCoordinator::new(&campaign_root)
        .unwrap()
        .authorize_arm(duplicate_selector)
        .is_err());
    let mut default_worker = arm(7, 20);
    let worker_option = default_worker
        .argv
        .iter()
        .position(|token| token == "--workers")
        .unwrap();
    default_worker.argv.drain(worker_option..=worker_option + 1);
    CampaignCoordinator::new(&campaign_root)
        .unwrap()
        .authorize_arm(default_worker)
        .expect("omitted --workers has the documented effective value one");

    coordinator.persist(&campaign_root).unwrap();

    coordinator.authorize_attempt(7, 20, 0).unwrap();
    coordinator.persist(&campaign_root).unwrap();
    coordinator
        .record_quarantine(7, 20, 0, "mechanical fixture failure".to_owned())
        .unwrap();
    coordinator.authorize_attempt(7, 20, 0).unwrap();
    coordinator.persist(&campaign_root).unwrap();
    let raw = write_shard(&campaign_root, &campaign, 7, 20, 0, 0, None);
    coordinator
        .record_accepted(&campaign_root, 7, 20, 0)
        .unwrap();
    let attempts = &coordinator.receipt().attempts;
    assert_eq!(attempts.len(), 2);
    assert_eq!(attempts[0].stream_address, attempts[1].stream_address);
    assert_eq!(attempts[0].backend, attempts[1].backend);
    assert_eq!(attempts[0].backend_receipt, attempts[1].backend_receipt);
    assert_eq!(attempts[1].stream_address.root_seed, campaign.root_seed);
    assert_eq!(attempts[1].stream_address.purpose_tag, 3);
    assert_eq!(attempts[1].rng_algorithm, RngAlgorithm::ChaCha20);
    assert_eq!(attempts[1].rng_version, "rand_chacha 0.9.0");
    let ShardAttemptState::Accepted { record, .. } = &attempts[1].state else {
        panic!("recovery attempt must preserve accepted evidence");
    };
    assert_eq!(record.sha256.as_str(), format!("{:x}", Sha256::digest(raw)));
    assert!(coordinator.authorize_attempt(7, 20, 0).is_err());
    coordinator
        .record_completed(7, 20, 100, 0, DeterminantCount::NotEvaluated)
        .unwrap();
    assert!(matches!(
        coordinator.cell_state(7, 20),
        Some(CellExecutionState::Completed { .. })
    ));
    for &(q, n) in &[(7, 4), (5, 4), (3, 4)] {
        assert!(matches!(
            coordinator.cell_state(q, n),
            Some(CellExecutionState::Halted { .. })
        ));
    }
    assert!(matches!(
        coordinator.halt_state(),
        CampaignHaltState::Halted {
            cause: CampaignHaltCause::Acceptance { .. }
        }
    ));

    let (second_root, _, mut second_failure) = fixture();
    let second_campaign_root = second_root.join("coordinator-fixture");
    second_failure.authorize_arm(arm(7, 20)).unwrap();
    second_failure.persist(&second_campaign_root).unwrap();
    second_failure.authorize_attempt(7, 20, 0).unwrap();
    second_failure.persist(&second_campaign_root).unwrap();
    second_failure
        .record_quarantine(7, 20, 0, "first mechanical failure".to_owned())
        .unwrap();
    second_failure.authorize_attempt(7, 20, 0).unwrap();
    second_failure.persist(&second_campaign_root).unwrap();
    second_failure
        .record_quarantine(7, 20, 0, "recovery mechanical failure".to_owned())
        .unwrap();
    assert!(matches!(
        second_failure.cell_state(7, 20),
        Some(CellExecutionState::Halted { .. })
    ));
    assert!(second_failure.authorize_attempt(7, 20, 0).is_err());
    fs::remove_dir_all(second_root).unwrap();
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn exact_executor_requires_persisted_admission_retries_once_and_adopts_raw() {
    let scope = ExactCellScope { q: 7, n: 20 };

    let (unpersisted_root, unpersisted_manifest, _) = fixture();
    let unpersisted_campaign = unpersisted_root.join(unpersisted_manifest.campaign_id.to_string());
    let mut unpersisted_entries = 0_u8;
    assert!(execute_scheduled_cell_with_evaluator(
        &unpersisted_campaign,
        scope,
        1,
        |_, _, _, _| {
            unpersisted_entries += 1;
            Err(ScheduleError::InvalidWorkItem(
                "unreachable fixture evaluator".to_owned(),
            ))
        },
    )
    .is_err());
    assert_eq!(unpersisted_entries, 0);
    assert!(!unpersisted_campaign.join("derived").exists());

    let (retry_root, retry_manifest, mut retry) = fixture();
    let retry_campaign = retry_root.join(retry_manifest.campaign_id.to_string());
    retry.authorize_arm(arm(7, 20)).unwrap();
    retry.persist(&retry_campaign).unwrap();
    let mut mismatched_worker_entries = 0_u8;
    assert!(
        execute_scheduled_cell_with_evaluator(&retry_campaign, scope, 2, |_, _, _, _| {
            mismatched_worker_entries += 1;
            Err(ScheduleError::InvalidWorkItem(
                "mismatched worker evaluator must not run".to_owned(),
            ))
        },)
        .is_err()
    );
    assert_eq!(mismatched_worker_entries, 0);
    let mut addresses = Vec::new();
    let execution =
        execute_scheduled_cell_with_evaluator(&retry_campaign, scope, 1, |_, item, _, _| {
            addresses.push((item.q, item.n, item.shard_id, item.stream_index));
            Err(ScheduleError::InvalidWorkItem(
                "deterministic mechanical fixture".to_owned(),
            ))
        })
        .unwrap();
    assert_eq!(addresses.len(), 2);
    assert_eq!(addresses[0], addresses[1]);
    assert!(matches!(
        execution.terminal_state,
        CellExecutionState::Halted { .. }
    ));
    let exhausted = CampaignCoordinator::read(&retry_campaign).unwrap();
    assert_eq!(exhausted.receipt().attempts.len(), 2);
    assert!(exhausted
        .receipt()
        .attempts
        .iter()
        .all(|attempt| matches!(attempt.state, ShardAttemptState::Quarantined { .. })));
    let mut forbidden_entries = 0_u8;
    assert!(
        execute_scheduled_cell_with_evaluator(&retry_campaign, scope, 1, |_, _, _, _| {
            forbidden_entries += 1;
            Err(ScheduleError::InvalidWorkItem(
                "unreachable third attempt".to_owned(),
            ))
        },)
        .is_err()
    );
    assert_eq!(forbidden_entries, 0);

    let (adopt_root, adopt_manifest, mut adopt) = fixture();
    let adopt_campaign = adopt_root.join(adopt_manifest.campaign_id.to_string());
    adopt.authorize_arm(arm(7, 20)).unwrap();
    adopt.persist(&adopt_campaign).unwrap();
    adopt.authorize_attempt(7, 20, 0).unwrap();
    adopt.persist(&adopt_campaign).unwrap();
    write_shard(&adopt_campaign, &adopt_manifest, 7, 20, 0, 14, None);
    let mut adopted_entries = 0_u8;
    let adopted = execute_scheduled_cell_with_evaluator(&adopt_campaign, scope, 1, |_, _, _, _| {
        adopted_entries += 1;
        Err(ScheduleError::InvalidWorkItem(
            "durable raw must be adopted".to_owned(),
        ))
    })
    .unwrap();
    assert_eq!(adopted_entries, 0);
    assert_eq!(adopted.records.len(), 1);
    assert!(matches!(
        adopted.terminal_state,
        CellExecutionState::Completed { .. }
    ));
    let adopted_receipt = CampaignCoordinator::read(&adopt_campaign).unwrap();
    assert_eq!(adopted_receipt.receipt().attempts.len(), 1);
    assert!(matches!(
        adopted_receipt.receipt().attempts[0].state,
        ShardAttemptState::Accepted { .. }
    ));

    let (invalid_root, invalid_manifest, mut invalid) = fixture();
    let invalid_campaign = invalid_root.join(invalid_manifest.campaign_id.to_string());
    invalid.authorize_arm(arm(7, 20)).unwrap();
    invalid.persist(&invalid_campaign).unwrap();
    invalid.authorize_attempt(7, 20, 0).unwrap();
    invalid.persist(&invalid_campaign).unwrap();
    let invalid_raw = invalid_campaign.join(shard_record_file(7, 20, 0));
    fs::create_dir_all(invalid_raw.parent().unwrap()).unwrap();
    fs::write(&invalid_raw, b"{invalid durable shard").unwrap();
    let mut recovery_entries = 0_u8;
    let invalid_execution =
        execute_scheduled_cell_with_evaluator(&invalid_campaign, scope, 1, |_, _, _, _| {
            recovery_entries += 1;
            Err(ScheduleError::InvalidWorkItem(
                "deterministic recovery failure".to_owned(),
            ))
        })
        .unwrap();
    assert_eq!(recovery_entries, 1);
    assert!(matches!(
        invalid_execution.terminal_state,
        CellExecutionState::Halted { .. }
    ));
    assert!(!invalid_raw.exists());
    assert!(invalid_campaign
        .join(
            "derived/coordinator-fixture/campaign-coordinator/quarantine/\
             q7-n20-shard-000000-attempt-1.json",
        )
        .is_file());
    let invalid_receipt = CampaignCoordinator::read(&invalid_campaign).unwrap();
    assert_eq!(invalid_receipt.receipt().attempts.len(), 2);
    assert!(invalid_receipt
        .receipt()
        .attempts
        .iter()
        .all(|attempt| matches!(attempt.state, ShardAttemptState::Quarantined { .. })));

    fs::remove_dir_all(unpersisted_root).unwrap();
    fs::remove_dir_all(retry_root).unwrap();
    fs::remove_dir_all(adopt_root).unwrap();
    fs::remove_dir_all(invalid_root).unwrap();
}

#[test]
fn exact_executor_lock_contention_has_zero_sampler_entry() {
    let (root, campaign, mut coordinator) = fixture();
    let campaign_root = root.join(campaign.campaign_id.to_string());
    coordinator.authorize_arm(arm(7, 20)).unwrap();
    coordinator.persist(&campaign_root).unwrap();
    let lock_path = coordinator_lock_path(&campaign_root, &campaign.campaign_id);
    fs::create_dir_all(lock_path.parent().unwrap()).unwrap();
    let lock = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(lock_path)
        .unwrap();
    lock.try_lock().unwrap();
    let mut entries = 0_u8;
    assert!(execute_scheduled_cell_with_evaluator(
        &campaign_root,
        ExactCellScope { q: 7, n: 20 },
        1,
        |_, _, _, _| {
            entries += 1;
            Err(ScheduleError::InvalidWorkItem(
                "contended evaluator must not run".to_owned(),
            ))
        },
    )
    .is_err());
    assert_eq!(entries, 0);
    drop(lock);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn receipt_summary_and_sidecars_are_terminal_monotonic_and_closed() {
    let (root, campaign, mut coordinator) = fixture();
    let campaign_root = root.join(campaign.campaign_id.to_string());
    let checksums = campaign_root.join("checksums.sha256");
    fs::write(&checksums, b"raw-checksum-fixture\n").unwrap();
    assert!(emit_field_sidecar(
        &campaign_root,
        coordinator.receipt(),
        5,
        vec![artifact("summaries/q5.json", 'f')],
        FieldInterpretation::LiteratureSearchBasis {
            search_receipt: artifact("dev/studies/b488f02c/literature-search-2026-08-08.md", '1'),
            claim: LiteratureSearchClaim::NoLocatedQ5Q7NumericsSubjectToRecordedLimits,
        },
    )
    .is_err());

    accept_cell(&mut coordinator, &campaign_root, &campaign, 7, 20, 14, None);
    assert!(coordinator.assemble_field_summary(7).is_err());
    coordinator.persist(&campaign_root).unwrap();
    let first_receipt = coordinator.receipt().clone();
    accept_cell(&mut coordinator, &campaign_root, &campaign, 7, 4, 14, None);
    accept_cell(
        &mut coordinator,
        &campaign_root,
        &campaign,
        3,
        4,
        33,
        Some(44),
    );

    coordinator.authorize_arm(arm(5, 4)).unwrap();
    coordinator.persist(&campaign_root).unwrap();
    coordinator.authorize_attempt(5, 4, 0).unwrap();
    coordinator.persist(&campaign_root).unwrap();
    write_shard(&campaign_root, &campaign, 5, 4, 0, 10, Some(12));
    coordinator
        .record_accepted(&campaign_root, 5, 4, 0)
        .unwrap();
    coordinator.authorize_attempt(5, 4, 1).unwrap();
    coordinator.persist(&campaign_root).unwrap();
    coordinator
        .record_quarantine(5, 4, 1, "mechanical attempt one".to_owned())
        .unwrap();
    coordinator.authorize_attempt(5, 4, 1).unwrap();
    coordinator.persist(&campaign_root).unwrap();
    coordinator
        .record_quarantine(5, 4, 1, "mechanical recovery".to_owned())
        .unwrap();
    let CellExecutionState::Halted { summary, .. } = coordinator.cell_state(5, 4).unwrap() else {
        panic!("second quarantine must terminalize the partial cell");
    };
    assert_eq!(summary.matrix_count, 50);
    assert_eq!(summary.permanent_zero_count, 10);
    assert_eq!(
        summary.determinant,
        DeterminantCount::Evaluated {
            sample_count: 50,
            zero_count: 12,
        }
    );
    assert!(matches!(
        summary.terminal_state,
        CellTerminalState::Halted {
            reason: HaltReason::ExecutionFailure
        }
    ));

    let q7 = coordinator.assemble_field_summary(7).unwrap();
    assert_eq!(q7.rows.iter().map(|row| row.n).collect::<Vec<_>>(), [4, 20]);
    coordinator.persist(&campaign_root).unwrap();
    assert!(coordinator_receipt_path(&campaign_root, &campaign.campaign_id).is_file());
    let reloaded = CampaignCoordinator::read(&campaign_root).unwrap();
    assert_eq!(reloaded.assemble_field_summary(7).unwrap(), q7);
    let q5 = reloaded.assemble_field_summary(5).unwrap();
    assert_eq!(q5.quarantined.len(), 1);
    assert_eq!(q5.quarantined[0].error, "mechanical recovery");
    let q5_path = campaign_root.join(field_summary_file(5));
    fs::create_dir_all(q5_path.parent().unwrap()).unwrap();
    fs::write(&q5_path, serde_json::to_vec_pretty(&q5).unwrap()).unwrap();
    assert_eq!(
        read_field_summary(&campaign_root, 5)
            .expect("the terminal two-attempt quarantine projection is canonical"),
        q5
    );
    assert_eq!(reloaded.receipt().schema_version, 1);
    assert_eq!(reloaded.receipt().campaign_id, campaign.campaign_id);
    assert_eq!(
        reloaded.receipt().protocol_identity.path.as_str(),
        "dev/simulation_results/permanent-zero-fraction/protocol.md"
    );
    assert!(reloaded.receipt().arms.starts_with(&first_receipt.arms));
    assert!(reloaded
        .receipt()
        .attempts
        .starts_with(&first_receipt.attempts));
    assert_eq!(reloaded.receipt().arms[0].worker_count, 1);
    assert_eq!(
        reloaded.receipt().arms[0].scope,
        ExactCellScope { q: 7, n: 20 }
    );
    assert!(reloaded
        .receipt()
        .fields
        .iter()
        .any(|field| { field.q == 3 && field.execution == FieldExecutionState::Completed }));
    assert!(reloaded
        .receipt()
        .fields
        .iter()
        .any(|field| { field.q == 5 && field.execution == FieldExecutionState::Halted }));
    assert!(matches!(
        reloaded.halt_state(),
        CampaignHaltState::Halted { .. }
    ));

    let receipt_before = fs::read(coordinator_receipt_path(
        &campaign_root,
        &campaign.campaign_id,
    ))
    .unwrap();
    assert!(coordinator.authorize_arm(arm(7, 20)).is_err());
    coordinator.persist(&campaign_root).unwrap();
    assert_eq!(
        fs::read(coordinator_receipt_path(
            &campaign_root,
            &campaign.campaign_id
        ))
        .unwrap(),
        receipt_before,
        "a later persist cannot rewrite prior terminal or attempt evidence"
    );

    let sidecar_sources = vec![artifact("summaries/q7.json", 'f')];
    let q3 = emit_field_sidecar(
        &campaign_root,
        reloaded.receipt(),
        3,
        vec![artifact("summaries/q3.json", 'f')],
        FieldInterpretation::PublishedTargetComparison {
            target_table: artifact(
                "dev/simulation_results/permanent-zero-fraction/scheinerman2024-q3-targets-v1.csv",
                '2',
            ),
        },
    )
    .unwrap();
    let q5 = emit_field_sidecar(
        &campaign_root,
        reloaded.receipt(),
        5,
        vec![artifact("summaries/q5.json", 'f')],
        FieldInterpretation::LiteratureSearchBasis {
            search_receipt: artifact("dev/studies/b488f02c/literature-search-2026-08-08.md", '1'),
            claim: LiteratureSearchClaim::NoLocatedQ5Q7NumericsSubjectToRecordedLimits,
        },
    )
    .unwrap();
    let q7_path = emit_field_sidecar(
        &campaign_root,
        reloaded.receipt(),
        7,
        sidecar_sources.clone(),
        FieldInterpretation::LiteratureSearchBasis {
            search_receipt: artifact("dev/studies/b488f02c/literature-search-2026-08-08.md", '1'),
            claim: LiteratureSearchClaim::NoLocatedQ5Q7NumericsSubjectToRecordedLimits,
        },
    )
    .unwrap();
    for (path, q, status) in [
        (q3, 3, "completed"),
        (q5, 5, "halted"),
        (q7_path, 7, "completed"),
    ] {
        assert_eq!(
            path.strip_prefix(&campaign_root).unwrap(),
            PathBuf::from(format!(
                "derived/coordinator-fixture/campaign-coordinator/field-sidecars/q{q}.json"
            ))
        );
        let value: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(value["status"], status);
    }
    assert!(emit_field_sidecar(
        &campaign_root,
        reloaded.receipt(),
        7,
        sidecar_sources,
        FieldInterpretation::LiteratureSearchBasis {
            search_receipt: artifact("dev/studies/b488f02c/literature-search-2026-08-08.md", '1'),
            claim: LiteratureSearchClaim::NoLocatedQ5Q7NumericsSubjectToRecordedLimits,
        },
    )
    .is_err());
    assert_eq!(fs::read(&checksums).unwrap(), b"raw-checksum-fixture\n");
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn active_attempt_refuses_symlinked_raw_evidence_before_adoption() {
    use std::os::unix::fs::symlink;

    let (root, campaign, mut coordinator) = fixture();
    let campaign_root = root.join(campaign.campaign_id.to_string());
    coordinator.authorize_arm(arm(7, 20)).unwrap();
    coordinator.persist(&campaign_root).unwrap();
    coordinator.authorize_attempt(7, 20, 0).unwrap();
    coordinator.persist(&campaign_root).unwrap();

    let raw_path = campaign_root.join(shard_record_file(7, 20, 0));
    let target = root.join("symlink-target.json");
    let bytes = write_shard(&campaign_root, &campaign, 7, 20, 0, 14, None);
    fs::rename(&raw_path, &target).unwrap();
    symlink(&target, &raw_path).unwrap();

    let mut sampler_entries = 0_u8;
    let result = execute_scheduled_cell_with_evaluator(
        &campaign_root,
        ExactCellScope { q: 7, n: 20 },
        1,
        |_, _, _, _| {
            sampler_entries += 1;
            Err(ScheduleError::InvalidWorkItem(
                "symlink evidence must refuse before recovery".to_owned(),
            ))
        },
    );
    assert!(result.is_err());
    assert_eq!(sampler_entries, 0);
    assert_eq!(fs::read(&target).unwrap(), bytes);
    assert!(matches!(
        CampaignCoordinator::read(&campaign_root)
            .unwrap()
            .receipt()
            .attempts[0]
            .state,
        ShardAttemptState::Authorized
    ));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn receipt_reload_and_persist_refuse_forged_evidence_and_lifecycle_rewrites() {
    let (root, campaign, mut coordinator) = fixture();
    let campaign_root = root.join(campaign.campaign_id.to_string());
    accept_cell(&mut coordinator, &campaign_root, &campaign, 7, 20, 14, None);
    coordinator.persist(&campaign_root).unwrap();
    let canonical = coordinator.receipt().clone();

    let mut forged_counts = canonical.clone();
    let cell = campaign
        .cells
        .iter()
        .find(|cell| (cell.q, cell.n) == (7, 20))
        .unwrap();
    let forged_assessment = assess_completed_cell(
        coordinator.acceptance_plan(),
        cell,
        100,
        15,
        DeterminantCount::NotEvaluated,
    )
    .unwrap();
    let completed = forged_counts
        .cells
        .iter_mut()
        .find(|entry| (entry.q, entry.n) == (7, 20))
        .unwrap();
    completed.execution = CellExecutionState::Completed {
        assessment: forged_assessment,
        source_records: vec![artifact("forged/shard.json", '9')],
    };
    write_receipt(&campaign_root, &campaign.campaign_id, &forged_counts);
    assert!(CampaignCoordinator::read(&campaign_root).is_err());

    let mut forged_lifecycle = canonical.clone();
    forged_lifecycle.cells[1].execution = CellExecutionState::Scheduled { arm_index: 99 };
    write_receipt(&campaign_root, &campaign.campaign_id, &forged_lifecycle);
    assert!(CampaignCoordinator::read(&campaign_root).is_err());
    assert!(coordinator.persist(&campaign_root).is_err());

    write_receipt(&campaign_root, &campaign.campaign_id, &canonical);
    let manifest_path = campaign_root.join("manifest.json");
    let manifest_bytes = fs::read(&manifest_path).unwrap();
    let mut same_json_different_bytes = manifest_bytes.clone();
    same_json_different_bytes.push(b'\n');
    fs::write(&manifest_path, &same_json_different_bytes).unwrap();
    assert!(coordinator.persist(&campaign_root).is_err());
    fs::write(&manifest_path, manifest_bytes).unwrap();

    let mut unbound_arm = canonical;
    unbound_arm.arms.push(arm(3, 4));
    write_receipt(&campaign_root, &campaign.campaign_id, &unbound_arm);
    assert!(CampaignCoordinator::read(&campaign_root).is_err());

    let (exhausted_root, exhausted_manifest, mut exhausted) = fixture();
    let exhausted_campaign = exhausted_root.join(exhausted_manifest.campaign_id.to_string());
    exhausted.authorize_arm(arm(7, 20)).unwrap();
    exhausted.persist(&exhausted_campaign).unwrap();
    exhausted.authorize_attempt(7, 20, 0).unwrap();
    exhausted.persist(&exhausted_campaign).unwrap();
    exhausted
        .record_quarantine(7, 20, 0, "first failure".to_owned())
        .unwrap();
    exhausted.authorize_attempt(7, 20, 0).unwrap();
    exhausted.persist(&exhausted_campaign).unwrap();
    exhausted
        .record_quarantine(7, 20, 0, "recovery failure".to_owned())
        .unwrap();
    let mut forged_retry = exhausted.receipt().clone();
    for cell in &mut forged_retry.cells {
        cell.execution = if (cell.q, cell.n) == (7, 20) {
            CellExecutionState::Scheduled { arm_index: 0 }
        } else {
            CellExecutionState::Pending
        };
    }
    for field in &mut forged_retry.fields {
        field.execution = FieldExecutionState::InProgress;
    }
    forged_retry.halt = CampaignHaltState::Running;
    write_receipt(
        &exhausted_campaign,
        &exhausted_manifest.campaign_id,
        &forged_retry,
    );
    assert!(CampaignCoordinator::read(&exhausted_campaign).is_err());
    fs::remove_dir_all(exhausted_root).unwrap();
    fs::remove_dir_all(root).unwrap();
}
