//! Behavioral contracts of the Zen 3 benchmark protocol tooling: addendum
//! schema, paired bootstrap statistics, receipt acceptance on deterministic
//! fixtures, the protocol document's frozen settings, and the shared runner.

use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use tuning_campaign_support::abtest::{
    bonferroni_confidence, bootstrap_seed, decide, median, pair_orders, paired_bootstrap_speedup,
    wilson_interval_95, ArmOrder, BootstrapInterval, Decision, Margins, PairedObservation,
    SplitMix64,
};
use tuning_campaign_support::host::{CoreArm, HostObservation};
use tuning_campaign_support::journal::{
    CheckpointStore, ExecutionLog, JournalEvent, JournalRecord, ResumeIdentity, TerminalState,
};
use tuning_campaign_support::protocol::{
    sha256_hex, ArmBuilds, ArtifactPin, Batching, BuildIdentity, CacheState, CellDeclaration,
    CellObjective, CellRole, CodeIdentity, CodewordSource, ComplexityBudget, DecoderArmKind,
    DecoderCell, DecoderInput, EffectRule, FamilyAddendum, FamilyIdentity, FamilyPurpose,
    FamilyWise, Frozen, Holdout, MetricKind, Normalization, NormalizationKind, Precision,
    ProtocolRef, QualityTolerance, ReceiptLabel, ResolutionEvidence, RunnerPlan, Scaling, Schedule,
    SearchBudget, SharedInput, Stopping, StoppingKind, WorkerDeclaration, Workload,
    ADDENDUM_SCHEMA_ID, PROTOCOL_ID, PROTOCOL_VERSION, RECEIPT_SCHEMA_ID, RUNNER_LIFECYCLE_SCHEMA,
    SHARED_PRODUCING_MANIFEST, SHARED_SETTINGS,
};
use tuning_campaign_support::provenance::ProducingInputs;
use tuning_campaign_support::receipt::{
    evaluate, render_markdown, ArmQuality, ArmRecord, BenchmarkReceipt, CampaignFacts, CellClaim,
    CellOutcome, CellRecord, CellStatus, CheckpointRecord, DecoderArmSettings,
    DecoderQualityRecord, ExecutionRecord, IterationDistribution, LockRecord, LogRecord,
    PairRecord, Severity, SourceIdentity, Verdict, WindowRecord, WorkerReport, CHECKPOINT_DIR,
    LOG_FILE, PLAN_FILE, RECEIPT_FILE,
};
use tuning_campaign_support::repository::{is_snapshot_copy, listed_files, repository_root};
use tuning_campaign_support::schema;
use tuning_campaign_support::scratch::Scratch;

const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";
const CAMPAIGN_SEED: u64 = 7;

fn scratch(name: &str) -> Scratch {
    tuning_campaign_support::scratch::scratch(&format!("gf2-f547c394-{name}"))
}

/// Scratch-repository locations of the shared documents and family addenda.
const PROTOCOL_DOC: &str = "protocol/protocol.md";
const SCHEMA_DOC: &str = "protocol/addendum.schema.json";
const CONTRACT_DOC: &str = "contract/measurement-contract.md";
const ADDENDUM_DOC: &str = "campaign/addendum-fixture.json";
/// SHA-256 of the earlier-version documents the fixtures pin: the version-1
/// schema, and the version-2 and version-3 documents the published smoke
/// pilots of those versions pin.
const V1_SCHEMA_SHA256: &str = "44dd132a6203c5d04676bbc9d596915a0aeb4c250fb88975ab930a81ffe6aca1";
const V2_PROTOCOL_SHA256: &str = "9e0193ecce1453119a575e07d6ff84bf8c82eca3ce38d53f3b01fafe14df0eae";
const V2_SCHEMA_SHA256: &str = "4900f412729a76cd6ecf94de21b1f3fb36239f977975490ab8e182397a6fadef";
const V3_PROTOCOL_SHA256: &str = "1d42ac3c965c816f4488a5524d17aa05fdea15f6011c6806a10754561f9f3c1c";
const V3_SCHEMA_SHA256: &str = "513f37627dd82dbce95f49ff3f714af83131320fdbd0fe704baffc18a57412cd";

fn put(root: &Path, relative: &str, bytes: &[u8]) {
    let target = root.join(relative);
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::write(target, bytes).unwrap();
}

/// Bytes of the committed file named by the glob `name` whose SHA-256 is
/// `sha256`, snapshot copies included.
fn committed(name: &str, sha256: &str) -> Vec<u8> {
    let root = repository_root().unwrap();
    listed_files(&root, name)
        .unwrap()
        .into_iter()
        .map(|path| fs::read(root.join(path)).unwrap())
        .find(|bytes| sha256_hex(bytes) == sha256)
        .unwrap_or_else(|| panic!("no committed {name} has SHA-256 {sha256}"))
}

/// The live shared document declaring `identity`, path and bytes.
fn live(input: SharedInput, identity: &str) -> (String, Vec<u8>) {
    let root = repository_root().unwrap();
    let path = input.locate(&root, identity).unwrap();
    let bytes = fs::read(root.join(&path)).unwrap();
    (path, bytes)
}

#[test]
fn byte_identical_shared_document_copies_are_one_live_document() {
    let root = scratch("shared-copies");
    let identity = SharedInput::contract_identity();
    let contract = format!("# {identity}\n");
    for directory in ["live", "copies/first"] {
        put(
            &root,
            &format!("{directory}/measurement-contract.md"),
            contract.as_bytes(),
        );
    }
    git(&root, &["init", "-q"]);
    assert_eq!(
        SharedInput::Contract.locate(&root, &identity).unwrap(),
        "copies/first/measurement-contract.md"
    );
    put(
        &root,
        "edited/measurement-contract.md",
        format!("{contract}\nEdited.\n").as_bytes(),
    );
    assert_eq!(
        SharedInput::Contract
            .locate(&root, &identity)
            .unwrap_err()
            .to_string(),
        "2 live measurement-contract.md files carry the identity, not one"
    );
}

/// The directory of the one committed receipt of `campaign_id`.
fn committed_receipt(campaign_id: &str) -> PathBuf {
    let root = repository_root().unwrap();
    let found: Vec<_> = listed_files(&root, RECEIPT_FILE)
        .unwrap()
        .into_iter()
        .filter(|path| !is_snapshot_copy(path))
        .filter(|path| {
            let value: Value = serde_json::from_slice(&fs::read(root.join(path)).unwrap()).unwrap();
            value["campaign_id"] == campaign_id
        })
        .collect();
    assert_eq!(found.len(), 1, "{campaign_id}: {found:?}");
    root.join(&found[0]).parent().unwrap().to_path_buf()
}

/// Copies the live current protocol document and addendum schema into `root`.
fn stage_current(root: &Path) {
    let protocol = live(
        SharedInput::Protocol,
        &SharedInput::protocol_identity(PROTOCOL_VERSION),
    );
    put(root, PROTOCOL_DOC, &protocol.1);
    put(
        root,
        SCHEMA_DOC,
        &live(SharedInput::AddendumSchema, ADDENDUM_SCHEMA_ID).1,
    );
}

/// Stages the version-1 protocol documents and the contract into a scratch
/// repository root.
fn stage_repo(root: &Path) {
    put(
        root,
        PROTOCOL_DOC,
        &live(SharedInput::Protocol, &SharedInput::protocol_identity(1)).1,
    );
    put(
        root,
        SCHEMA_DOC,
        &committed("addendum*.schema.json", V1_SCHEMA_SHA256),
    );
    put(
        root,
        CONTRACT_DOC,
        &live(SharedInput::Contract, &SharedInput::contract_identity()).1,
    );
    fs::create_dir_all(root.join(Path::new(ADDENDUM_DOC).parent().unwrap())).unwrap();
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

/// Copies the runner's shared producing-input closure into a scratch repo,
/// beside the scratch protocol document.
fn stage_runner_producing(root: &Path) {
    let source_root = repository_root().unwrap();
    let (protocol, _) = live(
        SharedInput::Protocol,
        &SharedInput::protocol_identity(PROTOCOL_VERSION),
    );
    let manifest = Path::new(&protocol).with_file_name(SHARED_PRODUCING_MANIFEST);
    let manifest = manifest.to_str().unwrap();
    let producing = ProducingInputs::read_at(&source_root, manifest).unwrap();
    put(
        root,
        &Path::new(PROTOCOL_DOC)
            .with_file_name(SHARED_PRODUCING_MANIFEST)
            .to_string_lossy(),
        &fs::read(source_root.join(manifest)).unwrap(),
    );
    for relative in &producing.build_inputs {
        put(
            root,
            relative,
            &fs::read(source_root.join(relative)).unwrap(),
        );
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
        cold_calls: None,
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
        schema: "zen3-benchmark-addendum-v1".into(),
        protocol: ProtocolRef {
            id: PROTOCOL_ID.into(),
            version: 1,
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
            ledger_path: None,
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
    let relative = ADDENDUM_DOC;
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
        quality: None,
        calibrated: None,
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
    claim_confidence: Option<f64>,
    claim_alpha: Option<f64>,
    quality: Option<DecoderQualityRecord>,
    execution_quality: Option<DecoderQualityRecord>,
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
    /// Owns the tree `repo` and `dir` point into.
    _root: Scratch,
    repo: PathBuf,
    dir: PathBuf,
}

#[derive(Clone, Copy)]
struct ResumeFixture {
    repeat_first: bool,
}

#[derive(Clone, Copy, Default)]
struct PilotFixture<'a> {
    family_id: Option<&'a str>,
    claim_alpha: Option<f64>,
    omit_claim: bool,
    /// Protocol version of the pilot's addendum; the family's by default.
    protocol_version: Option<u32>,
}

fn fixture_pilot_receipt(repo: &Path, addendum: ArtifactPin) -> BenchmarkReceipt {
    BenchmarkReceipt {
        session_hosts: Vec::new(),
        trial_ledger: None,
        schema: RECEIPT_SCHEMA_ID.into(),
        campaign_id: "fixture-pilot".into(),
        family_id: "fixture-family".into(),
        issue: "f547c394".into(),
        receipt_path: "receipts/pilot/receipt.json".into(),
        label: ReceiptLabel::Pilot,
        campaign_seed: CAMPAIGN_SEED,
        settings: SHARED_SETTINGS,
        settings_deviation: false,
        protocol: pin(repo, PROTOCOL_DOC),
        contract: pin(repo, CONTRACT_DOC),
        addendum_schema: pin(repo, SCHEMA_DOC),
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
            wrapper: "scripts/bench-flock.sh".into(),
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
    build_receipt_with_history(
        name,
        addendum,
        specs,
        resume_after_first.then_some(ResumeFixture { repeat_first }),
        &[],
        PilotFixture::default(),
        mutate,
    )
}

fn build_receipt_with_pilot_family(
    name: &str,
    addendum: &FamilyAddendum,
    specs: &[CellSpec],
    pilot_family: &str,
) -> Built {
    build_receipt_with_history(
        name,
        addendum,
        specs,
        None,
        &[],
        PilotFixture {
            family_id: Some(pilot_family),
            claim_alpha: None,
            ..PilotFixture::default()
        },
        |_| {},
    )
}

fn build_receipt_with_pilot_alpha(
    name: &str,
    addendum: &FamilyAddendum,
    specs: &[CellSpec],
    claim_alpha: f64,
) -> Built {
    build_receipt_with_history(
        name,
        addendum,
        specs,
        None,
        &[],
        PilotFixture {
            family_id: None,
            claim_alpha: Some(claim_alpha),
            ..PilotFixture::default()
        },
        |_| {},
    )
}

fn build_receipt_with_history(
    name: &str,
    addendum: &FamilyAddendum,
    specs: &[CellSpec],
    resume: Option<ResumeFixture>,
    prior_failed: &[&str],
    pilot_fixture: PilotFixture<'_>,
    mutate: impl FnOnce(&mut BenchmarkReceipt),
) -> Built {
    let root = scratch(name);
    let repo = root.join("repo");
    stage_repo(&repo);
    if addendum.protocol.version >= 2 {
        // An earlier version uses the documents its published smoke receipts pin.
        match addendum.protocol.version {
            2 => {
                put(
                    &repo,
                    PROTOCOL_DOC,
                    &committed("protocol.md", V2_PROTOCOL_SHA256),
                );
                put(
                    &repo,
                    SCHEMA_DOC,
                    &committed("addendum*.schema.json", V2_SCHEMA_SHA256),
                );
            }
            3 => {
                put(
                    &repo,
                    PROTOCOL_DOC,
                    &committed("protocol.md", V3_PROTOCOL_SHA256),
                );
                put(
                    &repo,
                    SCHEMA_DOC,
                    &committed("addendum*.schema.json", V3_SCHEMA_SHA256),
                );
            }
            _ => stage_current(&repo),
        }
        fs::write(
            repo.join(addendum.family_wise.ledger_path.as_ref().unwrap()),
            b"",
        )
        .unwrap();
    }
    let dir = repo.join("receipts/confirmation");
    fs::create_dir_all(&dir).unwrap();
    let pilot_addendum_path = "campaign/addendum-fixture-pilot.json";
    let mut pilot_addendum = addendum.clone();
    pilot_addendum.effect.measurement_resolution = None;
    pilot_addendum.effect.resolution_evidence = None;
    for cell in &mut pilot_addendum.cells {
        cell.role = CellRole::Exploratory;
    }
    if let Some(version) = pilot_fixture.protocol_version {
        pilot_addendum.protocol.version = version;
        pilot_addendum.schema = format!("zen3-benchmark-addendum-v{version}");
    }
    let mut pilot_addendum_bytes = serde_json::to_vec_pretty(&pilot_addendum).unwrap();
    pilot_addendum_bytes.push(b'\n');
    fs::write(repo.join(pilot_addendum_path), pilot_addendum_bytes).unwrap();
    let pilot_path = "receipts/pilot/receipt.json";
    let pilot_dir = repo.join(pilot_path).parent().unwrap().to_path_buf();
    fs::create_dir_all(&pilot_dir).unwrap();
    fs::create_dir_all(pilot_dir.join("inputs")).unwrap();
    let pilot_addendum_pin = ArtifactPin::capture(
        &repo,
        &pilot_dir,
        pilot_addendum_path,
        "inputs/family-addendum.json",
    )
    .unwrap();
    let mut pilot = fixture_pilot_receipt(&repo, pilot_addendum_pin);
    if addendum.protocol.version >= 3 {
        pilot.family_id = addendum.family.id.clone();
        if let Some(family) = pilot_fixture.family_id {
            pilot.family_id = family.into();
        }
        let ledger = tuning_campaign_support::trial_ledger::reserve(
            &repo,
            &pilot_dir,
            &pilot_addendum,
            &pilot.campaign_id,
            &pilot.addendum.sha256,
            &[],
        )
        .unwrap();
        let alpha = tuning_campaign_support::trial_ledger::attempt_alpha(
            &ledger,
            &pilot_dir,
            &pilot_addendum,
        )
        .unwrap();
        let entries = tuning_campaign_support::trial_ledger::decode(
            &fs::read(pilot_dir.join(&ledger.snapshot)).unwrap(),
            &pilot_addendum.family.id,
        )
        .unwrap();
        let corrected_alpha = alpha
            / f64::from(tuning_campaign_support::trial_ledger::comparisons(&entries).unwrap());
        pilot.trial_ledger = Some(ledger);
        let cpus = vec![pilot.host.affinity.cpus()[0]];
        pilot.cells = specs
            .iter()
            .map(|spec| {
                let declared = pilot_addendum.cell(spec.id).unwrap();
                let baseline_arm = fixture_arm_name("baseline", declared.builds.baseline);
                let candidate_arm = fixture_arm_name("candidate", declared.builds.candidate);
                let pairs = pairs(
                    spec,
                    CellRole::Exploratory,
                    spec.id,
                    &cpus,
                    &baseline_arm,
                    &candidate_arm,
                );
                let observations: Vec<_> = pairs
                    .iter()
                    .map(|pair| PairedObservation {
                        baseline_ns_per_call: pair.baseline.ns_per_call,
                        candidate_ns_per_call: pair.candidate.ns_per_call,
                    })
                    .collect();
                let interval = paired_bootstrap_speedup(
                    &observations,
                    SHARED_SETTINGS.bootstrap_resamples,
                    corrected_alpha,
                    bootstrap_seed(CAMPAIGN_SEED, spec.id),
                )
                .unwrap();
                let mut record = CellRecord {
                    cell_id: spec.id.into(),
                    key: spec.id.into(),
                    role: CellRole::Exploratory,
                    baseline_arm,
                    candidate_arm,
                    core_arm: declared.core_arm,
                    resolved_cpus: cpus.clone(),
                    status: CellStatus::Measured,
                    unavailable_reason: None,
                    pairs,
                    claimed: Some(CellClaim {
                        decision: decide(&interval, &pilot_addendum.margins(declared).unwrap())
                            .unwrap(),
                        margins: pilot_addendum.margins(declared).unwrap(),
                        interval,
                    }),
                    decoder_quality: None,
                    checkpoint_sha256: None,
                };
                if let Some(alpha) = pilot_fixture.claim_alpha {
                    record.claimed.as_mut().unwrap().interval.alpha = alpha;
                }
                if pilot_fixture.omit_claim {
                    record.claimed = None;
                }
                record
            })
            .collect();
    }
    let mut pilot_bytes = serde_json::to_vec_pretty(&pilot).unwrap();
    pilot_bytes.push(b'\n');
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
    let shared = |input: SharedInput, relative: &str| {
        ArtifactPin::capture(&repo, &dir, relative, input.snapshot()).unwrap()
    };
    let protocol_pin = shared(SharedInput::Protocol, PROTOCOL_DOC);
    let contract_pin = shared(SharedInput::Contract, CONTRACT_DOC);
    let schema_pin = shared(SharedInput::AddendumSchema, SCHEMA_DOC);
    let addendum_pin =
        ArtifactPin::capture(&repo, &dir, ADDENDUM_DOC, "inputs/family-addendum.json").unwrap();
    fs::create_dir_all(dir.join("inputs/resolution-evidence")).unwrap();
    fs::write(
        dir.join("inputs/resolution-evidence/receipt.json"),
        &pilot_bytes,
    )
    .unwrap();
    if addendum.protocol.version >= 3 {
        for (pin, snapshot) in [
            (
                &pilot.addendum,
                "inputs/resolution-evidence/family-addendum.json",
            ),
            (
                pilot.trial_ledger.as_ref().unwrap(),
                "inputs/resolution-evidence/trial-ledger.jsonl",
            ),
        ] {
            fs::write(dir.join(snapshot), pin.verify_content(&pilot_dir).unwrap()).unwrap();
        }
    }
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
        "producing_manifest": "producing-inputs.json",
        "issue": "f547c394",
        "label": "confirmation",
        "campaign_seed": CAMPAIGN_SEED,
        "addendum": ADDENDUM_DOC,
        "lock_path": "/tmp/gf2-ccx1.lock",
        "wrapper": "scripts/bench-flock.sh",
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
        "max_cells_per_session": if resume.is_some() { Some(1) } else { None }
    });
    let mut plan_bytes = serde_json::to_vec_pretty(&plan).unwrap();
    plan_bytes.push(b'\n');
    fs::write(dir.join(PLAN_FILE), &plan_bytes).unwrap();
    let plan = RunnerPlan::decode(&plan_bytes).unwrap();
    plan.validate(&addendum).unwrap();
    let host = HostObservation::observe().unwrap();
    let identity = identity(&protocol_pin, &producing, &plan, &arms, &host);
    for failed in prior_failed {
        let prior_stage = repo.join(failed);
        fs::create_dir_all(prior_stage.join("inputs")).unwrap();
        tuning_campaign_support::trial_ledger::reserve(
            &repo,
            &prior_stage,
            &addendum,
            failed,
            &addendum_pin.sha256,
            &[sha256_hex(failed.as_bytes())],
        )
        .unwrap();
    }
    let ledger = (addendum.protocol.version >= 2).then(|| {
        tuning_campaign_support::trial_ledger::reserve(
            &repo,
            &dir,
            &addendum,
            campaign,
            &addendum_pin.sha256,
            &tuning_campaign_support::trial_ledger::candidate_ids(&addendum, &plan, &arms).unwrap(),
        )
        .unwrap()
    });
    let facts = CampaignFacts {
        host: (addendum.protocol.version >= 2).then(|| host.clone()),
        trial_ledger: ledger.clone(),
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
    if addendum.protocol.version >= 2 {
        log.append(
            JournalEvent::DriverDiagnostic,
            None,
            json!({"kind":"host-observation", "observation":host}),
        )
        .unwrap();
    }
    let mut checkpoints =
        CheckpointStore::create_new(dir.join(CHECKPOINT_DIR), campaign, identity).unwrap();
    let cpus = vec![host.affinity.cpus()[0]];
    let alpha = if let Some(pin) = &ledger {
        tuning_campaign_support::trial_ledger::attempt_alpha(pin, &dir, &addendum).unwrap()
    } else {
        0.05
    };
    let comparisons = if let Some(pin) = &ledger {
        tuning_campaign_support::trial_ledger::verify(
            pin,
            &dir,
            &addendum,
            campaign,
            &addendum_pin.sha256,
            &tuning_campaign_support::trial_ledger::candidate_ids(&addendum, &plan, &arms).unwrap(),
        )
        .unwrap()
    } else {
        addendum.family_comparisons()
    };
    let confidence = bonferroni_confidence(alpha, comparisons).unwrap();
    let mut cells = Vec::new();
    for (index, spec) in specs.iter().enumerate() {
        if resume.is_some() && index == 1 {
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
            if resume.is_some_and(|fixture| fixture.repeat_first) {
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
            if addendum.protocol.version >= 2 {
                for pair in &mut record.pairs {
                    for (execution, quality) in [
                        (
                            &mut pair.baseline,
                            spec.execution_quality
                                .as_ref()
                                .or(spec.quality.as_ref())
                                .map(|q| &q.baseline),
                        ),
                        (
                            &mut pair.candidate,
                            spec.execution_quality
                                .as_ref()
                                .or(spec.quality.as_ref())
                                .map(|q| &q.candidate),
                        ),
                    ] {
                        execution.quality = quality.cloned();
                        execution.calibrated = Some(declared.cold_calls.is_none());
                        execution.cache_state_applied = declared.cache_state;
                        if let Some(calls) = declared.cold_calls {
                            for window in &mut execution.windows {
                                window.elapsed_ns = (window.ns_per_call() * calls as f64) as u64;
                                window.calls = calls;
                            }
                        }
                    }
                }
            }
            for pair in &record.pairs {
                let run_order = match pair.order {
                    ArmOrder::BaselineFirst => {
                        [("baseline", &pair.baseline), ("candidate", &pair.candidate)]
                    }
                    ArmOrder::CandidateFirst => {
                        [("candidate", &pair.candidate), ("baseline", &pair.baseline)]
                    }
                };
                for (role, execution) in run_order {
                    let execution_case = json!({
                        "key": key,
                        "cell_id": spec.id,
                        "arm": execution.arm,
                        "role": role,
                        "pair": pair.index
                    });
                    log.append(
                        JournalEvent::ChildSpawn,
                        Some(execution_case.clone()),
                        json!({"pid": execution.pid}),
                    )
                    .unwrap();
                    log.append(
                        JournalEvent::ExecutionProgress,
                        Some(execution_case),
                        json!({
                            "ns_per_call": execution.ns_per_call,
                            "windows": execution.windows,
                            "quality": execution.quality
                        }),
                    )
                    .unwrap();
                }
            }
            if spec.claim {
                let observations: Vec<_> = record
                    .pairs
                    .iter()
                    .map(|pair| PairedObservation {
                        baseline_ns_per_call: pair.baseline.ns_per_call,
                        candidate_ns_per_call: pair.candidate.ns_per_call,
                    })
                    .collect();
                let interval = if addendum.protocol.version >= 3 {
                    paired_bootstrap_speedup(
                        &observations,
                        SHARED_SETTINGS.bootstrap_resamples,
                        alpha / f64::from(comparisons),
                        bootstrap_seed(CAMPAIGN_SEED, &key),
                    )
                } else {
                    tuning_campaign_support::abtest::paired_bootstrap_speedup_legacy(
                        &observations,
                        SHARED_SETTINGS.bootstrap_resamples,
                        spec.claim_confidence.unwrap_or(confidence),
                        bootstrap_seed(CAMPAIGN_SEED, &key),
                    )
                }
                .unwrap();
                let margins = addendum.margins(declared).unwrap();
                let mut claim = CellClaim {
                    interval,
                    decision: decide(&interval, &margins).unwrap(),
                    margins,
                };
                if let Some(alpha) = spec.claim_alpha {
                    claim.interval.alpha = alpha;
                }
                record.claimed = Some(claim);
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
    let sessions = if resume.is_some() { 2 } else { 1 };
    let mut receipt = BenchmarkReceipt {
        session_hosts: if addendum.protocol.version >= 2 {
            vec![host.clone()]
        } else {
            Vec::new()
        },
        trial_ledger: ledger,
        schema: RECEIPT_SCHEMA_ID.into(),
        campaign_id: campaign.into(),
        family_id: addendum.family.id.clone(),
        issue: "f547c394".into(),
        receipt_path: "receipts/confirmation/receipt.json".into(),
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
            observation: if addendum.protocol.version >= 3 {
                "inherited-fd-and-independent-flock-conflict".into()
            } else {
                "fixture".into()
            },
            wrapper: if addendum.protocol.version >= 3 {
                String::new()
            } else {
                "scripts/bench-flock.sh".into()
            },
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
    Built {
        _root: root,
        repo,
        dir,
    }
}

fn spec(id: &'static str, speedup: f64) -> CellSpec {
    CellSpec {
        id,
        speedup,
        jitter: 0.02,
        unavailable: None,
        claim: true,
        claim_confidence: None,
        claim_alpha: None,
        quality: None,
        execution_quality: None,
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
    let schema_value: Value =
        serde_json::from_slice(&committed("addendum*.schema.json", V1_SCHEMA_SHA256)).unwrap();
    let bytes = committed(
        "addendum-protocol-smoke.json",
        "307b6a2baf2da5025a26820ed44be55a212747440f3d9d00e09bdaf8b1d8a69c",
    );
    let instance: Value = serde_json::from_slice(&bytes).unwrap();
    assert!(schema::validate(&schema_value, &instance).is_empty());
    let smoke = FamilyAddendum::decode(&bytes).unwrap();
    smoke.validate().unwrap();
    assert_eq!(smoke.family.id, "protocol-smoke");
    let pilot_bytes = committed(
        "addendum-protocol-smoke-pilot.json",
        "6f762913988f8cb517f5e17c2832fff06d22359a7f1eb8c59d2f47a7ffe0feb4",
    );
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
    let first = paired_bootstrap_speedup(&observations, 2000, 0.05, 11).unwrap();
    let second = paired_bootstrap_speedup(&observations, 2000, 0.05, 11).unwrap();
    assert_eq!(first, second);
    assert!(first.lower <= 2.0 && 2.0 <= first.upper, "{first:?}");
    assert!((first.estimate - 2.0).abs() < 1e-9);
    assert!(first.upper - first.lower < 0.2, "{first:?}");
    assert_eq!(first.seed, 11);
    assert!(paired_bootstrap_speedup(&observations[..1], 2000, 0.05, 1).is_err());
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
        alpha: 0.05,
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
    assert_eq!(
        decide(&interval(0.95, 0.99), &margins).unwrap(),
        Decision::NotWorse
    );
    assert_eq!(
        decide(&interval(0.7, 0.85), &margins).unwrap(),
        Decision::Regressed
    );
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
fn acceptance_identifies_shared_pins_by_digest_not_by_source_path() {
    let family = addendum(vec![cell(
        "a",
        CellObjective::Improvement,
        CellRole::Confirmatory,
        CoreArm::SingleCore,
    )]);
    let built = build_receipt(
        "relocated-pins",
        &family,
        &[spec("a", 2.0)],
        false,
        false,
        |_| {},
    );
    let receipt =
        BenchmarkReceipt::decode(&fs::read(built.dir.join(RECEIPT_FILE)).unwrap()).unwrap();
    for (pin, (path, bytes)) in [
        (
            &receipt.protocol,
            live(SharedInput::Protocol, &SharedInput::protocol_identity(1)),
        ),
        (
            &receipt.contract,
            live(SharedInput::Contract, &SharedInput::contract_identity()),
        ),
    ] {
        assert_ne!(pin.path, path);
        assert_eq!(pin.sha256, sha256_hex(&bytes));
    }
    let summary = evaluate(&built.dir).unwrap();
    assert_eq!(summary.verdict, Verdict::Accepted, "{:?}", summary.findings);

    let built = build_receipt(
        "other-content-pin",
        &family,
        &[spec("a", 2.0)],
        false,
        false,
        |receipt| {
            receipt.protocol.sha256 = receipt.contract.sha256.clone();
            receipt.protocol.snapshot = receipt.contract.snapshot.clone();
        },
    );
    let summary = evaluate(&built.dir).unwrap();
    assert_eq!(summary.verdict, Verdict::Rejected);
    assert!(summary.findings.iter().any(|finding| {
        finding.rule == "P-02" && finding.message.contains("identifies no protocol edition")
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
    let confirmation_path = "receipts/confirmation/receipt.json";
    for (name, evidence, expected) in [
        (
            "missing-resolution-evidence",
            "receipts/missing-pilot/receipt.json",
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
        frame_bit_errors: Vec::new(),
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
    let text = String::from_utf8(
        live(
            SharedInput::Protocol,
            &SharedInput::protocol_identity(PROTOCOL_VERSION),
        )
        .1,
    )
    .unwrap();
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
    for (name, value, _) in SHARED_SETTINGS.table() {
        assert_eq!(found.get(name), Some(&value), "protocol.md row for {name}");
    }
    let rules = |source: &str| {
        source
            .split("P-")
            .filter_map(|suffix| suffix.get(..2))
            .filter(|digits| digits.bytes().all(|byte| byte.is_ascii_digit()))
            .map(|digits| format!("P-{digits}"))
            .collect::<std::collections::BTreeSet<_>>()
    };
    assert_eq!(
        rules(&text),
        rules(include_str!("../src/receipt.rs")),
        "the document must name exactly the acceptance rules the evaluator enforces"
    );
}

#[test]
fn declared_margins_must_clear_resolution_at_all_three_boundaries() {
    for version in [3, PROTOCOL_VERSION] {
        let mut family = addendum(Vec::new());
        family.protocol.version = version;
        family.schema = format!("zen3-benchmark-addendum-v{version}");
        family.family_wise.ledger_path = Some("family-ledger.jsonl".into());
        let resolution = family.effect.measurement_resolution.unwrap();
        for margin in [
            &mut family.effect.worthwhile_speedup,
            &mut family.effect.equivalence_margin,
            &mut family.effect.material_gap_threshold,
        ] {
            *margin = Some(1.0 + resolution);
        }
        let errors = family.validate().unwrap_err();
        for name in [
            "worthwhile_speedup",
            "equivalence_margin",
            "material_gap_threshold",
        ] {
            assert!(
                errors.iter().any(|error| error.contains(name)),
                "v{version} exact-resolution boundary must reject {name}: {errors:?}"
            );
        }

        for margin in [
            &mut family.effect.worthwhile_speedup,
            &mut family.effect.equivalence_margin,
            &mut family.effect.material_gap_threshold,
        ] {
            *margin = Some((1.0 + resolution) * 1.01);
        }
        family.validate().unwrap();
    }
}

#[test]
fn bootstrap_uses_declared_corrected_alpha_for_all_family_sizes() {
    let observations = vec![
        PairedObservation {
            baseline_ns_per_call: 100.0,
            candidate_ns_per_call: 100.0,
        },
        PairedObservation {
            baseline_ns_per_call: 120.0,
            candidate_ns_per_call: 80.0,
        },
    ];
    for comparisons in [1, 2, 5, 10, 25] {
        let alpha = 0.05 / f64::from(comparisons);
        let interval = paired_bootstrap_speedup(&observations, 10_000, alpha, 17).unwrap();
        assert_eq!(interval.resamples, 10_000);
        assert!((interval.confidence - (1.0 - alpha)).abs() < f64::EPSILON);
    }
    let boundary = BootstrapInterval {
        estimate: 1.2,
        lower: 1.1,
        upper: 1.3,
        confidence: 0.95,
        alpha: 0.05,
        resamples: 10_000,
        seed: 17,
        pairs: 2,
    };
    assert_eq!(
        decide(
            &boundary,
            &Margins {
                improvement: boundary.lower,
                equivalence: 1.0,
            },
        )
        .unwrap(),
        Decision::Improved,
        "a lower endpoint at the decision boundary is accepted"
    );
}

#[test]
fn flagged_windows_include_the_exact_boundary_without_pooling_arms() {
    assert_eq!(
        tuning_campaign_support::abtest::flagged_windows(&[1.0, 1.0, 1.0, 1.0, 2.0], 2.0).unwrap(),
        1
    );
    let baseline = [100.0, 100.0, 100.0, 100.0, 100.0];
    let candidate = [600.0, 600.0, 600.0, 600.0, 600.0];
    let large_effect = [10_000.0, 10_000.0, 10_000.0, 10_000.0, 10_000.0];
    assert_eq!(
        tuning_campaign_support::abtest::flagged_windows(&baseline, 2.0).unwrap(),
        0
    );
    assert_eq!(
        tuning_campaign_support::abtest::flagged_windows(&candidate, 2.0).unwrap(),
        0
    );
    assert_eq!(
        tuning_campaign_support::abtest::flagged_windows(&large_effect, 2.0).unwrap(),
        0
    );
}

#[test]
fn protocol_document_guard_includes_justifications_and_every_rule_it_declares() {
    let text = String::from_utf8(
        live(
            SharedInput::Protocol,
            &SharedInput::protocol_identity(PROTOCOL_VERSION),
        )
        .1,
    )
    .unwrap();
    let rows = SHARED_SETTINGS.table();
    for (name, value, justification) in rows {
        let row = format!("| `{name}` | `{value}` | {justification} |");
        assert!(text.contains(&row), "protocol.md semantic row for {name}");
    }
    let declared: std::collections::BTreeSet<_> = text
        .split("P-")
        .filter_map(|suffix| suffix.get(..2))
        .filter(|digits| digits.bytes().all(|byte| byte.is_ascii_digit()))
        .map(|digits| format!("P-{digits}"))
        .collect();
    let enforced: std::collections::BTreeSet<_> = include_str!("../src/receipt.rs")
        .split("P-")
        .filter_map(|suffix| suffix.get(..2))
        .filter(|digits| digits.bytes().all(|byte| byte.is_ascii_digit()))
        .map(|digits| format!("P-{digits}"))
        .collect();
    assert_eq!(declared, enforced);
}

#[test]
fn v1_prior_trial_receipts_must_be_repository_relative() {
    let mut family = addendum(Vec::new());
    family.family_wise.prior_confirmatory_trials = 1;
    family
        .family_wise
        .prior_trials
        .push(tuning_campaign_support::protocol::PriorTrial {
            receipt: "../outside/receipt.json".into(),
            sha256: "a".repeat(64),
            outcome: "fail".into(),
        });
    assert!(family
        .validate()
        .unwrap_err()
        .iter()
        .any(|error| error.contains("prior trial")));
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
    assert_eq!(json["schema"], "zen3-benchmark-acceptance-v2");
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
    family.protocol.version = PROTOCOL_VERSION;
    family.schema = ADDENDUM_SCHEMA_ID.into();
    family.family_wise.ledger_path = Some("family-ledger.jsonl".into());
    fs::write(repo.join("family-ledger.jsonl"), b"").unwrap();
    stage_current(&repo);
    family.cells[0].decoder = Some(decoder(DecoderArmKind::MatchedAlgorithm));
    family.cells[1].cache_state = CacheState::Cold;
    family.cells[1].cold_calls = Some(1000);
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
        "producing_manifest": "producing-inputs.json",
        "issue": "f547c394",
        "label": "pilot",
        "campaign_seed": 5,
        "addendum": ADDENDUM_DOC,
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
    let unrelated_notes = repo.join("notes/unrelated-session-notes.md");
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
    let out = repo.join("receipts/runner-contract");
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
    assert_eq!(
        tuning_campaign_support::receipt::evaluate_version(&out, Some(PROTOCOL_VERSION))
            .unwrap()
            .verdict,
        Verdict::Accepted
    );
    let wrong = tuning_campaign_support::receipt::evaluate_version(&out, Some(1)).unwrap();
    assert_eq!(wrong.verdict, Verdict::Rejected);
    assert!(wrong.findings.iter().any(|f| f.rule == "P-01"));
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
    assert_eq!(
        receipt.source.producing.manifest_path,
        "producing-inputs.json"
    );
    assert!(receipt
        .source
        .producing
        .behavior_sha256
        .contains_key("producer.rs"));
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

#[test]
fn runner_check_validates_a_plan_and_measures_nothing() {
    let root = scratch("runner-check");
    let repo = root.join("repo");
    stage_repo(&repo);
    stage_current(&repo);
    git(&repo, &["init", "-q"]);
    let mut family = addendum(vec![cell(
        "first",
        CellObjective::Improvement,
        CellRole::Exploratory,
        CoreArm::SingleCore,
    )]);
    family.protocol.version = PROTOCOL_VERSION;
    family.schema = ADDENDUM_SCHEMA_ID.into();
    family.family_wise.ledger_path = Some("family-ledger.jsonl".into());
    fs::write(repo.join("family-ledger.jsonl"), b"").unwrap();
    write_addendum(&repo, &family);
    // `check` reads no executable: the plan's arm names an absent one.
    let plan = check_plan();
    let plan_path = root.join("plan.json");
    fs::write(&plan_path, serde_json::to_vec_pretty(&plan).unwrap()).unwrap();
    let runner = env!("CARGO_BIN_EXE_benchmark-ab-runner");
    let check = |path: &Path| {
        Command::new(runner)
            .arg("check")
            .arg(path)
            .current_dir(&repo)
            .output()
            .unwrap()
    };

    let before: BTreeSet<PathBuf> = fs::read_dir(&repo)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    let accepted = check(&plan_path);
    let stdout = String::from_utf8_lossy(&accepted.stdout);
    assert_eq!(
        accepted.status.code(),
        Some(0),
        "{stdout}\n{}",
        String::from_utf8_lossy(&accepted.stderr)
    );
    assert!(stdout.contains(&family.family.id), "{stdout}");
    assert!(stdout.contains("1 cells"), "{stdout}");
    let after: BTreeSet<PathBuf> = fs::read_dir(&repo)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    assert_eq!(before, after);

    let mut undeclared = plan.clone();
    undeclared["cells"][0]["cell_id"] = json!("second");
    let undeclared_path = root.join("undeclared.json");
    fs::write(
        &undeclared_path,
        serde_json::to_vec_pretty(&undeclared).unwrap(),
    )
    .unwrap();
    let rejected = check(&undeclared_path);
    assert_ne!(rejected.status.code(), Some(0));
    let stderr = String::from_utf8_lossy(&rejected.stderr);
    assert!(stderr.contains("second"), "{stderr}");
}

/// A plan of one exploratory single-core cell over the fixture arm.
fn check_plan() -> Value {
    let arm = json!({
        "build": "conservative-portable",
        "description": "fixture arm",
        "executable": "target/release/absent-arm",
        "arguments": [],
        "environment": {},
        "rustflags": null,
        "tuning_profile": null
    });
    json!({
        "schema": "zen3-benchmark-plan-v1",
        "campaign_id": "check-contract",
        "producing_manifest": "producing-inputs.json",
        "issue": "f547c394",
        "label": "pilot",
        "campaign_seed": 11,
        "addendum": ADDENDUM_DOC,
        "lock_path": "/tmp/unused.lock",
        "wrapper": "flock",
        "timing_override": null,
        "arms": {"baseline": arm, "candidate": arm},
        "cells": [{
            "cell_id": "first",
            "baseline_arm": "baseline",
            "candidate_arm": "candidate",
            "case": {"words": 4096, "seed": 1},
            "pilot_pairs": 6
        }],
        "max_cells_per_session": 1
    })
}

#[test]
fn runner_check_rejects_an_addendum_its_schema_edition_rejects() {
    let root = scratch("runner-check-schema");
    let repo = root.join("repo");
    stage_repo(&repo);
    stage_current(&repo);
    git(&repo, &["init", "-q"]);
    let mut family = addendum(vec![cell(
        "first",
        CellObjective::Improvement,
        CellRole::Exploratory,
        CoreArm::SingleCore,
    )]);
    family.protocol.version = PROTOCOL_VERSION;
    family.schema = ADDENDUM_SCHEMA_ID.into();
    family.family.purpose = FamilyPurpose::DecoderFamily;
    family.cells[0].decoder = Some(decoder(DecoderArmKind::MatchedAlgorithm));
    family.family_wise.ledger_path = Some("family-ledger.jsonl".into());
    fs::write(repo.join("family-ledger.jsonl"), b"").unwrap();
    let plan_path = root.join("plan.json");
    fs::write(
        &plan_path,
        serde_json::to_vec_pretty(&check_plan()).unwrap(),
    )
    .unwrap();
    let check = |instance: &Value| {
        put(
            &repo,
            ADDENDUM_DOC,
            &serde_json::to_vec_pretty(instance).unwrap(),
        );
        Command::new(env!("CARGO_BIN_EXE_benchmark-ab-runner"))
            .arg("check")
            .arg(&plan_path)
            .current_dir(&repo)
            .output()
            .unwrap()
    };

    let valid = serde_json::to_value(&family).unwrap();
    let accepted = check(&valid);
    assert_eq!(
        accepted.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&accepted.stderr)
    );

    // Each value decodes and passes the semantic rules; only the schema
    // constrains it.
    for (pointer, value) in [
        ("/family/description", ""),
        ("/cells/0/decoder/code/identity", ""),
        ("/cells/0/decoder/input/llr_source", ""),
        ("/cells/0/decoder/input/llr_sha256", ""),
        ("/cells/0/decoder/input/llr_sha256", " "),
        ("/cells/0/decoder/input/llr_sha256", "x"),
    ] {
        let mut mutated = valid.clone();
        *mutated.pointer_mut(pointer).unwrap() = json!(value);
        let decoded = FamilyAddendum::decode(&serde_json::to_vec(&mutated).unwrap()).unwrap();
        decoded.validate().unwrap();
        let rejected = check(&mutated);
        let stderr = String::from_utf8_lossy(&rejected.stderr);
        assert_ne!(rejected.status.code(), Some(0), "{pointer} = {value:?}");
        assert!(
            stderr.contains("schema violation"),
            "{pointer} = {value:?}: {stderr}"
        );
    }
}

#[test]
fn v1_published_receipts_keep_their_pinned_rules_and_refuse_v2() {
    for campaign_id in [
        "pilot-f547c394-20260906t230725z",
        "confirmation-f547c394-20260906t230920z",
    ] {
        let dir = committed_receipt(campaign_id);
        let summary = tuning_campaign_support::receipt::evaluate_version(&dir, Some(1)).unwrap();
        assert_eq!(summary.verdict, Verdict::Accepted, "{:?}", summary.findings);
        let wrong = tuning_campaign_support::receipt::evaluate_version(&dir, Some(2)).unwrap();
        assert_eq!(wrong.verdict, Verdict::Rejected);
        assert!(wrong.findings.iter().any(|f| f.rule == "P-01"));
    }
}

#[test]
fn material_host_changes_reject_resume_but_observation_time_load_memory_do_not() {
    let original = HostObservation::observe().unwrap();
    let mut benign = original.clone();
    benign.observed_utc = "2026-09-08T00:00:00Z".into();
    benign.load_average = [10.0, 20.0, 30.0];
    benign.available_memory_kib /= 2;
    assert!(original.material_equivalent(&benign));
    let family = v2_family();
    let built = build_receipt(
        "v2-host",
        &family,
        &[spec("quality", 2.0)],
        false,
        false,
        |_| {},
    );
    let records = ExecutionLog::validate_prefix(
        &fs::read(built.dir.join(LOG_FILE)).unwrap(),
        "fixture-campaign",
    )
    .unwrap();
    let mut facts: CampaignFacts = serde_json::from_value(records[0].details.clone()).unwrap();
    facts.host = Some(original.clone());
    let mut resumed = facts.clone();
    resumed.host = Some(benign);
    assert!(facts.resume_equivalent(&resumed));
    for change in 0..8 {
        let mut changed = original.clone();
        match change {
            0 => changed.cpu_model.push_str("-changed"),
            1 => changed.os_kernel.push_str("-changed"),
            2 => {
                changed.governors.insert("cpu0".into(), "changed".into());
            }
            3 => changed.smt_active = Some(!changed.smt_active.unwrap_or(false)),
            4 => changed.topology.cpus[0].core_id += 100,
            5 => {
                changed.affinity = tuning_campaign_support::host::CpuAffinity::parse("999").unwrap()
            }
            6 => changed.hostname.push_str("-changed"),
            _ => changed.cpu_flags.push("changed".into()),
        }
        assert!(
            !original.material_equivalent(&changed),
            "material field {change}"
        );
        resumed.host = Some(changed);
        assert!(
            !facts.resume_equivalent(&resumed),
            "resume material field {change}"
        );
    }
}

fn v2_family() -> FamilyAddendum {
    let mut family = addendum(vec![cell(
        "quality",
        CellObjective::NonRegression,
        CellRole::Confirmatory,
        CoreArm::SingleCore,
    )]);
    family.protocol.version = 2;
    family.schema = "zen3-benchmark-addendum-v2".into();
    family.family_wise.ledger_path = Some("family-ledger.jsonl".into());
    family
}

fn v3_family() -> FamilyAddendum {
    let mut family = v2_family();
    family.protocol.version = 3;
    family.schema = "zen3-benchmark-addendum-v3".into();
    family
}

#[test]
fn v3_resolution_derives_the_pilot_half_width_and_binds_its_family() {
    let mut underdeclared = v3_family();
    underdeclared.effect.measurement_resolution = Some(0.000_001);
    let underdeclared = build_receipt(
        "v3-underdeclared-resolution",
        &underdeclared,
        &[spec("quality", 2.0)],
        false,
        false,
        |_| {},
    );
    let summary = evaluate(&underdeclared.dir).unwrap();
    assert_eq!(summary.verdict, Verdict::Rejected, "{:?}", summary.findings);
    assert!(summary
        .findings
        .iter()
        .any(|finding| { finding.rule == "P-03" && finding.message.contains("pilot half-width") }));

    let family = v3_family();
    let cross_family = build_receipt_with_pilot_family(
        "v3-cross-family-resolution",
        &family,
        &[spec("quality", 2.0)],
        "another-family",
    );
    let summary = evaluate(&cross_family.dir).unwrap();
    assert_eq!(summary.verdict, Verdict::Rejected, "{:?}", summary.findings);
    assert!(summary
        .findings
        .iter()
        .any(|finding| { finding.rule == "P-03" && finding.message.contains("different family") }));
}

#[test]
fn v3_resolution_uses_raw_pilot_pairs_without_a_decision_claim() {
    for (name, resolution, expected) in [
        ("v3-pilot-without-claim", 0.02, Verdict::Accepted),
        (
            "v3-pilot-without-claim-underdeclared",
            0.000_001,
            Verdict::Rejected,
        ),
    ] {
        let mut family = v3_family();
        family.effect.measurement_resolution = Some(resolution);
        let built = build_receipt_with_history(
            name,
            &family,
            &[spec("quality", 2.0)],
            None,
            &[],
            PilotFixture {
                omit_claim: true,
                ..PilotFixture::default()
            },
            |_| {},
        );
        let summary = evaluate(&built.dir).unwrap();
        assert_eq!(summary.verdict, expected, "{:?}", summary.findings);
        if expected == Verdict::Rejected {
            assert!(summary.findings.iter().any(|finding| {
                finding.rule == "P-03" && finding.message.contains("pilot half-width")
            }));
        }
    }
}

#[test]
fn v3_binds_claimed_alpha_in_receipts_and_pilot_resolution() {
    let family = v3_family();
    let claimed = build_receipt(
        "v3-altered-claimed-alpha",
        &family,
        &[CellSpec {
            claim_alpha: Some(0.02),
            ..spec("quality", 2.0)
        }],
        false,
        false,
        |_| {},
    );
    let summary = evaluate(&claimed.dir).unwrap();
    assert_eq!(summary.verdict, Verdict::Rejected, "{:?}", summary.findings);
    assert!(
        summary
            .findings
            .iter()
            .any(|finding| finding.rule == "P-20"),
        "{:?}",
        summary.findings
    );

    let pilot = build_receipt_with_pilot_alpha(
        "v3-altered-pilot-alpha",
        &family,
        &[spec("quality", 2.0)],
        0.02,
    );
    let summary = evaluate(&pilot.dir).unwrap();
    assert_eq!(summary.verdict, Verdict::Rejected, "{:?}", summary.findings);
    assert!(summary
        .findings
        .iter()
        .any(|finding| { finding.rule == "P-03" && finding.message.contains("pilot interval") }));
}

#[test]
fn v3_lock_evidence_records_observed_lock_facts_without_a_wrapper_identity() {
    let family = v3_family();
    let built = build_receipt(
        "v3-lock-observation",
        &family,
        &[spec("quality", 2.0)],
        false,
        false,
        |_| {},
    );
    let accepted = evaluate(&built.dir).unwrap();
    assert_eq!(
        accepted.verdict,
        Verdict::Accepted,
        "{:?}",
        accepted.findings
    );
    assert!(!accepted
        .findings
        .iter()
        .any(|finding| finding.rule == "P-07"));

    let receipt_path = built.dir.join(RECEIPT_FILE);
    let mut receipt = BenchmarkReceipt::decode(&fs::read(&receipt_path).unwrap()).unwrap();
    receipt.lock.observation = "fixture".into();
    fs::write(receipt_path, serde_json::to_vec_pretty(&receipt).unwrap()).unwrap();
    let rejected = evaluate(&built.dir).unwrap();
    assert!(rejected
        .findings
        .iter()
        .any(|finding| finding.rule == "P-07"));
}

fn frame_quality(decoder: &DecoderCell, candidate_worse: bool) -> ArmQuality {
    let mut result = quality(20, matched_settings());
    result.frames = decoder.input.frames;
    result.bits = result.frames * decoder.code.k;
    result.frame_bit_errors = (0..result.frames)
        .map(|i| u64::from(candidate_worse || i % 2 == 0))
        .collect();
    result.frame_errors = result.frame_bit_errors.iter().filter(|e| **e > 0).count() as u64;
    result.bit_errors = result.frame_errors;
    result.fer = result.frame_errors as f64 / result.frames as f64;
    result.ber = result.bit_errors as f64 / result.bits as f64;
    let fer = wilson_interval_95(result.frame_errors, result.frames).unwrap();
    let ber = tuning_campaign_support::abtest::frame_ber_interval(
        &result.frame_bit_errors,
        decoder.code.k,
        0.95,
    )
    .unwrap();
    result.fer_interval = [fer.0, fer.1];
    result.ber_interval = [ber.0, ber.1];
    result.interval_method = "frame-hoeffding-95+fer-wilson-95".into();
    result
}

#[test]
fn v2_decoder_denominators_points_and_clustered_intervals_are_enforced() {
    let mut family = v2_family();
    let mut decoder = decoder(DecoderArmKind::FastestQualityCompatible);
    decoder.quality_tolerance.fer_ratio_max = 1.5;
    family.cells[0].decoder = Some(decoder.clone());
    let good = frame_quality(&decoder, false);
    for defect in 0..5 {
        let mut candidate = good.clone();
        match defect {
            1 => {
                let mut altered_input = decoder.clone();
                altered_input.input.frames += 1;
                candidate = frame_quality(&altered_input, false);
            }
            2 => {
                let mut altered_code = decoder.clone();
                altered_code.code.k += 1;
                candidate = frame_quality(&altered_code, false);
            }
            3 => candidate.ber *= 2.0,
            4 => candidate = frame_quality(&decoder, true),
            _ => {}
        }
        let built = build_receipt(
            &format!("v2-quality-{defect}"),
            &family,
            &[CellSpec {
                quality: Some(DecoderQualityRecord {
                    baseline: good.clone(),
                    candidate,
                }),
                ..spec("quality", 1.0)
            }],
            false,
            false,
            |_| {},
        );
        let summary = evaluate(&built.dir).unwrap();
        if (1..=3).contains(&defect) {
            assert_eq!(
                summary.verdict,
                Verdict::Rejected,
                "{defect}: {:?}",
                summary.findings
            );
            let expected = match defect {
                1 => "frame count differs from frozen input",
                2 => "bit denominator differs from frozen information-bit identity",
                _ => "BER point differs from error counts",
            };
            assert!(summary
                .findings
                .iter()
                .any(|f| f.rule == "P-18" && f.message.contains(expected)));
        } else {
            assert_eq!(summary.verdict, Verdict::Accepted, "{:?}", summary.findings);
            assert_eq!(
                summary.cells[0].outcome,
                if defect == 4 {
                    CellOutcome::QualityIncompatible
                } else {
                    CellOutcome::Pass
                }
            );
        }
    }
    let clustered: Vec<_> = (0..1000).map(|i| if i < 100 { 1000 } else { 0 }).collect();
    let frame =
        tuning_campaign_support::abtest::frame_ber_interval(&clustered, 1000, 0.95).unwrap();
    let bits = wilson_interval_95(100_000, 1_000_000).unwrap();
    assert!(frame.1 - frame.0 > 10.0 * (bits.1 - bits.0));
}

#[test]
fn cold_cells_reject_precalibrated_plans_and_execute_only_timed_calls() {
    let mut family = v2_family();
    family.cells[0].cache_state = CacheState::Cold;
    assert!(family
        .validate()
        .unwrap_err()
        .iter()
        .any(|e| e.contains("cold_calls")));
    family.cells[0].cold_calls = Some(3);
    family.validate().unwrap();
    let mut calls = 0;
    let mut progress = Vec::new();
    let samples = tuning_campaign_support::timing::execution_windows_fixed_or_calibrated(
        0,
        2,
        std::time::Duration::from_millis(1),
        Some(3),
        &mut |_| calls += 1,
        |event| {
            progress.push(event);
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(calls, 6);
    assert!(progress.iter().all(|event| matches!(
        event,
        tuning_campaign_support::timing::TimingProgress::WindowComplete(_)
    )));
    assert_eq!(samples.iter().map(|s| s.calls).sum::<u64>(), 6);
}

#[test]
fn ledger_preserves_failed_attempts_and_rejects_omissions_and_wrong_correction() {
    use tuning_campaign_support::trial_ledger;
    let root = scratch("ledger-v2");
    let family = v2_family();
    fs::write(root.join("family-ledger.jsonl"), b"").unwrap();
    let mut last = None;
    for campaign in ["successful", "failed", "confirmation"] {
        let stage = root.join(campaign);
        fs::create_dir_all(stage.join("inputs")).unwrap();
        let candidates = vec![sha256_hex(campaign.as_bytes())];
        let pin = trial_ledger::reserve(
            &root,
            &stage,
            &family,
            campaign,
            &"a".repeat(64),
            &candidates,
        )
        .unwrap();
        let count = trial_ledger::verify(
            &pin,
            &stage,
            &family,
            campaign,
            &"a".repeat(64),
            &candidates,
        )
        .unwrap();
        last = Some((stage, pin, count));
    }
    let retry_stage = root.join("retry");
    fs::create_dir_all(retry_stage.join("inputs")).unwrap();
    assert!(trial_ledger::reserve(
        &root,
        &retry_stage,
        &family,
        "retry",
        &"b".repeat(64),
        &[sha256_hex(b"failed")]
    )
    .unwrap_err()
    .to_string()
    .contains("attempt limit"));
    let (stage, pin, count) = last.unwrap();
    assert_eq!(count, 3);
    let bytes = fs::read(stage.join(&pin.snapshot)).unwrap();
    let removed: Vec<u8> = bytes
        .split_inclusive(|b| *b == b'\n')
        .enumerate()
        .filter(|(index, _)| *index != 1)
        .flat_map(|(_, line)| line.iter().copied())
        .collect();
    assert!(trial_ledger::decode(&removed, &family.family.id).is_err());
    let confidence = bonferroni_confidence(0.05, count).unwrap();
    assert_ne!(confidence, bonferroni_confidence(0.05, count - 1).unwrap());
    let complete = build_receipt_with_history(
        "complete-v2-chain",
        &family,
        &[spec("quality", 2.0)],
        None,
        &["failed-prior"],
        PilotFixture::default(),
        |_| {},
    );
    let accepted = evaluate(&complete.dir).unwrap();
    assert_eq!(
        accepted.verdict,
        Verdict::Accepted,
        "{:?}",
        accepted.findings
    );
    assert_eq!(accepted.family.as_ref().unwrap().comparisons, 2);
    let receipt =
        BenchmarkReceipt::decode(&fs::read(complete.dir.join(RECEIPT_FILE)).unwrap()).unwrap();
    let ledger_pin = receipt.trial_ledger.unwrap();
    let bytes = fs::read(complete.dir.join(&ledger_pin.snapshot)).unwrap();
    let missing_failed: Vec<u8> = bytes
        .split_inclusive(|b| *b == b'\n')
        .skip(1)
        .flatten()
        .copied()
        .collect();
    assert!(trial_ledger::decode(&missing_failed, &family.family.id).is_err());
    fs::write(complete.dir.join(&ledger_pin.snapshot), missing_failed).unwrap();
    let omitted = evaluate(&complete.dir).unwrap();
    assert_eq!(omitted.verdict, Verdict::Rejected);
    assert!(omitted.findings.iter().any(|f| f.rule == "P-22"));
    let built = build_receipt(
        "wrong-v2-correction",
        &family,
        &[CellSpec {
            claim_confidence: Some(confidence),
            ..spec("quality", 2.0)
        }],
        false,
        false,
        |_| {},
    );
    let summary = evaluate(&built.dir).unwrap();
    assert_eq!(summary.verdict, Verdict::Rejected);
    assert!(summary.findings.iter().any(|f| f.rule == "P-20"));
}

#[test]
fn family_summary_names_first_attempt_alpha_distinctly_from_the_family_total() {
    let family = v2_family();
    let built = build_receipt(
        "first-attempt-alpha-naming",
        &family,
        &[spec("quality", 2.0)],
        false,
        false,
        |_| {},
    );
    let summary = evaluate(&built.dir).unwrap();
    assert_eq!(summary.verdict, Verdict::Accepted, "{:?}", summary.findings);
    let fam = summary.family.as_ref().unwrap();

    // t = 1: the ledger holds only this attempt's own reservation.
    let expected_attempt_alpha = family.family_wise.alpha / (1.0 * 2.0);
    assert_eq!(fam.family_alpha, family.family_wise.alpha, "frozen total");
    assert_eq!(fam.attempt_alpha, expected_attempt_alpha, "t=1 allocation");
    assert_ne!(fam.family_alpha, fam.attempt_alpha);
    assert_eq!(
        fam.corrected_alpha,
        fam.attempt_alpha / f64::from(fam.comparisons)
    );

    let receipt =
        BenchmarkReceipt::decode(&fs::read(built.dir.join(RECEIPT_FILE)).unwrap()).unwrap();
    let ledger_pin = receipt.trial_ledger.clone().unwrap();
    let recomputed =
        tuning_campaign_support::trial_ledger::attempt_alpha(&ledger_pin, &built.dir, &family)
            .unwrap();
    assert_eq!(fam.attempt_alpha, recomputed);

    let markdown = render_markdown(&summary);
    assert!(markdown.contains(&format!("family-wise alpha {}", fam.family_alpha)));
    assert!(markdown.contains(&format!("attempt alpha {}", fam.attempt_alpha)));
    assert!(!markdown.contains(&format!("family-wise alpha {}", fam.attempt_alpha)));
}

#[test]
fn family_summary_names_later_attempt_alpha_distinctly_from_the_family_total() {
    let family = v2_family();
    let built = build_receipt_with_history(
        "later-attempt-alpha-naming",
        &family,
        &[spec("quality", 2.0)],
        None,
        &["prior-attempt"],
        PilotFixture::default(),
        |_| {},
    );
    let summary = evaluate(&built.dir).unwrap();
    assert_eq!(summary.verdict, Verdict::Accepted, "{:?}", summary.findings);
    let fam = summary.family.as_ref().unwrap();

    // t = 2: one prior reservation precedes this attempt's own.
    let expected_attempt_alpha = family.family_wise.alpha / (2.0 * 3.0);
    assert_eq!(fam.family_alpha, family.family_wise.alpha, "frozen total");
    assert_eq!(fam.attempt_alpha, expected_attempt_alpha, "t=2 allocation");
    assert_ne!(fam.family_alpha, fam.attempt_alpha);
    assert_eq!(
        fam.corrected_alpha,
        fam.attempt_alpha / f64::from(fam.comparisons)
    );

    let receipt =
        BenchmarkReceipt::decode(&fs::read(built.dir.join(RECEIPT_FILE)).unwrap()).unwrap();
    let ledger_pin = receipt.trial_ledger.clone().unwrap();
    let recomputed =
        tuning_campaign_support::trial_ledger::attempt_alpha(&ledger_pin, &built.dir, &family)
            .unwrap();
    assert_eq!(fam.attempt_alpha, recomputed);

    let markdown = render_markdown(&summary);
    assert!(markdown.contains(&format!("family-wise alpha {}", fam.family_alpha)));
    assert!(markdown.contains(&format!("attempt alpha {}", fam.attempt_alpha)));
    assert!(!markdown.contains(&format!("family-wise alpha {}", fam.attempt_alpha)));
}

#[test]
fn v2_outlier_flags_do_not_depend_on_cross_arm_effect_size() {
    let family = v2_family();
    let built = build_receipt(
        "v2-outliers",
        &family,
        &[CellSpec {
            jitter: 0.3,
            ..spec("quality", 100.0)
        }],
        false,
        false,
        |_| {},
    );
    let summary = evaluate(&built.dir).unwrap();
    assert_eq!(summary.verdict, Verdict::Accepted, "{:?}", summary.findings);
    assert_eq!(summary.cells[0].flagged_windows, 0);
    let receipt =
        BenchmarkReceipt::decode(&fs::read(built.dir.join(RECEIPT_FILE)).unwrap()).unwrap();
    let pooled: Vec<_> = receipt.cells[0]
        .pairs
        .iter()
        .flat_map(|p| [&p.baseline, &p.candidate])
        .flat_map(|e| e.windows.iter().map(|w| w.ns_per_call()))
        .collect();
    assert!(tuning_campaign_support::abtest::flagged_windows(&pooled, 2.0).unwrap() > 0);
}

#[test]
fn v2_rejects_a_freeze_timestamp_after_the_opening_record() {
    let mut family = v2_family();
    family.frozen.frozen_utc = Some("2999-01-01T00:00:00Z".into());
    let built = build_receipt(
        "future-freeze",
        &family,
        &[spec("quality", 2.0)],
        false,
        false,
        |_| {},
    );
    let summary = evaluate(&built.dir).unwrap();
    assert_eq!(summary.verdict, Verdict::Rejected);
    assert!(summary
        .findings
        .iter()
        .any(|f| f.rule == "P-23" && f.message.contains("timestamp")));
}

#[test]
fn v2_unresolved_bootstrap_endpoint_resolution_prevents_confirmation() {
    let mut family = v2_family();
    family.effect.measurement_resolution = Some(1e-12);
    let built = build_receipt(
        "endpoint-resolution",
        &family,
        &[CellSpec {
            jitter: 0.3,
            ..spec("quality", 2.0)
        }],
        false,
        false,
        |_| {},
    );
    let summary = evaluate(&built.dir).unwrap();
    assert_eq!(summary.verdict, Verdict::Accepted, "{:?}", summary.findings);
    assert_eq!(summary.cells[0].outcome, CellOutcome::NotConfirmatory);
    assert!(summary
        .findings
        .iter()
        .any(|f| f.rule == "P-20" && f.message.contains("resolution")));
}

#[test]
fn v3_family_producing_manifest_is_selected_by_the_plan_and_bound_to_facts() {
    let built = build_receipt(
        "v3-family-producing",
        &v3_family(),
        &[spec("quality", 2.0)],
        false,
        false,
        |_| {},
    );
    let bytes = fs::read(built.dir.join(PLAN_FILE)).unwrap();
    let mut value: Value = serde_json::from_slice(&bytes).unwrap();
    value["producing_manifest"] = json!("producing-inputs.json");
    let plan_bytes = serde_json::to_vec(&value).unwrap();
    let plan = RunnerPlan::decode(&plan_bytes)
        .expect("a plan must select the family producing-input manifest");
    let receipt =
        BenchmarkReceipt::decode(&fs::read(built.dir.join(RECEIPT_FILE)).unwrap()).unwrap();
    let records = ExecutionLog::validate_prefix(
        &fs::read(built.dir.join(LOG_FILE)).unwrap(),
        "fixture-campaign",
    )
    .unwrap();
    let mut facts: CampaignFacts = serde_json::from_value(records[0].details.clone()).unwrap();
    facts.plan_sha256 = sha256_hex(&plan_bytes);
    let checkpoint = serde_json::from_slice(
        &fs::read(built.dir.join(CHECKPOINT_DIR).join("manifest.json")).unwrap(),
    )
    .unwrap();
    assert!(facts
        .validate_receipt(&receipt, &plan, &plan_bytes, &v3_family(), &checkpoint)
        .is_empty());
    value["producing_manifest"] = json!("another-family.json");
    let changed_bytes = serde_json::to_vec(&value).unwrap();
    let changed = RunnerPlan::decode(&changed_bytes).unwrap();
    facts.plan_sha256 = sha256_hex(&changed_bytes);
    assert!(facts
        .validate_receipt(
            &receipt,
            &changed,
            &changed_bytes,
            &v3_family(),
            &checkpoint
        )
        .iter()
        .any(|e| e.contains("producing")));
    value.as_object_mut().unwrap().remove("producing_manifest");
    let omitted_bytes = serde_json::to_vec(&value).unwrap();
    let omitted = RunnerPlan::decode(&omitted_bytes).unwrap();
    facts.plan_sha256 = sha256_hex(&omitted_bytes);
    assert!(
        facts
            .validate_receipt(
                &receipt,
                &omitted,
                &omitted_bytes,
                &v3_family(),
                &checkpoint
            )
            .iter()
            .any(|e| e.contains("producing")),
        "an omitted field selects the shared manifest, not the family manifest"
    );
    assert_eq!(
        Path::new(&omitted.producing_manifest_path(&facts.protocol.path)),
        Path::new(&facts.protocol.path).with_file_name(SHARED_PRODUCING_MANIFEST),
        "the shared manifest lies beside the pinned protocol document"
    );
    for invalid in ["/tmp/manifest.json", "../manifest.json", "family/*.json"] {
        value["producing_manifest"] = json!(invalid);
        assert!(
            RunnerPlan::decode(&serde_json::to_vec(&value).unwrap()).is_err(),
            "{invalid}"
        );
    }
}

#[test]
fn v3_decoder_quality_preserves_exact_vectors_but_allows_process_diagnostics() {
    let mut family = v3_family();
    let decoder = decoder(DecoderArmKind::FastestQualityCompatible);
    family.cells[0].decoder = Some(decoder.clone());
    let good = frame_quality(&decoder, false);
    for defect in 0..4 {
        let mut observed = good.clone();
        observed.memory_bytes += 4096;
        observed.latency_ns_p50 += 100;
        match defect {
            1 => {
                observed.frame_bit_errors.swap(0, 1);
            }
            2 => {
                observed.settings.iteration_cap += 1;
            }
            3 => {
                observed.iterations.max += 1;
            }
            _ => {}
        }
        let built = build_receipt(
            &format!("v3-quality-diagnostics-{defect}"),
            &family,
            &[CellSpec {
                quality: Some(DecoderQualityRecord {
                    baseline: good.clone(),
                    candidate: good.clone(),
                }),
                execution_quality: Some(DecoderQualityRecord {
                    baseline: observed.clone(),
                    candidate: observed,
                }),
                ..spec("quality", 1.0)
            }],
            false,
            false,
            |_| {},
        );
        let summary = evaluate(&built.dir).unwrap();
        assert_eq!(
            summary.verdict,
            if defect == 0 {
                Verdict::Accepted
            } else {
                Verdict::Rejected
            },
            "{defect}: {:?}",
            summary.findings
        );
    }
}

const INTERRUPTED_CAMPAIGN: &str = "interrupted-campaign";

/// Candidate arm of the interrupted campaign. It counts candidate executions
/// and, while `$STALL` exists, blocks the ninth one: pair 2 of cell `second`,
/// after that cell's first two pairs completed. Every other execution runs the
/// smoke workload.
const STALLING_CANDIDATE: &str = r#"n=$(( $(cat "$COUNTER" 2>/dev/null || echo 0) + 1 ))
echo "$n" > "$COUNTER"
if [ "$n" -eq 9 ] && [ -e "$STALL" ]; then
  echo "$$" > "$STALLED.tmp" && mv "$STALLED.tmp" "$STALLED"
  exec sleep 600
fi
exec "$WORKLOAD""#;

/// A finalized receipt directory and its execution log.
struct InterruptedCampaign {
    /// Owns the tree `out` points into.
    _root: Scratch,
    out: PathBuf,
    records: Vec<JournalRecord>,
}

fn record_key(record: &JournalRecord) -> Option<&str> {
    record.case.as_ref()?.get("key")?.as_str()
}

fn kill_group(group: u32) {
    let _ = rustix::process::kill_process_group(
        rustix::process::Pid::from_raw(group as i32).unwrap(),
        rustix::process::Signal::KILL,
    );
}

/// Runs a three-cell pilot through the runner, kills its first session inside
/// cell `second` as an operator stop would, resumes it under the same plan and
/// stage, and finalizes the receipt.
fn interrupted_campaign(name: &str) -> InterruptedCampaign {
    let root = scratch(name);
    let repo = root.join("repo");
    stage_repo(&repo);
    stage_runner_producing(&repo);
    let ids = ["first", "second", "third"];
    let mut family = addendum(
        ids.iter()
            .map(|id| {
                // Fixed-call cold windows skip calibration and keep the test short.
                CellDeclaration {
                    cache_state: CacheState::Cold,
                    cold_calls: Some(100),
                    ..cell(
                        id,
                        CellObjective::Improvement,
                        CellRole::Exploratory,
                        CoreArm::SingleCore,
                    )
                }
            })
            .collect(),
    );
    family.protocol.version = PROTOCOL_VERSION;
    family.schema = ADDENDUM_SCHEMA_ID.into();
    family.family_wise.ledger_path = Some("family-ledger.jsonl".into());
    // Without margins the pilot cells carry no claim, so an edited sample is
    // judged by its journal provenance alone.
    family.effect.equivalence_margin = None;
    family.effect.measurement_resolution = None;
    family.effect.resolution_evidence = None;
    fs::write(repo.join("family-ledger.jsonl"), b"").unwrap();
    stage_current(&repo);
    write_addendum(&repo, &family);
    git(&repo, &["init", "-q"]);
    let lock = root.join("bench.lock");
    fs::write(&lock, b"").unwrap();
    let stall = root.join("stall");
    let stalled = root.join("stalled");
    fs::write(&stall, b"").unwrap();
    let workload = env!("CARGO_BIN_EXE_ab-smoke-workload");
    let plan = json!({
        "schema": "zen3-benchmark-plan-v1",
        "campaign_id": INTERRUPTED_CAMPAIGN,
        "producing_manifest": "producing-inputs.json",
        "issue": "f547c394",
        "label": "pilot",
        "campaign_seed": 11,
        "addendum": ADDENDUM_DOC,
        "lock_path": lock,
        "wrapper": "flock",
        "timing_override": {"windows_per_execution": 2, "window_target_ms": 3},
        "arms": {
            "baseline": {
                "build": "conservative-portable",
                "description": "xor-fold 2 pass",
                "executable": workload,
                "arguments": [],
                "environment": {"GF2_SMOKE_PASSES": "2"},
                "rustflags": null,
                "tuning_profile": null
            },
            "candidate": {
                "build": "conservative-portable",
                "description": "xor-fold 1 pass behind a stall switch",
                "executable": "/bin/sh",
                "arguments": ["-c", STALLING_CANDIDATE],
                "environment": {
                    "GF2_SMOKE_PASSES": "1",
                    "WORKLOAD": workload,
                    "COUNTER": root.join("candidate-executions"),
                    "STALL": stall,
                    "STALLED": stalled
                },
                "rustflags": null,
                "tuning_profile": null
            }
        },
        "cells": ids.iter().enumerate().map(|(index, id)| json!({
            "cell_id": id,
            "baseline_arm": "baseline",
            "candidate_arm": "candidate",
            "case": {"words": 4096, "seed": index + 1},
            "pilot_pairs": 6
        })).collect::<Vec<_>>(),
        "max_cells_per_session": null
    });
    let plan_path = root.join("plan.json");
    fs::write(&plan_path, serde_json::to_vec_pretty(&plan).unwrap()).unwrap();
    let stage = root.join("stage");
    let runner = env!("CARGO_BIN_EXE_benchmark-ab-runner");
    let session = || {
        let mut command = Command::new("flock");
        command
            .arg("-x")
            .arg(&lock)
            .args(["taskset", "-c", "0-2", runner, "run"])
            .arg(&stage)
            .arg(&plan_path)
            .current_dir(&repo);
        command
    };
    // The first session gets its own process group, so killing that group
    // stops the lock wrapper and the runner together, mid-cell.
    let mut first = session()
        .process_group(0)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    // Wait until the runner has journaled the stalled child's spawn: from then
    // on it only waits for that child, so the kill lands between records.
    let spawn_journaled = |pid: u32| {
        fs::read_to_string(stage.join(LOG_FILE))
            .unwrap_or_default()
            .lines()
            .filter_map(|line| serde_json::from_str::<JournalRecord>(line).ok())
            .any(|record| {
                record.event == JournalEvent::ChildSpawn
                    && record.details["pid"].as_u64() == Some(u64::from(pid))
            })
    };
    let deadline = Instant::now() + Duration::from_secs(6);
    let stalled_child = loop {
        if let Some(pid) = fs::read_to_string(&stalled)
            .ok()
            .and_then(|text| text.trim().parse::<u32>().ok())
            .filter(|pid| spawn_journaled(*pid))
        {
            break pid;
        }
        assert!(
            first.try_wait().unwrap().is_none(),
            "the first session ended before the candidate stalled"
        );
        assert!(Instant::now() < deadline, "the candidate never stalled");
        std::thread::sleep(Duration::from_millis(5));
    };
    kill_group(first.id());
    // The runner starts every arm in its own process group.
    kill_group(stalled_child);
    first.wait().unwrap();
    fs::remove_file(&stall).unwrap();
    let resumed = session().output().unwrap();
    assert_eq!(
        resumed.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&resumed.stderr)
    );
    let out = repo.join("receipts/interrupted");
    let finalize = Command::new(runner)
        .arg("finalize")
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
    let records =
        ExecutionLog::validate_prefix(&fs::read(out.join(LOG_FILE)).unwrap(), INTERRUPTED_CAMPAIGN)
            .unwrap();
    InterruptedCampaign {
        _root: root,
        out,
        records,
    }
}

/// Sequences of the `cell-start` records of one cell.
fn cell_starts(records: &[JournalRecord], key: &str) -> Vec<u64> {
    records
        .iter()
        .filter(|record| record.event == JournalEvent::CellStart && record_key(record) == Some(key))
        .map(|record| record.sequence)
        .collect()
}

#[test]
fn runner_restarts_an_interrupted_cell_and_acceptance_accepts_the_receipt() {
    let campaign = interrupted_campaign("interrupted-accepted");
    let summary = evaluate(&campaign.out).unwrap();
    assert_eq!(summary.verdict, Verdict::Accepted, "{:?}", summary.findings);
    assert_eq!(
        tuning_campaign_support::receipt::evaluate_version(&campaign.out, Some(PROTOCOL_VERSION))
            .unwrap()
            .verdict,
        Verdict::Accepted
    );
    assert_eq!(summary.sessions, 2);
    assert!(summary.resumed);
    for id in ["first", "second", "third"] {
        assert_eq!(outcome(&summary, id), CellOutcome::Pilot);
    }
    let records = &campaign.records;
    assert_eq!(cell_starts(records, "first").len(), 1);
    assert_eq!(cell_starts(records, "third").len(), 1);
    let second = cell_starts(records, "second");
    assert_eq!(second.len(), 2, "only the interrupted cell starts again");
    let interrupted = records
        .iter()
        .find(|record| record.event == JournalEvent::Interrupted)
        .unwrap();
    assert_eq!(interrupted.details["cause"], "missing-terminal-record");
    let abandoned: Vec<_> = records
        .iter()
        .filter(|record| record.event == JournalEvent::CellAbandoned)
        .collect();
    assert_eq!(abandoned.len(), 1);
    assert_eq!(record_key(abandoned[0]), Some("second"));
    assert_eq!(abandoned[0].details, json!({"attempt_start": second[0]}));
    assert!(interrupted.sequence < abandoned[0].sequence && abandoned[0].sequence < second[1]);
    let spawned = |attempt: std::ops::Range<u64>| {
        records
            .iter()
            .filter(|record| {
                record.event == JournalEvent::ChildSpawn
                    && record_key(record) == Some("second")
                    && attempt.contains(&record.sequence)
            })
            .map(|record| record.details["pid"].as_u64().unwrap() as u32)
            .collect::<BTreeSet<_>>()
    };
    let abandoned_samples = records
        .iter()
        .filter(|record| {
            record.event == JournalEvent::ExecutionProgress
                && record_key(record) == Some("second")
                && (second[0]..second[1]).contains(&record.sequence)
        })
        .count();
    assert!(
        abandoned_samples >= 4,
        "two pairs completed before the stop"
    );
    let receipt =
        BenchmarkReceipt::decode(&fs::read(campaign.out.join(RECEIPT_FILE)).unwrap()).unwrap();
    let accepted: BTreeSet<u32> = receipt
        .cells
        .iter()
        .find(|cell| cell.cell_id == "second")
        .unwrap()
        .pairs
        .iter()
        .flat_map(|pair| [pair.baseline.pid, pair.candidate.pid])
        .collect();
    assert_eq!(accepted, spawned(second[1]..u64::MAX));
    assert!(accepted.is_disjoint(&spawned(second[0]..second[1])));
}

fn copy_dir(source: &Path, target: &Path) {
    fs::create_dir_all(target).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let destination = target.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &destination);
        } else {
            fs::copy(entry.path(), destination).unwrap();
        }
    }
}

/// Edits the journal as a whole, renumbers it into a valid journal and pins
/// the new digest in the receipt.
fn rewrite_log(dir: &Path, edit: impl FnOnce(&mut Vec<JournalRecord>)) {
    let path = dir.join(LOG_FILE);
    let mut records =
        ExecutionLog::validate_prefix(&fs::read(&path).unwrap(), INTERRUPTED_CAMPAIGN).unwrap();
    edit(&mut records);
    let mut bytes = Vec::new();
    for (sequence, record) in records.iter_mut().enumerate() {
        record.sequence = sequence as u64;
        bytes.extend(serde_json::to_vec(record).unwrap());
        bytes.push(b'\n');
    }
    ExecutionLog::validate_prefix(&bytes, INTERRUPTED_CAMPAIGN).unwrap();
    fs::write(&path, &bytes).unwrap();
    let receipt_path = dir.join(RECEIPT_FILE);
    let mut receipt = BenchmarkReceipt::decode(&fs::read(&receipt_path).unwrap()).unwrap();
    receipt.execution_log.sha256 = sha256_hex(&bytes);
    fs::write(receipt_path, serde_json::to_vec_pretty(&receipt).unwrap()).unwrap();
}

/// Edits one receipt cell and republishes a consistent checkpoint store.
fn rewrite_cell(dir: &Path, cell_id: &str, edit: impl FnOnce(&mut CellRecord)) {
    let receipt_path = dir.join(RECEIPT_FILE);
    let mut receipt = BenchmarkReceipt::decode(&fs::read(&receipt_path).unwrap()).unwrap();
    edit(
        receipt
            .cells
            .iter_mut()
            .find(|cell| cell.cell_id == cell_id)
            .unwrap(),
    );
    let plan = RunnerPlan::decode(&fs::read(dir.join(PLAN_FILE)).unwrap()).unwrap();
    let root = dir.join(CHECKPOINT_DIR);
    let identity = CheckpointStore::inspect(&root).unwrap().identity().clone();
    fs::remove_dir_all(&root).unwrap();
    let mut store = CheckpointStore::create_new(&root, &receipt.campaign_id, identity).unwrap();
    for cell in &mut receipt.cells {
        let planned = plan
            .cells
            .iter()
            .find(|planned| planned.cell_id == cell.cell_id)
            .unwrap();
        let case = json!({"key": cell.key, "cell_id": cell.cell_id, "case": planned.case});
        cell.checkpoint_sha256 = None;
        cell.checkpoint_sha256 = Some(store.accept(&cell.key, &case, cell).unwrap().sha256);
    }
    receipt.checkpoints.manifest_sha256 =
        sha256_hex(&fs::read(root.join("manifest.json")).unwrap());
    fs::write(&receipt_path, serde_json::to_vec_pretty(&receipt).unwrap()).unwrap();
}

/// Asserts that P-11 alone rejects the receipt, naming cell `second`.
fn assert_rejected_by_p11(dir: &Path, expected: &str) {
    let summary = evaluate(dir).unwrap();
    assert_eq!(summary.verdict, Verdict::Rejected, "{expected}");
    let errors: Vec<_> = summary
        .findings
        .iter()
        .filter(|finding| finding.severity == Severity::Error)
        .collect();
    assert!(
        errors.iter().all(|finding| finding.rule == "P-11"),
        "{expected}: {errors:?}"
    );
    assert!(
        errors.iter().any(|finding| {
            finding.cell.as_deref() == Some("second") && finding.message.contains(expected)
        }),
        "{expected}: {errors:?}"
    );
}

#[test]
fn acceptance_rejects_unrecorded_restarts_double_completions_and_abandoned_samples() {
    let campaign = interrupted_campaign("interrupted-rejected");
    let records = &campaign.records;
    let second = cell_starts(records, "second");
    let variant = |name: &str| {
        let dir = campaign.out.with_file_name(name);
        copy_dir(&campaign.out, &dir);
        dir
    };

    let unrecorded = variant("unrecorded-restart");
    rewrite_log(&unrecorded, |records| {
        records.retain(|record| record.event != JournalEvent::CellAbandoned);
    });
    assert_rejected_by_p11(&unrecorded, "without an abandonment record");

    let completed_twice = variant("completed-twice");
    rewrite_log(&completed_twice, |records| {
        let index = records
            .iter()
            .position(|record| {
                record.event == JournalEvent::CellComplete && record_key(record) == Some("second")
            })
            .unwrap();
        let duplicate = records[index].clone();
        records.insert(index + 1, duplicate);
    });
    assert_rejected_by_p11(&completed_twice, "completed 2 times");

    let journaled = |event: JournalEvent| {
        records
            .iter()
            .find(|record| {
                record.event == event
                    && record_key(record) == Some("second")
                    && (second[0]..second[1]).contains(&record.sequence)
                    && record.case.as_ref().unwrap()["arm"] == "candidate"
                    && record.case.as_ref().unwrap()["pair"] == 0
            })
            .unwrap()
    };
    let pid = journaled(JournalEvent::ChildSpawn).details["pid"]
        .as_u64()
        .unwrap() as u32;
    let progress = journaled(JournalEvent::ExecutionProgress);
    let windows: Vec<WindowRecord> =
        serde_json::from_value(progress.details["windows"].clone()).unwrap();
    let ns_per_call = progress.details["ns_per_call"].as_f64().unwrap();
    let abandoned_sample = variant("abandoned-sample");
    rewrite_cell(&abandoned_sample, "second", |cell| {
        let execution = &mut cell.pairs[0].candidate;
        execution.pid = pid;
        execution.windows = windows;
        execution.ns_per_call = ns_per_call;
    });
    assert_rejected_by_p11(&abandoned_sample, "abandoned attempt");

    let unjournaled_sample = variant("unjournaled-sample");
    rewrite_cell(&unjournaled_sample, "second", |cell| {
        let execution = &mut cell.pairs[1].baseline;
        execution.windows[0].elapsed_ns += 1;
        let mut values: Vec<f64> = execution
            .windows
            .iter()
            .map(|window| window.ns_per_call())
            .collect();
        execution.ns_per_call = median(&mut values).unwrap();
    });
    assert_rejected_by_p11(&unjournaled_sample, "differ from the executions");
}

#[test]
fn resolution_evidence_accepts_pilots_from_version_3_to_the_citing_version() {
    for (family_version, pilot_version, expected) in [
        (PROTOCOL_VERSION, 3, Verdict::Accepted),
        (PROTOCOL_VERSION, PROTOCOL_VERSION, Verdict::Accepted),
        (3, PROTOCOL_VERSION, Verdict::Rejected),
    ] {
        let mut family = v3_family();
        family.protocol.version = family_version;
        family.schema = format!("zen3-benchmark-addendum-v{family_version}");
        let built = build_receipt_with_history(
            &format!("v{pilot_version}-pilot-for-v{family_version}"),
            &family,
            &[spec("quality", 2.0)],
            None,
            &[],
            PilotFixture {
                protocol_version: Some(pilot_version),
                ..PilotFixture::default()
            },
            |_| {},
        );
        let summary = evaluate(&built.dir).unwrap();
        assert_eq!(
            summary.verdict, expected,
            "v{pilot_version} pilot for v{family_version}: {:?}",
            summary.findings
        );
        if expected == Verdict::Rejected {
            assert!(summary.findings.iter().any(|finding| {
                finding.rule == "P-03" && finding.message.contains("versioned pilot identity")
            }));
        }
    }
}

#[test]
fn v3_receipts_keep_rejecting_a_restarted_cell_and_refuse_later_rules() {
    let dir = committed_receipt("26465e6c-v3-and-popcnt-pilot");
    let summary = tuning_campaign_support::receipt::evaluate_version(&dir, Some(3)).unwrap();
    assert_eq!(summary.verdict, Verdict::Rejected);
    assert!(summary.findings.iter().any(|finding| {
        finding.rule == "P-11"
            && finding.cell.as_deref() == Some("and-popcnt-w4096-vs-scalar-control")
            && finding.message.contains("started 2 times")
    }));
    let later =
        tuning_campaign_support::receipt::evaluate_version(&dir, Some(PROTOCOL_VERSION)).unwrap();
    assert_eq!(later.verdict, Verdict::Rejected);
    assert!(later.findings.iter().any(|finding| finding.rule == "P-01"));
}

mod arm_wire {
    use super::{cell, CellObjective, CellRole, CoreArm};
    use serde::{Deserialize, Serialize};
    use serde_json::{json, Value};
    use tuning_campaign_support::arm::{ArmRequest, PairPosition, ARM_REQUEST_SCHEMA};
    use tuning_campaign_support::protocol::{CacheState, PlanCell, SHARED_SETTINGS};
    use tuning_campaign_support::transport;

    /// The mirror of an arm that names the position with its own enumeration
    /// and leaves the decoder opaque.
    #[derive(Debug, Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    struct TypedMirror {
        schema: String,
        cell_id: String,
        arm: String,
        role: PairPosition,
        pair: u32,
        case: Value,
        cache_state: CacheState,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cold_calls: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        decoder: Option<Value>,
        windows: u32,
        window_target_ms: u32,
        cpus: Vec<u32>,
        workers_declared: u32,
    }

    /// The mirror of an arm that keeps the position and the cache state as
    /// strings.
    #[derive(Debug, Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    struct StringMirror {
        schema: String,
        cell_id: String,
        arm: String,
        role: String,
        pair: u32,
        case: Value,
        cache_state: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cold_calls: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        decoder: Option<Value>,
        windows: u32,
        window_target_ms: u32,
        cpus: Vec<u32>,
        workers_declared: u32,
    }

    /// A mirror that names every field but declares two of them in the wrong
    /// order.
    #[derive(Debug, Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    struct ReorderedMirror {
        schema: String,
        arm: String,
        cell_id: String,
        role: String,
        pair: u32,
        case: Value,
        cache_state: String,
        windows: u32,
        window_target_ms: u32,
        cpus: Vec<u32>,
        workers_declared: u32,
    }

    /// A mirror carrying a field the runner does not send.
    #[derive(Debug, Deserialize, Serialize)]
    struct ExtraFieldMirror {
        schema: String,
        cell_id: String,
        arm: String,
        role: String,
        pair: u32,
        case: Value,
        cache_state: String,
        windows: u32,
        window_target_ms: u32,
        cpus: Vec<u32>,
        workers_declared: u32,
        #[serde(default)]
        repetitions: u32,
    }

    /// A mirror that spells an absent optional field `null`.
    #[derive(Debug, Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    struct NullOptionalMirror {
        schema: String,
        cell_id: String,
        arm: String,
        role: String,
        pair: u32,
        case: Value,
        cache_state: String,
        #[serde(default)]
        cold_calls: Option<u64>,
        #[serde(default)]
        decoder: Option<Value>,
        windows: u32,
        window_target_ms: u32,
        cpus: Vec<u32>,
        workers_declared: u32,
    }

    fn plan_cell() -> PlanCell {
        PlanCell {
            cell_id: "first".to_owned(),
            baseline_arm: "baseline".to_owned(),
            candidate_arm: "candidate".to_owned(),
            case: json!({"seed": 1, "words": 4096}),
            pilot_pairs: None,
        }
    }

    #[test]
    fn arm_mirrors_round_trip_the_runners_validation_request() {
        let declared = cell(
            "first",
            CellObjective::Improvement,
            CellRole::Exploratory,
            CoreArm::SingleCore,
        );
        let request = ArmRequest::validation(&plan_cell(), &declared, "baseline");
        assert_eq!(request.schema, ARM_REQUEST_SCHEMA);
        assert_eq!(request.role, PairPosition::Validation);
        assert_eq!((request.windows, request.window_target_ms), (0, 0));
        let bytes = transport::encode_case(&request).expect("the runner's encoder");
        assert!(
            !bytes.contains("cold_calls") && !bytes.contains("decoder"),
            "{bytes}"
        );

        let typed: TypedMirror = transport::decode_case(&bytes).expect("the arm-common mirror");
        assert_eq!(typed.role, PairPosition::Validation);
        assert_eq!(transport::encode_case(&typed).expect("re-encodes"), bytes);
        let loose: StringMirror = transport::decode_case(&bytes).expect("the string mirror");
        assert_eq!(loose.role, "validation");
        assert_eq!(transport::encode_case(&loose).expect("re-encodes"), bytes);

        for reason in [
            transport::decode_case::<ReorderedMirror>(&bytes).expect_err("field order"),
            transport::decode_case::<ExtraFieldMirror>(&bytes).expect_err("field set"),
            transport::decode_case::<NullOptionalMirror>(&bytes).expect_err("null optional"),
        ] {
            assert!(reason.contains("canonical"), "{reason}");
        }
    }

    #[test]
    fn a_timed_request_takes_a_pair_side_and_refuses_the_validation_position() {
        let declared = cell(
            "first",
            CellObjective::Improvement,
            CellRole::Exploratory,
            CoreArm::SingleCore,
        );
        for (position, spelling) in [
            (PairPosition::Baseline, "baseline"),
            (PairPosition::Candidate, "candidate"),
        ] {
            let request = ArmRequest::timed(
                &plan_cell(),
                &declared,
                "baseline",
                position,
                3,
                vec![0],
                &SHARED_SETTINGS,
            )
            .expect("a pair side is a timed position");
            assert_eq!(request.windows, SHARED_SETTINGS.windows_per_execution);
            assert_eq!(request.window_target_ms, SHARED_SETTINGS.window_target_ms);
            assert_eq!(request.pair, 3);
            let bytes = transport::encode_case(&request).expect("the runner's encoder");
            assert!(
                bytes.contains(&format!("\"role\":\"{spelling}\"")),
                "{bytes}"
            );
        }
        let refused = ArmRequest::timed(
            &plan_cell(),
            &declared,
            "baseline",
            PairPosition::Validation,
            0,
            vec![0],
            &SHARED_SETTINGS,
        )
        .expect_err("a campaign execution cannot take the validation position");
        assert!(refused.contains("validation"), "{refused}");
    }
}

#[test]
fn staged_smoke_pauses_resumes_and_finalize_refuses_the_stage() {
    use tuning_campaign_support::arm::{PairPosition, SmokeRecord};
    let root = scratch("staged-smoke");
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
    ]);
    family.protocol.version = PROTOCOL_VERSION;
    family.schema = ADDENDUM_SCHEMA_ID.into();
    family.family_wise.ledger_path = Some("family-ledger.jsonl".into());
    family.effect.measurement_resolution = None;
    family.effect.resolution_evidence = None;
    fs::write(repo.join("family-ledger.jsonl"), b"").unwrap();
    stage_current(&repo);
    write_addendum(&repo, &family);
    git(&repo, &["init", "-q"]);
    let lock = root.join("absent.lock");
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
        "campaign_id": "smoke-stage",
        "producing_manifest": "producing-inputs.json",
        "issue": "f547c394",
        "label": "smoke",
        "campaign_seed": 11,
        "addendum": ADDENDUM_DOC,
        "lock_path": lock,
        "wrapper": "flock",
        "timing_override": null,
        "arms": {"baseline": arm("2"), "candidate": arm("1")},
        "cells": [
            {"cell_id": "first", "baseline_arm": "baseline", "candidate_arm": "candidate", "case": {"words": 4096, "seed": 1}, "pilot_pairs": 6},
            {"cell_id": "second", "baseline_arm": "baseline", "candidate_arm": "candidate", "case": {"words": 4096, "seed": 2}, "pilot_pairs": 6}
        ],
        "max_cells_per_session": 1
    });
    let plan_path = root.join("plan.json");
    fs::write(&plan_path, serde_json::to_vec_pretty(&plan).unwrap()).unwrap();
    let stage = root.join("stage");
    let record = root.join("record.json");
    let runner = env!("CARGO_BIN_EXE_benchmark-ab-runner");
    let smoke = || {
        Command::new(runner)
            .args(["smoke"])
            .arg(&plan_path)
            .arg("--stage")
            .arg(&stage)
            .arg("--record")
            .arg(&record)
            .current_dir(&repo)
            .output()
            .unwrap()
    };
    let journal = || -> Vec<JournalRecord> {
        ExecutionLog::validate_prefix(&fs::read(stage.join(LOG_FILE)).unwrap(), "smoke-stage")
            .unwrap()
    };
    let cells_of = |records: &[JournalRecord], event: JournalEvent| -> Vec<String> {
        records
            .iter()
            .filter(|record| record.event == event)
            .map(|record| {
                record.case.as_ref().unwrap()["cell_id"]
                    .as_str()
                    .unwrap()
                    .to_owned()
            })
            .collect()
    };

    let first = smoke();
    assert_eq!(
        first.status.code(),
        Some(3),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let paused = journal();
    assert_eq!(cells_of(&paused, JournalEvent::CellStart), ["first"]);
    assert_eq!(cells_of(&paused, JournalEvent::CellComplete), ["first"]);
    assert_eq!(paused.last().unwrap().event, JournalEvent::Paused);
    assert!(!record.exists(), "a paused staged smoke wrote its record");

    let second = smoke();
    assert_eq!(
        second.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&second.stderr)
    );
    let complete = journal();
    assert_eq!(
        cells_of(&complete, JournalEvent::CellStart),
        ["first", "second"]
    );
    assert_eq!(complete.last().unwrap().event, JournalEvent::Complete);
    assert_eq!(
        complete
            .iter()
            .filter(|record| record.event == JournalEvent::ChildSpawn)
            .count(),
        4
    );

    for entry in &complete {
        assert!(
            !matches!(
                entry.event,
                JournalEvent::ExecutionProgress
                    | JournalEvent::WindowProgress
                    | JournalEvent::LockHold
            ),
            "{entry:?}"
        );
        assert!(entry.details.get("ns_per_call").is_none(), "{entry:?}");
        if let Some(windows) = entry.details.get("windows") {
            assert_eq!(windows, &json!(0), "{entry:?}");
        }
    }
    assert!(!lock.exists(), "the staged smoke created the plan's lock");
    assert!(
        fs::read(repo.join("family-ledger.jsonl"))
            .unwrap()
            .is_empty(),
        "the staged smoke reserved a ledger attempt"
    );
    assert!(!stage.join(RECEIPT_FILE).exists());

    let written: SmokeRecord = serde_json::from_slice(&fs::read(&record).unwrap()).unwrap();
    assert_eq!(
        written
            .cells
            .iter()
            .map(|cell| cell.cell_id.as_str())
            .collect::<Vec<_>>(),
        ["first", "second"]
    );
    assert!(written
        .cells
        .iter()
        .flat_map(|cell| &cell.arms)
        .all(|arm| arm.windows == 0 && arm.role == PairPosition::Validation));

    let refused = Command::new(runner)
        .args(["finalize"])
        .arg(&stage)
        .arg(root.join("out"))
        .current_dir(&repo)
        .output()
        .unwrap();
    assert!(!refused.status.success());
    let reason = String::from_utf8_lossy(&refused.stderr);
    assert!(reason.contains("is a non-timed smoke stage"), "{reason}");
    assert!(!root.join("out").exists());
}

mod arm_smoke {
    use super::{
        addendum, cell, git, scratch, write_addendum, CellObjective, CellRole, CoreArm,
        ADDENDUM_SCHEMA_ID, PROTOCOL_VERSION,
    };
    use serde_json::{json, Value};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Output};
    use tuning_campaign_support::arm::{PairPosition, SmokeRecord, SMOKE_RECORD_SCHEMA};
    use tuning_campaign_support::protocol::{sha256_hex, CacheState};
    use tuning_campaign_support::scratch::Scratch;

    const ADDENDUM: &str = super::ADDENDUM_DOC;
    const CELLS: [&str; 2] = ["first", "second"];

    /// A conforming arm: the crate's own synthetic workload, which serves the
    /// validation position with one untimed dispatch.
    fn conforming(passes: &str) -> Value {
        json!({
            "build": "conservative-portable",
            "description": format!("xor-fold {passes} pass"),
            "executable": env!("CARGO_BIN_EXE_ab-smoke-workload"),
            "arguments": [],
            "environment": {"GF2_SMOKE_PASSES": passes},
            "rustflags": null,
            "tuning_profile": null
        })
    }

    /// An arm whose whole behavior is the shell script `body`, so a test fixes
    /// exactly what the child writes on its standard output.
    fn scripted(description: &str, body: &str) -> Value {
        json!({
            "build": "conservative-portable",
            "description": description,
            "executable": "/bin/sh",
            "arguments": ["-c", body],
            "environment": {},
            "rustflags": null,
            "tuning_profile": null
        })
    }

    /// A plan over two warm cells, the conforming baseline and `candidate`.
    struct Fixture {
        _root: Scratch,
        repo: PathBuf,
        plan: PathBuf,
        record: PathBuf,
        lock: PathBuf,
    }

    fn fixture(name: &str, candidate: Value, cell_ids: [&str; 2]) -> Fixture {
        let root = scratch(name);
        let repo = root.join("repo");
        fs::create_dir_all(repo.join(Path::new(ADDENDUM).parent().unwrap())).unwrap();
        let mut family = addendum(
            CELLS
                .iter()
                .map(|id| {
                    cell(
                        id,
                        CellObjective::Improvement,
                        CellRole::Exploratory,
                        CoreArm::SingleCore,
                    )
                })
                .collect(),
        );
        family.protocol.version = PROTOCOL_VERSION;
        family.schema = ADDENDUM_SCHEMA_ID.into();
        family.family_wise.ledger_path = Some("family-ledger.jsonl".into());
        fs::write(repo.join("family-ledger.jsonl"), b"").unwrap();
        write_addendum(&repo, &family);
        git(&repo, &["init", "-q"]);
        let lock = root.join("absent.lock");
        let plan = json!({
            "schema": "zen3-benchmark-plan-v1",
            "campaign_id": "smoke-contract",
            "producing_manifest": "producing-inputs.json",
            "issue": "f547c394",
            "label": "smoke",
            "campaign_seed": 13,
            "addendum": ADDENDUM,
            "lock_path": lock,
            "wrapper": "flock",
            "timing_override": null,
            "arms": {"baseline": conforming("2"), "candidate": candidate},
            "cells": cell_ids.iter().enumerate().map(|(index, id)| json!({
                "cell_id": id,
                "baseline_arm": "baseline",
                "candidate_arm": "candidate",
                "case": {"words": 4096, "seed": index + 1},
                "pilot_pairs": 6
            })).collect::<Vec<_>>(),
            "max_cells_per_session": null
        });
        let plan_path = root.join("plan.json");
        fs::write(&plan_path, serde_json::to_vec_pretty(&plan).unwrap()).unwrap();
        Fixture {
            repo,
            plan: plan_path,
            record: root.join("record.json"),
            lock,
            _root: root,
        }
    }

    impl Fixture {
        /// Runs the smoke subcommand from the repository root, as a harness does.
        fn smoke(&self, record: Option<&Path>) -> Output {
            let mut command = Command::new(env!("CARGO_BIN_EXE_benchmark-ab-runner"));
            command.arg("smoke").arg(&self.plan);
            if let Some(path) = record {
                command.arg("--record").arg(path);
            }
            command.current_dir(&self.repo).output().unwrap()
        }

        /// Every file in the repository, outside source control's own tree.
        fn tree(&self) -> Vec<String> {
            let mut paths = Vec::new();
            walk(&self.repo, &self.repo, &mut paths);
            paths.sort();
            paths
        }
    }

    fn walk(root: &Path, dir: &Path, paths: &mut Vec<String>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let relative = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .into_owned();
            if relative == ".git" {
                continue;
            }
            if path.is_dir() {
                walk(root, &path, paths);
            } else {
                paths.push(relative);
            }
        }
    }

    fn failure(output: &Output) -> String {
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        String::from_utf8_lossy(&output.stderr).into_owned()
    }

    #[test]
    fn smoke_drives_every_arm_untimed_and_reproduces_its_record() {
        let fixture = fixture("smoke-conforming", conforming("1"), CELLS);
        let before = fixture.tree();
        let first = fixture.smoke(Some(&fixture.record));
        assert_eq!(
            first.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&first.stderr)
        );
        let stdout = String::from_utf8_lossy(&first.stdout);
        assert!(
            stdout.contains("4 validation dispatches over 2 cells, 0 timing windows"),
            "{stdout}"
        );
        let bytes = fs::read(&fixture.record).unwrap();
        let record: SmokeRecord = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(record.schema, SMOKE_RECORD_SCHEMA);
        assert_eq!(record.campaign_id, "smoke-contract");
        assert_eq!(record.issue, "f547c394");
        assert_eq!(record.family, "fixture-family");
        assert_eq!(record.addendum, ADDENDUM);
        assert_eq!(
            record.addendum_sha256,
            sha256_hex(&fs::read(fixture.repo.join(ADDENDUM)).unwrap())
        );
        assert_eq!(
            record.plan_sha256,
            sha256_hex(&fs::read(&fixture.plan).unwrap())
        );
        let workload = sha256_hex(&fs::read(env!("CARGO_BIN_EXE_ab-smoke-workload")).unwrap());
        assert_eq!(
            record
                .cells
                .iter()
                .map(|cell| cell.cell_id.as_str())
                .collect::<Vec<_>>(),
            CELLS
        );
        for cell in &record.cells {
            assert_eq!(cell.cache_state, CacheState::Warm);
            assert_eq!(
                cell.arms
                    .iter()
                    .map(|arm| arm.arm.as_str())
                    .collect::<Vec<_>>(),
                ["baseline", "candidate"]
            );
            for arm in &cell.arms {
                assert_eq!(arm.role, PairPosition::Validation);
                assert_eq!(arm.windows, 0);
                assert_eq!(arm.executable_sha256, workload);
                assert_eq!(arm.cache_state_declared, CacheState::Warm);
                assert_eq!(arm.cache_state_applied, CacheState::Warm);
                assert_eq!(arm.selected_path, "xor-fold-scalar");
            }
        }
        assert_eq!(
            fixture.tree(),
            before,
            "the smoke wrote inside the repository"
        );
        assert!(!fixture.lock.exists(), "the smoke created the plan's lock");
        assert!(fs::read(fixture.repo.join("family-ledger.jsonl"))
            .unwrap()
            .is_empty());

        let again = fixture._root.join("record-again.json");
        assert!(fixture.smoke(Some(&again)).status.success());
        assert_eq!(fs::read(&again).unwrap(), bytes);
    }

    #[test]
    fn smoke_fails_an_arm_that_reports_a_timing_window() {
        const TIMED: &str = r#"cat >/dev/null
printf 'GF2_TUNING_RESULT={"schema":"zen3-benchmark-arm-result-v1","windows":[{"calls":8,"elapsed_ns":9}],"cache_state_applied":"warm","workers_observed":1,"cpus_observed":[0],"selected_path":"scripted","conversion":null,"quality":null}\n'"#;
        let fixture = fixture("smoke-window", scripted("times a window", TIMED), CELLS);
        let reason = failure(&fixture.smoke(Some(&fixture.record)));
        assert!(
            reason.contains("cell first arm candidate")
                && reason.contains("1 timing windows")
                && reason.contains("measures none"),
            "{reason}"
        );
        assert!(!fixture.record.exists());
    }

    #[test]
    fn smoke_fails_an_arm_that_writes_no_result_line() {
        let fixture = fixture(
            "smoke-silent",
            scripted("writes nothing", "cat >/dev/null"),
            CELLS,
        );
        let reason = failure(&fixture.smoke(None));
        assert!(
            reason.contains("cell first arm candidate") && reason.contains("0 lines"),
            "{reason}"
        );
    }

    #[test]
    fn smoke_fails_a_malformed_result_line() {
        const MALFORMED: &str = r#"cat >/dev/null
printf 'GF2_TUNING_RESULT={"schema":"zen3-benchmark-arm-result-v1","windows":[]\n'"#;
        let fixture = fixture(
            "smoke-malformed",
            scripted("truncates its line", MALFORMED),
            CELLS,
        );
        let reason = failure(&fixture.smoke(None));
        assert!(
            reason.contains("cell first arm candidate") && reason.contains("child result"),
            "{reason}"
        );
    }

    #[test]
    fn smoke_fails_a_plan_cell_the_addendum_does_not_declare() {
        let fixture = fixture("smoke-undeclared", conforming("1"), ["first", "undeclared"]);
        let reason = failure(&fixture.smoke(None));
        assert!(
            reason.contains("plan invalid") && reason.contains("\"undeclared\""),
            "{reason}"
        );
    }
}
