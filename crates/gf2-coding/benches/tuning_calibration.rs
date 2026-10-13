//! Coding-owned producer of the offline tuning campaign for the decode
//! selector fields. The neutral driver consumes this executable's sealed owner
//! manifest and treats its cases and evidence as opaque bytes. Only a guarded
//! fresh child installs tuning; reporting, validation, analysis and owner
//! emission never resolve process-global tuning.

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::hint::black_box;
use std::io::{self, Read};
use std::sync::{Mutex, OnceLock};

use gf2_coding::channel::AwgnChannel;
use gf2_coding::ldpc::{
    decide_batch_route, BatchRoute, DecodedWord, DecoderConfig, LaneBackend, LaneCapability,
    LdpcBatchDecoder, LdpcCode, LdpcDecoder, QuasiCyclicLdpc,
};
use gf2_coding::llr::Llr;
use gf2_coding::pooled::PooledBatchDecoder;
use gf2_coding::traits::{BatchSoftDecoder, DecodeOutcome};
use gf2_coding::tuning::{
    CodingSelectors, CodingTuning, CodingTuningCodec, DecodeSelectors, GRANULES_PER_TASK,
    LANE_MAX_EDGES, LANE_MIN_FRAMES,
};
use gf2_coding::CodeRate;
use gf2_core::compute::field::run_in_dedicated_parallel_pool;
use gf2_core::tuning::{
    self, AssemblyProvenance, CanonicalValue, CompiledProfileProvenance, GitRevision,
    HarnessSchema, MeasurementProvenance, PreparedEnvelope, ProfileId, ProfileRegistry,
    ProfileRegistryBuilder, RepoRelPath, Rfc3339Utc, SectionCodec, SectionResolution, Sha256,
    TuningSection,
};
use gf2_core::BitVec;
use gf2_kernels_simd::ldpc_lanes::LANES;
use rand::rngs::StdRng;
use rand::SeedableRng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256 as Sha256Hasher};
use tuning_campaign_support::campaign::{
    execution_with_progress, measurement_environment, AcceptedResult, AcceptedResultsBundle,
    ArtifactIdentity, CandidateBlock, CanonicalJson, ChildOutcome, ChildResult, DeclaredCounts,
    EmitOwnerRequest, LaunchUnit, ManifestRequest, OwnerManifest, OwnerOperation, OwnerResponse,
    Sha256Digest, Task, Token, UnitIdentity, ACCEPTED_RESULTS_SCHEMA, FEATURE_CONTRACT,
    MANIFEST_SCHEMA, RESULT_SCHEMA, THREAD_CONTRACT, TOOLCHAIN_CONTRACT,
};
use tuning_campaign_support::journal::{atomic_write_new, CHECKPOINT_SCHEMA, JOURNAL_SCHEMA};
use tuning_campaign_support::seed::{bank_role, fixture_seed, EXTENT_SEED_ROOT, SEED_DERIVATION};
use tuning_campaign_support::statistics::{
    analyze_extent, execution_median, CandidateSeries, DecisionReason, ExtentDecision,
};
use tuning_campaign_support::timing::{TimingSample, EXECUTIONS, FIXTURE_BANKS, WINDOWS};
use tuning_campaign_support::transport::{read_guarded_case, write_result_line, FRESH_CASE_VAR};

const OWNER: &str = "gf2-coding";
const OWNER_PROTOCOL: &str = "coding-tuning-campaign-v1";
const BEHAVIOR_TOKEN: &str = CodingTuningCodec::HARNESS_SCHEMA;
const CASE_SCHEMA: &str = "coding-tuning-campaign-v1";
const PAYLOAD_SCHEMA: &str = "coding-tuning-campaign-v1";
const SEED_SCHEMA: &str = "fixture-seeds-v3";
const RAW_SAMPLE_SCHEMA: &str = "raw-timing-samples-v3";
const PHASE: &str = "decode-extent";
const PROCESS: &str = "coding-producer";
const FRAME_ROLE: u64 = 0x0d00;
const POOL_THREADS: usize = 4;
const FORCED_PROFILE_ID: &str = "coding-decode-forced";
const HARNESS_PATH: &str = "crates/gf2-coding/benches/tuning_calibration.rs";
const DIRECT_ENTRY: &str = "ldpc-batch-decoder";
const POOLED_ENTRY: &str = "pooled-ldpc-batch-decoder";

const MAX_ITERATIONS: usize = 50;
/// Occupied lanes of the one-wave batches of the `lane_min_frames` grid.
const OCCUPIED_LANES: [usize; 6] = [1, 2, 3, 4, 6, 8];
const TASK_GRANULES: [usize; 5] = [1, 2, 4, 8, 16];
const POOLED_WAVES: usize = 16;
const POOLED_CODES: [usize; 2] = [0, 1];
/// Per-mille factors on a code's noise deviation, by frame index in a batch,
/// so that the frames of one wave stop at different iterations.
const SIGMA_SCALE_MILLI: [u32; 8] = [940, 1000, 880, 1030, 970, 910, 1060, 850];

/// A code of the calibration inventory. Its frames are the all-zero codeword,
/// a codeword of every linear code, through AWGN.
struct CodeSpec {
    name: &'static str,
    build: fn() -> LdpcCode,
    /// Per-mille noise deviation of a unit-energy BPSK symbol.
    noise_sigma_milli: u32,
}

/// The codes in increasing edge count; [`codes`] checks the order.
const CODES: [CodeSpec; 3] = [
    CodeSpec {
        name: "nr5g-bg2-z16",
        build: || LdpcCode::from_quasi_cyclic(&QuasiCyclicLdpc::nr_5g(2, 16)),
        noise_sigma_milli: 1200,
    },
    CodeSpec {
        name: "dvb-t2-short-r1-2",
        build: || LdpcCode::dvb_t2_short(CodeRate::Rate1_2),
        noise_sigma_milli: 800,
    },
    CodeSpec {
        name: "dvb-t2-normal-r1-2",
        build: || LdpcCode::dvb_t2_normal(CodeRate::Rate1_2),
        noise_sigma_milli: 750,
    },
];

struct BuiltCode {
    code: LdpcCode,
    frame_len: usize,
    edges: usize,
}

fn codes() -> Result<&'static [BuiltCode], String> {
    static BUILT: OnceLock<Vec<BuiltCode>> = OnceLock::new();
    let built = BUILT.get_or_init(|| {
        CODES
            .iter()
            .map(|spec| {
                let code = (spec.build)();
                let layout = LdpcDecoder::with_config(code.clone(), decoder_config());
                let layout = layout.edge_layout();
                BuiltCode {
                    frame_len: layout.n(),
                    edges: layout.edges(),
                    code,
                }
            })
            .collect()
    });
    if built.windows(2).any(|pair| pair[0].edges >= pair[1].edges) {
        return Err("calibration codes are not in increasing edge count".to_owned());
    }
    Ok(built)
}

fn decoder_config() -> DecoderConfig {
    DecoderConfig::default()
}

/// A decode selector field of the coding tuning section.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum Field {
    LaneMinFrames,
    LaneMaxEdges,
    GranulesPerTask,
}

impl Field {
    const ALL: [Self; 3] = [
        Self::LaneMinFrames,
        Self::LaneMaxEdges,
        Self::GranulesPerTask,
    ];
    const FAMILY: &'static str = "decode";

    const fn key(self) -> &'static str {
        match self {
            Self::LaneMinFrames => "lane_min_frames",
            Self::LaneMaxEdges => "lane_max_edges",
            Self::GranulesPerTask => "granules_per_task",
        }
    }

    fn name(self) -> String {
        format!("{}.{}", Self::FAMILY, self.key())
    }

    const fn seed_tag(self) -> u64 {
        match self {
            Self::LaneMinFrames => 32,
            Self::LaneMaxEdges => 33,
            Self::GranulesPerTask => 34,
        }
    }

    const fn conservative(self) -> usize {
        match self {
            Self::LaneMinFrames => LANE_MIN_FRAMES,
            Self::LaneMaxEdges => LANE_MAX_EDGES,
            Self::GranulesPerTask => GRANULES_PER_TASK.get(),
        }
    }

    /// Candidates in increasing order, the conservative value among them.
    fn candidates(self) -> Result<Vec<usize>, String> {
        let mut candidates = match self {
            Self::LaneMinFrames => OCCUPIED_LANES.to_vec(),
            Self::LaneMaxEdges => codes()?.iter().map(|code| code.edges).collect(),
            Self::GranulesPerTask => TASK_GRANULES.to_vec(),
        };
        if !candidates.contains(&self.conservative()) {
            candidates.push(self.conservative());
        }
        candidates.sort_unstable();
        Ok(candidates)
    }

    /// The batch shapes a field is decided over, one grid axis per field:
    /// one-wave batches of each occupied-lane count on the smallest code,
    /// one full wave on each code, and pooled batches on [`POOLED_CODES`]
    /// with the lane route open and closed, so that a selected task size
    /// holds on either route.
    fn strata(self) -> Vec<Stratum> {
        match self {
            Self::LaneMinFrames => OCCUPIED_LANES
                .into_iter()
                .map(|frames| Stratum {
                    code: 0,
                    frames,
                    open_lanes: true,
                })
                .collect(),
            Self::LaneMaxEdges => (0..CODES.len())
                .map(|code| Stratum {
                    code,
                    frames: LANES,
                    open_lanes: true,
                })
                .collect(),
            Self::GranulesPerTask => POOLED_CODES
                .into_iter()
                .flat_map(|code| {
                    [false, true].map(|open_lanes| Stratum {
                        code,
                        frames: POOLED_WAVES * LANES,
                        open_lanes,
                    })
                })
                .collect(),
        }
    }

    /// The selectors a case installs: `candidate` in this field, and in the
    /// lane bounds the field does not measure the values `stratum` names.
    fn selectors(self, stratum: &Stratum, candidate: usize) -> Result<DecodeSelectors, String> {
        let (min_frames, max_edges) = if stratum.open_lanes {
            (1, usize::MAX)
        } else {
            (LANE_MIN_FRAMES, LANE_MAX_EDGES)
        };
        match self {
            Self::LaneMinFrames => {
                DecodeSelectors::try_new(candidate, max_edges, GRANULES_PER_TASK.get())
            }
            Self::LaneMaxEdges => {
                DecodeSelectors::try_new(min_frames, candidate, GRANULES_PER_TASK.get())
            }
            Self::GranulesPerTask => DecodeSelectors::try_new(min_frames, max_edges, candidate),
        }
        .map_err(|error| error.to_string())
    }

    const fn entry(self) -> &'static str {
        match self {
            Self::GranulesPerTask => POOLED_ENTRY,
            Self::LaneMinFrames | Self::LaneMaxEdges => DIRECT_ENTRY,
        }
    }

    /// The schedule of a batch under [`Self::selectors`], decided as the
    /// decoders decide it and without resolving process tuning.
    fn schedule(self, stratum: &Stratum, candidate: usize) -> Result<Schedule, String> {
        let selectors = self.selectors(stratum, candidate)?;
        let route = decide_batch_route(
            &selectors,
            decoder_config().algorithm(),
            LaneCapability::Present(LaneBackend::detected()),
            stratum.frames.min(LANES),
            codes()?[stratum.code].edges,
        );
        let task_frames = match self {
            Self::GranulesPerTask => LANES
                .checked_mul(candidate)
                .ok_or("task size overflows usize")?,
            Self::LaneMinFrames | Self::LaneMaxEdges => stratum.frames,
        };
        Ok(Schedule::new(route, task_frames, stratum.frames))
    }
}

/// One batch shape of a field's grid. Every wave of the batch has the same
/// number of frames, so one route describes the batch.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Stratum {
    code: usize,
    frames: usize,
    /// Whether the lane bounds the field does not measure open the lane
    /// route; they hold their conservative values otherwise.
    open_lanes: bool,
}

impl Stratum {
    fn token(&self) -> Result<Token, String> {
        token(format!(
            "{}-f{}-{}",
            CODES[self.code].name,
            self.frames,
            if self.open_lanes { "open" } else { "closed" }
        ))
    }
}

/// How a batch decodes: the route of its waves and its split into tasks.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Schedule {
    route: String,
    task_frames: usize,
    tasks: usize,
}

impl Schedule {
    fn new(route: BatchRoute, task_frames: usize, frames: usize) -> Self {
        let task_frames = task_frames.min(frames);
        Self {
            route: route_name(route).to_owned(),
            task_frames,
            tasks: frames.div_ceil(task_frames),
        }
    }
}

const fn route_name(route: BatchRoute) -> &'static str {
    match route {
        BatchRoute::SingleFrame => "single-frame",
        BatchRoute::Lane(LaneBackend::Portable) => "lane-portable",
        BatchRoute::Lane(LaneBackend::Avx2) => "lane-avx2",
    }
}

const fn backend_name(backend: LaneBackend) -> &'static str {
    match backend {
        LaneBackend::Portable => "portable",
        LaneBackend::Avx2 => "avx2",
    }
}

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
struct CodingCase {
    schema: String,
    identity: UnitIdentity,
    protocol_sha256: Sha256Digest,
    field: Field,
    stratum_index: usize,
    stratum: Stratum,
    code: String,
    candidate: usize,
    pool_threads: usize,
    max_iterations: usize,
    seeds: SeedInventory,
    channels: ChildChannels,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SelectorEvidence {
    lane_min_frames: usize,
    lane_max_edges: usize,
    granules_per_task: usize,
}

impl From<&DecodeSelectors> for SelectorEvidence {
    fn from(selectors: &DecodeSelectors) -> Self {
        Self {
            lane_min_frames: selectors.lane_min_frames(),
            lane_max_edges: selectors.lane_max_edges(),
            granules_per_task: selectors.granules_per_task().get(),
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
    active_decode: SelectorEvidence,
    envelope_sha256: String,
    content_sha256: String,
    wrapper_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CapabilityEvidence {
    required_features: String,
    pool_threads: usize,
    lane_backend: String,
    entry: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CodeEvidence {
    name: String,
    frame_len: usize,
    edges: usize,
    workspace_bytes: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SemanticEvidence {
    fixture_count: usize,
    frames_per_fixture: usize,
    operands_sha256: String,
    batch_results_sha256: String,
    single_frame_results_sha256: String,
    single_frame_equal: bool,
    converged_frames: usize,
    iterations: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CodingPayload {
    schema: String,
    field: Field,
    stratum_index: usize,
    stratum: Stratum,
    candidate: usize,
    schedule: Schedule,
    code: CodeEvidence,
    profile: ProfileEvidence,
    capability: CapabilityEvidence,
    semantics: SemanticEvidence,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct FieldAnalysis {
    field: String,
    selected: usize,
    /// Whether the emitted section states `selected`; the field is omitted
    /// and keeps its conservative value otherwise.
    emitted: bool,
    decision: ExtentDecision<usize>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct AnalysisOutput {
    schema: String,
    fields: Vec<FieldAnalysis>,
    accepted_result_count: usize,
}

/// The inputs of one field on one code, for a holdout to stay clear of.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct InventoryEntry {
    field: String,
    code: String,
    frame_len: usize,
    edges: usize,
    noise_sigma_milli: u32,
    sigma_scale_milli: Vec<u32>,
    max_iterations: usize,
    batch_frames: Vec<usize>,
    seeds: SeedInventory,
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
    inventory: Vec<InventoryEntry>,
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

fn bank_seed(field: Field, code: usize, bank: usize) -> u64 {
    fixture_seed(
        EXTENT_SEED_ROOT,
        field.seed_tag(),
        code as u64,
        bank_role(FRAME_ROLE, bank),
    )
}

fn seed_inventory(field: Field, code: usize) -> SeedInventory {
    SeedInventory {
        schema: SEED_SCHEMA.to_owned(),
        root: EXTENT_SEED_ROOT,
        derivation: SEED_DERIVATION.to_owned(),
        field_tag: field.seed_tag(),
        shape_key: code as u64,
        streams: (0..FIXTURE_BANKS)
            .map(|bank| SeedStream {
                bank,
                role: bank_role(FRAME_ROLE, bank),
                seed: bank_seed(field, code, bank),
            })
            .collect(),
    }
}

fn candidate_token(candidate: usize) -> Result<Token, String> {
    token(format!("v{candidate}"))
}

fn candidate_block(field: Field, stratum: &Stratum) -> Result<CandidateBlock, String> {
    Ok(CandidateBlock {
        phase: token(PHASE)?,
        field: token(field.name())?,
        stratum: stratum.token()?,
        base_candidates: field
            .candidates()?
            .into_iter()
            .map(candidate_token)
            .collect::<Result<_, _>>()?,
    })
}

/// Every unit of the campaign in acquisition order: per field and stratum,
/// the block order of [`CandidateBlock::candidates_for`].
fn acquisition_order() -> Result<Vec<(Field, usize, usize, Task)>, String> {
    let mut order = Vec::new();
    for field in Field::ALL {
        let candidates = field.candidates()?;
        for (stratum_index, stratum) in field.strata().iter().enumerate() {
            let block = candidate_block(field, stratum)?;
            let tasks = std::iter::once(Task::Probe)
                .chain((0..EXECUTIONS).map(|execution| Task::Measure { execution }));
            for task in tasks {
                for ordered in block
                    .candidates_for(task)
                    .map_err(|error| error.to_string())?
                {
                    let rank = block
                        .base_candidates
                        .iter()
                        .position(|base| *base == ordered)
                        .ok_or("candidate block order names an undeclared candidate")?;
                    order.push((field, stratum_index, candidates[rank], task));
                }
            }
        }
    }
    Ok(order)
}

fn declared_cells() -> Result<u64, String> {
    let mut cells = 0;
    for field in Field::ALL {
        cells += field.strata().len() * field.candidates()?.len();
    }
    Ok(cells as u64)
}

fn declared_counts() -> Result<DeclaredCounts, String> {
    DeclaredCounts::for_cells(declared_cells()?).map_err(|error| error.to_string())
}

fn cell_identity(
    campaign_id: &Token,
    field: Field,
    stratum: &Stratum,
    candidate: usize,
    task: Task,
) -> Result<UnitIdentity, String> {
    Ok(UnitIdentity {
        protocol: token(OWNER_PROTOCOL)?,
        owner: token(OWNER)?,
        campaign_id: campaign_id.clone(),
        phase: token(PHASE)?,
        field: token(field.name())?,
        stratum: stratum.token()?,
        candidate: candidate_token(candidate)?,
        task,
    })
}

fn coding_case(
    request: &ManifestRequest,
    field: Field,
    stratum_index: usize,
    candidate: usize,
    task: Task,
) -> Result<CodingCase, String> {
    let stratum = field.strata()[stratum_index].clone();
    Ok(CodingCase {
        schema: CASE_SCHEMA.to_owned(),
        identity: cell_identity(&request.campaign_id, field, &stratum, candidate, task)?,
        protocol_sha256: request.protocol_sha256.clone(),
        field,
        stratum_index,
        code: CODES[stratum.code].name.to_owned(),
        seeds: seed_inventory(field, stratum.code),
        stratum,
        candidate,
        pool_threads: POOL_THREADS,
        max_iterations: MAX_ITERATIONS,
        channels: ChildChannels {
            journal_schema: JOURNAL_SCHEMA.to_owned(),
            checkpoint_schema: CHECKPOINT_SCHEMA.to_owned(),
            raw_sample_schema: RAW_SAMPLE_SCHEMA.to_owned(),
            execution_log: request.channels.execution_log.clone(),
            checkpoints: request.channels.checkpoints.clone(),
        },
    })
}

fn validate_case(case: &CodingCase) -> Result<(), String> {
    case.identity
        .task
        .validate()
        .map_err(|error| error.to_string())?;
    let strata = case.field.strata();
    let stratum = strata
        .get(case.stratum_index)
        .ok_or("coding fresh case names an undeclared stratum")?;
    let file_name = |path: &std::path::Path| {
        path.file_name()
            .and_then(|name| name.to_str())
            .map(str::to_owned)
    };
    if case.schema != CASE_SCHEMA
        || case.identity.protocol.as_str() != OWNER_PROTOCOL
        || case.identity.owner.as_str() != OWNER
        || case.identity.phase.as_str() != PHASE
        || case.identity.field.as_str() != case.field.name()
        || case.stratum != *stratum
        || !(stratum.frames <= LANES || stratum.frames.is_multiple_of(LANES))
        || case.code != CODES[stratum.code].name
        || !case.field.candidates()?.contains(&case.candidate)
        || case.identity.stratum != stratum.token()?
        || case.identity.candidate != candidate_token(case.candidate)?
        || case.pool_threads != POOL_THREADS
        || case.max_iterations != MAX_ITERATIONS
        || case.seeds != seed_inventory(case.field, stratum.code)
        || case.channels.journal_schema != JOURNAL_SCHEMA
        || case.channels.checkpoint_schema != CHECKPOINT_SCHEMA
        || case.channels.raw_sample_schema != RAW_SAMPLE_SCHEMA
        || !case.channels.execution_log.is_absolute()
        || !case.channels.checkpoints.is_absolute()
        || file_name(&case.channels.execution_log).as_deref() != Some("execution.log")
        || file_name(&case.channels.checkpoints).as_deref() != Some("checkpoints")
        || case.channels.execution_log.parent() != case.channels.checkpoints.parent()
    {
        return Err("coding fresh case disagrees with the declared protocol".to_owned());
    }
    Ok(())
}

fn campaign_manifest(request: ManifestRequest) -> Result<OwnerManifest, String> {
    // Emission names the owner profile by the campaign ID; refuse an ID the
    // loader would reject before any cell is measured.
    ProfileId::parse(request.campaign_id.as_str()).map_err(|_| {
        format!(
            "campaign ID `{}` is not a lowercase kebab-case tuning profile ID",
            request.campaign_id.as_str()
        )
    })?;
    request
        .channels
        .validate()
        .map_err(|error| format!("invalid campaign channels: {error}"))?;
    let process = request
        .processes
        .iter()
        .find(|process| process.id.as_str() == PROCESS)
        .ok_or("manifest request lacks the staged coding producer")?;
    process
        .verify_staged()
        .map_err(|error| format!("invalid staged coding process descriptor: {error}"))?;
    if fs::canonicalize(env::current_exe().map_err(|error| error.to_string())?)
        .map_err(|error| error.to_string())?
        != process.executable
    {
        return Err("coding process descriptor does not name this producer".to_owned());
    }
    if process.arguments != ["--fresh-child"] || process.environment != measurement_environment() {
        return Err("coding process descriptor violates the fresh-child contract".to_owned());
    }
    let process_id = process.id.clone();
    let mut ordered_units = Vec::new();
    for (field, stratum_index, candidate, task) in acquisition_order()? {
        let case = coding_case(&request, field, stratum_index, candidate, task)?;
        ordered_units.push(
            LaunchUnit::new(
                ordered_units.len() as u64,
                case.identity.clone(),
                process_id.clone(),
                canonical_json(&case)?,
            )
            .map_err(|error| error.to_string())?,
        );
    }
    let mut candidate_blocks = Vec::new();
    for field in Field::ALL {
        for stratum in field.strata() {
            candidate_blocks.push(candidate_block(field, &stratum)?);
        }
    }
    let mut manifest = OwnerManifest {
        schema: MANIFEST_SCHEMA.to_owned(),
        owner: token(OWNER)?,
        owner_protocol: token(OWNER_PROTOCOL)?,
        behavior_token: token(BEHAVIOR_TOKEN)?,
        campaign_id: request.campaign_id,
        phases: vec![token(PHASE)?],
        candidate_blocks,
        counts: declared_counts()?,
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
    let order = acquisition_order()?;
    if manifest.counts != declared_counts()? || manifest.ordered_units.len() != order.len() {
        return Err("coding manifest accounting is not the declared grid".to_owned());
    }
    for (unit, (field, stratum_index, candidate, task)) in manifest.ordered_units.iter().zip(order)
    {
        validate_manifest_unit(unit, field, stratum_index, candidate, task)?;
    }
    Ok(())
}

fn validate_manifest_unit(
    unit: &LaunchUnit,
    field: Field,
    stratum_index: usize,
    candidate: usize,
    task: Task,
) -> Result<(), String> {
    if unit.process.as_str() != PROCESS {
        return Err("coding manifest unit names the wrong process".to_owned());
    }
    let case: CodingCase = unit.case.decode().map_err(|error| error.to_string())?;
    validate_case(&case)?;
    if case.identity != unit.identity
        || case.field != field
        || case.stratum_index != stratum_index
        || case.candidate != candidate
        || case.identity.task != task
    {
        return Err("coding manifest unit is out of declared acquisition order".to_owned());
    }
    Ok(())
}

fn coding_registry() -> Result<ProfileRegistry, String> {
    ProfileRegistryBuilder::new()
        .register::<CodingTuning, CodingTuningCodec>()
        .and_then(ProfileRegistryBuilder::build)
        .map_err(|error| format!("cannot build coding owner registry: {error}"))
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

fn forced_profile(
    selectors: &DecodeSelectors,
) -> Result<(PreparedEnvelope, ProfileEvidence), String> {
    let id = ProfileId::parse(FORCED_PROFILE_ID).map_err(|error| error.to_string())?;
    let section = CodingTuning::from_selectors(CodingSelectors {
        encode: CodingSelectors::CONSERVATIVE.encode,
        decode: selectors.clone(),
    });
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
    let registry = coding_registry()?;
    let document = registry
        .to_json(&compiled, &fixed_assembly()?)
        .map_err(|error| format!("cannot encode forced coding owner: {error}"))?;
    let reopened = registry
        .from_json(&document)
        .map_err(|error| format!("cannot strictly reopen forced coding owner: {error}"))?;
    if registry
        .to_json(&reopened, &fixed_assembly()?)
        .map_err(|error| format!("cannot re-encode forced coding owner: {error}"))?
        != document
    {
        return Err("forced coding owner is not canonically stable".to_owned());
    }
    let ids: Vec<_> = reopened.section_ids().collect();
    if ids != [CodingTuning::ID.as_str()] {
        return Err(format!("forced coding owner has section IDs {ids:?}"));
    }
    let content_sha256 = reopened
        .verified_assembly()
        .ok_or("forced coding owner lacks verified assembly")?
        .content_sha256
        .as_str()
        .to_owned();
    let value: serde_json::Value =
        serde_json::from_str(&document).map_err(|error| error.to_string())?;
    let wrapper = canonical(
        value
            .get("sections")
            .and_then(|sections| sections.get(CodingTuning::ID.as_str()))
            .ok_or("forced coding owner lacks its exact section wrapper")?,
    )?;
    let evidence = ProfileEvidence {
        profile_id: FORCED_PROFILE_ID.to_owned(),
        owner_section_id: CodingTuning::ID.as_str().to_owned(),
        section_schema_version: CodingTuningCodec::SCHEMA_VERSION,
        harness_schema: CodingTuningCodec::HARNESS_SCHEMA.to_owned(),
        resolution: "installed".to_owned(),
        active_decode: selectors.into(),
        envelope_sha256: sha256(document.as_bytes()),
        content_sha256,
        wrapper_sha256: sha256(wrapper.as_bytes()),
    };
    Ok((reopened, evidence))
}

/// The frames of fixture bank `bank`: a batch of another size on the same
/// field and code is a prefix or an extension of it.
fn bank_frames(field: Field, stratum: &Stratum, frame_len: usize, bank: usize) -> Vec<Vec<Llr>> {
    let spec = &CODES[stratum.code];
    let mut rng = StdRng::seed_from_u64(bank_seed(field, stratum.code, bank));
    (0..stratum.frames)
        .map(|frame| {
            let scale = SIGMA_SCALE_MILLI[frame % SIGMA_SCALE_MILLI.len()];
            let sigma = f64::from(spec.noise_sigma_milli) * f64::from(scale) / 1e6;
            let channel = AwgnChannel::from_variance(sigma * sigma);
            let gain = 2.0 / channel.variance();
            (0..frame_len)
                .map(|_| Llr::new((gain * channel.transmit(1.0, &mut rng)) as f32))
                .collect()
        })
        .collect()
}

fn bank_digest(stratum: &Stratum, bank: usize, frames: &[Vec<Llr>]) -> String {
    let mut hash = Sha256Hasher::new();
    hash.update(b"gf2-coding-decode-fixture-v1\0");
    hash.update(CODES[stratum.code].name.as_bytes());
    hash.update([0]);
    hash.update((bank as u64).to_le_bytes());
    hash.update((frames.len() as u64).to_le_bytes());
    for frame in frames {
        hash.update((frame.len() as u64).to_le_bytes());
        for llr in frame {
            hash.update(llr.value().to_bits().to_le_bytes());
        }
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

fn operands_digest(bank_digests: &[String]) -> String {
    tuple_digest(b"gf2-coding-decode-operands-v1", bank_digests)
}

fn results_digest(results: &[(BitVec, DecodeOutcome)]) -> String {
    let mut hash = Sha256Hasher::new();
    hash.update(b"gf2-coding-decode-output-v1\0");
    hash.update((results.len() as u64).to_le_bytes());
    for (word, outcome) in results {
        hash.update((word.len() as u64).to_le_bytes());
        for limb in word.words() {
            hash.update(limb.to_le_bytes());
        }
        hash.update((outcome.iterations as u64).to_le_bytes());
        hash.update([
            u8::from(outcome.converged),
            u8::from(outcome.syndrome_check_passed),
        ]);
    }
    format!("{:x}", hash.finalize())
}

/// The operand digest of a field's stratum, regenerated from the seed
/// derivation and kept for the later results of the same stratum.
fn expected_operands_digest(field: Field, stratum: &Stratum) -> Result<String, String> {
    static KNOWN: Mutex<BTreeMap<(Field, usize, usize), String>> = Mutex::new(BTreeMap::new());
    let key = (field, stratum.code, stratum.frames);
    let mut known = KNOWN
        .lock()
        .map_err(|_| "operand digest cache is poisoned")?;
    if let Some(digest) = known.get(&key) {
        return Ok(digest.clone());
    }
    let frame_len = codes()?[stratum.code].frame_len;
    let digests: Vec<_> = (0..FIXTURE_BANKS)
        .map(|bank| bank_digest(stratum, bank, &bank_frames(field, stratum, frame_len, bank)))
        .collect();
    let digest = operands_digest(&digests);
    known.insert(key, digest.clone());
    Ok(digest)
}

fn require_measurement_environment() -> Result<(), String> {
    for (name, expected) in measurement_environment() {
        if env::var(&name).as_deref() != Ok(expected.as_str()) {
            return Err(format!(
                "the measurement environment requires {name}={expected}"
            ));
        }
    }
    Ok(())
}

/// The public batch entry point of a case, built under the installed section.
enum Entry {
    Direct(Box<LdpcBatchDecoder>),
    Pooled(PooledBatchDecoder<LdpcBatchDecoder>),
}

impl Entry {
    fn decode(
        &mut self,
        frames: &[&[Llr]],
        decoded: &mut [BitVec],
        outcomes: &mut [DecodeOutcome],
    ) {
        match self {
            Self::Direct(decoder) => decoder.decode_batch(frames, decoded, outcomes),
            Self::Pooled(decoder) => decoder.decode_batch(frames, decoded, outcomes),
        }
    }
}

struct Observed {
    schedule: Schedule,
    workspace_bytes: usize,
    batch: Vec<(BitVec, DecodeOutcome)>,
    samples: Vec<TimingSample>,
}

/// Installs the case's selectors, holds each batch result to the single-frame
/// decoder and, for a timed task, times whole batch decode calls on one
/// decoder and one set of decoded words.
fn execute_case(case: CodingCase, case_json: CanonicalJson) -> Result<ChildResult, String> {
    validate_case(&case)?;
    require_measurement_environment()?;
    let (field, stratum) = (case.field, &case.stratum);
    let selectors = field.selectors(stratum, case.candidate)?;
    let (prepared, profile_evidence) = forced_profile(&selectors)?;
    tuning::install(prepared)
        .map_err(|error| format!("cannot install forced coding owner: {error}"))?;
    let active = gf2_coding::tuning::active();
    match active.resolution {
        SectionResolution::Installed {
            profile_id,
            section_id,
            measurement: MeasurementProvenance::Inherited,
        } if profile_id.as_str() == FORCED_PROFILE_ID
            && section_id == CodingTuning::ID
            && active.section.decode() == &selectors => {}
        _ => return Err("forced coding owner did not resolve as exact Installed".to_owned()),
    }

    let built = &codes()?[stratum.code];
    let fixtures: Vec<_> = (0..FIXTURE_BANKS)
        .map(|bank| bank_frames(field, stratum, built.frame_len, bank))
        .collect();
    let bank_digests: Vec<_> = fixtures
        .iter()
        .enumerate()
        .map(|(bank, frames)| bank_digest(stratum, bank, frames))
        .collect();
    let mut reference = LdpcDecoder::with_config(built.code.clone(), decoder_config());
    let mut single = Vec::with_capacity(FIXTURE_BANKS * stratum.frames);
    for frame in fixtures.iter().flatten() {
        let mut word = BitVec::new();
        let outcome = reference.decode_codeword_into(frame, MAX_ITERATIONS, &mut word);
        single.push((word, outcome));
    }

    let expected = field.schedule(stratum, case.candidate)?;
    let case_sha256 = case_json.digest();
    let observed = run_in_dedicated_parallel_pool(POOL_THREADS, || {
        if rayon::current_num_threads() != POOL_THREADS {
            return Err("dedicated pool does not have the declared width".to_owned());
        }
        let new_decoder = || {
            LdpcBatchDecoder::new(
                built.code.clone(),
                decoder_config(),
                MAX_ITERATIONS,
                DecodedWord::Codeword,
            )
        };
        let direct = new_decoder();
        if direct.selectors() != &selectors {
            return Err("batch decoder did not read the installed selectors".to_owned());
        }
        let route = direct.batch_route(stratum.frames);
        let workspace_bytes = direct.workspace_bytes();
        let (mut entry, task_frames) = match field {
            Field::GranulesPerTask => {
                let pool = PooledBatchDecoder::new(new_decoder);
                if pool.workers() != POOL_THREADS {
                    return Err("pooled decoder does not hold one decoder per worker".to_owned());
                }
                let task_frames = pool.task_frames();
                (Entry::Pooled(pool), task_frames)
            }
            Field::LaneMinFrames | Field::LaneMaxEdges => {
                (Entry::Direct(Box::new(direct)), stratum.frames)
            }
        };
        let schedule = Schedule::new(route, task_frames, stratum.frames);
        if schedule != expected {
            return Err(format!(
                "public batch decode took schedule {schedule:?}, the selectors give {expected:?}"
            ));
        }

        let views: Vec<Vec<&[Llr]>> = fixtures
            .iter()
            .map(|frames| frames.iter().map(Vec::as_slice).collect())
            .collect();
        let mut decoded = vec![BitVec::new(); stratum.frames];
        let mut outcomes = vec![DecodeOutcome::default(); stratum.frames];
        let mut batch = Vec::with_capacity(single.len());
        for frames in &views {
            entry.decode(frames, &mut decoded, &mut outcomes);
            batch.extend(decoded.iter().cloned().zip(outcomes.iter().copied()));
        }
        if batch != single {
            return Err("batch decode disagrees with the single-frame decoder".to_owned());
        }
        let samples = match case.identity.task {
            Task::Probe => Vec::new(),
            Task::Measure { .. } => {
                let mut body = |bank: usize| {
                    entry.decode(&views[bank], &mut decoded, &mut outcomes);
                    black_box(&outcomes);
                };
                execution_with_progress(&case.identity, &case_sha256, &mut body, &mut io::stderr())
                    .map_err(|error| format!("timing/progress failed: {error}"))?
            }
        };
        Ok(Observed {
            schedule,
            workspace_bytes,
            batch,
            samples,
        })
    })?;

    let payload = CodingPayload {
        schema: PAYLOAD_SCHEMA.to_owned(),
        field,
        stratum_index: case.stratum_index,
        stratum: stratum.clone(),
        candidate: case.candidate,
        schedule: observed.schedule,
        code: CodeEvidence {
            name: case.code.clone(),
            frame_len: built.frame_len,
            edges: built.edges,
            workspace_bytes: observed.workspace_bytes,
        },
        profile: profile_evidence,
        capability: CapabilityEvidence {
            required_features: FEATURE_CONTRACT.to_owned(),
            pool_threads: POOL_THREADS,
            lane_backend: backend_name(LaneBackend::detected()).to_owned(),
            entry: field.entry().to_owned(),
        },
        semantics: SemanticEvidence {
            fixture_count: fixtures.len(),
            frames_per_fixture: stratum.frames,
            operands_sha256: operands_digest(&bank_digests),
            batch_results_sha256: results_digest(&observed.batch),
            single_frame_results_sha256: results_digest(&single),
            single_frame_equal: observed.batch == single,
            converged_frames: single
                .iter()
                .filter(|(_, outcome)| outcome.converged)
                .count(),
            iterations: single.iter().map(|(_, outcome)| outcome.iterations).sum(),
        },
    };
    Ok(ChildResult {
        schema: RESULT_SCHEMA.to_owned(),
        identity: case.identity,
        case_sha256,
        outcome: ChildOutcome::Complete,
        samples: observed.samples,
        payload: canonical_json(&payload)?,
    })
}

fn run_fresh_child() -> Result<(), String> {
    let case: CodingCase =
        read_guarded_case(env::var(FRESH_CASE_VAR).ok().as_deref(), io::stdin().lock())?;
    let case_json = canonical_json(&case)?;
    let result = execute_case(case, case_json)?;
    write_result_line(io::stdout().lock(), &result)
        .map_err(|error| format!("cannot write child result: {error}"))
}

fn validate_result(unit: &LaunchUnit, result: &ChildResult) -> Result<(), String> {
    unit.validate().map_err(|error| error.to_string())?;
    let case: CodingCase = unit.case.decode().map_err(|error| error.to_string())?;
    validate_case(&case)?;
    if result.schema != RESULT_SCHEMA
        || case.identity != unit.identity
        || result.identity != unit.identity
        || result.case_sha256 != unit.case.digest()
        || result.outcome != ChildOutcome::Complete
    {
        return Err("coding result neutral identity does not match its unit".to_owned());
    }
    match case.identity.task {
        Task::Probe if !result.samples.is_empty() => {
            return Err("coding probe contains timing samples".to_owned());
        }
        Task::Measure { execution } => {
            if result.samples.len() as u64 != WINDOWS {
                return Err("coding timed result does not contain every window".to_owned());
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
                    "coding timing samples violate execution, repetition, or calibration order"
                        .to_owned(),
                );
            }
            execution_median(&result.samples, execution)
                .map_err(|error| format!("invalid coding timing samples: {error}"))?;
        }
        Task::Probe => {}
    }
    let payload: CodingPayload = result.payload.decode().map_err(|error| error.to_string())?;
    let (field, stratum) = (case.field, &case.stratum);
    let (_, expected_profile) = forced_profile(&field.selectors(stratum, case.candidate)?)?;
    let built = &codes()?[stratum.code];
    if payload.schema != PAYLOAD_SCHEMA
        || payload.field != field
        || payload.stratum_index != case.stratum_index
        || payload.stratum != *stratum
        || payload.candidate != case.candidate
        || payload.schedule != field.schedule(stratum, case.candidate)?
        || payload.code.name != case.code
        || payload.code.frame_len != built.frame_len
        || payload.code.edges != built.edges
        || payload.profile != expected_profile
        || payload.capability.required_features != FEATURE_CONTRACT
        || payload.capability.pool_threads != POOL_THREADS
        || payload.capability.lane_backend != backend_name(LaneBackend::detected())
        || payload.capability.entry != field.entry()
        || payload.semantics.fixture_count != FIXTURE_BANKS
        || payload.semantics.frames_per_fixture != stratum.frames
        || payload.semantics.operands_sha256 != expected_operands_digest(field, stratum)?
        || !payload.semantics.single_frame_equal
        || payload.semantics.batch_results_sha256 != payload.semantics.single_frame_results_sha256
        || Sha256Digest::new(payload.semantics.batch_results_sha256.clone()).is_err()
    {
        return Err("coding result fails owner semantic validation".to_owned());
    }
    Ok(())
}

fn result_sha256(result: &ChildResult) -> Result<Sha256Digest, String> {
    result.digest().map_err(|error| error.to_string())
}

/// Decides each field by the campaign's extent decision over its strata. A
/// field is emitted when the decision selects a candidate other than the
/// conservative value.
fn analyze_results(bundle: &AcceptedResultsBundle) -> Result<AnalysisOutput, String> {
    let order = acquisition_order()?;
    if bundle.schema != ACCEPTED_RESULTS_SCHEMA || bundle.accepted.len() != order.len() {
        return Err("coding owner input does not contain every accepted result".to_owned());
    }
    let campaign_id = bundle.accepted[0].unit.identity.campaign_id.clone();
    let first_case: CodingCase = bundle.accepted[0]
        .unit
        .case
        .decode()
        .map_err(|error| error.to_string())?;
    let mut by_key = BTreeMap::new();
    for (ordinal, (accepted, (field, stratum_index, candidate, task))) in
        bundle.accepted.iter().zip(order).enumerate()
    {
        let case: CodingCase = accepted
            .unit
            .case
            .decode()
            .map_err(|error| error.to_string())?;
        if accepted.unit.ordinal != ordinal as u64
            || by_key
                .insert(accepted.unit.key.as_str().to_owned(), accepted)
                .is_some()
            || accepted.checkpoint_sha256.as_str().len() != 64
            || accepted.unit.identity.campaign_id != campaign_id
            || case.protocol_sha256 != first_case.protocol_sha256
            || case.channels != first_case.channels
        {
            return Err("coding accepted-result order or identity is invalid".to_owned());
        }
        validate_manifest_unit(&accepted.unit, field, stratum_index, candidate, task)?;
        validate_result(&accepted.unit, &accepted.result)?;
    }

    let mut fields = Vec::with_capacity(Field::ALL.len());
    for field in Field::ALL {
        let strata = field.strata();
        let mut series = Vec::new();
        for candidate in field.candidates()? {
            let mut schedules = Vec::with_capacity(strata.len());
            let mut medians = Vec::with_capacity(strata.len());
            for stratum in &strata {
                schedules.push(field.schedule(stratum, candidate)?);
                let mut executions = Vec::with_capacity(EXECUTIONS as usize);
                for execution in 0..EXECUTIONS {
                    let task = Task::Measure { execution };
                    let key = cell_identity(&campaign_id, field, stratum, candidate, task)?
                        .key()
                        .map_err(|error| error.to_string())?;
                    let accepted: &AcceptedResult = by_key
                        .get(key.as_str())
                        .ok_or("coding owner input lacks a timed cell")?;
                    executions.push(
                        execution_median(&accepted.result.samples, execution)
                            .map_err(|error| error.to_string())?,
                    );
                }
                medians.push(executions);
            }
            series.push(CandidateSeries {
                candidate,
                schedules,
                strata: medians,
            });
        }
        let decision = analyze_extent(&series, &field.conservative()).map_err(|error| {
            format!("coding extent analysis of {} failed: {error}", field.name())
        })?;
        fields.push(FieldAnalysis {
            field: field.name(),
            selected: decision.selected,
            emitted: decision.reason == DecisionReason::SelectedNonDefault,
            decision,
        });
    }
    Ok(AnalysisOutput {
        schema: "coding-tuning-analysis-v1".to_owned(),
        fields,
        accepted_result_count: bundle.accepted.len(),
    })
}

impl AnalysisOutput {
    /// The fields the emitted section states, with their selected values.
    fn stated(&self) -> Result<Vec<(Field, usize)>, String> {
        let mut stated = Vec::new();
        for field in Field::ALL {
            let decided = self
                .fields
                .iter()
                .find(|decided| decided.field == field.name())
                .ok_or("coding analysis lacks a decode field")?;
            if decided.emitted {
                stated.push((field, decided.selected));
            }
        }
        Ok(stated)
    }
}

/// The section that states `stated` and nothing else, so every other
/// selector of the coding section keeps its conservative value.
fn emitted_section(stated: &[(Field, usize)]) -> Result<CodingTuning, String> {
    let family: BTreeMap<_, _> = stated
        .iter()
        .map(|(field, selected)| (field.key(), *selected))
        .collect();
    let mut body = BTreeMap::new();
    if !family.is_empty() {
        body.insert(Field::FAMILY, family);
    }
    let body = CanonicalValue::serialize(&body)
        .map_err(|error| format!("emitted coding body is not canonical: {error}"))?;
    CodingTuningCodec::decode_body(body)
        .map_err(|error| format!("emitted coding section is invalid: {error}"))
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
        .map_err(|error| format!("invalid coding measurement runtime: {error}"))?;
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
        .map_err(|error| format!("invalid coding measurement runtime: {error}"))?;
    let assembly_facts: AssemblyRuntime = assembly
        .runtime
        .decode()
        .map_err(|error| format!("invalid coding assembly runtime: {error}"))?;
    let case: CodingCase = bundle.accepted[0]
        .unit
        .case
        .decode()
        .map_err(|error| error.to_string())?;
    if !measurement.identity.resume_equivalent(&assembly.identity)
        || measurement.process.as_str() != PROCESS
        || assembly.process.as_str() != PROCESS
        || measurement.identity.feature_contract != FEATURE_CONTRACT
        || measurement.identity.thread_contract != THREAD_CONTRACT
        || measurement.identity.protocol_digest != case.protocol_sha256.as_str()
        || !measurement
            .identity
            .behavior_sha256
            .contains_key(HARNESS_PATH)
        || facts.toolchain.trim().is_empty()
        || !facts.toolchain.contains(TOOLCHAIN_CONTRACT)
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
        || assembly_facts.tool != HARNESS_PATH
    {
        return Err("coding observed provenance violates the producing contract".to_owned());
    }
    RepoRelPath::parse(&facts.receipt).map_err(|error| error.to_string())?;
    Rfc3339Utc::parse(&measurement.observed_utc).map_err(|error| error.to_string())?;
    Rfc3339Utc::parse(&assembly.observed_utc).map_err(|error| error.to_string())?;
    let binary = measurement
        .identity
        .executable_sha256
        .get(PROCESS)
        .ok_or("coding producing executable digest is absent")?;
    let actual = sha256(
        &fs::read(env::current_exe().map_err(|error| error.to_string())?)
            .map_err(|error| format!("cannot hash coding producer executable: {error}"))?,
    );
    if actual != *binary || assembly_facts.tool_sha256 != actual {
        return Err("coding owner emission executable differs from producing binary".to_owned());
    }
    Ok(())
}

fn assembly_provenance(request: &EmitOwnerRequest) -> Result<AssemblyProvenance, String> {
    let runtime: AssemblyRuntime = request
        .assembly
        .runtime
        .decode()
        .map_err(|error| format!("invalid coding assembly runtime: {error}"))?;
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
    section: CodingTuning,
    measurement: MeasurementProvenance,
    assembly: &AssemblyProvenance,
) -> Result<String, String> {
    let id = ProfileId::parse(profile_id).map_err(|error| error.to_string())?;
    let prepared = PreparedEnvelope::compiled(
        id.clone(),
        CompiledProfileProvenance {
            artifact_id: id.clone(),
        },
    )
    .insert_measured::<CodingTuning, CodingTuningCodec>(section, measurement)
    .map_err(|error| format!("cannot construct measured coding owner: {error}"))?
    .build()
    .map_err(|error| format!("cannot build measured coding owner: {error}"))?;
    coding_registry()?
        .to_json(&prepared, assembly)
        .map_err(|error| format!("cannot encode measured coding owner: {error}"))
}

fn emit_owner(request: EmitOwnerRequest) -> Result<(ArtifactIdentity, AnalysisOutput), String> {
    if request.output.exists() {
        return Err("coding owner output destination already exists".to_owned());
    }
    if !request.output.is_absolute() {
        return Err("coding owner output destination is not absolute".to_owned());
    }
    let bundle: AcceptedResultsBundle = request
        .accepted_results
        .read()
        .map_err(|error| format!("cannot reopen accepted coding results: {error}"))?;
    if bundle.manifest_sha256 != request.manifest_sha256 {
        return Err("coding owner input has the wrong manifest digest".to_owned());
    }
    if bundle
        .accepted
        .iter()
        .any(|accepted| accepted.unit.identity.campaign_id != request.campaign_id)
    {
        return Err("coding owner input has the wrong campaign identity".to_owned());
    }
    let analysis = analyze_results(&bundle)?;
    validate_observed_provenance(&request, &bundle)?;
    let registry = coding_registry()?;
    let section = emitted_section(&analysis.stated()?)?;
    let measurement = measurement_provenance(&request)?;
    let assembly = assembly_provenance(&request)?;
    let document = measured_owner_document(
        request.campaign_id.as_str(),
        section.clone(),
        measurement.clone(),
        &assembly,
    )?;
    atomic_write_new(&request.output, document.as_bytes())
        .map_err(|error| format!("cannot atomically publish coding owner: {error}"))?;
    let canonical_path = fs::canonicalize(&request.output)
        .map_err(|error| format!("cannot canonicalize coding owner: {error}"))?;
    let reopened_bytes = fs::read(&canonical_path)
        .map_err(|error| format!("cannot reopen coding owner bytes: {error}"))?;
    if reopened_bytes != document.as_bytes() {
        return Err("reopened coding owner bytes differ from emitted bytes".to_owned());
    }
    let reopened = registry
        .from_json(&document)
        .map_err(|error| format!("emitted coding owner fails strict reopen: {error}"))?;
    let projection = reopened
        .section::<CodingTuning>()
        .map_err(|error| error.to_string())?
        .ok_or("emitted coding owner lacks its section")?;
    if reopened.section_ids().collect::<Vec<_>>() != [CodingTuning::ID.as_str()]
        || reopened.profile_id().as_str() != request.campaign_id.as_str()
        || projection.section != &section
        || projection.measurement != &measurement
        || reopened
            .verified_assembly()
            .ok_or("emitted coding owner lacks verified assembly")?
            .provenance
            != assembly
        || registry
            .to_json(&reopened, &assembly)
            .map_err(|error| format!("cannot re-encode emitted coding owner: {error}"))?
            != document
    {
        return Err("emitted coding owner does not round-trip through its codec".to_owned());
    }
    Ok((
        ArtifactIdentity {
            path: canonical_path,
            sha256: Sha256Digest::of(&reopened_bytes),
        },
        analysis,
    ))
}

fn report(
    mode: &str,
    scope: &str,
    details: Vec<String>,
    inventory: Vec<InventoryEntry>,
) -> Result<ReportOutput, String> {
    let counts = declared_counts()?;
    Ok(ReportOutput {
        schema: "coding-tuning-owner-report-v1".to_owned(),
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
        inventory,
    })
}

fn codec_inventory() -> Result<Vec<String>, String> {
    let body = CodingTuningCodec::encode_body(&CodingTuning::CONSERVATIVE)
        .map_err(|error| format!("cannot encode coding inventory: {error}"))?;
    let value = serde_json::to_value(body)
        .map_err(|error| format!("cannot project coding inventory: {error}"))?;
    let mut fields = Vec::new();
    let families = value
        .as_object()
        .ok_or("coding selector inventory is not an object")?;
    for (family, members) in families {
        let members = members
            .as_object()
            .ok_or("coding selector family is not an object")?;
        for field in members.keys() {
            fields.push(format!("{family}.{field}"));
        }
    }
    Ok(fields)
}

/// Checks the declaration without decoding a frame: the codec inventory, the
/// grid, a forced profile, and the codec round trip of every emission.
fn self_check() -> Result<ReportOutput, String> {
    let inventory = codec_inventory()?;
    for field in Field::ALL {
        if !inventory.contains(&field.name()) {
            return Err(format!("coding codec inventory lacks {}", field.name()));
        }
        let candidates = field.candidates()?;
        let conservative = field.conservative();
        if candidates.windows(2).any(|pair| pair[0] >= pair[1])
            || candidates.iter().filter(|c| **c == conservative).count() != 1
        {
            return Err(format!(
                "{} candidates are not an extent grid",
                field.name()
            ));
        }
        for stratum in field.strata() {
            for candidate in &candidates {
                forced_profile(&field.selectors(&stratum, *candidate)?)?;
                field.schedule(&stratum, *candidate)?;
            }
        }
    }
    let registry = coding_registry()?;
    let mut nonconservative = Vec::with_capacity(Field::ALL.len());
    for field in Field::ALL {
        let candidate = field
            .candidates()?
            .into_iter()
            .find(|candidate| *candidate != field.conservative())
            .ok_or("a field has no candidate beside its conservative value")?;
        nonconservative.push((field, candidate));
    }
    let subsets = 1_usize << Field::ALL.len();
    for subset in 0..subsets {
        let stated: Vec<_> = nonconservative
            .iter()
            .enumerate()
            .filter(|(bit, _)| subset >> bit & 1 == 1)
            .map(|(_, stated)| *stated)
            .collect();
        let section = emitted_section(&stated)?;
        let id = ProfileId::parse(FORCED_PROFILE_ID).map_err(|error| error.to_string())?;
        let prepared =
            PreparedEnvelope::compiled(id.clone(), CompiledProfileProvenance { artifact_id: id })
                .insert(section.clone())
                .map_err(|error| error.to_string())?
                .build()
                .map_err(|error| error.to_string())?;
        let document = registry
            .to_json(&prepared, &fixed_assembly()?)
            .map_err(|error| format!("cannot encode an emission: {error}"))?;
        let reopened = registry
            .from_json(&document)
            .map_err(|error| format!("an emission fails strict reopen: {error}"))?;
        let projection = reopened
            .section::<CodingTuning>()
            .map_err(|error| error.to_string())?
            .ok_or("an emission lacks its section")?;
        if projection.section != &section {
            return Err("an emission does not round-trip through the codec".to_owned());
        }
    }
    report(
        "self-check",
        "complete-declaration-no-measurement",
        vec![
            format!("codec-inventory={}/{}", Field::ALL.len(), inventory.len()),
            format!("emission-round-trips={subsets}/{subsets}"),
            "profile-installation=not-performed".to_owned(),
        ],
        Vec::new(),
    )
}

fn calibration_inventory() -> Result<Vec<InventoryEntry>, String> {
    let built = codes()?;
    let mut inventory: Vec<InventoryEntry> = Vec::new();
    for field in Field::ALL {
        for stratum in field.strata() {
            let code = CODES[stratum.code].name;
            let existing = inventory
                .iter_mut()
                .find(|entry| entry.field == field.name() && entry.code == code);
            match existing {
                Some(entry) => {
                    if !entry.batch_frames.contains(&stratum.frames) {
                        entry.batch_frames.push(stratum.frames);
                    }
                }
                None => inventory.push(InventoryEntry {
                    field: field.name(),
                    code: code.to_owned(),
                    frame_len: built[stratum.code].frame_len,
                    edges: built[stratum.code].edges,
                    noise_sigma_milli: CODES[stratum.code].noise_sigma_milli,
                    sigma_scale_milli: SIGMA_SCALE_MILLI.to_vec(),
                    max_iterations: MAX_ITERATIONS,
                    batch_frames: vec![stratum.frames],
                    seeds: seed_inventory(field, stratum.code),
                }),
            }
        }
    }
    Ok(inventory)
}

fn list_grid() -> Result<ReportOutput, String> {
    let mut details = Vec::new();
    for field in Field::ALL {
        details.push(format!(
            "{}: candidates={:?} conservative={}",
            field.name(),
            field.candidates()?,
            field.conservative()
        ));
        for stratum in field.strata() {
            details.push(format!(
                "{}: stratum={} entry={}",
                field.name(),
                stratum.token()?.as_str(),
                field.entry()
            ));
        }
    }
    details.push(format!("decoder-config={:?}", decoder_config()));
    details.push("codeword=all-zero channel=bpsk-awgn".to_owned());
    details.push("profile-installation=not-performed".to_owned());
    report(
        "list-grid",
        "full-grid-no-measurement",
        details,
        calibration_inventory()?,
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
            format!("lane-backend={}", backend_name(LaneBackend::detected())),
            format!("lanes={LANES}"),
            format!("required-features={FEATURE_CONTRACT}"),
            "full-grid-checked=false".to_owned(),
            "profile-installation=not-performed".to_owned(),
        ],
        Vec::new(),
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
            Err("the coding owner has no conditionally derived manifest".to_owned())
        }
        OwnerOperation::EmitOwner { request } => {
            require_measurement_environment()?;
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
    let respond = |response: OwnerResponse| {
        write_result_line(io::stdout().lock(), &response)
            .map_err(|error| format!("cannot write owner response: {error}"))
    };
    match arguments.as_slice() {
        [mode] if mode == "--fresh-child" => run_fresh_child(),
        [mode] if mode == "--owner-operation" => {
            respond(owner_operation(decode_owner_operation(&read_stdin()?)?)?)
        }
        [mode] if mode == "--self-check" => respond(owner_operation(OwnerOperation::SelfCheck)?),
        [mode] if mode == "--list-grid" => respond(owner_operation(OwnerOperation::ListGrid)?),
        [mode] if mode == "--capability-report" => {
            respond(owner_operation(OwnerOperation::CapabilityReport)?)
        }
        _ => Err(
            "usage: tuning_calibration (--owner-operation|--fresh-child|--self-check|--list-grid|--capability-report)"
                .to_owned(),
        ),
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("coding tuning producer failed: {error}");
        std::process::exit(1);
    }
}

// A `harness = false` bench target compiles this module without running it.
#[cfg(test)]
#[allow(dead_code, unused_imports)]
mod tests {
    use super::*;
    use gf2_core::test_scratch::{scratch, Scratch};
    use std::process::{Command, Stdio};
    use tuning_campaign_support::campaign::{
        ObservedProvenance, ProcessDescriptor, SessionChannels,
    };
    use tuning_campaign_support::journal::ResumeIdentity;
    use tuning_campaign_support::transport::{parse_result, retain_libtest_result_lines};

    const ZERO_SHA: &str = "0000000000000000000000000000000000000000000000000000000000000000";
    const ZERO_REV: &str = "0000000000000000000000000000000000000000";
    /// A campaign ID in the form the extent-campaign launcher mints.
    const LAUNCHER_CAMPAIGN_ID: &str = "gf2-00000000-19700101t000000z-1";
    const PROBE_VAR: &str = "GF2_CODING_TUNING_PROBE";

    fn test_process() -> ProcessDescriptor {
        let executable = fs::canonicalize(env::current_exe().unwrap()).unwrap();
        ProcessDescriptor {
            id: token(PROCESS).unwrap(),
            executable_sha256: Sha256Digest::of(&fs::read(&executable).unwrap()),
            executable,
            arguments: vec!["--fresh-child".to_owned()],
            environment: measurement_environment(),
            working_directory: fs::canonicalize(env::current_dir().unwrap()).unwrap(),
        }
    }

    fn manifest_request(label: &str) -> (Scratch, ManifestRequest) {
        let directory = scratch(&format!("gf2-coding-tuning-{label}"));
        let stage = directory.path().to_path_buf();
        let request = ManifestRequest {
            campaign_id: token(LAUNCHER_CAMPAIGN_ID).unwrap(),
            protocol_sha256: Sha256Digest::new(ZERO_SHA).unwrap(),
            channels: SessionChannels {
                execution_log: stage.join("execution.log"),
                checkpoints: stage.join("checkpoints"),
                stage,
            },
            processes: vec![test_process()],
        };
        (directory, request)
    }

    fn cases(manifest: &OwnerManifest) -> Vec<CodingCase> {
        manifest
            .ordered_units
            .iter()
            .map(|unit| unit.case.decode().unwrap())
            .collect()
    }

    /// Synthesized cost of a field's candidates, the same in every stratum
    /// and execution.
    #[derive(Clone, Copy)]
    enum Curve {
        /// Cheapest at the candidate of this rank, rising with rank distance.
        MinimumAt(usize),
        /// One cost for every candidate.
        Flat,
    }

    impl Curve {
        fn at_conservative(field: Field) -> Self {
            Self::MinimumAt(rank(field, field.conservative()))
        }

        fn cost(self, rank: usize) -> u64 {
            match self {
                Self::MinimumAt(minimum) => 100 + 10 * rank.abs_diff(minimum) as u64,
                Self::Flat => 100,
            }
        }
    }

    fn rank(field: Field, candidate: usize) -> usize {
        field
            .candidates()
            .unwrap()
            .iter()
            .position(|declared| *declared == candidate)
            .unwrap()
    }

    fn synthetic_result(unit: &LaunchUnit, curve: impl Fn(Field) -> Curve) -> ChildResult {
        let case: CodingCase = unit.case.decode().unwrap();
        let (field, stratum) = (case.field, &case.stratum);
        let cost = curve(field).cost(rank(field, case.candidate));
        let samples = match case.identity.task {
            Task::Probe => Vec::new(),
            Task::Measure { execution } => (0..WINDOWS)
                .map(|repetition| TimingSample::new(execution, repetition, 1, cost).unwrap())
                .collect(),
        };
        let selectors = field.selectors(stratum, case.candidate).unwrap();
        let built = &codes().unwrap()[stratum.code];
        let payload = CodingPayload {
            schema: PAYLOAD_SCHEMA.to_owned(),
            field,
            stratum_index: case.stratum_index,
            stratum: stratum.clone(),
            candidate: case.candidate,
            schedule: field.schedule(stratum, case.candidate).unwrap(),
            code: CodeEvidence {
                name: case.code.clone(),
                frame_len: built.frame_len,
                edges: built.edges,
                workspace_bytes: 0,
            },
            profile: forced_profile(&selectors).unwrap().1,
            capability: CapabilityEvidence {
                required_features: FEATURE_CONTRACT.to_owned(),
                pool_threads: POOL_THREADS,
                lane_backend: backend_name(LaneBackend::detected()).to_owned(),
                entry: field.entry().to_owned(),
            },
            semantics: SemanticEvidence {
                fixture_count: FIXTURE_BANKS,
                frames_per_fixture: stratum.frames,
                operands_sha256: expected_operands_digest(field, stratum).unwrap(),
                batch_results_sha256: ZERO_SHA.to_owned(),
                single_frame_results_sha256: ZERO_SHA.to_owned(),
                single_frame_equal: true,
                converged_frames: 0,
                iterations: 0,
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

    fn synthetic_bundle(
        manifest: &OwnerManifest,
        curve: impl Fn(Field) -> Curve,
    ) -> AcceptedResultsBundle {
        AcceptedResultsBundle {
            schema: ACCEPTED_RESULTS_SCHEMA.to_owned(),
            manifest_sha256: manifest.manifest_sha256.clone(),
            accepted: manifest
                .ordered_units
                .iter()
                .map(|unit| AcceptedResult {
                    unit: unit.clone(),
                    result: synthetic_result(unit, &curve),
                    checkpoint_sha256: Sha256Digest::of(unit.key.as_str().as_bytes()),
                })
                .collect(),
        }
    }

    #[test]
    fn the_manifest_follows_the_declared_grid_under_the_codec_token() {
        let (_scratch, request) = manifest_request("grid");
        let manifest = campaign_manifest(request).unwrap();
        validate_manifest_order(&manifest).unwrap();
        assert_eq!(
            manifest.behavior_token.as_str(),
            CodingTuningCodec::HARNESS_SCHEMA
        );
        assert_eq!(manifest.owner.as_str(), OWNER);
        assert_eq!(manifest.counts, declared_counts().unwrap());
        assert_eq!(
            manifest.ordered_units.len() as u64,
            manifest.counts.accepted_results
        );
        assert_eq!(manifest.processes[0].environment, measurement_environment());

        let cases = cases(&manifest);
        for field in Field::ALL {
            assert!(cases.iter().any(|case| case.field == field), "{field:?}");
        }
        // Units of one field and code share their input streams.
        for pair in cases.windows(2) {
            if pair[0].field == pair[1].field && pair[0].code == pair[1].code {
                assert_eq!(pair[0].seeds, pair[1].seeds);
            }
        }
    }

    #[test]
    fn each_field_has_its_own_grid_axis() {
        let occupied: Vec<_> = Field::LaneMinFrames
            .strata()
            .iter()
            .map(|stratum| stratum.frames)
            .collect();
        assert!(occupied.windows(2).all(|pair| pair[0] < pair[1]));
        assert_eq!(occupied.last(), Some(&LANES));

        let edges: Vec<_> = Field::LaneMaxEdges
            .strata()
            .iter()
            .map(|stratum| codes().unwrap()[stratum.code].edges)
            .collect();
        assert!(edges.len() > 1 && edges.windows(2).all(|pair| pair[0] < pair[1]));

        for stratum in Field::GranulesPerTask.strata() {
            let tasks: Vec<_> = Field::GranulesPerTask
                .candidates()
                .unwrap()
                .into_iter()
                .map(|granules| {
                    Field::GranulesPerTask
                        .schedule(&stratum, granules)
                        .unwrap()
                        .tasks
                })
                .collect();
            assert!(tasks.windows(2).all(|pair| pair[0] > pair[1]), "{tasks:?}");
        }
    }

    #[test]
    fn conservative_lane_bounds_route_every_stratum_to_the_single_frame_decoder() {
        for field in [Field::LaneMinFrames, Field::LaneMaxEdges] {
            let candidates = field.candidates().unwrap();
            for stratum in field.strata() {
                let routes: Vec<_> = candidates
                    .iter()
                    .map(|candidate| field.schedule(&stratum, *candidate).unwrap().route)
                    .collect();
                let conservative = &routes[rank(field, field.conservative())];
                assert_eq!(conservative, route_name(BatchRoute::SingleFrame));
                let lane = route_name(BatchRoute::Lane(LaneBackend::detected()));
                assert!(routes.iter().any(|route| route == lane), "{stratum:?}");
            }
        }
    }

    #[test]
    fn the_self_check_and_the_grid_listing_run_without_measuring() {
        let checked = self_check().unwrap();
        let listed = list_grid().unwrap();
        for report in [&checked, &listed] {
            assert_eq!(report.behavior_token, CodingTuningCodec::HARNESS_SCHEMA);
            assert_eq!(report.cells, declared_cells().unwrap());
            assert!(report
                .details
                .contains(&"profile-installation=not-performed".to_owned()));
        }

        // The listing states the code, seeds and batch size of every case.
        let (_scratch, request) = manifest_request("inventory");
        for case in cases(&campaign_manifest(request).unwrap()) {
            let entry = listed
                .inventory
                .iter()
                .find(|entry| entry.field == case.field.name() && entry.code == case.code)
                .unwrap();
            assert_eq!(entry.seeds, case.seeds);
            assert!(entry.batch_frames.contains(&case.stratum.frames));
            assert_eq!(entry.max_iterations, case.max_iterations);
        }
    }

    fn probe_case(field: Field, stratum_index: usize, candidate: usize) -> CodingCase {
        let stage = env::temp_dir().join("gf2-coding-tuning-probe");
        let request = ManifestRequest {
            campaign_id: token("coding-probe").unwrap(),
            protocol_sha256: Sha256Digest::new(ZERO_SHA).unwrap(),
            channels: SessionChannels {
                execution_log: stage.join("execution.log"),
                checkpoints: stage.join("checkpoints"),
                stage,
            },
            processes: Vec::new(),
        };
        coding_case(&request, field, stratum_index, candidate, Task::Probe).unwrap()
    }

    /// Probe cases by name: a full wave under an open and under the
    /// conservative `lane_min_frames`, and a pooled batch.
    fn probe_cases() -> [(&'static str, CodingCase); 3] {
        let full_wave = Field::LaneMinFrames.strata().len() - 1;
        let pooled = Field::GranulesPerTask
            .strata()
            .iter()
            .position(|stratum| stratum.code == 0 && stratum.open_lanes)
            .unwrap();
        [
            (
                "lane",
                probe_case(Field::LaneMinFrames, full_wave, OCCUPIED_LANES[0]),
            ),
            (
                "single-frame",
                probe_case(Field::LaneMinFrames, full_wave, LANE_MIN_FRAMES),
            ),
            (
                "pooled",
                probe_case(Field::GranulesPerTask, pooled, TASK_GRANULES[1]),
            ),
        ]
    }

    #[test]
    fn an_unguarded_process_refuses_to_install_tuning() {
        let (_, case) = probe_cases().into_iter().next().unwrap();
        let case_json = canonical_json(&case).unwrap();
        let error = execute_case(case, case_json).unwrap_err();
        assert!(error.contains("measurement environment"), "{error}");
    }

    #[test]
    fn guarded_probe_children_install_the_candidate_and_decode_on_its_route() {
        for (name, case) in probe_cases() {
            let output = Command::new(env::current_exe().unwrap())
                .args(["probe_child_entry", "--nocapture"])
                .envs(measurement_environment())
                .env(PROBE_VAR, name)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{name} probe child failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let stdout = String::from_utf8(output.stdout).unwrap();
            let result: ChildResult = parse_result(&retain_libtest_result_lines(&stdout)).unwrap();
            let unit = LaunchUnit::new(
                0,
                case.identity.clone(),
                token(PROCESS).unwrap(),
                canonical_json(&case).unwrap(),
            )
            .unwrap();
            validate_result(&unit, &result).unwrap();

            let payload: CodingPayload = result.payload.decode().unwrap();
            let selectors = case.field.selectors(&case.stratum, case.candidate).unwrap();
            assert!(result.samples.is_empty(), "{name}");
            assert_eq!(payload.profile.resolution, "installed", "{name}");
            assert_eq!(payload.profile.active_decode, (&selectors).into(), "{name}");
            let route = match name {
                "single-frame" => BatchRoute::SingleFrame,
                _ => BatchRoute::Lane(LaneBackend::detected()),
            };
            assert_eq!(payload.schedule.route, route_name(route), "{name}");
            assert!(payload.semantics.single_frame_equal, "{name}");
        }
    }

    #[test]
    fn probe_child_entry() {
        let Ok(requested) = env::var(PROBE_VAR) else {
            return;
        };
        let (_, case) = probe_cases()
            .into_iter()
            .find(|(name, _)| *name == requested)
            .unwrap();
        let case_json = canonical_json(&case).unwrap();
        let result = execute_case(case, case_json).unwrap();
        write_result_line(io::stdout().lock(), &result).unwrap();
    }

    #[test]
    fn a_campaign_id_the_profile_loader_rejects_is_refused() {
        let (_scratch, request) = manifest_request("uppercase-id");
        let error = campaign_manifest(ManifestRequest {
            campaign_id: token("gf2-00000000-19700101T000000Z-1").unwrap(),
            ..request
        })
        .unwrap_err();
        assert!(error.contains("tuning profile ID"), "{error}");
    }

    #[test]
    fn owner_validation_rejects_changed_cases_and_results() {
        let (_scratch, request) = manifest_request("reject");
        let manifest = campaign_manifest(request).unwrap();
        let unit = manifest
            .ordered_units
            .iter()
            .find(|unit| unit.identity.task == Task::Measure { execution: 0 })
            .unwrap();
        let curve = Curve::at_conservative;

        let mut case: CodingCase = unit.case.decode().unwrap();
        case.candidate = usize::MAX - 1;
        assert!(validate_case(&case).is_err());
        let mut value: serde_json::Value = serde_json::from_str(unit.case.as_str()).unwrap();
        value["unknown"] = serde_json::json!(true);
        let unknown = CanonicalJson::new(serde_json::to_string(&value).unwrap()).unwrap();
        assert!(unknown.decode::<CodingCase>().is_err());

        let valid = synthetic_result(unit, curve);
        validate_result(unit, &valid).unwrap();

        let mut other_candidate = valid.clone();
        other_candidate.identity.candidate = token("v3").unwrap();
        assert!(validate_result(unit, &other_candidate).is_err());

        let mut reordered = valid.clone();
        reordered.samples.swap(0, 1);
        assert!(validate_result(unit, &reordered).is_err());

        let mut changed_calls = valid.clone();
        changed_calls.samples[2].calls = 2;
        assert!(validate_result(unit, &changed_calls).is_err());

        let mut other_route = valid.clone();
        let mut payload: CodingPayload = other_route.payload.decode().unwrap();
        payload.schedule.tasks += 1;
        other_route.payload = canonical_json(&payload).unwrap();
        assert!(validate_result(unit, &other_route).is_err());

        let mut unequal = valid;
        let mut payload: CodingPayload = unequal.payload.decode().unwrap();
        payload.semantics.single_frame_equal = false;
        unequal.payload = canonical_json(&payload).unwrap();
        assert!(validate_result(unit, &unequal).is_err());
    }

    fn resume_identity() -> ResumeIdentity {
        let executable = sha256(&fs::read(env::current_exe().unwrap()).unwrap());
        ResumeIdentity {
            protocol_digest: ZERO_SHA.to_owned(),
            source_revision: ZERO_REV.to_owned(),
            source_sha256: ZERO_SHA.to_owned(),
            ordered_work_manifest_sha256: ZERO_SHA.to_owned(),
            process_descriptors_sha256: ZERO_SHA.to_owned(),
            executable_sha256: BTreeMap::from([(PROCESS.to_owned(), executable)]),
            behavior_sha256: BTreeMap::from([(HARNESS_PATH.to_owned(), ZERO_SHA.to_owned())]),
            lifecycle_schema: "tuning-campaign-session-v1".to_owned(),
            lifecycle_behavior_sha256: ZERO_SHA.to_owned(),
            feature_contract: FEATURE_CONTRACT.to_owned(),
            thread_contract: THREAD_CONTRACT.to_owned(),
            host_identity: "test-host".to_owned(),
        }
    }

    /// Emits the owner envelope of a campaign whose results follow `curve`
    /// and returns the analysis with the reopened section.
    fn emit(label: &str, curve: impl Fn(Field) -> Curve) -> (AnalysisOutput, CodingTuning) {
        try_emit(label, curve, |_| {}).unwrap()
    }

    fn try_emit(
        label: &str,
        curve: impl Fn(Field) -> Curve,
        adjust: impl FnOnce(&mut EmitOwnerRequest),
    ) -> Result<(AnalysisOutput, CodingTuning), String> {
        let (root, request) = manifest_request(label);
        let manifest = campaign_manifest(request).unwrap();
        let bundle_bytes = serde_json::to_vec(&synthetic_bundle(&manifest, curve)).unwrap();
        let bundle_path = root.path().join("accepted.json");
        fs::write(&bundle_path, &bundle_bytes).unwrap();
        let identity = resume_identity();
        let producer_sha256 = identity.executable_sha256[PROCESS].clone();
        let request = EmitOwnerRequest {
            campaign_id: manifest.campaign_id.clone(),
            manifest_sha256: manifest.manifest_sha256.clone(),
            accepted_results: ArtifactIdentity {
                path: fs::canonicalize(bundle_path).unwrap(),
                sha256: Sha256Digest::of(&bundle_bytes),
            },
            measurement: ObservedProvenance {
                identity: identity.clone(),
                process: token(PROCESS).unwrap(),
                observed_utc: "2026-10-08T00:00:00Z".to_owned(),
                runtime: canonical_json(&MeasurementRuntime {
                    source_dirty: false,
                    toolchain: format!("rustc {TOOLCHAIN_CONTRACT}"),
                    cpu_model: "test-cpu".to_owned(),
                    cpu_features: vec!["test".to_owned()],
                    os_kernel: "test-kernel".to_owned(),
                    governor: "performance".to_owned(),
                    receipt: "test/receipt.md".to_owned(),
                })
                .unwrap(),
            },
            assembly: ObservedProvenance {
                identity,
                process: token(PROCESS).unwrap(),
                observed_utc: "2026-10-08T00:01:00Z".to_owned(),
                runtime: canonical_json(&AssemblyRuntime {
                    source_dirty: false,
                    tool: HARNESS_PATH.to_owned(),
                    tool_sha256: producer_sha256,
                })
                .unwrap(),
            },
            output: root.path().join("coding-owner.json"),
        };
        let mut request = request;
        adjust(&mut request);
        let (artifact, analysis) = emit_owner(request)?;
        let bytes = fs::read(&artifact.path).unwrap();
        assert_eq!(artifact.sha256, Sha256Digest::of(&bytes));
        let reopened = coding_registry()
            .unwrap()
            .from_json(std::str::from_utf8(&bytes).unwrap())
            .unwrap();
        assert_eq!(reopened.profile_id().as_str(), LAUNCHER_CAMPAIGN_ID);
        let section = reopened
            .section::<CodingTuning>()
            .unwrap()
            .unwrap()
            .section
            .clone();
        Ok((analysis, section))
    }

    /// The fields an emitted section states, as `family.field`.
    fn stated_fields(section: &CodingTuning) -> Vec<String> {
        let body = CodingTuningCodec::encode_body(section).unwrap();
        let value = serde_json::to_value(body).unwrap();
        value
            .as_object()
            .unwrap()
            .iter()
            .flat_map(|(family, fields)| {
                fields
                    .as_object()
                    .unwrap()
                    .keys()
                    .map(move |field| format!("{family}.{field}"))
            })
            .collect()
    }

    fn decided(analysis: &AnalysisOutput, field: Field) -> &FieldAnalysis {
        analysis
            .fields
            .iter()
            .find(|decided| decided.field == field.name())
            .unwrap()
    }

    #[test]
    fn emission_states_the_winning_field_and_omits_the_others() {
        let winner = OCCUPIED_LANES[3];
        let (analysis, section) = emit("emit-winner", |field| match field {
            Field::LaneMinFrames => Curve::MinimumAt(rank(field, winner)),
            Field::LaneMaxEdges => Curve::Flat,
            Field::GranulesPerTask => Curve::at_conservative(field),
        });

        let reasons: Vec<_> = Field::ALL
            .iter()
            .map(|field| decided(&analysis, *field).decision.reason)
            .collect();
        assert_eq!(
            reasons,
            [
                DecisionReason::SelectedNonDefault,
                DecisionReason::UnresolvedMinimum,
                DecisionReason::MeasuredDefault,
            ]
        );
        assert_eq!(decided(&analysis, Field::LaneMinFrames).selected, winner);
        assert_eq!(
            stated_fields(&section),
            [Field::LaneMinFrames.name()],
            "only the winning field is stated"
        );
        let decode = section.decode();
        assert_eq!(decode.lane_min_frames(), winner);
        assert_eq!(decode.lane_max_edges(), LANE_MAX_EDGES);
        assert_eq!(decode.granules_per_task(), GRANULES_PER_TASK);
    }

    #[test]
    fn source_control_metadata_does_not_decide_acceptance() {
        let flat = |field| Curve::at_conservative(field);
        try_emit("meta-revision", flat, |request| {
            request.assembly.identity.source_revision = "1".repeat(40);
        })
        .unwrap();
        try_emit("meta-dirty", flat, |request| {
            request.measurement.runtime = canonical_json(&MeasurementRuntime {
                source_dirty: true,
                toolchain: format!("rustc {TOOLCHAIN_CONTRACT}"),
                cpu_model: "test-cpu".to_owned(),
                cpu_features: vec!["test".to_owned()],
                os_kernel: "test-kernel".to_owned(),
                governor: "performance".to_owned(),
                receipt: "test/receipt.md".to_owned(),
            })
            .unwrap();
            request.assembly.runtime = canonical_json(&AssemblyRuntime {
                source_dirty: true,
                tool: HARNESS_PATH.to_owned(),
                tool_sha256: request.measurement.identity.executable_sha256[PROCESS].clone(),
            })
            .unwrap();
        })
        .unwrap();
        try_emit("meta-tree", flat, |request| {
            request.assembly.identity.source_sha256 = "1".repeat(64);
        })
        .unwrap();
        try_emit("content-source", flat, |request| {
            request
                .assembly
                .identity
                .behavior_sha256
                .insert(HARNESS_PATH.to_owned(), "1".repeat(64));
        })
        .unwrap_err();
    }

    #[test]
    fn emission_without_a_winning_field_states_no_selector() {
        let (analysis, section) = emit("emit-none", |field| match field {
            Field::LaneMinFrames => Curve::Flat,
            _ => Curve::at_conservative(field),
        });
        assert!(analysis.fields.iter().all(|decided| !decided.emitted));
        assert!(stated_fields(&section).is_empty());
        assert_eq!(section.selectors(), &CodingSelectors::CONSERVATIVE);
    }
}
