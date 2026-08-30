use std::fs;
use std::path::PathBuf;

use gf2_sim::permanent_campaign::acceptance::{
    assess_completed_cell, determinant_null_probability, AcceptanceFamily, AcceptancePlan,
};
use gf2_sim::permanent_campaign::coordinator::{
    coordinator_receipt_path, emit_field_sidecar, ArmInvocation, CampaignCoordinator,
    CampaignHaltState, CellExecutionState, FieldInterpretation, ShardAttempt,
    ShardAttemptOutcome,
};
use gf2_sim::permanent_campaign::driver::CampaignExecutionScope;
use gf2_sim::permanent_campaign::schema::{
    AcceptanceVerdict, ArtifactIdentity, Availability, Backend, CampaignManifest, CellSpec,
    CellTerminalState, DeterminantCount, DeterminantPlan, GitRevision, Provenance, RngAlgorithm,
    ShardSpec, StreamPurpose, SCHEMA_VERSION,
};

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
            cell(5, 4, DeterminantPlan::Evaluate),
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
            invocation: vec!["permanent_campaign".to_owned()],
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
        scope: CampaignExecutionScope::ExactCell { q, n },
        argv: vec![
            "permanent_campaign".to_owned(),
            "--q".to_owned(),
            q.to_string(),
            "--n".to_owned(),
            n.to_string(),
        ],
        worker_count: 1,
        executable_sha256: "b".repeat(64).parse().unwrap(),
    }
}

#[test]
fn acceptance_uses_one_budget_exact_log_tails_and_finite_n_determinants() {
    let campaign = manifest();
    let plan = AcceptancePlan::for_manifest(&campaign).unwrap();
    assert_eq!(plan.global_error(), 0.05);
    assert_eq!(
        plan.family_budget(AcceptanceFamily::PermanentFloor)
            + plan.family_budget(AcceptanceFamily::Determinant),
        plan.global_error()
    );
    assert_eq!(plan.family_test_count(AcceptanceFamily::PermanentFloor), 4);
    assert_eq!(plan.family_test_count(AcceptanceFamily::Determinant), 2);

    let underflow = assess_completed_cell(
        &plan,
        7,
        20,
        1_000_000,
        0,
        DeterminantCount::NotEvaluated,
    )
    .unwrap();
    assert!(underflow.permanent.test.log_p_value < -745.0);
    assert_eq!(underflow.permanent.test.verdict, AcceptanceVerdict::Rejected);
    assert!(matches!(
        underflow.summary.terminal_state,
        CellTerminalState::Completed { .. }
    ));
    assert!(underflow.determinant.is_none());

    let finite = determinant_null_probability(3, 4);
    let near_limit = determinant_null_probability(3, 100);
    assert!((near_limit - finite).abs() > 0.003);
}

#[test]
fn coordinator_enforces_first_cell_retry_and_contradiction_preservation() {
    let campaign = manifest();
    let mut coordinator = CampaignCoordinator::new(
        campaign.clone(),
        artifact("dev/simulation_results/permanent/manifest.json", 'c'),
        artifact("dev/simulation_results/permanent/protocol.md", 'd'),
    )
    .unwrap();
    assert!(coordinator.authorize_arm(arm(7, 4)).is_err());
    coordinator.authorize_arm(arm(7, 20)).unwrap();

    coordinator
        .record_attempt(ShardAttempt {
            q: 7,
            n: 20,
            shard_id: 0,
            attempt: 1,
            outcome: ShardAttemptOutcome::Quarantined {
                error: "mechanical fixture failure".to_owned(),
            },
        })
        .unwrap();
    coordinator
        .record_attempt(ShardAttempt {
            q: 7,
            n: 20,
            shard_id: 0,
            attempt: 2,
            outcome: ShardAttemptOutcome::Accepted {
                record: artifact("shards/q7/n20/shard-000000.json", 'e'),
            },
        })
        .unwrap();
    assert!(coordinator
        .record_attempt(ShardAttempt {
            q: 7,
            n: 20,
            shard_id: 0,
            attempt: 3,
            outcome: ShardAttemptOutcome::Quarantined {
                error: "forbidden third attempt".to_owned(),
            },
        })
        .is_err());

    let rejected = assess_completed_cell(
        coordinator.acceptance_plan(),
        7,
        20,
        100,
        0,
        DeterminantCount::NotEvaluated,
    )
    .unwrap();
    coordinator
        .record_completed(rejected, vec![artifact("shards/q7/n20/shard-000000.json", 'e')])
        .unwrap();
    assert!(matches!(
        coordinator.cell_state(7, 20).unwrap(),
        CellExecutionState::Completed { .. }
    ));
    assert!(matches!(
        coordinator.cell_state(7, 4).unwrap(),
        CellExecutionState::Halted { .. }
    ));
    assert!(matches!(coordinator.halt_state(), CampaignHaltState::Halted { .. }));
}

#[test]
fn terminal_cells_persist_then_assemble_one_field_summary_and_closed_sidecars() {
    let campaign = manifest();
    let mut coordinator = CampaignCoordinator::new(
        campaign,
        artifact("dev/simulation_results/permanent/manifest.json", 'c'),
        artifact("dev/simulation_results/permanent/protocol.md", 'd'),
    )
    .unwrap();
    for (q, n, zeros) in [(7, 20, 15), (7, 4, 16)] {
        coordinator.authorize_arm(arm(q, n)).unwrap();
        let completed = assess_completed_cell(
            coordinator.acceptance_plan(),
            q,
            n,
            100,
            zeros,
            DeterminantCount::NotEvaluated,
        )
        .unwrap();
        coordinator
            .record_completed(completed, vec![artifact(&format!("shards/q{q}/n{n}.json"), 'e')])
            .unwrap();
    }
    let summary = coordinator.assemble_field_summary(7).unwrap();
    assert_eq!(summary.rows.iter().map(|row| row.n).collect::<Vec<_>>(), [4, 20]);

    let root = std::env::temp_dir().join(format!(
        "gf2-coordinator-fixture-{}",
        std::process::id()
    ));
    let campaign_root = root.join("coordinator-fixture");
    fs::create_dir_all(&campaign_root).unwrap();
    coordinator.persist(&campaign_root).unwrap();
    assert!(coordinator_receipt_path(&campaign_root).is_file());
    let reloaded = CampaignCoordinator::read(&campaign_root).unwrap();
    assert_eq!(reloaded.assemble_field_summary(7).unwrap(), summary);

    let q5 = emit_field_sidecar(
        &campaign_root,
        reloaded.receipt(),
        5,
        vec![artifact("summaries/q5.json", 'f')],
        FieldInterpretation::LiteratureSearchBasis {
            search_receipt: artifact(
                "dev/studies/b488f02c/literature-search-2026-08-08.md",
                '1',
            ),
            licensed_statement: "the recorded search found no prior numerics for q in {5,7}, subject to its stated limits".to_owned(),
            limits: vec!["one general index is not systematic".to_owned()],
        },
    )
    .unwrap();
    assert_eq!(
        q5.strip_prefix(&campaign_root).unwrap(),
        PathBuf::from("derived/coordinator-fixture/campaign-coordinator/field-sidecars/q5.json")
    );
    assert!(emit_field_sidecar(
        &campaign_root,
        reloaded.receipt(),
        3,
        Vec::new(),
        FieldInterpretation::LiteratureSearchBasis {
            search_receipt: artifact("dev/studies/search.md", '1'),
            licensed_statement: "wrong field".to_owned(),
            limits: Vec::new(),
        },
    )
    .is_err());
    fs::remove_dir_all(root).unwrap();
}
