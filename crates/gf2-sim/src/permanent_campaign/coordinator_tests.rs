use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::permanent_campaign::acceptance::{
    assess_completed_cell, AcceptanceFamily, AcceptancePlan,
};
use crate::permanent_campaign::coordinator::{
    classify_q3_precision, coordinator_field_sidecar_path, coordinator_lock_path,
    coordinator_receipt_path, execute_campaign_cell_with_evaluator, parse_q3_target_table,
    ArmInvocation, CampaignCoordinator, CampaignHaltCause, CampaignHaltState, CellExecutionState,
    CoordinatorEvidenceSources, CoordinatorFieldInterpretation, CoordinatorFieldSidecar,
    CoordinatorFieldSidecarStatus, CoordinatorQ3ComparisonRow, ExactCellScope, FieldExecutionState,
    LiteratureSearchClaim, Q3IntervalRelation, Q3PrecisionClassification, Q3ReportedProbability,
    Q3SourceEvidence, ShardAttemptState,
};
use crate::permanent_campaign::provenance::{approve_emission, EmissionApproval};
use crate::permanent_campaign::schedule::{
    EvaluatedShard, PhaseDurations, ScheduleError, ShardRun, WorkItem,
};
use crate::permanent_campaign::schema::{
    field_summary_file, read_field_summary, shard_record_file, AcceptanceVerdict, ArtifactIdentity,
    Availability, Backend, CampaignManifest, CellSpec, CellTerminalState, DeterminantCount,
    DeterminantPlan, GitRevision, HaltReason, Provenance, RngAlgorithm, ShardRecord, ShardSpec,
    StreamAddress, StreamPurpose, SCHEMA_VERSION,
};
use sha2::{Digest, Sha256};

static FIXTURE_ID: AtomicU64 = AtomicU64::new(0);
static TEST_RUN_ID: OnceLock<u128> = OnceLock::new();

fn test_run_id() -> u128 {
    *TEST_RUN_ID.get_or_init(|| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("test clock is after the Unix epoch")
            .as_nanos()
    })
}

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
        accelerator_cost_table: None,
    }
}

fn arm_for_manifest(manifest: &CampaignManifest, q: u8, n: u16) -> ArmInvocation {
    let mut argv = manifest.provenance.invocation.clone();
    let q_index = argv.iter().position(|token| token == "--q").unwrap();
    argv[q_index + 1] = q.to_string();
    let n_index = argv.iter().position(|token| token == "--n").unwrap();
    argv[n_index + 1] = n.to_string();
    ArmInvocation {
        scope: ExactCellScope { q, n },
        argv,
        worker_count: 1,
        executable_sha256: manifest.provenance.binary_sha256.clone().unwrap(),
        accelerator_cost_table: None,
    }
}

fn fixture() -> (PathBuf, CampaignManifest, CampaignCoordinator) {
    fixture_from_manifest(manifest())
}

fn fixture_from_manifest(
    manifest: CampaignManifest,
) -> (PathBuf, CampaignManifest, CampaignCoordinator) {
    let id = FIXTURE_ID.fetch_add(1, Ordering::Relaxed);
    let fixture_root = std::env::temp_dir().join(format!(
        "gf2-coordinator-fixture-{}-{}-{id}",
        std::process::id(),
        test_run_id()
    ));
    let repository = fixture_root.join("checkout");
    let root = repository.join("dev/simulation_results/permanent-zero-fraction");
    let campaign_root = root.join(manifest.campaign_id.to_string());
    fs::create_dir_all(&campaign_root).unwrap();
    let bytes = serde_json::to_vec_pretty(&manifest).unwrap();
    fs::write(campaign_root.join("manifest.json"), &bytes).unwrap();
    fs::write(root.join("protocol.md"), b"fixture frozen protocol\n").unwrap();
    write_evidence_sources(&repository);
    let coordinator = CampaignCoordinator::new(&campaign_root, &evidence_source_paths()).unwrap();
    (root, manifest, coordinator)
}

fn evidence_source_paths() -> CoordinatorEvidenceSources {
    let q3_bytes = include_bytes!(
        "../../../../dev/simulation_results/permanent-zero-fraction/\
         scheinerman2024-q3-targets-v1.csv"
    );
    let search_bytes =
        include_bytes!("../../../../dev/studies/b488f02c/literature-search-2026-08-08.md");
    CoordinatorEvidenceSources {
        q3_targets: ArtifactIdentity {
            path: "dev/simulation_results/permanent-zero-fraction/\
                   scheinerman2024-q3-targets-v1.csv"
                .parse()
                .unwrap(),
            sha256: format!("{:x}", Sha256::digest(q3_bytes)).parse().unwrap(),
        },
        q5_q7_literature_search: ArtifactIdentity {
            path: "dev/studies/b488f02c/literature-search-2026-08-08.md"
                .parse()
                .unwrap(),
            sha256: format!("{:x}", Sha256::digest(search_bytes))
                .parse()
                .unwrap(),
        },
    }
}

fn write_evidence_sources(repository: &Path) {
    let sources = evidence_source_paths();
    let target = repository.join(sources.q3_targets.path.as_str());
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::write(
        target,
        include_bytes!(
            "../../../../dev/simulation_results/permanent-zero-fraction/\
             scheinerman2024-q3-targets-v1.csv"
        ),
    )
    .unwrap();
    let search = repository.join(sources.q5_q7_literature_search.path.as_str());
    fs::create_dir_all(search.parent().unwrap()).unwrap();
    fs::write(
        search,
        include_bytes!("../../../../dev/studies/b488f02c/literature-search-2026-08-08.md"),
    )
    .unwrap();
}

fn live_fixture() -> (PathBuf, PathBuf, CampaignManifest, EmissionApproval) {
    live_fixture_from_manifest(manifest())
}

fn live_fixture_from_manifest(
    mut manifest: CampaignManifest,
) -> (PathBuf, PathBuf, CampaignManifest, EmissionApproval) {
    let executable = std::env::current_exe().unwrap();
    manifest.provenance.binary_sha256 = Some(
        format!("{:x}", Sha256::digest(fs::read(executable).unwrap()))
            .parse()
            .unwrap(),
    );
    let id = FIXTURE_ID.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "gf2-live-coordinator-fixture-{}-{}-{id}",
        std::process::id(),
        test_run_id()
    ));
    let repository = root.join("checkout");
    let dataset = repository.join("dev/simulation_results/permanent-zero-fraction");
    let campaign_root = dataset.join(manifest.campaign_id.to_string());
    fs::create_dir_all(&campaign_root).unwrap();
    fs::write(
        campaign_root.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
    fs::write(dataset.join("protocol.md"), b"fixture frozen protocol\n").unwrap();
    write_evidence_sources(&repository);
    assert!(Command::new("git")
        .args(["init", "--quiet", "--initial-branch=main"])
        .current_dir(&repository)
        .status()
        .unwrap()
        .success());
    commit_fixture(&repository, "freeze fixture");
    let approval = approve_emission(&campaign_root).unwrap();
    (root, campaign_root, manifest, approval)
}

fn single_cell_manifest(q: u8) -> CampaignManifest {
    let mut campaign = manifest();
    campaign.cells = vec![cell(q, 20, DeterminantPlan::NotEvaluated)];
    campaign.cells[0].matrix_count = 100;
    campaign.cells[0].shard_size = 100;
    campaign.cells[0].shards[0].stream_index = (u64::from(q) << 32) | 20;
    campaign
}

fn commit_fixture(repository: &Path, message: &str) {
    assert!(Command::new("git")
        .args(["add", "--all"])
        .current_dir(repository)
        .status()
        .unwrap()
        .success());
    assert!(Command::new("git")
        .args([
            "-c",
            "user.name=fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "--quiet",
            "--no-gpg-sign",
            "-m",
            message,
        ])
        .current_dir(repository)
        .status()
        .unwrap()
        .success());
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

fn evaluated_shard(
    manifest: &CampaignManifest,
    item: &WorkItem,
    permanent_zeros: u64,
) -> EvaluatedShard {
    let mut histogram = vec![0; usize::from(item.q)];
    histogram[0] = permanent_zeros;
    histogram[1] = item.matrix_count - permanent_zeros;
    EvaluatedShard {
        run: ShardRun {
            record: ShardRecord {
                schema_version: SCHEMA_VERSION,
                shard_id: item.shard_id,
                stream_address: StreamAddress {
                    root_seed: manifest.root_seed,
                    q: item.q,
                    n: item.n,
                    purpose_tag: 3,
                    stream_index: item.stream_index,
                },
                matrix_count: item.matrix_count,
                permanent_zero_count: permanent_zeros,
                permanent_histogram: histogram,
                determinant: DeterminantCount::NotEvaluated,
            },
            timing: PhaseDurations {
                draw: Duration::ZERO,
                pack: Duration::ZERO,
                evaluate: Duration::ZERO,
                determinant: Duration::ZERO,
                count: Duration::ZERO,
            },
        },
    }
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
    campaign_id: &crate::permanent_campaign::schema::CampaignId,
    receipt: &crate::permanent_campaign::coordinator::CampaignCoordinatorReceipt,
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

fn million_sample_manifest(determinant_companion: DeterminantPlan) -> CampaignManifest {
    let mut campaign = manifest();
    campaign.cells = vec![cell(7, 20, determinant_companion)];
    campaign.cells[0].matrix_count = 1_000_000;
    campaign.cells[0].shard_size = 1_000_000;
    campaign
}

fn persist_million_sample_assessment(
    campaign: CampaignManifest,
    permanent_zero_count: u64,
    determinant_zero_count: Option<u64>,
) -> (PathBuf, PathBuf, CampaignCoordinator) {
    let (root, campaign, mut coordinator) = fixture_from_manifest(campaign);
    let campaign_root = root.join(campaign.campaign_id.to_string());
    coordinator
        .authorize_arm(arm_for_manifest(&campaign, 7, 20))
        .unwrap();
    coordinator.persist(&campaign_root).unwrap();
    coordinator.authorize_attempt(7, 20, 0).unwrap();
    coordinator.persist(&campaign_root).unwrap();
    write_shard(
        &campaign_root,
        &campaign,
        7,
        20,
        0,
        permanent_zero_count,
        determinant_zero_count,
    );
    coordinator
        .record_accepted(&campaign_root, 7, 20, 0)
        .unwrap();
    let determinant = determinant_zero_count.map_or(DeterminantCount::NotEvaluated, |zero_count| {
        DeterminantCount::Evaluated {
            sample_count: 1_000_000,
            zero_count,
        }
    });
    coordinator
        .record_completed(7, 20, 1_000_000, permanent_zero_count, determinant)
        .unwrap();
    coordinator.persist(&campaign_root).unwrap();
    let reloaded = CampaignCoordinator::read(&campaign_root).unwrap();
    (root, campaign_root, reloaded)
}

#[test]
fn coordinator_persists_exact_underflow_pass_and_finite_n_determinant_rejection() {
    let (underflow_root, _, underflow) = persist_million_sample_assessment(
        million_sample_manifest(DeterminantPlan::NotEvaluated),
        0,
        None,
    );
    let CellExecutionState::Completed { assessment, .. } = underflow.cell_state(7, 20).unwrap()
    else {
        panic!("the underflow observation remains completed contradiction evidence");
    };
    assert_eq!(assessment.permanent.test.successes, 0);
    assert_eq!(assessment.permanent.test.trials, 1_000_000);
    assert_eq!(assessment.permanent.test.null_probability, 1.0 / 7.0);
    assert!(assessment.permanent.test.log_p_value < -745.0);
    assert_eq!(assessment.permanent.test.level, 0.025);
    assert_eq!(
        assessment.permanent.test.verdict,
        AcceptanceVerdict::Rejected
    );
    assert_eq!(assessment.summary.matrix_count, 1_000_000);
    assert_eq!(assessment.summary.permanent_zero_count, 0);
    assert!(matches!(
        assessment.summary.terminal_state,
        CellTerminalState::Completed {
            permanent_verdict: AcceptanceVerdict::Rejected,
            ..
        }
    ));
    assert!(matches!(
        underflow.halt_state(),
        CampaignHaltState::Halted {
            cause: CampaignHaltCause::Acceptance {
                q: 7,
                n: 20,
                rejected_families,
            }
        } if rejected_families == &[AcceptanceFamily::PermanentFloor]
    ));

    let (pass_root, _, pass) = persist_million_sample_assessment(
        million_sample_manifest(DeterminantPlan::NotEvaluated),
        142_857,
        None,
    );
    let CellExecutionState::Completed { assessment, .. } = pass.cell_state(7, 20).unwrap() else {
        panic!("the floor-conforming observation completes");
    };
    assert_eq!(assessment.permanent.test.successes, 142_857);
    assert_eq!(assessment.permanent.test.trials, 1_000_000);
    assert_eq!(assessment.permanent.test.level, 0.025);
    assert_eq!(
        assessment.permanent.test.verdict,
        AcceptanceVerdict::Accepted
    );
    assert!(assessment.permanent.test.log_p_value.is_finite());
    assert!(matches!(pass.halt_state(), CampaignHaltState::Running));

    let (determinant_root, _, determinant) = persist_million_sample_assessment(
        million_sample_manifest(DeterminantPlan::Evaluate),
        142_857,
        Some(0),
    );
    let CellExecutionState::Completed { assessment, .. } = determinant.cell_state(7, 20).unwrap()
    else {
        panic!("the determinant contradiction remains completed evidence");
    };
    let determinant_evidence = assessment.determinant.unwrap().test;
    let finite_n = 1.0 - (1..=20).fold(1.0, |product, i| product * (1.0 - 7_f64.powi(-i)));
    assert_eq!(determinant_evidence.successes, 0);
    assert_eq!(determinant_evidence.trials, 1_000_000);
    assert_eq!(determinant_evidence.null_probability, finite_n);
    assert!(determinant_evidence.log_p_value < -745.0);
    assert_eq!(determinant_evidence.level, 0.025);
    assert_eq!(determinant_evidence.verdict, AcceptanceVerdict::Rejected);
    assert!(matches!(
        determinant.halt_state(),
        CampaignHaltState::Halted {
            cause: CampaignHaltCause::Acceptance {
                q: 7,
                n: 20,
                rejected_families,
            }
        } if rejected_families == &[AcceptanceFamily::Determinant]
    ));

    fs::remove_dir_all(underflow_root).unwrap();
    fs::remove_dir_all(pass_root).unwrap();
    fs::remove_dir_all(determinant_root).unwrap();
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
    assert!(
        CampaignCoordinator::new(&campaign_root, &evidence_source_paths())
            .unwrap()
            .authorize_arm(duplicate_selector)
            .is_err()
    );
    let mut default_worker = arm(7, 20);
    let worker_option = default_worker
        .argv
        .iter()
        .position(|token| token == "--workers")
        .unwrap();
    default_worker.argv.drain(worker_option..=worker_option + 1);
    CampaignCoordinator::new(&campaign_root, &evidence_source_paths())
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

    let (retry_root, retry_campaign, retry_manifest, approval) = live_fixture();
    let mut mismatched_worker_entries = 0_u8;
    assert!(execute_campaign_cell_with_evaluator(
        &retry_campaign,
        scope,
        2,
        arm_for_manifest(&retry_manifest, 7, 20).argv,
        approval,
        &evidence_source_paths(),
        |_, _, _, _| {
            mismatched_worker_entries += 1;
            Err(ScheduleError::InvalidWorkItem(
                "mismatched worker evaluator must not run".to_owned(),
            ))
        },
        |_| {},
    )
    .is_err());
    assert_eq!(mismatched_worker_entries, 0);
    let mut addresses = Vec::new();
    let execution = execute_campaign_cell_with_evaluator(
        &retry_campaign,
        scope,
        1,
        arm_for_manifest(&retry_manifest, 7, 20).argv,
        approve_emission(&retry_campaign).unwrap(),
        &evidence_source_paths(),
        |_, item, _, _| {
            let persisted = CampaignCoordinator::read(&retry_campaign).unwrap();
            assert!(matches!(
                persisted.receipt().attempts.last().unwrap().state,
                ShardAttemptState::Authorized
            ));
            addresses.push((item.q, item.n, item.shard_id, item.stream_index));
            Err(ScheduleError::InvalidWorkItem(
                "deterministic mechanical fixture".to_owned(),
            ))
        },
        |_| {},
    )
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
    assert!(execute_campaign_cell_with_evaluator(
        &retry_campaign,
        scope,
        1,
        arm_for_manifest(&retry_manifest, 7, 20).argv,
        approve_emission(&retry_campaign).unwrap(),
        &evidence_source_paths(),
        |_, _, _, _| {
            forbidden_entries += 1;
            Err(ScheduleError::InvalidWorkItem(
                "unreachable third attempt".to_owned(),
            ))
        },
        |_| {},
    )
    .is_ok());
    assert_eq!(forbidden_entries, 0);

    let (adopt_root, adopt_campaign, adopt_manifest, approval) = live_fixture();
    let mut adopt = CampaignCoordinator::new(&adopt_campaign, &evidence_source_paths()).unwrap();
    adopt
        .authorize_arm(arm_for_manifest(&adopt_manifest, 7, 20))
        .unwrap();
    adopt.persist(&adopt_campaign).unwrap();
    adopt.authorize_attempt(7, 20, 0).unwrap();
    adopt.persist(&adopt_campaign).unwrap();
    write_shard(&adopt_campaign, &adopt_manifest, 7, 20, 0, 14, None);
    let mut adopted_entries = 0_u8;
    let adopted = execute_campaign_cell_with_evaluator(
        &adopt_campaign,
        scope,
        1,
        arm_for_manifest(&adopt_manifest, 7, 20).argv,
        approval,
        &evidence_source_paths(),
        |_, _, _, _| {
            adopted_entries += 1;
            Err(ScheduleError::InvalidWorkItem(
                "durable raw must be adopted".to_owned(),
            ))
        },
        |_| {},
    )
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

    let (invalid_root, invalid_campaign, invalid_manifest, approval) = live_fixture();
    let mut invalid =
        CampaignCoordinator::new(&invalid_campaign, &evidence_source_paths()).unwrap();
    invalid
        .authorize_arm(arm_for_manifest(&invalid_manifest, 7, 20))
        .unwrap();
    invalid.persist(&invalid_campaign).unwrap();
    invalid.authorize_attempt(7, 20, 0).unwrap();
    invalid.persist(&invalid_campaign).unwrap();
    let invalid_raw = invalid_campaign.join(shard_record_file(7, 20, 0));
    fs::create_dir_all(invalid_raw.parent().unwrap()).unwrap();
    fs::write(&invalid_raw, b"{invalid durable shard").unwrap();
    let mut recovery_entries = 0_u8;
    let invalid_execution = execute_campaign_cell_with_evaluator(
        &invalid_campaign,
        scope,
        1,
        arm_for_manifest(&invalid_manifest, 7, 20).argv,
        approval,
        &evidence_source_paths(),
        |_, _, _, _| {
            recovery_entries += 1;
            Err(ScheduleError::InvalidWorkItem(
                "deterministic recovery failure".to_owned(),
            ))
        },
        |_| {},
    )
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

    fs::remove_dir_all(retry_root).unwrap();
    fs::remove_dir_all(adopt_root).unwrap();
    fs::remove_dir_all(invalid_root).unwrap();
}

#[test]
fn canonical_transaction_persists_authorization_before_sampling_and_raw_before_acceptance() {
    let (root, campaign_root, campaign, approval) = live_fixture();
    let mut evaluator_saw_authorized = false;
    let mut durability_saw_authorized = false;
    let execution = execute_campaign_cell_with_evaluator(
        &campaign_root,
        ExactCellScope { q: 7, n: 20 },
        1,
        arm_for_manifest(&campaign, 7, 20).argv,
        approval,
        &evidence_source_paths(),
        |manifest, item, _, _| {
            let persisted = CampaignCoordinator::read(&campaign_root).unwrap();
            assert!(matches!(
                persisted.receipt().attempts.last().unwrap().state,
                ShardAttemptState::Authorized
            ));
            evaluator_saw_authorized = true;
            Ok(evaluated_shard(manifest, item, 14))
        },
        |raw_path| {
            assert!(raw_path.is_file());
            let persisted = CampaignCoordinator::read(&campaign_root).unwrap();
            assert!(matches!(
                persisted.receipt().attempts.last().unwrap().state,
                ShardAttemptState::Authorized
            ));
            durability_saw_authorized = true;
        },
    )
    .unwrap();
    assert!(evaluator_saw_authorized);
    assert!(durability_saw_authorized);
    assert!(matches!(
        execution.terminal_state,
        CellExecutionState::Completed { .. }
    ));
    let persisted = CampaignCoordinator::read(&campaign_root).unwrap();
    assert!(matches!(
        persisted.receipt().attempts.last().unwrap().state,
        ShardAttemptState::Accepted { .. }
    ));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn persisted_arm_binds_one_repository_relative_accelerator_cost_snapshot() {
    let (root, campaign_root, mut campaign, _) = live_fixture();
    let repository = root.join("checkout");
    let relative_table = "dev/benchmarks/permanent_campaign/coordinator-costs.csv";
    let table_path = repository.join(relative_table);
    fs::create_dir_all(table_path.parent().unwrap()).unwrap();
    fs::write(&table_path, b"q,n,per_matrix_us\n5,20,17\n").unwrap();
    let mut accelerator_cell = campaign.cells[0].clone();
    accelerator_cell.q = 5;
    accelerator_cell.n = 20;
    accelerator_cell.shards[0].stream_index += 1;
    accelerator_cell.backend = Backend::Accelerator;
    campaign.cells.push(accelerator_cell);
    campaign.provenance.invocation.extend([
        "--accelerator-cost-table".to_owned(),
        relative_table.to_owned(),
        "--accelerator-launch-cap-ms".to_owned(),
        "500".to_owned(),
    ]);
    fs::write(
        campaign_root.join("manifest.json"),
        serde_json::to_vec_pretty(&campaign).unwrap(),
    )
    .unwrap();
    commit_fixture(&repository, "bind accelerator costs");

    let original = fs::read(&table_path).unwrap();
    let mut first_entries = 0_u8;
    let first = execute_campaign_cell_with_evaluator(
        &campaign_root,
        ExactCellScope { q: 7, n: 20 },
        1,
        arm_for_manifest(&campaign, 7, 20).argv,
        approve_emission(&campaign_root).unwrap(),
        &evidence_source_paths(),
        |_, _, _, _| {
            first_entries += 1;
            fs::write(&table_path, b"q,n,per_matrix_us\n5,20,19\n").unwrap();
            Err(ScheduleError::InvalidWorkItem(
                "stop after mutating the execution input".to_owned(),
            ))
        },
        |_| {},
    );
    assert!(first.is_err());
    assert_eq!(first_entries, 1);

    let mut refused_entries = 0_u8;
    assert!(execute_campaign_cell_with_evaluator(
        &campaign_root,
        ExactCellScope { q: 7, n: 20 },
        1,
        arm_for_manifest(&campaign, 7, 20).argv,
        approve_emission(&campaign_root).unwrap(),
        &evidence_source_paths(),
        |_, _, _, _| {
            refused_entries += 1;
            Err(ScheduleError::InvalidWorkItem(
                "changed cost input must refuse before sampling".to_owned(),
            ))
        },
        |_| {},
    )
    .is_err());
    assert_eq!(refused_entries, 0);

    fs::write(&table_path, original).unwrap();
    let persisted = CampaignCoordinator::read(&campaign_root).unwrap();
    let arm = &persisted.receipt().arms[0];
    let identity = arm.accelerator_cost_table.as_ref().unwrap();
    assert_eq!(identity.path.as_str(), relative_table);
    assert!(matches!(
        persisted.receipt().attempts[0].state,
        ShardAttemptState::Authorized
    ));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn exact_executor_lock_contention_has_zero_sampler_entry() {
    let (root, campaign_root, campaign, approval) = live_fixture();
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
    assert!(execute_campaign_cell_with_evaluator(
        &campaign_root,
        ExactCellScope { q: 7, n: 20 },
        1,
        arm_for_manifest(&campaign, 7, 20).argv,
        approval,
        &evidence_source_paths(),
        |_, _, _, _| {
            entries += 1;
            Err(ScheduleError::InvalidWorkItem(
                "contended evaluator must not run".to_owned(),
            ))
        },
        |_| {},
    )
    .is_err());
    assert_eq!(entries, 0);
    drop(lock);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn stale_scheduled_snapshot_cannot_overwrite_authorized_accepted_or_completed() {
    let (root, campaign, mut initial) = fixture();
    let campaign_root = root.join(campaign.campaign_id.to_string());
    initial.authorize_arm(arm(7, 20)).unwrap();
    initial.persist(&campaign_root).unwrap();
    let stale = CampaignCoordinator::read(&campaign_root).unwrap();

    let mut current = CampaignCoordinator::read(&campaign_root).unwrap();
    current.authorize_attempt(7, 20, 0).unwrap();
    current.persist(&campaign_root).unwrap();
    assert!(stale.persist(&campaign_root).is_err());
    assert!(matches!(
        CampaignCoordinator::read(&campaign_root)
            .unwrap()
            .receipt()
            .attempts
            .last()
            .unwrap()
            .state,
        ShardAttemptState::Authorized
    ));

    write_shard(&campaign_root, &campaign, 7, 20, 0, 14, None);
    current.record_accepted(&campaign_root, 7, 20, 0).unwrap();
    current
        .record_completed(7, 20, 100, 14, DeterminantCount::NotEvaluated)
        .unwrap();
    current.persist(&campaign_root).unwrap();
    assert!(stale.persist(&campaign_root).is_err());
    let terminal = CampaignCoordinator::read(&campaign_root).unwrap();
    assert!(matches!(
        terminal.receipt().attempts.last().unwrap().state,
        ShardAttemptState::Accepted { .. }
    ));
    assert!(matches!(
        terminal.cell_state(7, 20),
        Some(CellExecutionState::Completed { .. })
    ));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn receipt_summary_is_terminal_monotonic_and_closed() {
    let (root, campaign, mut coordinator) = fixture();
    let campaign_root = root.join(campaign.campaign_id.to_string());
    let checksums = campaign_root.join("checksums.sha256");
    fs::write(&checksums, b"raw-checksum-fixture\n").unwrap();
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

    assert_eq!(fs::read(&checksums).unwrap(), b"raw-checksum-fixture\n");
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn active_attempt_refuses_symlinked_raw_evidence_before_adoption() {
    use std::os::unix::fs::symlink;

    let (root, campaign_root, campaign, approval) = live_fixture();
    let mut coordinator =
        CampaignCoordinator::new(&campaign_root, &evidence_source_paths()).unwrap();
    coordinator
        .authorize_arm(arm_for_manifest(&campaign, 7, 20))
        .unwrap();
    coordinator.persist(&campaign_root).unwrap();
    coordinator.authorize_attempt(7, 20, 0).unwrap();
    coordinator.persist(&campaign_root).unwrap();

    let raw_path = campaign_root.join(shard_record_file(7, 20, 0));
    let target = root.join("symlink-target.json");
    let bytes = write_shard(&campaign_root, &campaign, 7, 20, 0, 14, None);
    fs::rename(&raw_path, &target).unwrap();
    symlink(&target, &raw_path).unwrap();

    let mut sampler_entries = 0_u8;
    let result = execute_campaign_cell_with_evaluator(
        &campaign_root,
        ExactCellScope { q: 7, n: 20 },
        1,
        arm_for_manifest(&campaign, 7, 20).argv,
        approval,
        &evidence_source_paths(),
        |_, _, _, _| {
            sampler_entries += 1;
            Err(ScheduleError::InvalidWorkItem(
                "symlink evidence must refuse before recovery".to_owned(),
            ))
        },
        |_| {},
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

#[cfg(unix)]
#[test]
fn transaction_refuses_symlinked_writer_ancestors_without_external_effects() {
    use std::os::unix::fs::symlink;

    for attacked in ["derived", "shards"] {
        let (root, campaign_root, campaign, approval) = live_fixture();
        let external = root.join(format!("external-{attacked}"));
        fs::create_dir_all(&external).unwrap();
        let sentinel = external.join("sentinel");
        fs::write(&sentinel, b"outside campaign\n").unwrap();
        symlink(&external, campaign_root.join(attacked)).unwrap();

        let mut sampler_entries = 0_u8;
        let result = execute_campaign_cell_with_evaluator(
            &campaign_root,
            ExactCellScope { q: 7, n: 20 },
            1,
            arm_for_manifest(&campaign, 7, 20).argv,
            approval,
            &evidence_source_paths(),
            |manifest, item, _, _| {
                sampler_entries += 1;
                Ok(evaluated_shard(manifest, item, 14))
            },
            |_| {},
        );
        assert!(result.is_err(), "{attacked} ancestor symlink was followed");
        assert_eq!(
            sampler_entries, 0,
            "{attacked} ancestor attack reached the sampler"
        );
        assert_eq!(fs::read(&sentinel).unwrap(), b"outside campaign\n");
        assert_eq!(
            fs::read_dir(&external).unwrap().count(),
            1,
            "{attacked} ancestor attack created external campaign evidence"
        );
        fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(unix)]
#[test]
fn transaction_refuses_symlinked_lock_and_receipt_entries() {
    use std::os::unix::fs::symlink;

    for attacked in ["execution.lock", "coordinator-receipt.json"] {
        let (root, campaign_root, campaign, approval) = live_fixture();
        let coordinator_dir = campaign_root
            .join("derived")
            .join(campaign.campaign_id.to_string())
            .join("campaign-coordinator");
        fs::create_dir_all(&coordinator_dir).unwrap();
        let target = root.join(format!("external-{attacked}"));
        fs::write(&target, b"outside campaign\n").unwrap();
        symlink(&target, coordinator_dir.join(attacked)).unwrap();

        let mut sampler_entries = 0_u8;
        let result = execute_campaign_cell_with_evaluator(
            &campaign_root,
            ExactCellScope { q: 7, n: 20 },
            1,
            arm_for_manifest(&campaign, 7, 20).argv,
            approval,
            &evidence_source_paths(),
            |manifest, item, _, _| {
                sampler_entries += 1;
                Ok(evaluated_shard(manifest, item, 14))
            },
            |_| {},
        );
        assert!(result.is_err(), "{attacked} symlink was followed");
        assert_eq!(sampler_entries, 0, "{attacked} attack reached the sampler");
        assert_eq!(fs::read(&target).unwrap(), b"outside campaign\n");
        fs::remove_dir_all(root).unwrap();
    }
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

#[cfg(unix)]
#[test]
fn invalid_raw_refuses_symlinked_quarantine_ancestor() {
    use std::os::unix::fs::symlink;

    let (root, campaign_root, campaign, approval) = live_fixture();
    let mut coordinator =
        CampaignCoordinator::new(&campaign_root, &evidence_source_paths()).unwrap();
    coordinator
        .authorize_arm(arm_for_manifest(&campaign, 7, 20))
        .unwrap();
    coordinator.persist(&campaign_root).unwrap();
    coordinator.authorize_attempt(7, 20, 0).unwrap();
    coordinator.persist(&campaign_root).unwrap();

    let raw = campaign_root.join(shard_record_file(7, 20, 0));
    fs::create_dir_all(raw.parent().unwrap()).unwrap();
    fs::write(&raw, b"{invalid durable raw").unwrap();
    let coordinator_dir = campaign_root
        .join("derived")
        .join(campaign.campaign_id.to_string())
        .join("campaign-coordinator");
    let external = root.join("external-quarantine");
    fs::create_dir_all(&external).unwrap();
    let sentinel = external.join("sentinel");
    fs::write(&sentinel, b"outside campaign\n").unwrap();
    symlink(&external, coordinator_dir.join("quarantine")).unwrap();

    let mut sampler_entries = 0_u8;
    let result = execute_campaign_cell_with_evaluator(
        &campaign_root,
        ExactCellScope { q: 7, n: 20 },
        1,
        arm_for_manifest(&campaign, 7, 20).argv,
        approval,
        &evidence_source_paths(),
        |_, _, _, _| {
            sampler_entries += 1;
            Err(ScheduleError::InvalidWorkItem(
                "invalid durable raw is handled before sampling".to_owned(),
            ))
        },
        |_| {},
    );
    assert!(result.is_err());
    assert_eq!(sampler_entries, 0);
    assert_eq!(fs::read(&sentinel).unwrap(), b"outside campaign\n");
    assert_eq!(fs::read_dir(&external).unwrap().count(), 1);
    assert_eq!(fs::read(&raw).unwrap(), b"{invalid durable raw");
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

#[cfg(unix)]
#[test]
fn coordinator_refuses_symlinked_protocol_identity() {
    use std::os::unix::fs::symlink;

    let (root, campaign, _) = fixture();
    let campaign_root = root.join(campaign.campaign_id.to_string());
    let protocol = root.join("protocol.md");
    let external = root.join("external-protocol.md");
    fs::write(&external, b"outside protocol\n").unwrap();
    fs::remove_file(&protocol).unwrap();
    symlink(&external, &protocol).unwrap();
    assert!(CampaignCoordinator::new(&campaign_root, &evidence_source_paths()).is_err());
    assert_eq!(fs::read(&external).unwrap(), b"outside protocol\n");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn q3_target_parser_is_exact_and_precision_boundaries_are_closed() {
    let canonical = include_bytes!(
        "../../../../dev/simulation_results/permanent-zero-fraction/\
         scheinerman2024-q3-targets-v1.csv"
    );
    let targets = parse_q3_target_table(canonical).unwrap();
    assert_eq!(targets.len(), 25);
    assert_eq!(targets.first().unwrap().n, 4);
    assert_eq!(targets.last().unwrap().n, 28);
    assert_eq!(targets[0].evidence, Q3SourceEvidence::ExactEnumeration);
    assert_eq!(targets[2].evidence, Q3SourceEvidence::MonteCarlo);
    assert_eq!(
        targets[0].reported_probability,
        Q3ReportedProbability::Reported { value: 0.3976 }
    );
    assert_eq!(
        targets[1].reported_probability,
        Q3ReportedProbability::Reported { value: 0.3744 }
    );

    let malformed = String::from_utf8(canonical.to_vec()).unwrap().replacen(
        "q,n,source_table,",
        "q,n,unexpected,source_table,",
        1,
    );
    assert!(parse_q3_target_table(malformed.as_bytes()).is_err());
    let missing_n = String::from_utf8(canonical.to_vec())
        .unwrap()
        .lines()
        .filter(|line| !line.starts_with("3,28,"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(parse_q3_target_table(missing_n.as_bytes()).is_err());
    let forged_exact_probability = String::from_utf8(canonical.to_vec()).unwrap().replacen(
        ",0.3976,0.397622690007,",
        ",0.9990,0.397622690007,",
        1,
    );
    assert!(parse_q3_target_table(forged_exact_probability.as_bytes()).is_err());
    let noncanonical_exact_probability = String::from_utf8(canonical.to_vec()).unwrap().replacen(
        ",0.3976,0.397622690007,",
        ",0.39760,0.397622690007,",
        1,
    );
    assert!(parse_q3_target_table(noncanonical_exact_probability.as_bytes()).is_err());

    assert_eq!(
        classify_q3_precision(Q3SourceEvidence::ExactEnumeration, 99.0, 1.0),
        Q3PrecisionClassification::PriorExact
    );
    assert_eq!(
        classify_q3_precision(Q3SourceEvidence::MonteCarlo, 0.899_999, 1.0),
        Q3PrecisionClassification::ExceedsPriorPrecision
    );
    assert_eq!(
        classify_q3_precision(Q3SourceEvidence::MonteCarlo, 0.9, 1.0),
        Q3PrecisionClassification::MatchesPriorPrecision
    );
    assert_eq!(
        classify_q3_precision(Q3SourceEvidence::MonteCarlo, 1.1, 1.0),
        Q3PrecisionClassification::MatchesPriorPrecision
    );
    assert_eq!(
        classify_q3_precision(Q3SourceEvidence::MonteCarlo, 1.100_001, 1.0),
        Q3PrecisionClassification::BelowPriorPrecision
    );
}

#[test]
fn transaction_refuses_mismatched_literature_identity_before_receipt_or_sampling() {
    let campaign = single_cell_manifest(7);
    let (root, campaign_root, campaign, approval) = live_fixture_from_manifest(campaign);
    let expected_sources = evidence_source_paths();
    let repository = root.join("checkout");
    fs::write(
        repository.join(expected_sources.q5_q7_literature_search.path.as_str()),
        b"# Unrelated document\n\n## Limitations\nNone recorded.\n\n## Conclusion\nNo claim.\n",
    )
    .unwrap();

    let mut evaluator_entries = 0_u8;
    let result = execute_campaign_cell_with_evaluator(
        &campaign_root,
        ExactCellScope { q: 7, n: 20 },
        1,
        arm_for_manifest(&campaign, 7, 20).argv,
        approval,
        &expected_sources,
        |_, _, _, _| {
            evaluator_entries += 1;
            Err(ScheduleError::InvalidWorkItem(
                "identity mismatch must prevent evaluator entry".to_owned(),
            ))
        },
        |_| {},
    );
    assert!(result.is_err());
    assert_eq!(evaluator_entries, 0);
    assert!(!coordinator_receipt_path(&campaign_root, &campaign.campaign_id).exists());
    assert!(!campaign_root.join(shard_record_file(7, 20, 0)).exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn transaction_derives_strict_q3_and_literature_sidecars_from_bound_sources() {
    let mut campaign = single_cell_manifest(7);
    campaign
        .cells
        .push(cell(3, 4, DeterminantPlan::NotEvaluated));
    campaign
        .cells
        .push(cell(5, 4, DeterminantPlan::NotEvaluated));
    let (root, campaign_root, campaign, approval) = live_fixture_from_manifest(campaign);
    let expected_sources = evidence_source_paths();
    let q7_argv = arm_for_manifest(&campaign, 7, 20).argv;
    execute_campaign_cell_with_evaluator(
        &campaign_root,
        ExactCellScope { q: 7, n: 20 },
        1,
        q7_argv,
        approval,
        &expected_sources,
        |manifest, item, _, _| Ok(evaluated_shard(manifest, item, 14)),
        |_| {},
    )
    .unwrap();
    execute_campaign_cell_with_evaluator(
        &campaign_root,
        ExactCellScope { q: 5, n: 4 },
        1,
        arm_for_manifest(&campaign, 5, 4).argv,
        approve_emission(&campaign_root).unwrap(),
        &expected_sources,
        |manifest, item, _, _| Ok(evaluated_shard(manifest, item, 20)),
        |_| {},
    )
    .unwrap();
    execute_campaign_cell_with_evaluator(
        &campaign_root,
        ExactCellScope { q: 3, n: 4 },
        1,
        arm_for_manifest(&campaign, 3, 4).argv,
        approve_emission(&campaign_root).unwrap(),
        &expected_sources,
        |manifest, item, _, _| Ok(evaluated_shard(manifest, item, 70)),
        |_| {},
    )
    .unwrap();
    let persisted = CampaignCoordinator::read(&campaign_root).unwrap();
    assert_eq!(&persisted.receipt().evidence_sources, &expected_sources);

    let q3_path = coordinator_field_sidecar_path(&campaign_root, &campaign.campaign_id, 3);
    let q3_bytes = fs::read(&q3_path).unwrap();
    let q3: CoordinatorFieldSidecar = serde_json::from_slice(&q3_bytes).unwrap();
    assert_eq!(q3.status, CoordinatorFieldSidecarStatus::Completed);
    assert_eq!(q3.q, 3);
    assert_eq!(q3.source_records.len(), 1);
    let summary_bytes = fs::read(campaign_root.join(field_summary_file(3))).unwrap();
    assert_eq!(
        q3.field_summary.path.as_str(),
        "dev/simulation_results/permanent-zero-fraction/coordinator-fixture/summaries/q3.json"
    );
    assert_eq!(
        q3.field_summary.sha256.as_str(),
        format!("{:x}", Sha256::digest(&summary_bytes))
    );
    let CoordinatorFieldInterpretation::PublishedTargetComparison { target_table, rows } =
        q3.interpretation
    else {
        panic!("q=3 must carry the canonical target comparison");
    };
    assert_eq!(target_table, expected_sources.q3_targets);
    assert_eq!(rows.len(), 1);
    let CoordinatorQ3ComparisonRow::Completed {
        n,
        interval_relation,
        interval_excludes_published,
        precision_classification,
        ..
    } = rows[0]
    else {
        panic!("completed q=3 receipt row must stay completed");
    };
    assert_eq!(n, 4);
    assert_eq!(interval_relation, Q3IntervalRelation::Disjoint);
    assert!(interval_excludes_published);
    assert_eq!(
        precision_classification,
        Q3PrecisionClassification::PriorExact
    );

    let q7: CoordinatorFieldSidecar = serde_json::from_slice(
        &fs::read(coordinator_field_sidecar_path(
            &campaign_root,
            &campaign.campaign_id,
            7,
        ))
        .unwrap(),
    )
    .unwrap();
    let CoordinatorFieldInterpretation::ConditionalLiteratureSearch {
        search_receipt,
        claim,
    } = q7.interpretation
    else {
        panic!("q=7 must carry the bounded literature claim");
    };
    assert_eq!(
        search_receipt,
        expected_sources.q5_q7_literature_search.clone()
    );
    assert_eq!(
        claim,
        LiteratureSearchClaim::NoLocatedQ5Q7NumericsSubjectToRecordedLimits
    );
    let q5: CoordinatorFieldSidecar = serde_json::from_slice(
        &fs::read(coordinator_field_sidecar_path(
            &campaign_root,
            &campaign.campaign_id,
            5,
        ))
        .unwrap(),
    )
    .unwrap();
    assert!(matches!(
        q5.interpretation,
        CoordinatorFieldInterpretation::ConditionalLiteratureSearch {
            search_receipt,
            claim: LiteratureSearchClaim::NoLocatedQ5Q7NumericsSubjectToRecordedLimits,
        } if search_receipt == expected_sources.q5_q7_literature_search
    ));

    let mut forged: serde_json::Value = serde_json::from_slice(&q3_bytes).unwrap();
    forged["forged"] = serde_json::json!(true);
    assert!(serde_json::from_value::<CoordinatorFieldSidecar>(forged).is_err());
    let mut forged_nested: serde_json::Value = serde_json::from_slice(&q3_bytes).unwrap();
    forged_nested["interpretation"]["caller_prose"] = serde_json::json!("forged");
    assert!(serde_json::from_value::<CoordinatorFieldSidecar>(forged_nested).is_err());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn terminal_projection_recovers_without_sampling_and_refuses_conflicts() {
    let campaign = single_cell_manifest(7);
    let (root, campaign_root, campaign, approval) = live_fixture_from_manifest(campaign);
    let sidecar = coordinator_field_sidecar_path(&campaign_root, &campaign.campaign_id, 7);
    let mut first_entries = 0_u8;
    let first = execute_campaign_cell_with_evaluator(
        &campaign_root,
        ExactCellScope { q: 7, n: 20 },
        1,
        arm_for_manifest(&campaign, 7, 20).argv,
        approval,
        &evidence_source_paths(),
        |manifest, item, _, _| {
            first_entries += 1;
            Ok(evaluated_shard(manifest, item, 14))
        },
        |_| {
            fs::create_dir_all(sidecar.parent().unwrap()).unwrap();
            fs::write(&sidecar, b"{\"forged\":true}").unwrap();
        },
    );
    assert!(first.is_err());
    assert_eq!(first_entries, 1);
    assert!(matches!(
        CampaignCoordinator::read(&campaign_root)
            .unwrap()
            .cell_state(7, 20),
        Some(CellExecutionState::Completed { .. })
    ));
    assert!(campaign_root.join(field_summary_file(7)).is_file());

    fs::remove_file(&sidecar).unwrap();
    let mut retry_entries = 0_u8;
    execute_campaign_cell_with_evaluator(
        &campaign_root,
        ExactCellScope { q: 7, n: 20 },
        1,
        arm_for_manifest(&campaign, 7, 20).argv,
        approve_emission(&campaign_root).unwrap(),
        &evidence_source_paths(),
        |_, _, _, _| {
            retry_entries += 1;
            Err(ScheduleError::InvalidWorkItem(
                "terminal projection retry must not sample".to_owned(),
            ))
        },
        |_| {},
    )
    .unwrap();
    assert_eq!(retry_entries, 0);
    let canonical = fs::read(&sidecar).unwrap();
    let summary = fs::read(campaign_root.join(field_summary_file(7))).unwrap();

    let mut adopted_entries = 0_u8;
    execute_campaign_cell_with_evaluator(
        &campaign_root,
        ExactCellScope { q: 7, n: 20 },
        1,
        arm_for_manifest(&campaign, 7, 20).argv,
        approve_emission(&campaign_root).unwrap(),
        &evidence_source_paths(),
        |_, _, _, _| {
            adopted_entries += 1;
            Err(ScheduleError::InvalidWorkItem(
                "idempotent adoption must not sample".to_owned(),
            ))
        },
        |_| {},
    )
    .unwrap();
    assert_eq!(adopted_entries, 0);
    assert_eq!(fs::read(&sidecar).unwrap(), canonical);
    assert_eq!(
        fs::read(campaign_root.join(field_summary_file(7))).unwrap(),
        summary
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn zero_evidence_halt_publishes_raw_only_halted_projection() {
    let campaign = manifest();
    let (root, campaign_root, campaign, approval) = live_fixture_from_manifest(campaign);
    let execution = execute_campaign_cell_with_evaluator(
        &campaign_root,
        ExactCellScope { q: 7, n: 20 },
        1,
        arm_for_manifest(&campaign, 7, 20).argv,
        approval,
        &evidence_source_paths(),
        |_, _, _, _| {
            Err(ScheduleError::InvalidWorkItem(
                "zero-evidence mechanical halt".to_owned(),
            ))
        },
        |_| {},
    )
    .unwrap();
    assert!(matches!(
        execution.terminal_state,
        CellExecutionState::Halted { .. }
    ));
    let summary = read_field_summary(&campaign_root, 7).unwrap();
    assert!(summary
        .rows
        .iter()
        .all(|row| row.matrix_count == 0 && row.permanent_zero_count == 0));
    let sidecar: CoordinatorFieldSidecar = serde_json::from_slice(
        &fs::read(coordinator_field_sidecar_path(
            &campaign_root,
            &campaign.campaign_id,
            7,
        ))
        .unwrap(),
    )
    .unwrap();
    assert_eq!(sidecar.status, CoordinatorFieldSidecarStatus::Halted);
    assert!(sidecar.source_records.is_empty());
    let q3_bytes = fs::read(coordinator_field_sidecar_path(
        &campaign_root,
        &campaign.campaign_id,
        3,
    ))
    .unwrap();
    let q3: CoordinatorFieldSidecar = serde_json::from_slice(&q3_bytes).unwrap();
    let CoordinatorFieldInterpretation::PublishedTargetComparison { rows, .. } = q3.interpretation
    else {
        panic!("halted q=3 field must retain the typed target-comparison container");
    };
    assert!(matches!(
        rows.as_slice(),
        [CoordinatorQ3ComparisonRow::Halted {
            matrix_count: 0,
            permanent_zero_count: 0,
            ..
        }]
    ));
    let q3_json = String::from_utf8(q3_bytes).unwrap();
    for forbidden in [
        "source_target",
        "campaign_measurement",
        "estimate",
        "interval_relation",
        "precision_classification",
    ] {
        assert!(
            !q3_json.contains(forbidden),
            "halted q=3 leaked {forbidden}"
        );
    }
    fs::remove_dir_all(root).unwrap();
}
