//! Behavioral contracts of the Zen 3 benchmark protocol tooling: addendum
//! schema, paired bootstrap statistics, receipt acceptance on deterministic
//! fixtures, the protocol document's frozen settings, and the shared runner.

use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tuning_campaign_support::abtest::{
    bonferroni_confidence, bootstrap_seed, decide, pair_orders, paired_bootstrap_speedup,
    wilson_interval_95, ArmOrder, BootstrapInterval, Decision, Margins, PairedObservation,
    SplitMix64,
};
use tuning_campaign_support::host::{CoreArm, HostObservation};
use tuning_campaign_support::journal::{
    CheckpointStore, ExecutionLog, JournalEvent, ResumeIdentity, TerminalState,
};
use tuning_campaign_support::protocol::{
    sha256_hex, ArmBuilds, ArtifactPin, Batching, BuildIdentity, CacheState, CellDeclaration,
    CellObjective, CellRole, CodeIdentity, CodewordSource, ComplexityBudget, DecoderArmKind,
    DecoderCell, DecoderInput, EffectRule, FamilyAddendum, FamilyIdentity, FamilyPurpose,
    FamilyWise, Frozen, Holdout, MetricKind, Normalization, NormalizationKind, Precision,
    ProtocolRef, QualityTolerance, ReceiptLabel, ResolutionEvidence, RunnerPlan, Scaling, Schedule,
    SearchBudget, Stopping, StoppingKind, WorkerDeclaration, Workload, ADDENDUM_SCHEMA_ID,
    ADDENDUM_SCHEMA_PATH, CONTRACT_PATH, PROTOCOL_ID, PROTOCOL_PATH, PROTOCOL_VERSION,
    RECEIPT_SCHEMA_ID, RUNNER_LIFECYCLE_SCHEMA, SHARED_SETTINGS,
};
use tuning_campaign_support::provenance::ProducingInputs;
use tuning_campaign_support::receipt::{
    evaluate, render_markdown, ArmQuality, ArmRecord, BenchmarkReceipt, CampaignFacts, CellClaim,
    CellOutcome, CellRecord, CellStatus, CheckpointRecord, DecoderArmSettings,
    DecoderQualityRecord, ExecutionRecord, IterationDistribution, LockRecord, LogRecord,
    PairRecord, Severity, SourceIdentity, Verdict, WindowRecord, WorkerReport, CHECKPOINT_DIR,
    LOG_FILE, PLAN_FILE, RECEIPT_FILE,
};
use tuning_campaign_support::schema;

const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";
const CAMPAIGN_SEED: u64 = 7;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .unwrap()
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("gf2-f547c394-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Copies the committed protocol documents into a scratch repository root.
fn stage_repo(root: &Path) {
    for relative in [PROTOCOL_PATH, ADDENDUM_SCHEMA_PATH, CONTRACT_PATH] {
        let target = root.join(relative);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::copy(repo_root().join(relative), target).unwrap();
    }
    let source = "producer.rs";
    fs::write(root.join(source), b"fn measured_behavior() {}\n").unwrap();
    let manifest = json!({
        "schema": "tuning-campaign-producing-inputs-v1",
        "behavior_sources": [source],
        "lifecycle_sources": [source],
        "build_inputs": [source]
    });
    fs::write(
        root.join("producing-inputs.json"),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
}

/// Copies the runner's declared producing-input closure into a scratch repo.
fn stage_runner_producing(root: &Path) {
    const MANIFEST: &str = "dev/active/f547c394/producing-inputs.json";
    let source_root = repo_root();
    let producing = ProducingInputs::read_at(&source_root, MANIFEST).unwrap();
    for relative in
        std::iter::once(MANIFEST).chain(producing.build_inputs.iter().map(String::as_str))
    {
        let target = root.join(relative);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::copy(source_root.join(relative), target).unwrap();
    }
}

fn pin(root: &Path, relative: &str) -> ArtifactPin {
    ArtifactPin {
        path: relative.to_owned(),
        snapshot: format!("inputs/{}", relative.replace('/', "-")),
        sha256: sha256_hex(&fs::read(root.join(relative)).unwrap()),
    }
}

fn cell(id: &str, objective: CellObjective, role: CellRole, core_arm: CoreArm) -> CellDeclaration {
    CellDeclaration {
        cell_id: id.to_owned(),
        objective,
        role,
        workload: Workload {
            identity: "xor-fold".into(),
            size: BTreeMap::from([("words".to_owned(), 65_536)]),
            seed: 1,
        },
        metric_kind: MetricKind::KernelIsolated,
        scaling: Scaling::SingleCoreLatency,
        core_arm,
        workers: WorkerDeclaration {
            declared: 1,
            nested_pools_allowed: false,
        },
        cache_state: CacheState::Warm,
        builds: ArmBuilds {
            baseline: BuildIdentity::ConservativePortable,
            candidate: BuildIdentity::ConservativePortable,
        },
        conversion_costs_included: false,
        decoder: None,
    }
}

fn addendum(cells: Vec<CellDeclaration>) -> FamilyAddendum {
    FamilyAddendum {
        schema: ADDENDUM_SCHEMA_ID.into(),
        protocol: ProtocolRef {
            id: PROTOCOL_ID.into(),
            version: PROTOCOL_VERSION,
        },
        family: FamilyIdentity {
            id: "fixture-family".into(),
            issue: "f547c394".into(),
            purpose: FamilyPurpose::KernelFamily,
            description: "deterministic fixture family".into(),
        },
        frozen: Frozen {
            frozen_utc: Some("2026-09-06T00:00:00Z".into()),
        },
        effect: EffectRule {
            worthwhile_speedup: Some(1.5),
            rationale: "fixture".into(),
            measurement_resolution: Some(0.05),
            resolution_evidence: Some(ResolutionEvidence {
                receipt: "__fixture_pilot__".into(),
                sha256: "0".repeat(64),
            }),
            equivalence_margin: Some(1.1),
            equivalence_rationale: "fixture".into(),
            material_gap_threshold: Some(1.5),
            material_gap_rationale: "fixture".into(),
        },
        complexity_budget: ComplexityBudget {
            max_new_unsafe_kernels: 0,
            max_added_source_lines: Some(100),
            maintenance_rationale: "fixture".into(),
        },
        family_wise: FamilyWise {
            alpha: 0.05,
            prior_confirmatory_trials: 0,
            prior_trials: Vec::new(),
        },
        search_budget: SearchBudget {
            max_pilot_trials_per_cell: 2,
            max_confirmatory_attempts_per_candidate: 1,
        },
        holdout: Holdout {
            required: false,
            cells: Vec::new(),
        },
        cells,
    }
}

fn write_addendum(root: &Path, addendum: &FamilyAddendum) {
    let relative = "dev/active/f547c394/addendum-fixture.json";
    let mut bytes = serde_json::to_vec_pretty(addendum).unwrap();
    bytes.push(b'\n');
    fs::write(root.join(relative), bytes).unwrap();
}

/// Deterministic windows: five per execution, jittered around `ns`.
fn execution(
    arm: &str,
    mixer: &mut SplitMix64,
    ns: f64,
    jitter: f64,
    cpus: &[u32],
) -> ExecutionRecord {
    let windows: Vec<WindowRecord> = (0..SHARED_SETTINGS.windows_per_execution)
        .map(|_| {
            let unit = (mixer.next_u64() >> 11) as f64 / (1u64 << 53) as f64;
            let value = ns * (1.0 + jitter * (2.0 * unit - 1.0));
            WindowRecord {
                calls: 1000,
                elapsed_ns: (value * 1000.0).round().max(1.0) as u64,
            }
        })
        .collect();
    let mut values: Vec<f64> = windows.iter().map(|window| window.ns_per_call()).collect();
    values.sort_by(|a, b| a.partial_cmp(b).unwrap());
    ExecutionRecord {
        arm: arm.to_owned(),
        pid: 4242,
        ns_per_call: values[values.len() / 2],
        windows,
        cache_state_applied: CacheState::Warm,
        workers_observed: 1,
        cpus_observed: cpus.to_vec(),
        selected_path: Some("fixture".into()),
        conversion: None,
        elapsed_ns: 1_000_000,
    }
}

struct CellSpec {
    id: &'static str,
    speedup: f64,
    jitter: f64,
    unavailable: Option<&'static str>,
    claim: bool,
    quality: Option<DecoderQualityRecord>,
}

fn pairs(
    spec: &CellSpec,
    role: CellRole,
    key: &str,
    cpus: &[u32],
    baseline_arm: &str,
    candidate_arm: &str,
) -> Vec<PairRecord> {
    let count = match role {
        CellRole::Exploratory => SHARED_SETTINGS.pilot_min_pairs,
        _ => SHARED_SETTINGS.confirmatory_pairs,
    } as usize;
    let mut mixer = SplitMix64::new(bootstrap_seed(CAMPAIGN_SEED, key) ^ 0x5eed);
    pair_orders(bootstrap_seed(CAMPAIGN_SEED, key), count)
        .into_iter()
        .enumerate()
        .map(|(index, order)| PairRecord {
            index: index as u32,
            order,
            baseline: execution(baseline_arm, &mut mixer, 200.0, spec.jitter, cpus),
            candidate: execution(
                candidate_arm,
                &mut mixer,
                200.0 / spec.speedup,
                spec.jitter,
                cpus,
            ),
        })
        .collect()
}

fn fixture_arm_name(side: &str, build: BuildIdentity) -> String {
    let suffix = match build {
        BuildIdentity::ConservativePortable => "portable",
        BuildIdentity::TunedPortable => "tuned",
        BuildIdentity::Native => "native",
        BuildIdentity::External => "external",
    };
    format!("{side}-{suffix}")
}

fn identity(
    protocol: &ArtifactPin,
    producing: &tuning_campaign_support::provenance::ProducingSnapshot,
    plan: &RunnerPlan,
    arms: &BTreeMap<String, ArmRecord>,
    host: &HostObservation,
) -> ResumeIdentity {
    ResumeIdentity {
        protocol_digest: protocol.sha256.clone(),
        source_revision: COMMIT.into(),
        source_sha256: producing.identity_sha256().unwrap(),
        ordered_work_manifest_sha256: sha256_hex(&serde_json::to_vec(&plan.cells).unwrap()),
        process_descriptors_sha256: sha256_hex(&serde_json::to_vec(arms).unwrap()),
        executable_sha256: arms
            .iter()
            .map(|(name, arm)| (name.clone(), arm.executable_sha256.clone()))
            .collect(),
        behavior_sha256: producing.behavior_sha256.clone(),
        lifecycle_schema: RUNNER_LIFECYCLE_SCHEMA.into(),
        lifecycle_behavior_sha256: producing.lifecycle_sha256().unwrap(),
        feature_contract: "release".into(),
        thread_contract: "RAYON_NUM_THREADS=unset".into(),
        host_identity: host.affinity.host_identity().unwrap(),
    }
}

struct Built {
    repo: PathBuf,
    dir: PathBuf,
}

fn fixture_pilot_receipt(repo: &Path, addendum: ArtifactPin) -> BenchmarkReceipt {
    BenchmarkReceipt {
        schema: RECEIPT_SCHEMA_ID.into(),
        campaign_id: "fixture-pilot".into(),
        issue: "f547c394".into(),
        receipt_path: "dev/bench_results/f547c394/pilot/receipt.json".into(),
        label: ReceiptLabel::Pilot,
        campaign_seed: CAMPAIGN_SEED,
        settings: SHARED_SETTINGS,
        settings_deviation: false,
        protocol: pin(repo, PROTOCOL_PATH),
        contract: pin(repo, CONTRACT_PATH),
        addendum_schema: pin(repo, ADDENDUM_SCHEMA_PATH),
        addendum,
        source: SourceIdentity {
            revision: COMMIT.into(),
            producing: ProducingInputs::capture(repo, "producing-inputs.json").unwrap(),
        },
        toolchain: "rustc fixture".into(),
        host: HostObservation::observe().unwrap(),
        lock: LockRecord {
            lock_path: "/tmp/gf2-ccx1.lock".into(),
            holder_pid: 4242,
            observation: "fixture".into(),
            wrapper: "dev/scripts/ccx1-bench-flock.sh".into(),
        },
        workers: WorkerReport {
            environment_rayon_threads: None,
            runner_threads: 1,
        },
        execution_log: LogRecord {
            path: LOG_FILE.into(),
            sha256: sha256_hex(b"pilot log"),
            sessions: 1,
            resumed: false,
        },
        checkpoints: CheckpointRecord {
            manifest_path: format!("{CHECKPOINT_DIR}/manifest.json"),
            manifest_sha256: sha256_hex(b"pilot checkpoints"),
        },
        arms: BTreeMap::new(),
        cells: Vec::new(),
    }
}

/// Writes a complete receipt directory: journal, checkpoints and receipt.
fn build_receipt(
    name: &str,
    addendum: &FamilyAddendum,
    specs: &[CellSpec],
    resume_after_first: bool,
    repeat_first: bool,
    mutate: impl FnOnce(&mut BenchmarkReceipt),
) -> Built {
    let root = scratch(name);
    let repo = root.join("repo");
    stage_repo(&repo);
    let dir = repo.join("dev/bench_results/f547c394/confirmation");
    fs::create_dir_all(&dir).unwrap();
    let pilot_addendum_path = "dev/active/f547c394/addendum-fixture-pilot.json";
    let mut pilot_addendum = addendum.clone();
    pilot_addendum.effect.measurement_resolution = None;
    pilot_addendum.effect.resolution_evidence = None;
    for cell in &mut pilot_addendum.cells {
        cell.role = CellRole::Exploratory;
    }
    let mut pilot_addendum_bytes = serde_json::to_vec_pretty(&pilot_addendum).unwrap();
    pilot_addendum_bytes.push(b'\n');
    fs::write(repo.join(pilot_addendum_path), pilot_addendum_bytes).unwrap();
    let pilot_path = "dev/bench_results/f547c394/pilot/receipt.json";
    let pilot = fixture_pilot_receipt(&repo, pin(&repo, pilot_addendum_path));
    let mut pilot_bytes = serde_json::to_vec_pretty(&pilot).unwrap();
    pilot_bytes.push(b'\n');
    fs::create_dir_all(repo.join(pilot_path).parent().unwrap()).unwrap();
    fs::write(repo.join(pilot_path), &pilot_bytes).unwrap();
    let mut addendum = addendum.clone();
    if addendum
        .effect
        .resolution_evidence
        .as_ref()
        .is_some_and(|evidence| evidence.receipt == "__fixture_pilot__")
    {
        addendum.effect.resolution_evidence = Some(ResolutionEvidence {
            receipt: pilot_path.into(),
            sha256: sha256_hex(&pilot_bytes),
        });
    }
    write_addendum(&repo, &addendum);
    let protocol_pin =
        ArtifactPin::capture(&repo, &dir, PROTOCOL_PATH, "inputs/protocol.md").unwrap();
    let contract_pin =
        ArtifactPin::capture(&repo, &dir, CONTRACT_PATH, "inputs/measurement-contract.md").unwrap();
    let schema_pin = ArtifactPin::capture(
        &repo,
        &dir,
        ADDENDUM_SCHEMA_PATH,
        "inputs/addendum.schema.json",
    )
    .unwrap();
    let addendum_pin = ArtifactPin::capture(
        &repo,
        &dir,
        "dev/active/f547c394/addendum-fixture.json",
        "inputs/family-addendum.json",
    )
    .unwrap();
    fs::create_dir_all(dir.join("inputs/resolution-evidence")).unwrap();
    fs::write(
        dir.join("inputs/resolution-evidence/receipt.json"),
        &pilot_bytes,
    )
    .unwrap();
    let producing = ProducingInputs::capture_to(
        &repo,
        "producing-inputs.json",
        &dir.join("inputs/producing"),
    )
    .unwrap();
    let campaign = "fixture-campaign";
    let mut arms = BTreeMap::new();
    for spec in specs {
        let declared = addendum.cell(spec.id).unwrap();
        for (side, build) in [
            ("baseline", declared.builds.baseline),
            ("candidate", declared.builds.candidate),
        ] {
            let name = fixture_arm_name(side, build);
            arms.entry(name.clone()).or_insert_with(|| ArmRecord {
                build,
                description: name.clone(),
                executable_path: "/usr/bin/true".into(),
                executable_sha256: sha256_hex(name.as_bytes()),
                arguments: Vec::new(),
                environment: BTreeMap::new(),
                rustflags: None,
                tuning_profile: None,
            });
        }
    }
    let plan_arms: BTreeMap<String, Value> = arms
        .iter()
        .map(|(name, arm)| {
            (
                name.clone(),
                json!({
                    "build": arm.build,
                    "description": arm.description,
                    "executable": arm.executable_path,
                    "arguments": arm.arguments,
                    "environment": arm.environment,
                    "rustflags": arm.rustflags,
                    "tuning_profile": arm.tuning_profile,
                }),
            )
        })
        .collect();
    let plan = json!({
        "schema": "zen3-benchmark-plan-v1",
        "campaign_id": campaign,
        "issue": "f547c394",
        "label": "confirmation",
        "campaign_seed": CAMPAIGN_SEED,
        "addendum": "dev/active/f547c394/addendum-fixture.json",
        "lock_path": "/tmp/gf2-ccx1.lock",
        "wrapper": "dev/scripts/ccx1-bench-flock.sh",
        "timing_override": null,
        "arms": plan_arms,
        "cells": specs.iter().map(|spec| {
            let declared = addendum.cell(spec.id).unwrap();
            json!({
                "cell_id": spec.id,
                "baseline_arm": fixture_arm_name("baseline", declared.builds.baseline),
                "candidate_arm": fixture_arm_name("candidate", declared.builds.candidate),
                "case": {"fixture": spec.id},
                "pilot_pairs": null
            })
        }).collect::<Vec<_>>(),
        "max_cells_per_session": if resume_after_first { Some(1) } else { None }
    });
    let mut plan_bytes = serde_json::to_vec_pretty(&plan).unwrap();
    plan_bytes.push(b'\n');
    fs::write(dir.join(PLAN_FILE), &plan_bytes).unwrap();
    let plan = RunnerPlan::decode(&plan_bytes).unwrap();
    plan.validate(&addendum).unwrap();
    let host = HostObservation::observe().unwrap();
    let identity = identity(&protocol_pin, &producing, &plan, &arms, &host);
    let facts = CampaignFacts {
        plan_sha256: sha256_hex(&plan_bytes),
        identity: identity.clone(),
        protocol: protocol_pin.clone(),
        contract: contract_pin.clone(),
        addendum_schema: schema_pin.clone(),
        addendum: addendum_pin.clone(),
        source: SourceIdentity {
            revision: COMMIT.into(),
            producing: producing.clone(),
        },
        toolchain: "rustc fixture".into(),
        settings: SHARED_SETTINGS,
        settings_deviation: false,
        arms: arms.clone(),
    };
    let mut log = ExecutionLog::create_new(&dir, campaign, "session-1").unwrap();
    log.append(JournalEvent::CampaignStart, None, json!(facts))
        .unwrap();
    log.append(
        JournalEvent::OrchestrationStart,
        None,
        json!({"kind": "execution-log-announced", "path": log.path()}),
    )
    .unwrap();
    let mut checkpoints =
        CheckpointStore::create_new(dir.join(CHECKPOINT_DIR), campaign, identity).unwrap();
    let cpus = vec![host.affinity.cpus()[0]];
    let confidence = bonferroni_confidence(0.05, addendum.family_comparisons()).unwrap();
    let mut cells = Vec::new();
    for (index, spec) in specs.iter().enumerate() {
        if resume_after_first && index == 1 {
            log.terminal(TerminalState::Paused, json!({"measured_cells": 1}))
                .unwrap();
            log = ExecutionLog::resume(dir.join(LOG_FILE), campaign, "session-2").unwrap();
            log.append(
                JournalEvent::OrchestrationStart,
                None,
                json!({"kind": "execution-log-announced", "path": log.path()}),
            )
            .unwrap();
            log.append(
                JournalEvent::Omission,
                Some(json!({"key": specs[0].id})),
                json!({"kind": "completed-in-prior-session"}),
            )
            .unwrap();
            if repeat_first {
                log.append(
                    JournalEvent::CellStart,
                    Some(json!({"key": specs[0].id})),
                    json!({}),
                )
                .unwrap();
            }
        }
        let declared = addendum.cell(spec.id).unwrap();
        let baseline_arm = fixture_arm_name("baseline", declared.builds.baseline);
        let candidate_arm = fixture_arm_name("candidate", declared.builds.candidate);
        let key = spec.id.to_owned();
        let case = json!({
            "key": key,
            "cell_id": spec.id,
            "case": {"fixture": spec.id}
        });
        log.append(JournalEvent::CellStart, Some(case.clone()), json!({}))
            .unwrap();
        let mut record = CellRecord {
            cell_id: spec.id.into(),
            key: key.clone(),
            role: declared.role,
            baseline_arm: baseline_arm.clone(),
            candidate_arm: candidate_arm.clone(),
            core_arm: declared.core_arm,
            resolved_cpus: Vec::new(),
            status: CellStatus::Unavailable,
            unavailable_reason: spec.unavailable.map(str::to_owned),
            pairs: Vec::new(),
            claimed: None,
            decoder_quality: spec.quality.clone(),
            checkpoint_sha256: None,
        };
        if spec.unavailable.is_none() {
            record.status = CellStatus::Measured;
            record.resolved_cpus = cpus.clone();
            record.pairs = pairs(
                spec,
                declared.role,
                &key,
                &cpus,
                &baseline_arm,
                &candidate_arm,
            );
            if spec.claim {
                let observations: Vec<_> = record
                    .pairs
                    .iter()
                    .map(|pair| PairedObservation {
                        baseline_ns_per_call: pair.baseline.ns_per_call,
                        candidate_ns_per_call: pair.candidate.ns_per_call,
                    })
                    .collect();
                let interval = paired_bootstrap_speedup(
                    &observations,
                    SHARED_SETTINGS.bootstrap_resamples,
                    confidence,
                    bootstrap_seed(CAMPAIGN_SEED, &key),
                )
                .unwrap();
                let margins = addendum.margins(declared).unwrap();
                record.claimed = Some(CellClaim {
                    interval,
                    decision: decide(&interval, &margins).unwrap(),
                    margins,
                });
            }
        }
        let accepted = checkpoints.accept(&key, &case, &record).unwrap();
        record.checkpoint_sha256 = Some(accepted.sha256);
        log.append(
            JournalEvent::CheckpointAccepted,
            Some(case.clone()),
            json!({}),
        )
        .unwrap();
        log.append(JournalEvent::CellComplete, Some(case), json!({}))
            .unwrap();
        cells.push(record);
    }
    log.terminal(TerminalState::Complete, json!({})).unwrap();
    drop(log);
    let log_bytes = fs::read(dir.join(LOG_FILE)).unwrap();
    let manifest_bytes = fs::read(dir.join(CHECKPOINT_DIR).join("manifest.json")).unwrap();
    let sessions = if resume_after_first { 2 } else { 1 };
    let mut receipt = BenchmarkReceipt {
        schema: RECEIPT_SCHEMA_ID.into(),
        campaign_id: campaign.into(),
        issue: "f547c394".into(),
        receipt_path: "dev/bench_results/f547c394/confirmation/receipt.json".into(),
        label: ReceiptLabel::Confirmation,
        campaign_seed: CAMPAIGN_SEED,
        settings: SHARED_SETTINGS,
        settings_deviation: false,
        protocol: protocol_pin,
        contract: contract_pin,
        addendum_schema: schema_pin,
        addendum: addendum_pin,
        source: SourceIdentity {
            revision: COMMIT.into(),
            producing,
        },
        toolchain: "rustc fixture".into(),
        host,
        lock: LockRecord {
            lock_path: "/tmp/gf2-ccx1.lock".into(),
            holder_pid: 4242,
            observation: "fixture".into(),
            wrapper: "dev/scripts/ccx1-bench-flock.sh".into(),
        },
        workers: WorkerReport {
            environment_rayon_threads: None,
            runner_threads: 1,
        },
        execution_log: LogRecord {
            path: LOG_FILE.into(),
            sha256: sha256_hex(&log_bytes),
            sessions,
            resumed: sessions > 1,
        },
        checkpoints: CheckpointRecord {
            manifest_path: format!("{CHECKPOINT_DIR}/manifest.json"),
            manifest_sha256: sha256_hex(&manifest_bytes),
        },
        arms,
        cells,
    };
    mutate(&mut receipt);
    fs::write(
        dir.join(RECEIPT_FILE),
        serde_json::to_vec_pretty(&receipt).unwrap(),
    )
    .unwrap();
    Built { repo, dir }
}

fn spec(id: &'static str, speedup: f64) -> CellSpec {
    CellSpec {
        id,
        speedup,
        jitter: 0.02,
        unavailable: None,
        claim: true,
        quality: None,
    }
}

fn outcome(summary: &tuning_campaign_support::receipt::AcceptanceSummary, id: &str) -> CellOutcome {
    summary
        .cells
        .iter()
        .find(|cell| cell.cell_id == id)
        .unwrap()
        .outcome
}

#[test]
fn addendum_schema_accepts_the_frozen_smoke_addendum_and_rejects_unknown_fields() {
    let root = repo_root();
    let schema_value: Value =
        serde_json::from_slice(&fs::read(root.join(ADDENDUM_SCHEMA_PATH)).unwrap()).unwrap();
    let bytes = fs::read(root.join("dev/active/f547c394/addendum-protocol-smoke.json")).unwrap();
    let instance: Value = serde_json::from_slice(&bytes).unwrap();
    assert!(schema::validate(&schema_value, &instance).is_empty());
    let smoke = FamilyAddendum::decode(&bytes).unwrap();
    smoke.validate().unwrap();
    assert_eq!(smoke.family.id, "protocol-smoke");
    let pilot_bytes =
        fs::read(root.join("dev/active/f547c394/addendum-protocol-smoke-pilot.json")).unwrap();
    let pilot_instance: Value = serde_json::from_slice(&pilot_bytes).unwrap();
    assert!(schema::validate(&schema_value, &pilot_instance).is_empty());
    let pilot = FamilyAddendum::decode(&pilot_bytes).unwrap();
    pilot.validate().unwrap();
    assert_eq!(pilot.family.id, "protocol-smoke-pilot");
    let mut extra = instance.clone();
    extra["effect"]["bonus"] = json!(1);
    let violations = schema::validate(&schema_value, &extra);
    assert!(violations
        .iter()
        .any(|v| v.instance_path == "/effect/bonus" && v.message.contains("bonus")));
    assert!(FamilyAddendum::decode(serde_json::to_vec(&extra).unwrap().as_slice()).is_err());
    let mut wrong = instance.clone();
    wrong["family_wise"]["alpha"] = json!(0.1);
    assert!(!schema::validate(&schema_value, &wrong).is_empty());
    let decoded = FamilyAddendum::decode(serde_json::to_vec(&wrong).unwrap().as_slice()).unwrap();
    assert!(decoded.validate().is_err());
}

#[test]
fn unresolved_required_settings_make_a_cell_non_confirmatory() {
    let mut family = addendum(vec![cell(
        "a",
        CellObjective::Improvement,
        CellRole::Confirmatory,
        CoreArm::SingleCore,
    )]);
    family.effect.worthwhile_speedup = None;
    family.frozen = Frozen { frozen_utc: None };
    family.validate().unwrap();
    let unresolved = family.unresolved_settings(&family.cells[0]);
    assert!(unresolved.contains(&"effect.worthwhile_speedup".to_owned()));
    assert!(family.margins(&family.cells[0]).is_none());
    let built = build_receipt(
        "unresolved",
        &family,
        &[CellSpec {
            claim: false,
            ..spec("a", 2.0)
        }],
        false,
        false,
        |_| {},
    );
    let summary = evaluate(&built.dir).unwrap();
    assert_eq!(summary.verdict, Verdict::Accepted);
    assert_eq!(outcome(&summary, "a"), CellOutcome::NotConfirmatory);
    assert!(!summary.qualifies);
    assert_eq!(summary.cells[0].unresolved_settings, unresolved);
}

#[test]
fn paired_bootstrap_interval_is_deterministic_and_brackets_known_ratios() {
    let mut mixer = SplitMix64::new(3);
    let observations: Vec<_> = (0..24)
        .map(|_| {
            let unit = (mixer.next_u64() >> 11) as f64 / (1u64 << 53) as f64;
            let baseline = 100.0 * (1.0 + 0.04 * (unit - 0.5));
            PairedObservation {
                baseline_ns_per_call: baseline,
                candidate_ns_per_call: baseline / 2.0,
            }
        })
        .collect();
    let first = paired_bootstrap_speedup(&observations, 2000, 0.95, 11).unwrap();
    let second = paired_bootstrap_speedup(&observations, 2000, 0.95, 11).unwrap();
    assert_eq!(first, second);
    assert!(first.lower <= 2.0 && 2.0 <= first.upper, "{first:?}");
    assert!((first.estimate - 2.0).abs() < 1e-9);
    assert!(first.upper - first.lower < 0.2, "{first:?}");
    assert_eq!(first.seed, 11);
    assert!(paired_bootstrap_speedup(&observations[..1], 2000, 0.95, 1).is_err());
}

#[test]
fn decisions_follow_confidence_bound_margins_not_significance() {
    let margins = Margins {
        improvement: 1.5,
        equivalence: 1.1,
    };
    let interval = |lower: f64, upper: f64| BootstrapInterval {
        estimate: (lower + upper) / 2.0,
        lower,
        upper,
        confidence: 0.95,
        resamples: 1000,
        seed: 1,
        pairs: 24,
    };
    assert_eq!(
        decide(&interval(1.6, 1.9), &margins).unwrap(),
        Decision::Improved
    );
    assert_eq!(
        decide(&interval(1.2, 1.4), &margins).unwrap(),
        Decision::NotWorse
    );
    // A "significant" 3% slowdown is still not worse under the equivalence margin.
    assert_eq!(
        decide(&interval(0.95, 0.99), &margins).unwrap(),
        Decision::NotWorse
    );
    assert_eq!(
        decide(&interval(0.7, 0.85), &margins).unwrap(),
        Decision::Regressed
    );
    // A wide interval around 1 that crosses the equivalence floor is inconclusive.
    assert_eq!(
        decide(&interval(0.85, 1.3), &margins).unwrap(),
        Decision::Inconclusive
    );
    assert!(decide(
        &interval(1.0, 1.1),
        &Margins {
            improvement: 1.0,
            equivalence: 1.1
        }
    )
    .is_err());
}

#[test]
fn bonferroni_level_widens_with_the_family_and_prior_trials() {
    assert!((bonferroni_confidence(0.05, 1).unwrap() - 0.95).abs() < 1e-12);
    assert!((bonferroni_confidence(0.05, 5).unwrap() - 0.99).abs() < 1e-12);
    assert!(bonferroni_confidence(0.05, 0).is_err());
    let mut family = addendum(vec![
        cell(
            "a",
            CellObjective::Improvement,
            CellRole::Confirmatory,
            CoreArm::SingleCore,
        ),
        cell(
            "b",
            CellObjective::NonRegression,
            CellRole::Confirmatory,
            CoreArm::SingleCore,
        ),
        cell(
            "p",
            CellObjective::Improvement,
            CellRole::Exploratory,
            CoreArm::SingleCore,
        ),
    ]);
    assert_eq!(family.family_comparisons(), 2);
    family.family_wise.prior_confirmatory_trials = 3;
    family.family_wise.prior_trials = (0..3)
        .map(|index| tuning_campaign_support::protocol::PriorTrial {
            receipt: format!("prior-{index}.json"),
            sha256: sha256_hex(&[index]),
            outcome: "fail".into(),
        })
        .collect();
    family.validate().unwrap();
    assert_eq!(family.family_comparisons(), 5);
}

#[test]
fn pair_orders_are_counterbalanced_and_seed_determined() {
    let orders = pair_orders(99, 24);
    assert_eq!(orders, pair_orders(99, 24));
    assert_ne!(orders, pair_orders(100, 24));
    for block in orders.chunks(2) {
        assert_ne!(block[0], block[1]);
    }
    assert_eq!(
        orders
            .iter()
            .filter(|order| **order == ArmOrder::BaselineFirst)
            .count(),
        12
    );
    assert_eq!(pair_orders(5, 7).len(), 7);
}

#[test]
fn acceptance_passes_a_confirmed_improvement_receipt() {
    let family = addendum(vec![cell(
        "a",
        CellObjective::Improvement,
        CellRole::Confirmatory,
        CoreArm::SingleCore,
    )]);
    let built = build_receipt("pass", &family, &[spec("a", 2.0)], false, false, |_| {});
    let summary = evaluate(&built.dir).unwrap();
    assert_eq!(summary.verdict, Verdict::Accepted, "{:?}", summary.findings);
    assert_eq!(outcome(&summary, "a"), CellOutcome::Pass);
    assert!(summary.qualifies);
    let interval = summary.cells[0].interval.unwrap();
    assert!(interval.lower >= 1.5 && interval.lower <= 2.0 && interval.upper >= 2.0);
    assert_eq!(interval.pairs, 24);
    assert_eq!(interval.resamples, SHARED_SETTINGS.bootstrap_resamples);
    assert!(!summary.resumed);
    assert!(summary
        .findings
        .iter()
        .all(|finding| finding.severity == Severity::Note));
}

#[test]
fn acceptance_fails_a_regression_beyond_the_equivalence_margin() {
    let family = addendum(vec![cell(
        "a",
        CellObjective::NonRegression,
        CellRole::Confirmatory,
        CoreArm::SingleCore,
    )]);
    let built = build_receipt("fail", &family, &[spec("a", 0.8)], false, false, |_| {});
    let summary = evaluate(&built.dir).unwrap();
    assert_eq!(summary.verdict, Verdict::Accepted, "{:?}", summary.findings);
    assert_eq!(outcome(&summary, "a"), CellOutcome::Fail);
    assert_eq!(summary.cells[0].decision, Some(Decision::Regressed));
    assert!(!summary.qualifies);
}

#[test]
fn acceptance_marks_an_interval_spanning_the_margin_inconclusive() {
    let mut family = addendum(vec![cell(
        "a",
        CellObjective::NonRegression,
        CellRole::Confirmatory,
        CoreArm::SingleCore,
    )]);
    family.effect.equivalence_margin = Some(1.02);
    let built = build_receipt(
        "inconclusive",
        &family,
        &[CellSpec {
            jitter: 0.12,
            ..spec("a", 1.0 / 1.02)
        }],
        false,
        false,
        |_| {},
    );
    let summary = evaluate(&built.dir).unwrap();
    assert_eq!(summary.verdict, Verdict::Accepted, "{:?}", summary.findings);
    let cell = &summary.cells[0];
    let interval = cell.interval.unwrap();
    assert!(
        interval.lower < 1.0 / 1.02 && interval.upper >= 1.0 / 1.02,
        "{interval:?}"
    );
    assert_eq!(cell.outcome, CellOutcome::Inconclusive);
    assert!(!summary.qualifies);
}

#[test]
fn acceptance_rejects_a_receipt_with_missing_protocol_identity() {
    let family = addendum(vec![cell(
        "a",
        CellObjective::Improvement,
        CellRole::Confirmatory,
        CoreArm::SingleCore,
    )]);
    let built = build_receipt(
        "missing-identity",
        &family,
        &[spec("a", 2.0)],
        false,
        false,
        |receipt| {
            receipt.protocol.snapshot = "inputs/missing-protocol.md".into();
            receipt.addendum.sha256 = sha256_hex(b"other");
        },
    );
    let summary = evaluate(&built.dir).unwrap();
    assert_eq!(summary.verdict, Verdict::Rejected);
    assert!(summary
        .findings
        .iter()
        .any(|f| f.rule == "P-02" && f.severity == Severity::Error));
    assert!(summary
        .findings
        .iter()
        .any(|f| f.rule == "P-03" && f.message.contains("content digest")));
    assert!(!summary.qualifies);
    // An intact receipt is independently verifiable without a repository.
    let intact = build_receipt("portable", &family, &[spec("a", 2.0)], false, false, |_| {});
    let summary = evaluate(&intact.dir).unwrap();
    assert_eq!(summary.verdict, Verdict::Accepted, "{:?}", summary.findings);
}

#[test]
fn acceptance_rejects_a_snapshot_path_that_escapes_the_receipt() {
    let family = addendum(vec![cell(
        "a",
        CellObjective::Improvement,
        CellRole::Confirmatory,
        CoreArm::SingleCore,
    )]);
    let built = build_receipt(
        "escaping-snapshot",
        &family,
        &[spec("a", 2.0)],
        false,
        false,
        |receipt| receipt.protocol.snapshot = "inputs/../../protocol.md".into(),
    );
    let summary = evaluate(&built.dir).unwrap();
    assert_eq!(summary.verdict, Verdict::Rejected);
    assert!(summary.findings.iter().any(|finding| {
        finding.rule == "P-02"
            && finding.severity == Severity::Error
            && finding.message.contains("literal relative path")
    }));
}

#[test]
fn acceptance_rejects_a_missing_snapshot_despite_matching_source_bytes() {
    let family = addendum(vec![cell(
        "a",
        CellObjective::Improvement,
        CellRole::Confirmatory,
        CoreArm::SingleCore,
    )]);
    let built = build_receipt(
        "missing-snapshot",
        &family,
        &[spec("a", 2.0)],
        false,
        false,
        |_| {},
    );
    let receipt =
        BenchmarkReceipt::decode(&fs::read(built.dir.join(RECEIPT_FILE)).unwrap()).unwrap();
    fs::remove_file(built.dir.join(receipt.protocol.snapshot)).unwrap();
    let summary = evaluate(&built.dir).unwrap();
    assert_eq!(summary.verdict, Verdict::Rejected);
    assert!(summary.findings.iter().any(|finding| {
        finding.rule == "P-02"
            && finding.severity == Severity::Error
            && finding.message.contains("content snapshot")
    }));
}

#[test]
fn acceptance_rejects_a_digest_that_does_not_match_the_snapshot() {
    let family = addendum(vec![cell(
        "a",
        CellObjective::Improvement,
        CellRole::Confirmatory,
        CoreArm::SingleCore,
    )]);
    let built = build_receipt(
        "snapshot-digest-mismatch",
        &family,
        &[spec("a", 2.0)],
        false,
        false,
        |receipt| receipt.protocol.sha256 = sha256_hex(b"not the committed protocol"),
    );
    let summary = evaluate(&built.dir).unwrap();
    assert_eq!(summary.verdict, Verdict::Rejected);
    assert!(summary.findings.iter().any(|finding| {
        finding.rule == "P-02"
            && finding.severity == Severity::Error
            && finding.message.contains("content digest")
    }));
}

#[test]
fn source_control_metadata_does_not_change_acceptance() {
    let family = addendum(vec![cell(
        "a",
        CellObjective::Improvement,
        CellRole::Confirmatory,
        CoreArm::SingleCore,
    )]);
    let built = build_receipt(
        "metadata-independent",
        &family,
        &[spec("a", 2.0)],
        false,
        false,
        |_| {},
    );
    let mut receipt =
        BenchmarkReceipt::decode(&fs::read(built.dir.join(RECEIPT_FILE)).unwrap()).unwrap();
    receipt.source.revision = "a rapidly changing locator".into();
    fs::write(
        built.dir.join(RECEIPT_FILE),
        serde_json::to_vec_pretty(&receipt).unwrap(),
    )
    .unwrap();
    let summary = evaluate(&built.dir).unwrap();
    assert_eq!(summary.verdict, Verdict::Accepted, "{:?}", summary.findings);
}

#[test]
fn acceptance_rejects_changed_producing_input_snapshot() {
    let family = addendum(vec![cell(
        "a",
        CellObjective::Improvement,
        CellRole::Confirmatory,
        CoreArm::SingleCore,
    )]);
    let built = build_receipt(
        "changed-producing-input",
        &family,
        &[spec("a", 2.0)],
        false,
        false,
        |_| {},
    );
    fs::write(
        built.dir.join("inputs/producing/producer.rs"),
        b"fn measured_behavior_changed() {}\n",
    )
    .unwrap();
    let summary = evaluate(&built.dir).unwrap();
    assert_eq!(summary.verdict, Verdict::Rejected);
    assert!(summary.findings.iter().any(|finding| {
        finding.rule == "P-05"
            && finding.severity == Severity::Error
            && finding
                .message
                .contains("producing-input snapshot content differs")
    }));
}

#[test]
fn exploratory_only_receipts_never_qualify_for_production_selection() {
    let family = addendum(vec![cell(
        "pilot",
        CellObjective::Improvement,
        CellRole::Exploratory,
        CoreArm::SingleCore,
    )]);
    let built = build_receipt(
        "pilot-qualification",
        &family,
        &[spec("pilot", 2.0)],
        false,
        false,
        |_| {},
    );
    let summary = evaluate(&built.dir).unwrap();
    assert_eq!(summary.verdict, Verdict::Accepted, "{:?}", summary.findings);
    assert_eq!(outcome(&summary, "pilot"), CellOutcome::Pilot);
    assert!(
        !summary.qualifies,
        "exploration supplies no confirmatory evidence"
    );
}

#[test]
fn acceptance_rejects_every_self_consistent_frozen_fact_replacement() {
    let family = addendum(vec![cell(
        "a",
        CellObjective::Improvement,
        CellRole::Exploratory,
        CoreArm::SingleCore,
    )]);
    for replacement in [
        "protocol-pin",
        "contract-pin",
        "schema-pin",
        "addendum-pin",
        "executable",
        "execution-environment",
        "producing-inputs",
        "plan",
        "settings",
        "toolchain",
        "arm-descriptor",
        "checkpoint-identity",
        "missing-plan",
        "plan-case",
        "plan-arm",
    ] {
        let built = build_receipt(
            &format!("replaced-{replacement}"),
            &family,
            &[spec("a", 2.0)],
            false,
            false,
            |_| {},
        );
        let receipt_path = built.dir.join(RECEIPT_FILE);
        let mut receipt = BenchmarkReceipt::decode(&fs::read(&receipt_path).unwrap()).unwrap();
        match replacement {
            "protocol-pin" | "contract-pin" | "schema-pin" | "addendum-pin" => {
                let pin = match replacement {
                    "protocol-pin" => &mut receipt.protocol,
                    "contract-pin" => &mut receipt.contract,
                    "schema-pin" => &mut receipt.addendum_schema,
                    _ => &mut receipt.addendum,
                };
                let mut bytes = fs::read(built.dir.join(&pin.snapshot)).unwrap();
                bytes.push(b' ');
                fs::write(built.dir.join(&pin.snapshot), &bytes).unwrap();
                pin.sha256 = sha256_hex(&bytes);
                pin.verify_content(&built.dir).unwrap();
            }
            "executable" => {
                receipt
                    .arms
                    .get_mut("candidate-portable")
                    .unwrap()
                    .executable_sha256 = sha256_hex(b"another executable");
            }
            "execution-environment" => {
                receipt
                    .arms
                    .get_mut("candidate-portable")
                    .unwrap()
                    .environment
                    .insert("GF2_SMOKE_PASSES".into(), "3".into());
            }
            "producing-inputs" => {
                fs::write(
                    built.repo.join("producer.rs"),
                    b"fn replacement_measured_behavior() {}\n",
                )
                .unwrap();
                let snapshot_root = built.dir.join("inputs/producing");
                fs::remove_dir_all(&snapshot_root).unwrap();
                receipt.source.producing = ProducingInputs::capture_to(
                    &built.repo,
                    "producing-inputs.json",
                    &snapshot_root,
                )
                .unwrap();
            }
            "missing-plan" => {
                fs::remove_file(built.dir.join(PLAN_FILE)).unwrap();
            }
            "plan" | "plan-case" | "plan-arm" => {
                let plan_path = built.dir.join("plan.json");
                let mut plan: Value =
                    serde_json::from_slice(&fs::read(&plan_path).unwrap()).unwrap();
                match replacement {
                    "plan-case" => plan["cells"][0]["case"] = json!({"fixture": "different-input"}),
                    "plan-arm" => plan["cells"][0]["candidate_arm"] = json!("baseline-portable"),
                    _ => plan["max_cells_per_session"] = json!(1),
                }
                fs::write(plan_path, serde_json::to_vec_pretty(&plan).unwrap()).unwrap();
            }
            "settings" => {
                receipt.settings.window_target_ms += 1;
                receipt.settings_deviation = true;
            }
            "toolchain" => receipt.toolchain = "rustc replacement".into(),
            "arm-descriptor" => {
                receipt
                    .arms
                    .get_mut("candidate-portable")
                    .unwrap()
                    .description = "self-consistent replacement candidate".into();
            }
            "checkpoint-identity" => {}
            _ => unreachable!(),
        }
        if !matches!(replacement, "missing-plan" | "plan-case" | "plan-arm") {
            // Publish a completely consistent replacement content closure. The
            // opening record remains byte-for-byte unchanged: only its binding
            // can distinguish these replacements from valid standalone facts.
            let plan_path = built.dir.join(PLAN_FILE);
            let mut plan: RunnerPlan = RunnerPlan::decode(&fs::read(&plan_path).unwrap()).unwrap();
            for (name, arm) in &receipt.arms {
                let planned = plan.arms.get_mut(name).unwrap();
                planned.description = arm.description.clone();
                planned.environment = arm.environment.clone();
            }
            if replacement == "settings" {
                plan.timing_override = Some(tuning_campaign_support::protocol::TimingOverride {
                    windows_per_execution: receipt.settings.windows_per_execution,
                    window_target_ms: receipt.settings.window_target_ms,
                });
            }
            fs::write(&plan_path, serde_json::to_vec_pretty(&plan).unwrap()).unwrap();
            let mut replacement_identity = identity(
                &receipt.protocol,
                &receipt.source.producing,
                &plan,
                &receipt.arms,
                &receipt.host,
            );
            if replacement == "checkpoint-identity" {
                replacement_identity.feature_contract = "replacement-release".into();
            }
            let checkpoint_root = built.dir.join(CHECKPOINT_DIR);
            fs::remove_dir_all(&checkpoint_root).unwrap();
            let mut store = CheckpointStore::create_new(
                &checkpoint_root,
                &receipt.campaign_id,
                replacement_identity,
            )
            .unwrap();
            for cell in &mut receipt.cells {
                let planned = plan
                    .cells
                    .iter()
                    .find(|p| p.cell_id == cell.cell_id)
                    .unwrap();
                let case = json!({"key": cell.key, "cell_id": cell.cell_id, "case": planned.case});
                cell.checkpoint_sha256 = None;
                cell.checkpoint_sha256 = Some(store.accept(&cell.key, &case, cell).unwrap().sha256);
            }
            receipt.checkpoints.manifest_sha256 =
                sha256_hex(&fs::read(checkpoint_root.join("manifest.json")).unwrap());
            CheckpointStore::inspect(&checkpoint_root).unwrap();
        }
        fs::write(&receipt_path, serde_json::to_vec_pretty(&receipt).unwrap()).unwrap();
        let summary = evaluate(&built.dir).unwrap();
        assert_eq!(summary.verdict, Verdict::Rejected, "{replacement}");
        if !matches!(replacement, "missing-plan" | "plan-case" | "plan-arm") {
            assert!(
                summary
                    .findings
                    .iter()
                    .all(|finding| finding.severity != Severity::Error || finding.rule == "P-23"),
                "replacement must fail specifically at the freeze: {replacement}: {:?}",
                summary.findings
            );
        }
        assert!(
            summary
                .findings
                .iter()
                .any(|finding| finding.rule == "P-23" && finding.severity == Severity::Error),
            "{replacement}: {:?}",
            summary.findings
        );
    }
}

#[test]
fn acceptance_rejects_an_addendum_not_frozen_before_measurement() {
    let family = addendum(vec![cell(
        "a",
        CellObjective::Improvement,
        CellRole::Confirmatory,
        CoreArm::SingleCore,
    )]);
    let built = build_receipt(
        "unfrozen-addendum",
        &family,
        &[spec("a", 2.0)],
        false,
        false,
        |_| {},
    );
    let receipt_path = built.dir.join(RECEIPT_FILE);
    let mut receipt = BenchmarkReceipt::decode(&fs::read(&receipt_path).unwrap()).unwrap();
    let alternate = "inputs/alternate-addendum.json";
    fs::copy(
        built.dir.join(&receipt.addendum.snapshot),
        built.dir.join(alternate),
    )
    .unwrap();
    receipt.addendum.snapshot = alternate.into();
    fs::write(receipt_path, serde_json::to_vec_pretty(&receipt).unwrap()).unwrap();
    let summary = evaluate(&built.dir).unwrap();
    assert_eq!(summary.verdict, Verdict::Rejected);
    assert!(summary.findings.iter().any(|finding| {
        finding.rule == "P-23"
            && finding.severity == Severity::Error
            && finding.message.contains("campaign-start")
    }));
}

#[test]
fn acceptance_rejects_missing_and_self_referential_resolution_evidence() {
    let confirmation_path = "dev/bench_results/f547c394/confirmation/receipt.json";
    for (name, evidence, expected) in [
        (
            "missing-resolution-evidence",
            "dev/bench_results/f547c394/missing-pilot/receipt.json",
            "resolution evidence",
        ),
        (
            "self-resolution-evidence",
            confirmation_path,
            "receipt under evaluation",
        ),
    ] {
        let mut family = addendum(vec![cell(
            "a",
            CellObjective::Improvement,
            CellRole::Confirmatory,
            CoreArm::SingleCore,
        )]);
        family.effect.resolution_evidence = Some(ResolutionEvidence {
            receipt: evidence.into(),
            sha256: "0".repeat(64),
        });
        let built = build_receipt(name, &family, &[spec("a", 2.0)], false, false, |_| {});
        let summary = evaluate(&built.dir).unwrap();
        assert_eq!(summary.verdict, Verdict::Rejected, "{name}");
        assert!(summary.findings.iter().any(|finding| {
            finding.rule == "P-03"
                && finding.severity == Severity::Error
                && finding.message.contains(expected)
        }));
    }
}

#[test]
fn acceptance_accepts_a_resumed_run_and_rejects_a_repeated_cell() {
    let family = addendum(vec![
        cell(
            "a",
            CellObjective::Improvement,
            CellRole::Confirmatory,
            CoreArm::SingleCore,
        ),
        cell(
            "b",
            CellObjective::NonRegression,
            CellRole::Confirmatory,
            CoreArm::SingleCore,
        ),
    ]);
    let built = build_receipt(
        "resumed",
        &family,
        &[spec("a", 2.0), spec("b", 1.0)],
        true,
        false,
        |_| {},
    );
    let summary = evaluate(&built.dir).unwrap();
    assert_eq!(summary.verdict, Verdict::Accepted, "{:?}", summary.findings);
    assert_eq!(summary.sessions, 2);
    assert!(summary.resumed);
    assert_eq!(outcome(&summary, "a"), CellOutcome::Pass);
    assert_eq!(outcome(&summary, "b"), CellOutcome::Pass);
    // A session that starts a completed cell again contradicts checkpoint/resume.
    let repeated = build_receipt(
        "repeated",
        &family,
        &[spec("a", 2.0), spec("b", 1.0)],
        true,
        true,
        |_| {},
    );
    let summary = evaluate(&repeated.dir).unwrap();
    assert_eq!(summary.verdict, Verdict::Rejected);
    assert!(summary
        .findings
        .iter()
        .any(|f| f.rule == "P-11" && f.message.contains("started 2 times")));
}

#[test]
fn acceptance_preserves_unavailable_and_not_material_cells() {
    let family = addendum(vec![
        cell(
            "a",
            CellObjective::Improvement,
            CellRole::Confirmatory,
            CoreArm::SingleCore,
        ),
        cell(
            "twelve",
            CellObjective::NonRegression,
            CellRole::Confirmatory,
            CoreArm::PhysicalCores12,
        ),
        cell(
            "p",
            CellObjective::Improvement,
            CellRole::Exploratory,
            CoreArm::SingleCore,
        ),
    ]);
    let built = build_receipt(
        "negative",
        &family,
        &[
            spec("a", 1.05),
            CellSpec {
                unavailable: Some("affinity mask holds 6 physical cores, 12 required"),
                ..spec("twelve", 1.0)
            },
            spec("p", 3.0),
        ],
        false,
        false,
        |_| {},
    );
    let summary = evaluate(&built.dir).unwrap();
    assert_eq!(summary.verdict, Verdict::Accepted, "{:?}", summary.findings);
    assert_eq!(outcome(&summary, "a"), CellOutcome::NotMaterial);
    assert_eq!(outcome(&summary, "twelve"), CellOutcome::Unavailable);
    assert_eq!(outcome(&summary, "p"), CellOutcome::Pilot);
    assert!(!summary.qualifies);
    // An unavailable outcome is still a durable checkpointed result. A valid
    // alternate reason in the receipt must not replace that frozen result.
    let receipt_path = built.dir.join(RECEIPT_FILE);
    let original_receipt = fs::read(&receipt_path).unwrap();
    let mut receipt = BenchmarkReceipt::decode(&original_receipt).unwrap();
    receipt
        .cells
        .iter_mut()
        .find(|cell| cell.cell_id == "twelve")
        .unwrap()
        .unavailable_reason = Some("another locally plausible unavailable reason".into());
    fs::write(&receipt_path, serde_json::to_vec_pretty(&receipt).unwrap()).unwrap();
    let summary = evaluate(&built.dir).unwrap();
    assert_eq!(summary.verdict, Verdict::Rejected);
    assert!(summary.findings.iter().any(|finding| {
        finding.rule == "P-12"
            && finding.cell.as_deref() == Some("twelve")
            && finding.message.contains("checkpoint result")
    }));
    fs::write(&receipt_path, original_receipt).unwrap();
    let unavailable_unit = built
        .dir
        .join(CHECKPOINT_DIR)
        .join("units")
        .join(format!("{}.json", sha256_hex(b"twelve")));
    let unit_bytes = fs::read(&unavailable_unit).unwrap();
    fs::remove_file(&unavailable_unit).unwrap();
    let missing = evaluate(&built.dir).unwrap();
    assert_eq!(missing.verdict, Verdict::Rejected);
    assert_eq!(outcome(&missing, "twelve"), CellOutcome::Invalid);
    assert!(missing
        .findings
        .iter()
        .any(|finding| finding.rule == "P-12" && finding.cell.as_deref() == Some("twelve")));
    fs::write(unavailable_unit, unit_bytes).unwrap();
    // Dropping a declared cell from the receipt is a rejected incomplete negative result.
    let mut receipt = BenchmarkReceipt::decode(&fs::read(&receipt_path).unwrap()).unwrap();
    receipt.cells.retain(|cell| cell.cell_id != "twelve");
    fs::write(receipt_path, serde_json::to_vec_pretty(&receipt).unwrap()).unwrap();
    let summary = evaluate(&built.dir).unwrap();
    assert_eq!(summary.verdict, Verdict::Rejected);
    assert!(summary.findings.iter().any(|f| f.rule == "P-21"));
    let markdown = render_markdown(&summary);
    assert!(markdown.contains("P-21") && markdown.contains("NotMaterial"));
}

fn decoder(arm_kind: DecoderArmKind) -> DecoderCell {
    DecoderCell {
        arm_kind,
        code: CodeIdentity {
            identity: "nr-bg1-z384-mother".into(),
            n: 26_112,
            k: 8_448,
            h_sha256: sha256_hex(b"h"),
        },
        input: DecoderInput {
            llr_source: "recorded".into(),
            llr_sha256: Some(sha256_hex(b"llr")),
            frames: 1000,
            seed: 42,
            codeword_source: CodewordSource::Both,
            snr_db: 1.0,
        },
        precision: Precision::F32,
        schedule: Schedule::Flooding,
        normalization: Normalization {
            kind: NormalizationKind::NormalizedMinSum,
            factor: Some(0.75),
        },
        iteration_cap: 50,
        stopping: Stopping {
            kind: StoppingKind::Syndrome,
            crc: None,
        },
        batching: Batching {
            batch_size: 1,
            batch_fill_included: true,
        },
        quality_tolerance: QualityTolerance {
            fer_ratio_max: 1.1,
            confidence: 0.95,
        },
        rate_matching: None,
    }
}

fn quality(frame_errors: u64, settings: DecoderArmSettings) -> ArmQuality {
    let frames = 1000;
    let bits = frames * 8_448;
    let bit_errors = frame_errors * 40;
    let fer = wilson_interval_95(frame_errors, frames).unwrap();
    let ber = wilson_interval_95(bit_errors, bits).unwrap();
    ArmQuality {
        frames,
        frame_errors,
        bits,
        bit_errors,
        fer: frame_errors as f64 / frames as f64,
        fer_interval: [fer.0, fer.1],
        ber: bit_errors as f64 / bits as f64,
        ber_interval: [ber.0, ber.1],
        interval_method: "wilson-95".into(),
        iterations: IterationDistribution {
            mean: 7.5,
            p50: 7,
            p90: 12,
            max: 50,
        },
        memory_bytes: 1 << 20,
        latency_ns_p50: 250_000,
        settings,
    }
}

fn matched_settings() -> DecoderArmSettings {
    DecoderArmSettings {
        precision: Precision::F32,
        schedule: Schedule::Flooding,
        normalization: Normalization {
            kind: NormalizationKind::NormalizedMinSum,
            factor: Some(0.75),
        },
        iteration_cap: 50,
        stopping: Stopping {
            kind: StoppingKind::Syndrome,
            crc: None,
        },
        batch_size: 1,
    }
}

#[test]
fn decoder_cells_require_quality_intervals_and_matched_settings() {
    let mut family = addendum(vec![
        cell(
            "matched",
            CellObjective::Improvement,
            CellRole::Confirmatory,
            CoreArm::SingleCore,
        ),
        cell(
            "fastest",
            CellObjective::ComparatorGap,
            CellRole::Confirmatory,
            CoreArm::SingleCore,
        ),
    ]);
    family.family.purpose = FamilyPurpose::DecoderFamily;
    family.cells[0].decoder = Some(decoder(DecoderArmKind::MatchedAlgorithm));
    family.cells[1].decoder = Some(decoder(DecoderArmKind::FastestQualityCompatible));
    family.cells[1].builds.candidate = BuildIdentity::External;
    family.validate().unwrap();
    let built = build_receipt(
        "decoder",
        &family,
        &[
            CellSpec {
                quality: Some(DecoderQualityRecord {
                    baseline: quality(20, matched_settings()),
                    candidate: quality(20, matched_settings()),
                }),
                ..spec("matched", 2.0)
            },
            CellSpec {
                quality: Some(DecoderQualityRecord {
                    baseline: quality(20, matched_settings()),
                    candidate: quality(
                        60,
                        DecoderArmSettings {
                            schedule: Schedule::Layered,
                            ..matched_settings()
                        },
                    ),
                }),
                ..spec("fastest", 2.0)
            },
        ],
        false,
        false,
        |_| {},
    );
    let summary = evaluate(&built.dir).unwrap();
    assert_eq!(summary.verdict, Verdict::Accepted, "{:?}", summary.findings);
    assert_eq!(outcome(&summary, "matched"), CellOutcome::Pass);
    assert_eq!(
        outcome(&summary, "fastest"),
        CellOutcome::QualityIncompatible
    );
    // Missing quality or diverging matched settings invalidate the cell.
    let mut receipt =
        BenchmarkReceipt::decode(&fs::read(built.dir.join(RECEIPT_FILE)).unwrap()).unwrap();
    receipt.cells[0]
        .decoder_quality
        .as_mut()
        .unwrap()
        .candidate
        .settings
        .iteration_cap = 20;
    receipt.cells[1].decoder_quality = None;
    fs::write(
        built.dir.join(RECEIPT_FILE),
        serde_json::to_vec_pretty(&receipt).unwrap(),
    )
    .unwrap();
    let summary = evaluate(&built.dir).unwrap();
    assert_eq!(summary.verdict, Verdict::Rejected);
    assert_eq!(outcome(&summary, "matched"), CellOutcome::Invalid);
    assert_eq!(outcome(&summary, "fastest"), CellOutcome::Invalid);
    assert!(summary.findings.iter().any(|f| f.rule == "P-19"));
    assert!(summary.findings.iter().any(|f| f.rule == "P-18"));
}

#[test]
fn protocol_document_pins_the_frozen_shared_settings() {
    let text = fs::read_to_string(repo_root().join(PROTOCOL_PATH)).unwrap();
    assert!(text.contains(&format!("`{PROTOCOL_ID}` version {PROTOCOL_VERSION}")));
    let mut found = BTreeMap::new();
    for line in text.lines() {
        let columns: Vec<&str> = line.split('|').map(str::trim).collect();
        if columns.len() >= 4 && columns[1].starts_with('`') && columns[1].ends_with('`') {
            found.insert(
                columns[1].trim_matches('`').to_owned(),
                columns[2].trim_matches('`').to_owned(),
            );
        }
    }
    for (name, value) in SHARED_SETTINGS.table() {
        assert_eq!(found.get(name), Some(&value), "protocol.md row for {name}");
    }
    for rule in 1..=22 {
        assert!(
            text.contains(&format!("P-{rule:02}")),
            "protocol.md lacks rule P-{rule:02}"
        );
    }
}

#[test]
fn acceptance_summary_markdown_renders_from_the_summary_only() {
    let family = addendum(vec![cell(
        "a",
        CellObjective::Improvement,
        CellRole::Confirmatory,
        CoreArm::SingleCore,
    )]);
    let built = build_receipt(
        "markdown",
        &family,
        &[spec("a", 2.0)],
        false,
        false,
        |receipt| {
            receipt.label = ReceiptLabel::Smoke;
        },
    );
    let summary = evaluate(&built.dir).unwrap();
    let markdown = render_markdown(&summary);
    assert_eq!(markdown, render_markdown(&summary));
    assert!(markdown.contains("smoke receipt"));
    assert!(markdown.contains(&summary.receipt_sha256));
    assert!(markdown.contains("| `a` |"));
    let json: Value = serde_json::to_value(&summary).unwrap();
    assert_eq!(json["schema"], "zen3-benchmark-acceptance-v1");
}

fn git(repo: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(args)
        .current_dir(repo)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?} failed");
}

#[test]
fn runner_announces_the_log_before_work_and_resumes_without_repeating() {
    let root = scratch("runner");
    let repo = root.join("repo");
    stage_repo(&repo);
    stage_runner_producing(&repo);
    let mut family = addendum(vec![
        cell(
            "first",
            CellObjective::Improvement,
            CellRole::Exploratory,
            CoreArm::SingleCore,
        ),
        cell(
            "second",
            CellObjective::NonRegression,
            CellRole::Exploratory,
            CoreArm::SingleCore,
        ),
        cell(
            "six",
            CellObjective::NonRegression,
            CellRole::Exploratory,
            CoreArm::PhysicalCores6,
        ),
    ]);
    family.effect.measurement_resolution = None;
    family.effect.resolution_evidence = None;
    write_addendum(&repo, &family);
    git(&repo, &["init", "-q"]);
    git(&repo, &["add", "."]);
    git(
        &repo,
        &[
            "-c",
            "user.name=fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "-q",
            "-m",
            "fixture",
        ],
    );
    let lock = root.join("bench.lock");
    fs::write(&lock, b"").unwrap();
    let workload = env!("CARGO_BIN_EXE_ab-smoke-workload");
    let arm = |passes: &str| {
        json!({
            "build": "conservative-portable",
            "description": format!("xor-fold {passes} pass"),
            "executable": workload,
            "arguments": [],
            "environment": {"GF2_SMOKE_PASSES": passes},
            "rustflags": null,
            "tuning_profile": null
        })
    };
    let plan = json!({
        "schema": "zen3-benchmark-plan-v1",
        "campaign_id": "runner-contract",
        "issue": "f547c394",
        "label": "pilot",
        "campaign_seed": 5,
        "addendum": "dev/active/f547c394/addendum-fixture.json",
        "lock_path": lock,
        "wrapper": "flock",
        "timing_override": {"windows_per_execution": 2, "window_target_ms": 3},
        "arms": {"baseline": arm("2"), "candidate": arm("1")},
        "cells": [
            {"cell_id": "first", "baseline_arm": "baseline", "candidate_arm": "candidate", "case": {"words": 4096, "seed": 1}, "pilot_pairs": 6},
            {"cell_id": "second", "baseline_arm": "candidate", "candidate_arm": "candidate", "case": {"words": 4096, "seed": 2}, "pilot_pairs": 6},
            {"cell_id": "six", "baseline_arm": "candidate", "candidate_arm": "candidate", "case": {"words": 4096, "seed": 3}, "pilot_pairs": 6}
        ],
        "max_cells_per_session": 1
    });
    let plan_path = root.join("plan.json");
    fs::write(&plan_path, serde_json::to_vec_pretty(&plan).unwrap()).unwrap();
    let stage = root.join("stage");
    let runner = env!("CARGO_BIN_EXE_benchmark-ab-runner");
    let run = || {
        Command::new("flock")
            .arg("-x")
            .arg(&lock)
            .args(["taskset", "-c", "0-2", runner, "run"])
            .arg(&stage)
            .arg(&plan_path)
            .current_dir(&repo)
            .output()
            .unwrap()
    };
    let first = run();
    let stdout = String::from_utf8_lossy(&first.stdout);
    assert_eq!(
        first.status.code(),
        Some(3),
        "{stdout}\n{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert!(
        stdout.starts_with("GF2_CAMPAIGN_EXECUTION_LOG="),
        "{stdout}"
    );
    let unrelated_jit = repo.join(".jit/unrelated-session-record.json");
    fs::create_dir_all(unrelated_jit.parent().unwrap()).unwrap();
    fs::write(&unrelated_jit, b"{\"status\":\"updated\"}\n").unwrap();
    git(&repo, &["add", ".jit/unrelated-session-record.json"]);
    git(
        &repo,
        &[
            "-c",
            "user.name=fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "-q",
            "-m",
            "unrelated JIT metadata",
        ],
    );
    // Outside the wrapper the runner refuses to measure.
    let unlocked = Command::new(runner)
        .args(["run"])
        .arg(&stage)
        .arg(&plan_path)
        .current_dir(&repo)
        .output()
        .unwrap();
    assert!(!unlocked.status.success());
    let second = run();
    assert_eq!(
        second.status.code(),
        Some(3),
        "{}",
        String::from_utf8_lossy(&second.stderr)
    );
    let unrelated_notes = repo.join("dev/active/unrelated-session-notes.md");
    fs::create_dir_all(unrelated_notes.parent().unwrap()).unwrap();
    fs::write(
        unrelated_notes,
        b"Unrelated working-tree notes must not change measurement identity.\n",
    )
    .unwrap();
    let third = run();
    assert_eq!(
        third.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&third.stderr)
    );
    let out = repo.join("dev/bench_results/f547c394/runner-contract");
    let finalize = Command::new(runner)
        .args(["finalize"])
        .arg(&stage)
        .arg(&out)
        .current_dir(&repo)
        .output()
        .unwrap();
    assert!(
        finalize.status.success(),
        "{}",
        String::from_utf8_lossy(&finalize.stderr)
    );
    let summary = evaluate(&out).unwrap();
    assert_eq!(summary.verdict, Verdict::Accepted, "{:?}", summary.findings);
    assert_eq!(summary.sessions, 3);
    assert!(summary.resumed);
    assert_eq!(outcome(&summary, "first"), CellOutcome::Pilot);
    assert_eq!(outcome(&summary, "second"), CellOutcome::Pilot);
    assert_eq!(outcome(&summary, "six"), CellOutcome::Unavailable);
    let six = summary
        .cells
        .iter()
        .find(|cell| cell.cell_id == "six")
        .unwrap();
    assert!(
        six.reason
            .as_deref()
            .unwrap()
            .contains("3 physical cores, 6 required"),
        "{six:?}"
    );
    assert_eq!(summary.cells[0].resolved_cpus, vec![0]);
    let receipt = BenchmarkReceipt::decode(&fs::read(out.join(RECEIPT_FILE)).unwrap()).unwrap();
    assert!(receipt.settings_deviation);
    assert!(receipt.lock.holder_pid > 0);
    assert_eq!(receipt.workers.runner_threads, 1);
    assert!(!receipt.host.topology.cpus.is_empty());
    assert_eq!(receipt.host.affinity.cpus(), &[0, 1, 2]);
    assert!(receipt.cells[0]
        .pairs
        .iter()
        .all(|pair| pair.baseline.cpus_observed == vec![0]));
    let records =
        ExecutionLog::validate_prefix(&fs::read(out.join(LOG_FILE)).unwrap(), "runner-contract")
            .unwrap();
    let starts = records
        .iter()
        .filter(|record| record.event == JournalEvent::CellStart)
        .count();
    let omissions = records
        .iter()
        .filter(|record| {
            record.event == JournalEvent::Omission
                && record.details["kind"] == "completed-in-prior-session"
        })
        .count();
    assert_eq!(starts, 3);
    assert_eq!(omissions, 3);
    assert!(records
        .iter()
        .any(|record| record.event == JournalEvent::LockHold));
}
