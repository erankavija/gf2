//! Algebra-owned producer for the permanent extent campaign.
//!
//! The neutral driver consumes this executable's sealed owner manifest and
//! treats its cases and evidence as opaque bytes. Only a guarded fresh child
//! installs tuning. Reporting, validation, analysis, and owner emission never
//! resolve process-global tuning.
//!
//! The permanent row and sections 3, 6, 8, 8.1, and 10 of
//! `dev/active/a83583e0/premeasurement-protocol.md` are the authority for the
//! grid, fixtures, ordering, framing, accounting, and publication behavior.

use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::hint::black_box;
use std::io::{self, Read};

use gf2_algebra::packed::Bipedal3Matrix;
use gf2_algebra::permanent::parallel_bipedal3::{
    last_effective_chunk, last_effective_partition, permanent_chunk_len,
    reset_last_effective_partition, PermanentPartitionObservation,
};
use gf2_algebra::permanent::{permanent_bipedal3, permanent_bipedal3_parallel};
use gf2_algebra::tuning::{AlgebraTuning, AlgebraTuningCodec, PermanentSelectors};
use gf2_core::compute::field::run_in_dedicated_parallel_pool;
use gf2_core::gfp::Fp;
use gf2_core::rng::Lcg;
use gf2_core::tuning::{
    self, AssemblyProvenance, CompiledProfileProvenance, GitRevision, HarnessSchema,
    MeasurementProvenance, PreparedEnvelope, ProfileId, ProfileRegistry, ProfileRegistryBuilder,
    RepoRelPath, Rfc3339Utc, SectionCodec, SectionResolution, Sha256, TuningSection,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256 as Sha256Hasher};
use tuning_campaign_support::campaign::{
    execution_with_progress, measurement_environment, AcceptedResultsBundle, ArtifactIdentity,
    CandidateBlock, CanonicalJson, ChildOutcome, ChildResult, DeclaredCounts, EmitOwnerRequest,
    LaunchUnit, ManifestRequest, OwnerManifest, OwnerOperation, OwnerResponse, Sha256Digest, Task,
    Token, UnitIdentity, ACCEPTED_RESULTS_SCHEMA, FEATURE_CONTRACT, MANIFEST_SCHEMA, RESULT_SCHEMA,
    THREAD_CONTRACT, TOOLCHAIN_CONTRACT,
};
use tuning_campaign_support::journal::{atomic_write_new, CHECKPOINT_SCHEMA, JOURNAL_SCHEMA};
use tuning_campaign_support::seed::{bank_role, fixture_seed, EXTENT_SEED_ROOT, SEED_DERIVATION};
use tuning_campaign_support::statistics::{
    analyze_extent, execution_median, CandidateSeries, ExtentDecision,
};
use tuning_campaign_support::timing::{EXECUTIONS, FIXTURE_BANKS};
use tuning_campaign_support::transport::{
    read_guarded_case, write_result_line, FRESH_CASE_VALUE, FRESH_CASE_VAR,
};

const OWNER: &str = "gf2-algebra";
const OWNER_PROTOCOL: &str = "algebra-tuning-campaign-v1";
const BEHAVIOR_TOKEN: &str = "algebra-tuning-calibration-v1";
const CASE_SCHEMA: &str = "algebra-tuning-campaign-v1";
const PAYLOAD_SCHEMA: &str = "algebra-tuning-campaign-v1";
const SEED_SCHEMA: &str = "fixture-seeds-v3";
const RAW_SAMPLE_SCHEMA: &str = "raw-timing-samples-v3";
const PHASE: &str = "algebra-extent";
const FIELD: &str = "permanent.gray_chunk_subsets";
const PROCESS: &str = "algebra-producer";
const FIELD_TAG: u64 = 27;
const MATRIX_ROLE: u64 = 0x0c00;
const POOL_THREADS: usize = 4;
const REQUIRED_FEATURES: &str = FEATURE_CONTRACT;
const REQUIRED_TOOLCHAIN: &str = TOOLCHAIN_CONTRACT;
const FORCED_PROFILE_ID: &str = "a835-algebra-forced";
const HARNESS_PATH: &str = "crates/gf2-algebra/benches/tuning_calibration.rs";
const CANDIDATES: [usize; 5] = [4_096, 16_384, 65_536, 262_144, 1_048_576];
const DIMENSIONS: [usize; 3] = [20, 22, 24];

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SeedStream {
    bank: usize,
    role: u64,
    seed: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SeedInventory {
    schema: String,
    root: u64,
    derivation: String,
    field_tag: u64,
    shape_key: u64,
    streams: Vec<SeedStream>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ChildChannels {
    journal_schema: String,
    checkpoint_schema: String,
    raw_sample_schema: String,
    execution_log: std::path::PathBuf,
    checkpoints: std::path::PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct AlgebraCase {
    schema: String,
    identity: UnitIdentity,
    protocol_sha256: Sha256Digest,
    field: String,
    shape_index: usize,
    dimension: usize,
    candidate: usize,
    pool_threads: usize,
    seeds: SeedInventory,
    channels: ChildChannels,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct EffectivePartition {
    maximum_chunk_len: u64,
    chunk_count: u64,
    last_chunk_len: u64,
}

impl From<PermanentPartitionObservation> for EffectivePartition {
    fn from(value: PermanentPartitionObservation) -> Self {
        Self {
            maximum_chunk_len: value.maximum_chunk_len,
            chunk_count: value.chunk_count,
            last_chunk_len: value.last_chunk_len,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ProfileEvidence {
    profile_id: String,
    owner_section_id: String,
    section_schema_version: u32,
    harness_schema: String,
    resolution: String,
    active_gray_chunk_subsets: usize,
    envelope_sha256: String,
    content_sha256: String,
    wrapper_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CapabilityEvidence {
    required_features: String,
    pool_threads: usize,
    parallel_route: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SemanticEvidence {
    fixture_count: usize,
    operands_sha256: String,
    parallel_results_sha256: String,
    serial_results_sha256: String,
    serial_equal: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct AlgebraPayload {
    schema: String,
    field: String,
    shape_index: usize,
    dimension: usize,
    requested_chunk_subsets: usize,
    observed_requested_chunk_subsets: usize,
    effective_partition: EffectivePartition,
    profile: ProfileEvidence,
    capability: CapabilityEvidence,
    semantics: SemanticEvidence,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct AnalysisOutput {
    schema: String,
    field: String,
    selected: usize,
    decision: ExtentDecision<usize>,
    accepted_result_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ReportOutput {
    schema: String,
    mode: String,
    scope: String,
    owner: String,
    owner_protocol: String,
    behavior_token: String,
    seed_schema: String,
    raw_sample_schema: String,
    cells: u64,
    accepted_results: u64,
    windows: u64,
    details: Vec<String>,
}

struct FixtureBank {
    matrix: Bipedal3Matrix,
    input_sha256: String,
    serial: Fp<3>,
}

fn token(value: impl Into<String>) -> Result<Token, String> {
    Token::new(value).map_err(|error| error.to_string())
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256Hasher::digest(bytes))
}

fn canonical<T: Serialize>(value: &T) -> Result<String, String> {
    serde_json::to_string(value).map_err(|error| format!("cannot encode canonical JSON: {error}"))
}

fn canonical_json<T: Serialize>(value: &T) -> Result<CanonicalJson, String> {
    CanonicalJson::from_serializable(value).map_err(|error| error.to_string())
}

fn read_stdin() -> Result<String, String> {
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .map_err(|error| format!("cannot read stdin: {error}"))?;
    Ok(input)
}

fn seed_inventory(shape_index: usize) -> SeedInventory {
    SeedInventory {
        schema: SEED_SCHEMA.to_owned(),
        root: EXTENT_SEED_ROOT,
        derivation: SEED_DERIVATION.to_owned(),
        field_tag: FIELD_TAG,
        shape_key: shape_index as u64,
        streams: (0..FIXTURE_BANKS)
            .map(|bank| SeedStream {
                bank,
                role: bank_role(MATRIX_ROLE, bank),
                seed: fixture_seed(
                    EXTENT_SEED_ROOT,
                    FIELD_TAG,
                    shape_index as u64,
                    bank_role(MATRIX_ROLE, bank),
                ),
            })
            .collect(),
    }
}

fn expected_partition(dimension: usize, candidate: usize) -> Result<EffectivePartition, String> {
    let exponent = u32::try_from(dimension).map_err(|_| "dimension does not fit u32")?;
    let total = 1_u64
        .checked_shl(exponent)
        .and_then(|value| value.checked_sub(1))
        .ok_or("dimension has no representable nonempty subset count")?;
    let requested = u64::try_from(candidate).map_err(|_| "candidate does not fit u64")?;
    if requested == 0 {
        return Err("permanent candidate must be positive".to_owned());
    }
    let chunk_count = 1 + (total - 1) / requested;
    Ok(EffectivePartition {
        maximum_chunk_len: requested.min(total),
        chunk_count,
        last_chunk_len: total - (chunk_count - 1) * requested,
    })
}

fn candidate_order(execution: u64) -> Result<Vec<usize>, String> {
    if execution >= EXECUTIONS {
        return Err(format!("execution {execution} is outside the protocol"));
    }
    let mut ordered = CANDIDATES.to_vec();
    let len = ordered.len();
    ordered.rotate_left(execution as usize % len);
    if execution % 2 == 1 {
        ordered.reverse();
    }
    Ok(ordered)
}

fn cell_identity(
    campaign_id: &Token,
    shape_index: usize,
    candidate: usize,
    task: Task,
) -> Result<UnitIdentity, String> {
    Ok(UnitIdentity {
        protocol: token(OWNER_PROTOCOL)?,
        owner: token(OWNER)?,
        campaign_id: campaign_id.clone(),
        phase: token(PHASE)?,
        field: token(FIELD)?,
        stratum: token(format!("n{}", DIMENSIONS[shape_index]))?,
        candidate: token(format!("q{candidate}"))?,
        task,
    })
}

fn algebra_case(
    request: &ManifestRequest,
    shape_index: usize,
    candidate: usize,
    task: Task,
) -> Result<AlgebraCase, String> {
    Ok(AlgebraCase {
        schema: CASE_SCHEMA.to_owned(),
        identity: cell_identity(&request.campaign_id, shape_index, candidate, task)?,
        protocol_sha256: request.protocol_sha256.clone(),
        field: FIELD.to_owned(),
        shape_index,
        dimension: DIMENSIONS[shape_index],
        candidate,
        pool_threads: POOL_THREADS,
        seeds: seed_inventory(shape_index),
        channels: ChildChannels {
            journal_schema: JOURNAL_SCHEMA.to_owned(),
            checkpoint_schema: CHECKPOINT_SCHEMA.to_owned(),
            raw_sample_schema: RAW_SAMPLE_SCHEMA.to_owned(),
            execution_log: request.channels.execution_log.clone(),
            checkpoints: request.channels.checkpoints.clone(),
        },
    })
}

fn validate_case(case: &AlgebraCase) -> Result<(), String> {
    case.identity
        .task
        .validate()
        .map_err(|error| error.to_string())?;
    if case.schema != CASE_SCHEMA
        || case.identity.protocol.as_str() != OWNER_PROTOCOL
        || case.identity.owner.as_str() != OWNER
        || case.identity.phase.as_str() != PHASE
        || case.identity.field.as_str() != FIELD
        || case.field != FIELD
        || case.shape_index >= DIMENSIONS.len()
        || case.dimension != DIMENSIONS[case.shape_index]
        || !CANDIDATES.contains(&case.candidate)
        || case.identity.stratum.as_str() != format!("n{}", case.dimension)
        || case.identity.candidate.as_str() != format!("q{}", case.candidate)
        || case.pool_threads != POOL_THREADS
        || case.seeds != seed_inventory(case.shape_index)
        || case.channels.journal_schema != JOURNAL_SCHEMA
        || case.channels.checkpoint_schema != CHECKPOINT_SCHEMA
        || case.channels.raw_sample_schema != RAW_SAMPLE_SCHEMA
        || !case.channels.execution_log.is_absolute()
        || !case.channels.checkpoints.is_absolute()
        || case
            .channels
            .execution_log
            .file_name()
            .and_then(|name| name.to_str())
            != Some("execution.log")
        || case
            .channels
            .checkpoints
            .file_name()
            .and_then(|name| name.to_str())
            != Some("checkpoints")
        || case.channels.execution_log.parent() != case.channels.checkpoints.parent()
    {
        return Err("algebra fresh case disagrees with the declared protocol".to_owned());
    }
    Ok(())
}

fn campaign_manifest(request: ManifestRequest) -> Result<OwnerManifest, String> {
    request
        .channels
        .validate()
        .map_err(|error| format!("invalid campaign channels: {error}"))?;
    let process = request
        .processes
        .iter()
        .find(|process| process.id.as_str() == PROCESS)
        .ok_or("manifest request lacks the staged algebra producer")?;
    process
        .verify_staged()
        .map_err(|error| format!("invalid staged algebra process descriptor: {error}"))?;
    if fs::canonicalize(env::current_exe().map_err(|error| error.to_string())?)
        .map_err(|error| error.to_string())?
        != process.executable
    {
        return Err("algebra process descriptor does not name this producer".to_owned());
    }
    if process.arguments != ["--fresh-child"] || process.environment != measurement_environment() {
        return Err("algebra process descriptor violates the fresh-child contract".to_owned());
    }
    let process_id = process.id.clone();
    let mut ordered_units = Vec::with_capacity(90);
    let mut push = |shape_index: usize, candidate: usize, task: Task| -> Result<(), String> {
        let case = algebra_case(&request, shape_index, candidate, task)?;
        let identity = case.identity.clone();
        let ordinal = ordered_units.len() as u64;
        ordered_units.push(
            LaunchUnit::new(
                ordinal,
                identity,
                process_id.clone(),
                canonical_json(&case)?,
            )
            .map_err(|error| error.to_string())?,
        );
        Ok(())
    };
    for shape_index in 0..DIMENSIONS.len() {
        for candidate in CANDIDATES {
            push(shape_index, candidate, Task::Probe)?;
        }
        for execution in 0..EXECUTIONS {
            for candidate in candidate_order(execution)? {
                push(shape_index, candidate, Task::Measure { execution })?;
            }
        }
    }
    let candidate_blocks = DIMENSIONS
        .iter()
        .map(|dimension| {
            Ok(CandidateBlock {
                phase: token(PHASE)?,
                field: token(FIELD)?,
                stratum: token(format!("n{dimension}"))?,
                base_candidates: CANDIDATES
                    .iter()
                    .map(|candidate| token(format!("q{candidate}")))
                    .collect::<Result<_, _>>()?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let mut manifest = OwnerManifest {
        schema: MANIFEST_SCHEMA.to_owned(),
        owner: token(OWNER)?,
        owner_protocol: token(OWNER_PROTOCOL)?,
        behavior_token: token(BEHAVIOR_TOKEN)?,
        campaign_id: request.campaign_id,
        phases: vec![token(PHASE)?],
        candidate_blocks,
        counts: DeclaredCounts::for_cells(15).map_err(|error| error.to_string())?,
        processes: request.processes,
        ordered_units,
        manifest_sha256: Sha256Digest::of(&[]),
    };
    manifest.seal().map_err(|error| error.to_string())?;
    validate_manifest_order(&manifest)?;
    Ok(manifest)
}

fn validate_manifest_order(manifest: &OwnerManifest) -> Result<(), String> {
    manifest.validate().map_err(|error| error.to_string())?;
    if manifest.counts.cells != 15
        || manifest.counts.probes != 15
        || manifest.counts.timed_children != 75
        || manifest.counts.accepted_results != 90
        || manifest.counts.windows != 375
        || manifest.counts.progress_records != 450
    {
        return Err("algebra manifest accounting is not the declared 15/90/375 grid".to_owned());
    }
    let mut index = 0;
    for shape_index in 0..DIMENSIONS.len() {
        for candidate in CANDIDATES {
            validate_manifest_unit(
                &manifest.ordered_units[index],
                shape_index,
                candidate,
                Task::Probe,
            )?;
            index += 1;
        }
        for execution in 0..EXECUTIONS {
            for candidate in candidate_order(execution)? {
                validate_manifest_unit(
                    &manifest.ordered_units[index],
                    shape_index,
                    candidate,
                    Task::Measure { execution },
                )?;
                index += 1;
            }
        }
    }
    if index != manifest.ordered_units.len() {
        return Err("algebra manifest contains trailing units".to_owned());
    }
    Ok(())
}

fn validate_manifest_unit(
    unit: &LaunchUnit,
    shape_index: usize,
    candidate: usize,
    task: Task,
) -> Result<(), String> {
    if unit.process.as_str() != PROCESS {
        return Err("algebra manifest unit names the wrong process".to_owned());
    }
    let case: AlgebraCase = unit.case.decode().map_err(|error| error.to_string())?;
    validate_case(&case)?;
    if case.identity != unit.identity
        || case.shape_index != shape_index
        || case.candidate != candidate
        || case.identity.task != task
    {
        return Err("algebra manifest unit is out of declared acquisition order".to_owned());
    }
    Ok(())
}

fn algebra_registry() -> Result<ProfileRegistry, String> {
    ProfileRegistryBuilder::new()
        .register::<AlgebraTuning, AlgebraTuningCodec>()
        .and_then(ProfileRegistryBuilder::build)
        .map_err(|error| format!("cannot build algebra owner registry: {error}"))
}

fn fixed_assembly() -> Result<AssemblyProvenance, String> {
    Ok(AssemblyProvenance {
        assembled_at: Rfc3339Utc::parse("1970-01-01T00:00:00Z")
            .map_err(|error| error.to_string())?,
        source_revision: GitRevision::parse("0000000000000000000000000000000000000000")
            .map_err(|error| error.to_string())?,
        source_dirty: false,
        tool: RepoRelPath::parse(HARNESS_PATH).map_err(|error| error.to_string())?,
        tool_sha256: Sha256::parse(
            "0000000000000000000000000000000000000000000000000000000000000000",
        )
        .map_err(|error| error.to_string())?,
    })
}

fn forced_profile(candidate: usize) -> Result<(PreparedEnvelope, ProfileEvidence), String> {
    let id = ProfileId::parse(FORCED_PROFILE_ID).map_err(|error| error.to_string())?;
    let section = AlgebraTuning::from_selectors(
        PermanentSelectors::try_new(candidate).map_err(|error| error.to_string())?,
    );
    let compiled = PreparedEnvelope::compiled(
        id.clone(),
        CompiledProfileProvenance {
            artifact_id: id.clone(),
        },
    )
    .insert(section)
    .map_err(|error| error.to_string())?
    .build()
    .map_err(|error| error.to_string())?;
    let registry = algebra_registry()?;
    let document = registry
        .to_json(&compiled, &fixed_assembly()?)
        .map_err(|error| format!("cannot encode forced algebra owner: {error}"))?;
    let reopened = registry
        .from_json(&document)
        .map_err(|error| format!("cannot strictly reopen forced algebra owner: {error}"))?;
    if registry
        .to_json(&reopened, &fixed_assembly()?)
        .map_err(|error| format!("cannot re-encode forced algebra owner: {error}"))?
        != document
    {
        return Err("forced algebra owner is not canonically stable".to_owned());
    }
    let ids: Vec<_> = reopened.section_ids().collect();
    if ids != [AlgebraTuning::ID.as_str()] {
        return Err(format!("forced algebra owner has section IDs {ids:?}"));
    }
    let content_sha256 = reopened
        .verified_assembly()
        .ok_or("forced algebra owner lacks verified assembly")?
        .content_sha256
        .as_str()
        .to_owned();
    let value: serde_json::Value =
        serde_json::from_str(&document).map_err(|error| error.to_string())?;
    let wrapper = canonical(
        value
            .get("sections")
            .and_then(|sections| sections.get(AlgebraTuning::ID.as_str()))
            .ok_or("forced algebra owner lacks its exact section wrapper")?,
    )?;
    let evidence = ProfileEvidence {
        profile_id: FORCED_PROFILE_ID.to_owned(),
        owner_section_id: AlgebraTuning::ID.as_str().to_owned(),
        section_schema_version: AlgebraTuningCodec::SCHEMA_VERSION,
        harness_schema: AlgebraTuningCodec::HARNESS_SCHEMA.to_owned(),
        resolution: "installed".to_owned(),
        active_gray_chunk_subsets: candidate,
        envelope_sha256: sha256(document.as_bytes()),
        content_sha256,
        wrapper_sha256: sha256(wrapper.as_bytes()),
    };
    Ok((reopened, evidence))
}

fn fixture_input_digest(dimension: usize, bank: usize, values: &[Fp<3>]) -> String {
    let mut hash = Sha256Hasher::new();
    hash.update(b"gf2-a83583e0-permanent-fixture-v1\0");
    hash.update((dimension as u64).to_le_bytes());
    hash.update((bank as u64).to_le_bytes());
    hash.update(MATRIX_ROLE.to_le_bytes());
    hash.update((values.len() as u64).to_le_bytes());
    for value in values {
        hash.update([value.value() as u8]);
    }
    format!("{:x}", hash.finalize())
}

fn result_digest(dimension: usize, values: &[Fp<3>]) -> String {
    let mut hash = Sha256Hasher::new();
    hash.update(b"gf2-a83583e0-permanent-output-v1");
    hash.update([0]);
    hash.update((dimension as u64).to_le_bytes());
    hash.update((values.len() as u64).to_le_bytes());
    for value in values {
        hash.update(value.value().to_le_bytes());
    }
    format!("{:x}", hash.finalize())
}

fn tuple_digest(label: &[u8], values: &[String]) -> String {
    let mut hash = Sha256Hasher::new();
    hash.update(label);
    hash.update([0]);
    for value in values {
        hash.update((value.len() as u64).to_le_bytes());
        hash.update(value.as_bytes());
    }
    format!("{:x}", hash.finalize())
}

fn fixture_banks(shape_index: usize, dimension: usize) -> Vec<FixtureBank> {
    (0..FIXTURE_BANKS)
        .map(|bank| {
            let mut rng = Lcg::new(fixture_seed(
                EXTENT_SEED_ROOT,
                FIELD_TAG,
                shape_index as u64,
                bank_role(MATRIX_ROLE, bank),
            ));
            let values: Vec<_> = (0..dimension * dimension)
                .map(|_| Fp::<3>::new(rng.next_u64() % 3))
                .collect();
            let input_sha256 = fixture_input_digest(dimension, bank, &values);
            let matrix = Bipedal3Matrix::from_row_major(&values, dimension, dimension);
            let serial = permanent_bipedal3(&matrix);
            FixtureBank {
                matrix,
                input_sha256,
                serial,
            }
        })
        .collect()
}

fn check_child_environment() -> Result<(), String> {
    for (name, expected) in [
        ("GF2_BENCH", "1"),
        ("RAYON_NUM_THREADS", "4"),
        ("RUSTUP_TOOLCHAIN", REQUIRED_TOOLCHAIN),
    ] {
        if env::var(name).as_deref() != Ok(expected) {
            return Err(format!("fresh child requires {name}={expected}"));
        }
    }
    Ok(())
}

fn require_campaign_environment() -> Result<(), String> {
    check_child_environment()
}

fn execute_case(
    case: AlgebraCase,
    case_json: CanonicalJson,
    enforce_production_grid: bool,
) -> Result<ChildResult, String> {
    if enforce_production_grid {
        validate_case(&case)?;
        check_child_environment()?;
    }
    let (prepared, profile_evidence) = forced_profile(case.candidate)?;
    tuning::install(prepared)
        .map_err(|error| format!("cannot install forced algebra owner: {error}"))?;
    let active = gf2_algebra::tuning::active();
    match active.resolution {
        SectionResolution::Installed {
            profile_id,
            section_id,
            measurement: MeasurementProvenance::Inherited,
        } if profile_id.as_str() == FORCED_PROFILE_ID
            && section_id == AlgebraTuning::ID
            && active.permanent().gray_chunk_subsets() == case.candidate => {}
        _ => return Err("forced algebra owner did not resolve as exact Installed".to_owned()),
    }
    if permanent_chunk_len() != case.candidate {
        return Err("public permanent selector did not expose the installed candidate".to_owned());
    }
    let fixtures = fixture_banks(case.shape_index, case.dimension);
    let expected = expected_partition(case.dimension, case.candidate)?;
    let case_sha256 = case_json.digest();
    let (parallel, pool_threads, samples, observed_requested, observed_partition) =
        run_in_dedicated_parallel_pool(POOL_THREADS, || {
            let mut progress = io::stderr();
            let pool_threads = rayon::current_num_threads();
            let mut parallel = Vec::with_capacity(FIXTURE_BANKS);
            let mut probe_partition = None;
            for fixture in &fixtures {
                reset_last_effective_partition();
                let result = permanent_bipedal3_parallel(&fixture.matrix);
                if last_effective_chunk() != case.candidate {
                    return Err("public permanent failed requested-chunk forwarding".to_owned());
                }
                let observed = last_effective_partition()
                    .map(EffectivePartition::from)
                    .ok_or("public permanent published no effective partition")?;
                if observed != expected {
                    return Err("public permanent effective partition is incorrect".to_owned());
                }
                match probe_partition {
                    Some(first) if first != observed => {
                        return Err(
                            "public permanent probes disagree on effective partition".to_owned()
                        );
                    }
                    None => probe_partition = Some(observed),
                    Some(_) => {}
                }
                if result != fixture.serial {
                    return Err(
                        "parallel permanent disagrees with public serial permanent".to_owned()
                    );
                }
                parallel.push(result);
            }
            let samples = match case.identity.task {
                Task::Probe => Vec::new(),
                Task::Measure { .. } => {
                    let mut body = |bank: usize| {
                        black_box(permanent_bipedal3_parallel(&fixtures[bank].matrix));
                    };
                    execution_with_progress(&case.identity, &case_sha256, &mut body, &mut progress)
                        .map_err(|error| format!("timing/progress failed: {error}"))?
                }
            };
            let observed_requested = last_effective_chunk();
            let observed_partition = last_effective_partition()
                .map(EffectivePartition::from)
                .ok_or("public permanent published no final effective partition")?;
            if observed_requested != case.candidate {
                return Err(
                    "public permanent final call lost requested-chunk forwarding".to_owned(),
                );
            }
            if Some(observed_partition) != probe_partition {
                return Err("public permanent final call changed effective partition".to_owned());
            }
            Ok::<_, String>((
                parallel,
                pool_threads,
                samples,
                observed_requested,
                observed_partition,
            ))
        })?;
    if pool_threads != POOL_THREADS {
        return Err(format!(
            "dedicated pool has {pool_threads} threads rather than {POOL_THREADS}"
        ));
    }
    let serial: Vec<_> = fixtures.iter().map(|fixture| fixture.serial).collect();
    let input_digests: Vec<_> = fixtures
        .iter()
        .map(|fixture| fixture.input_sha256.clone())
        .collect();
    let payload = AlgebraPayload {
        schema: PAYLOAD_SCHEMA.to_owned(),
        field: FIELD.to_owned(),
        shape_index: case.shape_index,
        dimension: case.dimension,
        requested_chunk_subsets: case.candidate,
        observed_requested_chunk_subsets: observed_requested,
        effective_partition: observed_partition,
        profile: profile_evidence,
        capability: CapabilityEvidence {
            required_features: REQUIRED_FEATURES.to_owned(),
            pool_threads,
            parallel_route: "public-permanent-bipedal3-parallel".to_owned(),
        },
        semantics: SemanticEvidence {
            fixture_count: fixtures.len(),
            operands_sha256: tuple_digest(b"gf2-a83583e0-operands-v1", &input_digests),
            parallel_results_sha256: result_digest(case.dimension, &parallel),
            serial_results_sha256: result_digest(case.dimension, &serial),
            serial_equal: parallel == serial,
        },
    };
    Ok(ChildResult {
        schema: RESULT_SCHEMA.to_owned(),
        identity: case.identity,
        case_sha256,
        outcome: ChildOutcome::Complete,
        samples,
        payload: canonical_json(&payload)?,
    })
}

fn run_fresh_child() -> Result<(), String> {
    let case: AlgebraCase =
        read_guarded_case(env::var(FRESH_CASE_VAR).ok().as_deref(), io::stdin().lock())?;
    let case_json = canonical_json(&case)?;
    let result = execute_case(case, case_json, true)?;
    write_result_line(io::stdout().lock(), &result)
        .map_err(|error| format!("cannot write child result: {error}"))
}

fn expected_operands_digest(shape_index: usize, dimension: usize) -> String {
    let digests: Vec<_> = (0..FIXTURE_BANKS)
        .map(|bank| {
            let mut rng = Lcg::new(fixture_seed(
                EXTENT_SEED_ROOT,
                FIELD_TAG,
                shape_index as u64,
                bank_role(MATRIX_ROLE, bank),
            ));
            let values: Vec<_> = (0..dimension * dimension)
                .map(|_| Fp::<3>::new(rng.next_u64() % 3))
                .collect();
            fixture_input_digest(dimension, bank, &values)
        })
        .collect();
    tuple_digest(b"gf2-a83583e0-operands-v1", &digests)
}

fn validate_result(unit: &LaunchUnit, result: &ChildResult) -> Result<(), String> {
    unit.validate().map_err(|error| error.to_string())?;
    let case: AlgebraCase = unit.case.decode().map_err(|error| error.to_string())?;
    validate_case(&case)?;
    if result.schema != RESULT_SCHEMA
        || case.identity != unit.identity
        || result.identity != unit.identity
        || result.case_sha256 != unit.case.digest()
        || result.outcome != ChildOutcome::Complete
    {
        return Err("algebra result neutral identity does not match its unit".to_owned());
    }
    match case.identity.task {
        Task::Probe if !result.samples.is_empty() => {
            return Err("algebra probe contains timing samples".to_owned());
        }
        Task::Measure { execution } => {
            if result.samples.len() != 5 {
                return Err("algebra timed result does not contain five windows".to_owned());
            }
            let calibrated_calls = result.samples[0].calls;
            if calibrated_calls == 0
                || result
                    .samples
                    .iter()
                    .enumerate()
                    .any(|(repetition, sample)| {
                        sample.execution != execution
                            || sample.repetition != repetition as u64
                            || sample.calls != calibrated_calls
                    })
            {
                return Err(
                    "algebra timing samples violate execution, repetition, or calibration order"
                        .to_owned(),
                );
            }
            execution_median(&result.samples, execution)
                .map_err(|error| format!("invalid algebra timing samples: {error}"))?;
        }
        Task::Probe => {}
    }
    let payload: AlgebraPayload = result.payload.decode().map_err(|error| error.to_string())?;
    let (_, expected_profile) = forced_profile(case.candidate)?;
    if payload.schema != PAYLOAD_SCHEMA
        || payload.field != FIELD
        || payload.shape_index != case.shape_index
        || payload.dimension != case.dimension
        || payload.requested_chunk_subsets != case.candidate
        || payload.observed_requested_chunk_subsets != case.candidate
        || payload.effective_partition != expected_partition(case.dimension, case.candidate)?
        || payload.profile != expected_profile
        || payload.capability.required_features != REQUIRED_FEATURES
        || payload.capability.pool_threads != POOL_THREADS
        || payload.capability.parallel_route != "public-permanent-bipedal3-parallel"
        || payload.semantics.fixture_count != FIXTURE_BANKS
        || payload.semantics.operands_sha256
            != expected_operands_digest(case.shape_index, case.dimension)
        || !payload.semantics.serial_equal
        || payload.semantics.parallel_results_sha256 != payload.semantics.serial_results_sha256
        || Sha256Digest::new(payload.semantics.parallel_results_sha256.clone()).is_err()
        || Sha256Digest::new(payload.semantics.serial_results_sha256.clone()).is_err()
    {
        return Err("algebra result fails owner semantic validation".to_owned());
    }
    Ok(())
}

fn result_sha256(result: &ChildResult) -> Result<Sha256Digest, String> {
    result.digest().map_err(|error| error.to_string())
}

fn analyze_results(bundle: &AcceptedResultsBundle) -> Result<AnalysisOutput, String> {
    if bundle.schema != ACCEPTED_RESULTS_SCHEMA || bundle.accepted.len() != 90 {
        return Err(
            "algebra owner input does not contain the exact 90 accepted results".to_owned(),
        );
    }
    let campaign_id = bundle.accepted[0].unit.identity.campaign_id.clone();
    let first_case: AlgebraCase = bundle.accepted[0]
        .unit
        .case
        .decode()
        .map_err(|error| error.to_string())?;
    let mut seen = BTreeSet::new();
    for (ordinal, accepted) in bundle.accepted.iter().enumerate() {
        let case: AlgebraCase = accepted
            .unit
            .case
            .decode()
            .map_err(|error| error.to_string())?;
        if accepted.unit.ordinal != ordinal as u64
            || !seen.insert(accepted.unit.key.clone())
            || accepted.checkpoint_sha256.as_str().len() != 64
            || accepted.unit.identity.campaign_id != campaign_id
            || case.protocol_sha256 != first_case.protocol_sha256
            || case.channels != first_case.channels
        {
            return Err("algebra accepted-result order or identity is invalid".to_owned());
        }
        validate_result(&accepted.unit, &accepted.result)?;
    }

    let mut cursor = 0;
    for shape_index in 0..DIMENSIONS.len() {
        for candidate in CANDIDATES {
            let unit = &bundle.accepted[cursor].unit;
            validate_manifest_unit(unit, shape_index, candidate, Task::Probe)?;
            cursor += 1;
        }
        for execution in 0..EXECUTIONS {
            for candidate in candidate_order(execution)? {
                let unit = &bundle.accepted[cursor].unit;
                validate_manifest_unit(unit, shape_index, candidate, Task::Measure { execution })?;
                cursor += 1;
            }
        }
    }

    let mut candidates = Vec::with_capacity(CANDIDATES.len());
    for candidate in CANDIDATES {
        let mut schedules = Vec::with_capacity(DIMENSIONS.len());
        let mut strata = Vec::with_capacity(DIMENSIONS.len());
        for (shape_index, dimension) in DIMENSIONS.into_iter().enumerate() {
            schedules.push(expected_partition(dimension, candidate)?);
            let mut executions = Vec::with_capacity(EXECUTIONS as usize);
            for execution in 0..EXECUTIONS {
                let accepted = bundle
                    .accepted
                    .iter()
                    .find(|accepted| {
                        let Ok(case) = accepted.unit.case.decode::<AlgebraCase>() else {
                            return false;
                        };
                        case.shape_index == shape_index
                            && case.candidate == candidate
                            && case.identity.task == Task::Measure { execution }
                    })
                    .ok_or("algebra owner input lacks a timed cell")?;
                executions.push(
                    execution_median(&accepted.result.samples, execution)
                        .map_err(|error| error.to_string())?,
                );
            }
            strata.push(executions);
        }
        candidates.push(CandidateSeries {
            candidate,
            schedules,
            strata,
        });
    }
    let decision = analyze_extent(
        &candidates,
        &AlgebraTuning::CONSERVATIVE.permanent().gray_chunk_subsets(),
    )
    .map_err(|error| format!("algebra extent analysis failed: {error}"))?;
    Ok(AnalysisOutput {
        schema: "algebra-tuning-analysis-v1".to_owned(),
        field: FIELD.to_owned(),
        selected: decision.selected,
        decision,
        accepted_result_count: bundle.accepted.len(),
    })
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct MeasurementRuntime {
    source_dirty: bool,
    toolchain: String,
    cpu_model: String,
    cpu_features: Vec<String>,
    os_kernel: String,
    governor: String,
    receipt: String,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct AssemblyRuntime {
    source_dirty: bool,
    tool: String,
    tool_sha256: String,
}

fn measurement_provenance(request: &EmitOwnerRequest) -> Result<MeasurementProvenance, String> {
    let runtime: MeasurementRuntime = request
        .measurement
        .runtime
        .decode()
        .map_err(|error| format!("invalid algebra measurement runtime: {error}"))?;
    let binary_sha256 = request
        .measurement
        .identity
        .executable_sha256
        .get(request.measurement.process.as_str())
        .ok_or("measurement process digest is absent")?;
    Ok(MeasurementProvenance::Calibrated {
        measured_at: Rfc3339Utc::parse(&request.measurement.observed_utc)
            .map_err(|error| error.to_string())?,
        source_revision: GitRevision::parse(&request.measurement.identity.source_revision)
            .map_err(|error| error.to_string())?,
        source_dirty: runtime.source_dirty,
        harness: RepoRelPath::parse(HARNESS_PATH).map_err(|error| error.to_string())?,
        harness_schema: HarnessSchema::parse(BEHAVIOR_TOKEN).map_err(|error| error.to_string())?,
        binary_sha256: Sha256::parse(binary_sha256).map_err(|error| error.to_string())?,
        toolchain: runtime.toolchain,
        host: request.measurement.identity.host_identity.clone(),
        cpu_model: runtime.cpu_model,
        cpu_features: runtime.cpu_features,
        os_kernel: runtime.os_kernel,
        governor: runtime.governor,
        receipt: RepoRelPath::parse(&runtime.receipt).map_err(|error| error.to_string())?,
    })
}

fn validate_observed_provenance(
    request: &EmitOwnerRequest,
    bundle: &AcceptedResultsBundle,
) -> Result<(), String> {
    let measurement = &request.measurement;
    let assembly = &request.assembly;
    let facts: MeasurementRuntime = measurement
        .runtime
        .decode()
        .map_err(|error| format!("invalid algebra measurement runtime: {error}"))?;
    let assembly_facts: AssemblyRuntime = assembly
        .runtime
        .decode()
        .map_err(|error| format!("invalid algebra assembly runtime: {error}"))?;
    let case: AlgebraCase = bundle.accepted[0]
        .unit
        .case
        .decode()
        .map_err(|error| error.to_string())?;
    if measurement.identity != assembly.identity
        || measurement.process.as_str() != PROCESS
        || assembly.process.as_str() != PROCESS
        || measurement.identity.feature_contract != FEATURE_CONTRACT
        || measurement.identity.thread_contract != THREAD_CONTRACT
        || measurement.identity.protocol_digest != case.protocol_sha256.as_str()
        || !measurement
            .identity
            .behavior_sha256
            .contains_key(HARNESS_PATH)
        || facts.source_dirty
        || facts.toolchain.trim().is_empty()
        || !facts.toolchain.contains(REQUIRED_TOOLCHAIN)
        || facts.cpu_model.trim().is_empty()
        || facts.cpu_features.is_empty()
        || facts.cpu_features.iter().any(|feature| {
            feature.is_empty()
                || !feature
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        })
        || facts.cpu_features.windows(2).any(|pair| pair[0] >= pair[1])
        || facts.os_kernel.trim().is_empty()
        || facts.governor.trim().is_empty()
        || measurement.identity.host_identity.trim().is_empty()
        || assembly_facts.source_dirty
        || assembly_facts.tool != HARNESS_PATH
    {
        return Err("algebra observed provenance violates the producing contract".to_owned());
    }
    RepoRelPath::parse(&facts.receipt).map_err(|error| error.to_string())?;
    Rfc3339Utc::parse(&measurement.observed_utc).map_err(|error| error.to_string())?;
    Rfc3339Utc::parse(&assembly.observed_utc).map_err(|error| error.to_string())?;
    let binary = measurement
        .identity
        .executable_sha256
        .get(PROCESS)
        .ok_or("algebra producing executable digest is absent")?;
    let actual = sha256(
        &fs::read(env::current_exe().map_err(|error| error.to_string())?)
            .map_err(|error| format!("cannot hash algebra producer executable: {error}"))?,
    );
    if actual != *binary || assembly_facts.tool_sha256 != actual {
        return Err("algebra owner emission executable differs from producing binary".to_owned());
    }
    Ok(())
}

fn assembly_provenance(request: &EmitOwnerRequest) -> Result<AssemblyProvenance, String> {
    let runtime: AssemblyRuntime = request
        .assembly
        .runtime
        .decode()
        .map_err(|error| format!("invalid algebra assembly runtime: {error}"))?;
    Ok(AssemblyProvenance {
        assembled_at: Rfc3339Utc::parse(&request.assembly.observed_utc)
            .map_err(|error| error.to_string())?,
        source_revision: GitRevision::parse(&request.assembly.identity.source_revision)
            .map_err(|error| error.to_string())?,
        source_dirty: runtime.source_dirty,
        tool: RepoRelPath::parse(&runtime.tool).map_err(|error| error.to_string())?,
        tool_sha256: Sha256::parse(&runtime.tool_sha256).map_err(|error| error.to_string())?,
    })
}

fn measured_owner_document(
    profile_id: &str,
    selected: usize,
    measurement: MeasurementProvenance,
    assembly: AssemblyProvenance,
) -> Result<String, String> {
    let id = ProfileId::parse(profile_id).map_err(|error| error.to_string())?;
    let section = AlgebraTuning::from_selectors(
        PermanentSelectors::try_new(selected).map_err(|error| error.to_string())?,
    );
    let prepared = PreparedEnvelope::compiled(
        id.clone(),
        CompiledProfileProvenance {
            artifact_id: id.clone(),
        },
    )
    .insert_measured::<AlgebraTuning, AlgebraTuningCodec>(section, measurement)
    .map_err(|error| format!("cannot construct measured algebra owner: {error}"))?
    .build()
    .map_err(|error| format!("cannot build measured algebra owner: {error}"))?;
    algebra_registry()?
        .to_json(&prepared, &assembly)
        .map_err(|error| format!("cannot encode measured algebra owner: {error}"))
}

fn emit_owner(request: EmitOwnerRequest) -> Result<(ArtifactIdentity, AnalysisOutput), String> {
    if request.output.exists() {
        return Err("algebra owner output destination already exists".to_owned());
    }
    if !request.output.is_absolute() {
        return Err("algebra owner output destination is not absolute".to_owned());
    }
    let bundle: AcceptedResultsBundle = request
        .accepted_results
        .read()
        .map_err(|error| format!("cannot reopen accepted algebra results: {error}"))?;
    if bundle.manifest_sha256 != request.manifest_sha256 {
        return Err("algebra owner input has the wrong manifest digest".to_owned());
    }
    if bundle
        .accepted
        .iter()
        .any(|accepted| accepted.unit.identity.campaign_id != request.campaign_id)
    {
        return Err("algebra owner input has the wrong campaign identity".to_owned());
    }
    let analysis = analyze_results(&bundle)?;
    validate_observed_provenance(&request, &bundle)?;
    let registry = algebra_registry()?;
    let measurement = measurement_provenance(&request)?;
    let assembly = assembly_provenance(&request)?;
    let document = measured_owner_document(
        request.campaign_id.as_str(),
        analysis.selected,
        measurement.clone(),
        assembly.clone(),
    )?;
    atomic_write_new(&request.output, document.as_bytes())
        .map_err(|error| format!("cannot atomically publish algebra owner: {error}"))?;
    let canonical_path = fs::canonicalize(&request.output)
        .map_err(|error| format!("cannot canonicalize algebra owner: {error}"))?;
    let reopened_bytes = fs::read(&canonical_path)
        .map_err(|error| format!("cannot reopen algebra owner bytes: {error}"))?;
    if reopened_bytes != document.as_bytes() {
        return Err("reopened algebra owner bytes differ from emitted bytes".to_owned());
    }
    let reopened = registry
        .from_json(&document)
        .map_err(|error| format!("emitted algebra owner fails strict reopen: {error}"))?;
    let projection = reopened
        .section::<AlgebraTuning>()
        .map_err(|error| error.to_string())?
        .ok_or("emitted algebra owner lacks its section")?;
    if reopened.section_ids().collect::<Vec<_>>() != [AlgebraTuning::ID.as_str()]
        || reopened.profile_id().as_str() != request.campaign_id.as_str()
        || projection.section.permanent().gray_chunk_subsets() != analysis.selected
        || projection.measurement != &measurement
        || reopened
            .verified_assembly()
            .ok_or("emitted algebra owner lacks verified assembly")?
            .provenance
            != assembly
    {
        return Err("emitted algebra owner has incorrect owner identity or selection".to_owned());
    }
    Ok((
        ArtifactIdentity {
            path: canonical_path,
            sha256: Sha256Digest::of(&reopened_bytes),
        },
        analysis,
    ))
}

fn report(mode: &str, scope: &str, details: Vec<String>) -> Result<ReportOutput, String> {
    let counts = DeclaredCounts::for_cells(15).map_err(|error| error.to_string())?;
    Ok(ReportOutput {
        schema: "algebra-tuning-owner-report-v1".to_owned(),
        mode: mode.to_owned(),
        scope: scope.to_owned(),
        owner: OWNER.to_owned(),
        owner_protocol: OWNER_PROTOCOL.to_owned(),
        behavior_token: BEHAVIOR_TOKEN.to_owned(),
        seed_schema: SEED_SCHEMA.to_owned(),
        raw_sample_schema: RAW_SAMPLE_SCHEMA.to_owned(),
        cells: counts.cells,
        accepted_results: counts.accepted_results,
        windows: counts.windows,
        details,
    })
}

fn codec_inventory() -> Result<Vec<String>, String> {
    let body = AlgebraTuningCodec::encode_body(&AlgebraTuning::CONSERVATIVE)
        .map_err(|error| format!("cannot encode algebra inventory: {error}"))?;
    let value = serde_json::to_value(body)
        .map_err(|error| format!("cannot project algebra inventory: {error}"))?;
    let mut fields = Vec::new();
    let families = value
        .as_object()
        .ok_or("algebra selector inventory is not an object")?;
    for (family, members) in families {
        let members = members
            .as_object()
            .ok_or("algebra selector family is not an object")?;
        for field in members.keys() {
            fields.push(format!("{family}.{field}"));
        }
    }
    Ok(fields)
}

fn self_check() -> Result<ReportOutput, String> {
    let _ = forced_profile(CANDIDATES[0])?;
    let inventory = codec_inventory()?;
    if inventory != [FIELD] {
        return Err(format!("algebra codec inventory is {inventory:?}"));
    }
    for dimension in DIMENSIONS {
        for candidate in CANDIDATES {
            expected_partition(dimension, candidate)?;
        }
    }
    report(
        "self-check",
        "complete-declaration-no-measurement",
        vec![
            "codec-inventory=1/1".to_owned(),
            "profile-installation=not-performed".to_owned(),
        ],
    )
}

fn list_grid() -> Result<ReportOutput, String> {
    report(
        "list-grid",
        "full-grid-no-measurement",
        vec![
            format!("dimensions={DIMENSIONS:?}"),
            format!("candidates={CANDIDATES:?}"),
            format!("execution-0={:?}", candidate_order(0)?),
            format!("execution-1={:?}", candidate_order(1)?),
            format!("execution-2={:?}", candidate_order(2)?),
            format!("execution-3={:?}", candidate_order(3)?),
            format!("execution-4={:?}", candidate_order(4)?),
            "profile-installation=not-performed".to_owned(),
        ],
    )
}

fn capability_report() -> Result<ReportOutput, String> {
    let width = run_in_dedicated_parallel_pool(POOL_THREADS, rayon::current_num_threads);
    if width != POOL_THREADS {
        return Err(format!("dedicated pool exposed width {width}"));
    }
    report(
        "capability-report",
        "representative-capabilities-no-grid-execution",
        vec![
            format!("dedicated-pool-threads={width}"),
            format!("required-features={REQUIRED_FEATURES}"),
            "full-grid-checked=false".to_owned(),
            "profile-installation=not-performed".to_owned(),
        ],
    )
}

fn owner_operation(operation: OwnerOperation) -> Result<OwnerResponse, String> {
    match operation {
        OwnerOperation::SelfCheck => Ok(OwnerResponse::SelfCheck {
            evidence: canonical_json(&self_check()?)?,
        }),
        OwnerOperation::ListGrid => Ok(OwnerResponse::ListGrid {
            evidence: canonical_json(&list_grid()?)?,
        }),
        OwnerOperation::CapabilityReport => Ok(OwnerResponse::CapabilityReport {
            evidence: canonical_json(&capability_report()?)?,
        }),
        OwnerOperation::CampaignManifest { request } => Ok(OwnerResponse::CampaignManifest {
            manifest: Box::new(campaign_manifest(request)?),
        }),
        OwnerOperation::ValidateResult { unit, result } => {
            validate_result(&unit, &result)?;
            Ok(OwnerResponse::ValidateResult {
                unit_key: unit.key.clone(),
                result_sha256: result_sha256(&result)?,
            })
        }
        OwnerOperation::DeriveManifest { .. } => {
            Err("the algebra owner has no conditionally derived manifest".to_owned())
        }
        OwnerOperation::EmitOwner { request } => {
            require_campaign_environment()?;
            let (artifact, analysis) = emit_owner(*request)?;
            Ok(OwnerResponse::EmitOwner {
                artifact,
                decisions: canonical_json(&analysis)?,
            })
        }
    }
}

fn decode_owner_operation(input: &str) -> Result<OwnerOperation, String> {
    CanonicalJson::new(input)
        .map_err(|error| format!("owner operation is not canonical: {error}"))?
        .decode()
        .map_err(|error| format!("owner operation is invalid: {error}"))
}

fn run() -> Result<(), String> {
    let arguments: Vec<_> = env::args().skip(1).collect();
    match arguments.as_slice() {
        [mode] if mode == "--fresh-child" => run_fresh_child(),
        [mode] if mode == "--owner-operation" => {
            let operation = decode_owner_operation(&read_stdin()?)?;
            let response = owner_operation(operation)?;
            write_result_line(io::stdout().lock(), &response)
                .map_err(|error| format!("cannot write owner response: {error}"))
        }
        [mode] if mode == "--self-check" => {
            let response = OwnerResponse::SelfCheck {
                evidence: canonical_json(&self_check()?)?,
            };
            write_result_line(io::stdout().lock(), &response)
                .map_err(|error| format!("cannot write self-check response: {error}"))
        }
        [mode] if mode == "--list-grid" => {
            let response = OwnerResponse::ListGrid {
                evidence: canonical_json(&list_grid()?)?,
            };
            write_result_line(io::stdout().lock(), &response)
                .map_err(|error| format!("cannot write grid response: {error}"))
        }
        [mode] if mode == "--capability-report" => {
            let response = OwnerResponse::CapabilityReport {
                evidence: canonical_json(&capability_report()?)?,
            };
            write_result_line(io::stdout().lock(), &response)
                .map_err(|error| format!("cannot write capability response: {error}"))
        }
        _ => Err(
            "usage: tuning_calibration (--owner-operation|--fresh-child|--self-check|--list-grid|--capability-report)"
                .to_owned(),
        ),
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("algebra tuning producer failed: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
#[allow(dead_code, unused_imports)]
mod tests {
    use super::*;
    use std::process::{Command, Stdio};
    use tuning_campaign_support::campaign::{
        AcceptedResult, ObservedProvenance, ProcessDescriptor, ProgressKind, ProgressRecord,
        ProgressTracker, SessionChannels,
    };
    use tuning_campaign_support::journal::ResumeIdentity;
    use tuning_campaign_support::statistics::DecisionReason;
    use tuning_campaign_support::timing::TimingSample;
    use tuning_campaign_support::transport::{encode_result_line, retain_libtest_result_lines};

    const ZERO_SHA: &str = "0000000000000000000000000000000000000000000000000000000000000000";
    const ZERO_REV: &str = "0000000000000000000000000000000000000000";

    fn test_root(label: &str) -> std::path::PathBuf {
        env::temp_dir().join(format!("gf2-a835-algebra-{label}-{}", std::process::id()))
    }

    fn test_process() -> ProcessDescriptor {
        let executable = fs::canonicalize(env::current_exe().unwrap()).unwrap();
        let working_directory = fs::canonicalize(env::current_dir().unwrap()).unwrap();
        let mut environment = std::collections::BTreeMap::new();
        environment.insert(FRESH_CASE_VAR.to_owned(), FRESH_CASE_VALUE.to_owned());
        environment.insert("GF2_BENCH".to_owned(), "1".to_owned());
        environment.insert("RAYON_NUM_THREADS".to_owned(), "4".to_owned());
        environment.insert("RUSTUP_TOOLCHAIN".to_owned(), REQUIRED_TOOLCHAIN.to_owned());
        ProcessDescriptor {
            id: token(PROCESS).unwrap(),
            executable_sha256: Sha256Digest::of(&fs::read(&executable).unwrap()),
            executable,
            arguments: vec!["--fresh-child".to_owned()],
            environment,
            working_directory,
        }
    }

    fn manifest_request(label: &str) -> ManifestRequest {
        let stage = test_root(label);
        ManifestRequest {
            campaign_id: token(format!("test-{label}")).unwrap(),
            protocol_sha256: Sha256Digest::new(ZERO_SHA).unwrap(),
            channels: SessionChannels {
                execution_log: stage.join("execution.log"),
                checkpoints: stage.join("checkpoints"),
                stage,
            },
            processes: vec![test_process()],
        }
    }

    #[derive(Clone, Copy)]
    enum SyntheticCurve {
        MeasuredDefault,
        SelectedNondefault,
        CrossStratumFallback,
    }

    fn synthetic_cost(curve: SyntheticCurve, shape_index: usize, rank: usize) -> u64 {
        match curve {
            SyntheticCurve::MeasuredDefault => [30, 20, 10, 20, 30][rank],
            SyntheticCurve::SelectedNondefault => [30, 10, 20, 30, 40][rank],
            SyntheticCurve::CrossStratumFallback if shape_index == 2 => [30, 25, 20, 30, 40][rank],
            SyntheticCurve::CrossStratumFallback => [30, 5, 20, 30, 40][rank],
        }
    }

    fn synthetic_result(unit: &LaunchUnit, curve: SyntheticCurve) -> ChildResult {
        let case: AlgebraCase = unit.case.decode().unwrap();
        let rank = CANDIDATES
            .iter()
            .position(|candidate| *candidate == case.candidate)
            .unwrap();
        let cost = synthetic_cost(curve, case.shape_index, rank);
        let samples = match case.identity.task {
            Task::Probe => Vec::new(),
            Task::Measure { execution } => (0..5)
                .map(|repetition| TimingSample::new(execution, repetition, 1, cost).unwrap())
                .collect(),
        };
        let (_, profile) = forced_profile(case.candidate).unwrap();
        let payload = AlgebraPayload {
            schema: PAYLOAD_SCHEMA.to_owned(),
            field: FIELD.to_owned(),
            shape_index: case.shape_index,
            dimension: case.dimension,
            requested_chunk_subsets: case.candidate,
            observed_requested_chunk_subsets: case.candidate,
            effective_partition: expected_partition(case.dimension, case.candidate).unwrap(),
            profile,
            capability: CapabilityEvidence {
                required_features: REQUIRED_FEATURES.to_owned(),
                pool_threads: POOL_THREADS,
                parallel_route: "public-permanent-bipedal3-parallel".to_owned(),
            },
            semantics: SemanticEvidence {
                fixture_count: FIXTURE_BANKS,
                operands_sha256: expected_operands_digest(case.shape_index, case.dimension),
                parallel_results_sha256: ZERO_SHA.to_owned(),
                serial_results_sha256: ZERO_SHA.to_owned(),
                serial_equal: true,
            },
        };
        ChildResult {
            schema: RESULT_SCHEMA.to_owned(),
            identity: unit.identity.clone(),
            case_sha256: unit.case.digest(),
            outcome: ChildOutcome::Complete,
            samples,
            payload: canonical_json(&payload).unwrap(),
        }
    }

    fn synthetic_bundle(manifest: &OwnerManifest, curve: SyntheticCurve) -> AcceptedResultsBundle {
        AcceptedResultsBundle {
            schema: ACCEPTED_RESULTS_SCHEMA.to_owned(),
            manifest_sha256: manifest.manifest_sha256.clone(),
            accepted: manifest
                .ordered_units
                .iter()
                .map(|unit| AcceptedResult {
                    unit: unit.clone(),
                    result: synthetic_result(unit, curve),
                    checkpoint_sha256: Sha256Digest::of(unit.key.as_str().as_bytes()),
                })
                .collect(),
        }
    }

    #[test]
    fn grid_order_counts_seeds_and_codec_inventory_are_exact() {
        let manifest = campaign_manifest(manifest_request("grid")).unwrap();
        validate_manifest_order(&manifest).unwrap();
        assert_eq!(manifest.ordered_units.len(), 90);
        assert_eq!(manifest.owner.as_str(), "gf2-algebra");
        assert_eq!(manifest.owner_protocol.as_str(), OWNER_PROTOCOL);
        assert_eq!(manifest.processes[0].id.as_str(), "algebra-producer");
        assert_eq!(manifest.processes[0].arguments, ["--fresh-child"]);
        assert_eq!(manifest.processes[0].environment, measurement_environment());
        assert_eq!(manifest.counts, DeclaredCounts::for_cells(15).unwrap());
        assert_eq!(manifest.counts.windows, 375);
        assert_eq!(manifest.counts.progress_records, 450);
        assert_eq!(codec_inventory().unwrap(), [FIELD]);
        assert_eq!(candidate_order(0).unwrap(), CANDIDATES);
        assert_eq!(
            candidate_order(1).unwrap(),
            [4_096, 1_048_576, 262_144, 65_536, 16_384]
        );
        assert_eq!(
            candidate_order(2).unwrap(),
            [65_536, 262_144, 1_048_576, 4_096, 16_384]
        );
        assert_eq!(
            candidate_order(3).unwrap(),
            [65_536, 16_384, 4_096, 1_048_576, 262_144]
        );
        assert_eq!(
            candidate_order(4).unwrap(),
            [1_048_576, 4_096, 16_384, 65_536, 262_144]
        );

        let first: AlgebraCase = manifest.ordered_units[0].case.decode().unwrap();
        let paired: AlgebraCase = manifest.ordered_units[1].case.decode().unwrap();
        assert_eq!(first.seeds, paired.seeds);
        assert_eq!(first.seeds.field_tag, 27);
        assert_eq!(first.seeds.shape_key, 0);
        assert_eq!(first.seeds.streams.len(), 8);
        assert_eq!(first.seeds.streams[0].role, 0x0c00);
        assert_eq!(first.seeds.streams[7].role, 0x70c00);
    }

    #[test]
    fn canonical_parsing_and_owner_validation_reject_changes() {
        let manifest = campaign_manifest(manifest_request("reject")).unwrap();
        let unit = &manifest.ordered_units[0];
        let mut case: AlgebraCase = unit.case.decode().unwrap();
        case.candidate = 3;
        assert!(validate_case(&case).is_err());
        let mut value: serde_json::Value = serde_json::from_str(unit.case.as_str()).unwrap();
        value["unknown"] = serde_json::json!(true);
        let unknown = CanonicalJson::new(serde_json::to_string(&value).unwrap()).unwrap();
        assert!(unknown.decode::<AlgebraCase>().is_err());
        assert!(CanonicalJson::new(format!(" {}", unit.case.as_str())).is_err());

        let mut result = synthetic_result(unit, SyntheticCurve::MeasuredDefault);
        result.identity.candidate = token("q3").unwrap();
        assert!(validate_result(unit, &result).is_err());
    }

    #[test]
    fn owner_rejects_resealed_case_identity_substitution() {
        let mut manifest = campaign_manifest(manifest_request("identity-manifest")).unwrap();
        let mut case: AlgebraCase = manifest.ordered_units[0].case.decode().unwrap();
        case.identity.campaign_id = token("substituted-campaign").unwrap();
        manifest.ordered_units[0].case = canonical_json(&case).unwrap();
        manifest.seal().unwrap();
        assert!(manifest.validate().is_ok());
        assert!(validate_manifest_order(&manifest).is_err());

        let manifest = campaign_manifest(manifest_request("identity-bundle")).unwrap();
        let mut bundle = synthetic_bundle(&manifest, SyntheticCurve::MeasuredDefault);
        let accepted = &mut bundle.accepted[0];
        let mut case: AlgebraCase = accepted.unit.case.decode().unwrap();
        case.identity.campaign_id = token("substituted-campaign").unwrap();
        accepted.unit.case = canonical_json(&case).unwrap();
        accepted.result.case_sha256 = accepted.unit.case.digest();
        let bundle: AcceptedResultsBundle = canonical_json(&bundle).unwrap().decode().unwrap();
        assert!(analyze_results(&bundle).is_err());
    }

    #[test]
    fn result_validation_rejects_sample_order_calls_and_execution_changes() {
        let manifest = campaign_manifest(manifest_request("sample-reject")).unwrap();
        let unit = manifest
            .ordered_units
            .iter()
            .find(|unit| unit.identity.task == Task::Measure { execution: 0 })
            .unwrap();
        let valid = synthetic_result(unit, SyntheticCurve::MeasuredDefault);

        let mut reordered = valid.clone();
        reordered.samples.swap(0, 1);
        assert!(validate_result(unit, &reordered).is_err());

        let mut changed_calls = valid.clone();
        changed_calls.samples[2].calls = 2;
        assert!(validate_result(unit, &changed_calls).is_err());

        let mut changed_execution = valid;
        changed_execution.samples[3].execution = 1;
        assert!(validate_result(unit, &changed_execution).is_err());
    }

    #[test]
    fn progress_and_result_share_the_exact_identity_and_samples() {
        let manifest = campaign_manifest(manifest_request("progress")).unwrap();
        let unit = manifest
            .ordered_units
            .iter()
            .find(|unit| unit.identity.task == Task::Measure { execution: 0 })
            .unwrap();
        let result = synthetic_result(unit, SyntheticCurve::MeasuredDefault);
        let mut tracker = ProgressTracker::new(unit.identity.clone(), unit.case.digest()).unwrap();
        let calls = result.samples[0].calls;
        let mut stderr = Vec::new();
        ProgressRecord::new(
            unit.identity.clone(),
            unit.case.digest(),
            ProgressKind::CalibrationComplete { calls },
        )
        .unwrap()
        .write_line(&mut stderr)
        .unwrap();
        for sample in &result.samples {
            ProgressRecord::new(
                unit.identity.clone(),
                unit.case.digest(),
                ProgressKind::WindowComplete {
                    repetition: sample.repetition,
                    calls: sample.calls,
                    elapsed_ns: sample.elapsed_ns,
                },
            )
            .unwrap()
            .write_line(&mut stderr)
            .unwrap();
        }
        for line in std::str::from_utf8(&stderr).unwrap().lines() {
            tracker
                .accept(ProgressRecord::parse_line(line).unwrap().unwrap())
                .unwrap();
        }
        let stdout = encode_result_line(&result).unwrap();
        assert_eq!(tracker.finish(&stdout).unwrap(), result);
        validate_result(unit, &result).unwrap();
    }

    #[test]
    fn fast_probe_child_installs_and_observes_the_public_route() {
        let executable = env::current_exe().unwrap();
        let output = Command::new(executable)
            .args(["fast_probe_child_entry", "--nocapture"])
            .env("GF2_ALGEBRA_FAST_PROBE", "child-v1")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "fast child failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).unwrap();
        let filtered = retain_libtest_result_lines(&stdout);
        let result: ChildResult =
            tuning_campaign_support::transport::parse_result(&filtered).unwrap();
        let payload: AlgebraPayload = result.payload.decode().unwrap();
        assert_eq!(payload.profile.resolution, "installed");
        assert_eq!(payload.requested_chunk_subsets, 4);
        assert_eq!(payload.observed_requested_chunk_subsets, 4);
        assert_eq!(
            payload.effective_partition,
            EffectivePartition {
                maximum_chunk_len: 4,
                chunk_count: 16,
                last_chunk_len: 3,
            }
        );
        assert_eq!(payload.capability.pool_threads, 4);
        assert!(payload.semantics.serial_equal);
    }

    #[test]
    fn fast_probe_child_entry() {
        if env::var("GF2_ALGEBRA_FAST_PROBE").as_deref() != Ok("child-v1") {
            return;
        }
        let identity = UnitIdentity {
            protocol: token(OWNER_PROTOCOL).unwrap(),
            owner: token(OWNER).unwrap(),
            campaign_id: token("fast-probe").unwrap(),
            phase: token(PHASE).unwrap(),
            field: token(FIELD).unwrap(),
            stratum: token("n6").unwrap(),
            candidate: token("q4").unwrap(),
            task: Task::Probe,
        };
        let case = AlgebraCase {
            schema: CASE_SCHEMA.to_owned(),
            identity,
            protocol_sha256: Sha256Digest::new(ZERO_SHA).unwrap(),
            field: FIELD.to_owned(),
            shape_index: 0,
            dimension: 6,
            candidate: 4,
            pool_threads: 4,
            seeds: seed_inventory(0),
            channels: ChildChannels {
                journal_schema: JOURNAL_SCHEMA.to_owned(),
                checkpoint_schema: CHECKPOINT_SCHEMA.to_owned(),
                raw_sample_schema: RAW_SAMPLE_SCHEMA.to_owned(),
                execution_log: "/tmp/fast-probe/execution.log".into(),
                checkpoints: "/tmp/fast-probe/checkpoints".into(),
            },
        };
        let case_json = canonical_json(&case).unwrap();
        let result = execute_case(case, case_json, false).unwrap();
        write_result_line(io::stdout().lock(), &result).unwrap();
    }

    #[test]
    fn production_boundary_probe_emits_observed_partition() {
        let executable = env::current_exe().unwrap();
        let output = Command::new(executable)
            .args(["production_boundary_probe_child_entry", "--nocapture"])
            .env("GF2_ALGEBRA_BOUNDARY_PROBE", "child-v1")
            .env("GF2_BENCH", "1")
            .env("RAYON_NUM_THREADS", "4")
            .env("RUSTUP_TOOLCHAIN", REQUIRED_TOOLCHAIN)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "production boundary child failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).unwrap();
        let filtered = retain_libtest_result_lines(&stdout);
        let result: ChildResult =
            tuning_campaign_support::transport::parse_result(&filtered).unwrap();
        let payload: AlgebraPayload = result.payload.decode().unwrap();
        assert_eq!(payload.dimension, 20);
        assert_eq!(payload.requested_chunk_subsets, 1_048_576);
        assert_eq!(payload.observed_requested_chunk_subsets, 1_048_576);
        assert_eq!(
            payload.effective_partition,
            expected_partition(20, 1_048_576).unwrap()
        );
        assert!(payload.semantics.serial_equal);
    }

    #[test]
    fn production_boundary_probe_child_entry() {
        if env::var("GF2_ALGEBRA_BOUNDARY_PROBE").as_deref() != Ok("child-v1") {
            return;
        }
        let identity = UnitIdentity {
            protocol: token(OWNER_PROTOCOL).unwrap(),
            owner: token(OWNER).unwrap(),
            campaign_id: token("production-boundary-probe").unwrap(),
            phase: token(PHASE).unwrap(),
            field: token(FIELD).unwrap(),
            stratum: token("n20").unwrap(),
            candidate: token("q1048576").unwrap(),
            task: Task::Probe,
        };
        let case = AlgebraCase {
            schema: CASE_SCHEMA.to_owned(),
            identity,
            protocol_sha256: Sha256Digest::new(ZERO_SHA).unwrap(),
            field: FIELD.to_owned(),
            shape_index: 0,
            dimension: 20,
            candidate: 1_048_576,
            pool_threads: 4,
            seeds: seed_inventory(0),
            channels: ChildChannels {
                journal_schema: JOURNAL_SCHEMA.to_owned(),
                checkpoint_schema: CHECKPOINT_SCHEMA.to_owned(),
                raw_sample_schema: RAW_SAMPLE_SCHEMA.to_owned(),
                execution_log: "/tmp/production-boundary-probe/execution.log".into(),
                checkpoints: "/tmp/production-boundary-probe/checkpoints".into(),
            },
        };
        let case_json = canonical_json(&case).unwrap();
        let result = execute_case(case, case_json, true).unwrap();
        write_result_line(io::stdout().lock(), &result).unwrap();
    }

    #[test]
    fn analysis_selects_nondefault_and_falls_back_on_cross_stratum_conflict() {
        let manifest = campaign_manifest(manifest_request("analysis-curves")).unwrap();

        let nondefault = analyze_results(&synthetic_bundle(
            &manifest,
            SyntheticCurve::SelectedNondefault,
        ))
        .unwrap();
        assert_eq!(nondefault.selected, 16_384);
        assert_eq!(
            nondefault.decision.reason,
            DecisionReason::SelectedNonDefault
        );

        let fallback = analyze_results(&synthetic_bundle(
            &manifest,
            SyntheticCurve::CrossStratumFallback,
        ))
        .unwrap();
        assert_eq!(
            fallback.selected,
            AlgebraTuning::CONSERVATIVE.permanent().gray_chunk_subsets()
        );
        assert_eq!(
            fallback.decision.reason,
            DecisionReason::CrossStratumConflict
        );
    }

    fn resume_identity() -> ResumeIdentity {
        let executable = env::current_exe().unwrap();
        let executable_sha256_value = sha256(&fs::read(executable).unwrap());
        let mut executable_sha256 = std::collections::BTreeMap::new();
        executable_sha256.insert(PROCESS.to_owned(), executable_sha256_value);
        let mut behavior_sha256 = std::collections::BTreeMap::new();
        behavior_sha256.insert(HARNESS_PATH.to_owned(), ZERO_SHA.to_owned());
        ResumeIdentity {
            protocol_digest: ZERO_SHA.to_owned(),
            source_revision: ZERO_REV.to_owned(),
            source_sha256: ZERO_SHA.to_owned(),
            ordered_work_manifest_sha256: ZERO_SHA.to_owned(),
            process_descriptors_sha256: ZERO_SHA.to_owned(),
            executable_sha256,
            behavior_sha256,
            lifecycle_schema: "tuning-campaign-session-v1".to_owned(),
            lifecycle_behavior_sha256: ZERO_SHA.to_owned(),
            feature_contract: REQUIRED_FEATURES.to_owned(),
            thread_contract: THREAD_CONTRACT.to_owned(),
            host_identity: "test-host".to_owned(),
        }
    }

    #[test]
    fn owner_emission_analyzes_and_strictly_reopens_the_algebra_envelope() {
        let root = test_root("emit");
        fs::create_dir_all(&root).unwrap();
        let manifest = campaign_manifest(ManifestRequest {
            channels: SessionChannels {
                execution_log: root.join("execution.log"),
                checkpoints: root.join("checkpoints"),
                stage: root.clone(),
            },
            ..manifest_request("emit-request")
        })
        .unwrap();
        let bundle = synthetic_bundle(&manifest, SyntheticCurve::SelectedNondefault);
        let bundle_path = root.join("accepted.json");
        let bundle_bytes = serde_json::to_vec(&bundle).unwrap();
        fs::write(&bundle_path, &bundle_bytes).unwrap();
        let bundle_path = fs::canonicalize(bundle_path).unwrap();
        let identity = resume_identity();
        let producer_sha256 = identity.executable_sha256[PROCESS].clone();
        let request = EmitOwnerRequest {
            campaign_id: manifest.campaign_id.clone(),
            manifest_sha256: manifest.manifest_sha256.clone(),
            accepted_results: ArtifactIdentity {
                path: bundle_path,
                sha256: Sha256Digest::of(&bundle_bytes),
            },
            measurement: ObservedProvenance {
                identity: identity.clone(),
                process: token(PROCESS).unwrap(),
                observed_utc: "2026-09-05T00:00:00Z".to_owned(),
                runtime: canonical_json(&MeasurementRuntime {
                    source_dirty: false,
                    toolchain: "rustc 1.95.0".to_owned(),
                    cpu_model: "test-cpu".to_owned(),
                    cpu_features: vec!["test".to_owned()],
                    os_kernel: "test-kernel".to_owned(),
                    governor: "performance".to_owned(),
                    receipt: "dev/benchmarks/tuning_profiles/test.md".to_owned(),
                })
                .unwrap(),
            },
            assembly: ObservedProvenance {
                identity,
                process: token(PROCESS).unwrap(),
                observed_utc: "2026-09-05T00:01:00Z".to_owned(),
                runtime: canonical_json(&AssemblyRuntime {
                    source_dirty: false,
                    tool: HARNESS_PATH.to_owned(),
                    tool_sha256: producer_sha256,
                })
                .unwrap(),
            },
            output: root.join("algebra-owner.json"),
        };
        let (artifact, analysis) = emit_owner(request).unwrap();
        assert_eq!(analysis.selected, 16_384);
        assert_eq!(analysis.decision.reason, DecisionReason::SelectedNonDefault);
        let bytes = fs::read(&artifact.path).unwrap();
        assert_eq!(artifact.sha256, Sha256Digest::of(&bytes));
        let text = String::from_utf8(bytes).unwrap();
        let reopened = algebra_registry().unwrap().from_json(&text).unwrap();
        assert_eq!(
            reopened.section_ids().collect::<Vec<_>>(),
            [AlgebraTuning::ID.as_str()]
        );
        assert_eq!(
            reopened
                .section::<AlgebraTuning>()
                .unwrap()
                .unwrap()
                .section
                .permanent()
                .gray_chunk_subsets(),
            16_384
        );
        fs::remove_dir_all(root).unwrap();
    }
}
