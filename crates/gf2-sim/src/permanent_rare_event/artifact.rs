//! Closed rare-event artifact schemas and immutable publication.
//!
//! The reader accepts one envelope version and four payload versions. It
//! checks canonical JSON bytes, sidecars, embedded identities, recomputed
//! dataset/run/attempt IDs, closed address sets, and attempt/checkpoint
//! lineage. Publication synchronizes both files and their directory before a
//! safe `RENAME_NOREPLACE`; an unsupported platform or filesystem refuses.

use gf2_stats::weighted::ScaledStudentInterval;
use num_bigint::BigUint;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::cmp::Ordering;
use std::fmt;

use super::{
    coverage_address, target_address, COVERAGE_REPLICATES, COVERAGE_RUNS,
    COVERAGE_TRAJECTORIES_PER_RUN, RARE_EVENT_PURPOSE_TAG, RARE_EVENT_ROOT_SEED, TARGET_BLOCK_SIZE,
    TARGET_RUNS, TARGET_TRAJECTORIES_PER_RUN,
};

/// The only accepted envelope schema.
pub const ENVELOPE_SCHEMA_V1: &str = "gf2.rare-event-artifact-envelope/v1";
/// The only accepted trajectory-checkpoint schema.
pub const CHECKPOINT_SCHEMA_V1: &str = "gf2.rare-event-trajectory-checkpoint/v1";
/// The only accepted execution-attempt schema.
pub const ATTEMPT_SCHEMA_V1: &str = "gf2.rare-event-execution-attempt/v1";
/// The only accepted target-final schema.
pub const TARGET_RECEIPT_SCHEMA_V1: &str = "gf2.rare-event-target-cross-check/v1";
/// The only accepted coverage-final schema.
pub const COVERAGE_RECEIPT_SCHEMA_V1: &str = "gf2.rare-event-coverage-validation/v1";
/// The only accepted strict configuration schema.
pub const CONFIGURATION_SCHEMA_V1: &str = "gf2.rare-event-configuration/v1";
/// The only accepted behavior-closure schema.
pub const BEHAVIOR_CLOSURE_SCHEMA_V1: &str = "gf2.rare-event-behavior-closure/v1";
/// Frozen partition identifier.
pub const ADDRESS_PARTITION_V1: &str = "gf2.rare-event-stream-partitions/v1";
/// Frozen proposal identifier.
pub const SAMPLER_V1: &str = "gf2-vperp-uniform-likelihood-ratio/v1";
/// Frozen address-to-seed identifier.
pub const ADDRESS_TO_SEED_V1: &str = "gf2-matrix-address-chacha20/v1";
/// Frozen RNG algorithm identifier.
pub const RNG_V1: &str = "rand_chacha::ChaCha20Rng/v0.9.0";
/// Frozen canonical serializer identifier.
pub const SERIALIZER_V1: &str = "gf2-rare-event-artifact-canonical-json/v1";
/// Published artifact JSON filename.
pub const ARTIFACT_JSON: &str = "artifact.json";
/// Published SHA-256 sidecar filename.
pub const ARTIFACT_SIDECAR: &str = "artifact.sha256";

const CAMPAIGN_ID: &str = "permanent-zero-fraction-20260829";
const MANIFEST_PATH: &str =
    "dev/simulation_results/permanent-zero-fraction/permanent-zero-fraction-20260829/manifest.json";
const MANIFEST_SHA256: &str = "5caa384d9c87f24562ee6d91c61c44dbc04674512b3dbe63e761ca0b9480ae57";
const PROTOCOL_PATH: &str = "dev/simulation_results/permanent-zero-fraction/protocol.md";
const PROTOCOL_SHA256: &str = "249f3de398cd234cdd9c1f1d352fc909394f3bacf13acda606d95da343693639";
const RAW_CAMPAIGN_ROOT: &str =
    "dev/simulation_results/permanent-zero-fraction/permanent-zero-fraction-20260829";

/// A strict immutable rare-event configuration.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RareEventConfigurationV1 {
    /// Exact schema identifier.
    pub configuration_schema: String,
    /// Repository-relative publication root outside raw campaign data.
    pub artifact_root: String,
    /// Pinned design object and bytes.
    pub design_identity: DesignIdentityV1,
    /// Scientific mode and allocation.
    pub scientific_identity: ScientificIdentityV1,
    /// Build receipt used to recreate the behavior closure.
    pub behavior: BehaviorIdentityV1,
}

/// Pinned committed preregistration identity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DesignIdentityV1 {
    /// Repository-relative preregistration path.
    pub path: String,
    /// Full producing Git revision.
    pub git_revision: String,
    /// Git blob object ID for the exact bytes.
    pub blob_id: String,
    /// SHA-256 of the exact committed bytes.
    pub content_sha256: String,
}

/// Complete embedded dataset identity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RareEventDatasetIdentityV1 {
    /// Frozen campaign authority.
    pub campaign: CampaignAuthorityV1,
    /// Pinned design and immutable configuration identity.
    pub preregistration: PreregistrationIdentityV1,
    /// Fixed target or coverage allocation.
    pub scientific: ScientificIdentityV1,
    /// Fixed interval, ESS, and coverage constants.
    pub statistical: StatisticalConstantsV1,
    /// Sampler, source closure, executable, and toolchain identity.
    pub behavior: BehaviorIdentityV1,
}

/// Frozen campaign authority used by every rare-event dataset.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CampaignAuthorityV1 {
    /// Frozen campaign identifier.
    pub campaign_id: String,
    /// Root-manifest path.
    pub manifest_path: String,
    /// Root-manifest SHA-256.
    pub manifest_sha256: String,
    /// Frozen-protocol path.
    pub protocol_path: String,
    /// Frozen-protocol SHA-256.
    pub protocol_sha256: String,
    /// Root seed rendered as exact ASCII hexadecimal.
    pub root_seed: String,
    /// Canonical purpose name.
    pub purpose_name: String,
    /// Manifested purpose tag.
    pub purpose_tag: u8,
    /// Closed partition identifier.
    pub address_partition: String,
}

impl Default for CampaignAuthorityV1 {
    fn default() -> Self {
        Self {
            campaign_id: CAMPAIGN_ID.into(),
            manifest_path: MANIFEST_PATH.into(),
            manifest_sha256: MANIFEST_SHA256.into(),
            protocol_path: PROTOCOL_PATH.into(),
            protocol_sha256: PROTOCOL_SHA256.into(),
            root_seed: format!("0x{RARE_EVENT_ROOT_SEED:016x}"),
            purpose_name: "RareEvent".into(),
            purpose_tag: RARE_EVENT_PURPOSE_TAG,
            address_partition: ADDRESS_PARTITION_V1.into(),
        }
    }
}

/// Pinned preregistration and externally digested immutable configuration.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreregistrationIdentityV1 {
    /// Pinned design identity.
    pub design: DesignIdentityV1,
    /// Repository-relative configuration path.
    pub configuration_path: String,
    /// Exact configuration schema.
    pub configuration_schema: String,
    /// SHA-256 of canonical configuration bytes.
    pub configuration_sha256: String,
}

/// Fixed target or coverage identity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum ScientificIdentityV1 {
    /// Registered target allocation.
    Target {
        /// Field order.
        q: u8,
        /// Row count.
        n: u16,
        /// Column/rank parameter.
        k: u8,
        /// Independent runs.
        runs: u16,
        /// Trajectories per run.
        trajectories_per_run: u32,
        /// Immutable checkpoint block size.
        block_size: u32,
    },
    /// Registered coverage allocation.
    Coverage {
        /// Ordered exact anchors.
        cases: Vec<CoverageCaseV1>,
        /// Interval replicates per field.
        replicates: u16,
        /// Independent runs per replicate.
        runs: u16,
        /// Trajectories per run.
        trajectories_per_run: u32,
        /// Immutable checkpoint block size.
        block_size: u32,
    },
}

impl ScientificIdentityV1 {
    /// Returns the exact registered target identity.
    #[must_use]
    pub fn target() -> Self {
        Self::Target {
            q: 3,
            n: 1_024,
            k: 3,
            runs: TARGET_RUNS,
            trajectories_per_run: TARGET_TRAJECTORIES_PER_RUN,
            block_size: TARGET_BLOCK_SIZE,
        }
    }

    /// Returns the exact registered coverage identity.
    #[must_use]
    pub fn coverage() -> Self {
        Self::Coverage {
            cases: vec![
                CoverageCaseV1::new(3, "907", "2187"),
                CoverageCaseV1::new(5, "17581", "78125"),
                CoverageCaseV1::new(7, "126295", "823543"),
            ],
            replicates: COVERAGE_REPLICATES,
            runs: COVERAGE_RUNS,
            trajectories_per_run: COVERAGE_TRAJECTORIES_PER_RUN,
            block_size: COVERAGE_TRAJECTORIES_PER_RUN,
        }
    }
}

/// One exact coverage anchor.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoverageCaseV1 {
    /// Field order.
    pub q: u8,
    /// Row count.
    pub n: u16,
    /// Column/rank parameter.
    pub k: u8,
    /// Reduced exact anchor.
    pub exact_anchor: ExactDecimalV1,
}

impl CoverageCaseV1 {
    fn new(q: u8, numerator: &str, denominator: &str) -> Self {
        Self {
            q,
            n: 3,
            k: 3,
            exact_anchor: ExactDecimalV1::new(numerator, denominator),
        }
    }
}

/// A reduced nonnegative exact value encoded as canonical decimal strings.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExactDecimalV1 {
    /// Canonical nonnegative base-ten numerator.
    pub numerator: String,
    /// Canonical positive base-ten denominator.
    pub denominator: String,
}

impl ExactDecimalV1 {
    /// Constructs an exact decimal pair; validation occurs at the artifact boundary.
    #[must_use]
    pub fn new(numerator: impl Into<String>, denominator: impl Into<String>) -> Self {
        Self {
            numerator: numerator.into(),
            denominator: denominator.into(),
        }
    }
}

/// Fixed statistical constants.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatisticalConstantsV1 {
    /// Nominal interval level, exactly `95/100`.
    pub nominal_interval_level: ExactDecimalV1,
    /// Exact `t_31` critical rational.
    pub student_critical: ExactDecimalV1,
    /// Final-weight ESS usability threshold.
    pub ess_threshold: ExactDecimalV1,
    /// Coverage acceptance count.
    pub coverage_acceptance: ExactDecimalV1,
    /// Sample-variance divisor expression.
    pub variance_divisor: String,
    /// Closed two-sided interval-rule identifier.
    pub interval_rule: String,
}

impl Default for StatisticalConstantsV1 {
    fn default() -> Self {
        Self {
            nominal_interval_level: ExactDecimalV1::new("19", "20"),
            student_critical: ExactDecimalV1::new("1019756723", "500000000"),
            ess_threshold: ExactDecimalV1::new("1", "100"),
            coverage_acceptance: ExactDecimalV1::new("9", "10"),
            variance_divisor: "runs-1".into(),
            interval_rule: "gf2.scaled-two-sided-student-clipped/v1".into(),
        }
    }
}

/// Complete estimator/build behavior identity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BehaviorIdentityV1 {
    /// Proposal identifier.
    pub sampler: String,
    /// Address-to-seed identifier.
    pub address_to_seed: String,
    /// RNG algorithm identifier.
    pub rng_algorithm: String,
    /// Locked RNG package version.
    pub rng_crate_version: String,
    /// Canonical serializer identifier.
    pub serializer: String,
    /// SHA-256 of canonical behavior-closure bytes.
    pub estimator_behavior_sha256: String,
    /// Descriptor whose digest is checked.
    pub closure: BehaviorClosureV1,
    /// Producing executable SHA-256.
    pub executable_sha256: String,
    /// Exact rustc version.
    pub rust_version: String,
    /// Exact Cargo version.
    pub cargo_version: String,
    /// Compilation target triple.
    pub compilation_target: String,
    /// Runtime-observed source revision.
    pub source_revision: String,
    /// Runtime-observed checkout dirty flag.
    pub source_dirty: bool,
}

/// Closed behavior-closure descriptor.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BehaviorClosureV1 {
    /// Exact descriptor schema.
    pub behavior_schema: String,
    /// Sorted enabled feature names.
    pub enabled_features: Vec<String>,
    /// Sorted repository source inputs.
    pub repository_inputs: Vec<RepositoryInputV1>,
    /// Root lockfile SHA-256.
    pub cargo_lock_sha256: String,
    /// Sorted resolved package closure.
    pub packages: Vec<PackageIdentityV1>,
    /// Sorted declared behavior-affecting environment inputs.
    pub environment_input_names: Vec<String>,
}

/// One source input reported by producing dependency information.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepositoryInputV1 {
    /// Repository-relative source path.
    pub path: String,
    /// Git blob object ID.
    pub blob_id: String,
    /// Content SHA-256.
    pub content_sha256: String,
}

/// One Cargo-resolved package identity.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageIdentityV1 {
    /// Package name.
    pub package: String,
    /// Exact version.
    pub version: String,
    /// Cargo source string.
    pub source: String,
    /// Registry checksum or explicit path sentinel.
    pub checksum: String,
}

/// Artifact envelope selecting one closed payload.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RareEventArtifactEnvelopeV1 {
    /// Exact common-envelope schema.
    pub envelope_schema: String,
    /// Closed artifact kind.
    pub artifact_kind: ArtifactKindV1,
    /// Schema-tagged closed payload.
    pub payload: RareEventPayloadV1,
}

/// Closed artifact kinds.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKindV1 {
    /// Immutable complete trajectory block.
    TrajectoryCheckpoint,
    /// Immutable start or terminal attempt phase.
    ExecutionAttempt,
    /// Target final receipt.
    TargetCrossCheck,
    /// Coverage final receipt.
    CoverageValidation,
}

/// Schema-tagged closed payload union.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "payload_schema", deny_unknown_fields)]
pub enum RareEventPayloadV1 {
    /// Trajectory checkpoint payload.
    #[serde(rename = "gf2.rare-event-trajectory-checkpoint/v1")]
    TrajectoryCheckpoint(Box<TrajectoryCheckpointV1>),
    /// Execution-attempt phase payload.
    #[serde(rename = "gf2.rare-event-execution-attempt/v1")]
    ExecutionAttempt(Box<ExecutionAttemptReceiptV1>),
    /// Target final payload.
    #[serde(rename = "gf2.rare-event-target-cross-check/v1")]
    TargetCrossCheck(Box<TargetCrossCheckReceiptV1>),
    /// Coverage final payload.
    #[serde(rename = "gf2.rare-event-coverage-validation/v1")]
    CoverageValidation(Box<CoverageValidationReceiptV1>),
}

/// One logical run address; this is an artifact semantic type, not a sampler address.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum RunAddressV1 {
    /// Target run `(r)`.
    Target {
        /// Run index.
        run: u16,
    },
    /// Coverage run `(q,b,r)`.
    Coverage {
        /// Field order.
        q: u8,
        /// Replicate index.
        replicate: u16,
        /// Run index.
        run: u16,
    },
}

/// One complete immutable trajectory checkpoint.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrajectoryCheckpointV1 {
    /// Complete dataset identity.
    pub dataset_identity: RareEventDatasetIdentityV1,
    /// Recomputed dataset ID.
    pub dataset_id: String,
    /// Logical run address.
    pub run_address: RunAddressV1,
    /// Recomputed run ID.
    pub run_id: String,
    /// Zero-based immutable block index.
    pub block_index: u16,
    /// Inclusive trajectory start.
    pub trajectory_start: u32,
    /// Exclusive trajectory end.
    pub trajectory_end: u32,
    /// Producing attempt ID.
    pub attempt_id: String,
    /// Producing start-artifact digest.
    pub attempt_start_sha256: String,
    /// Producing backend observation.
    pub producer: ProducerBackendV1,
    /// Matching accelerator-observation digest.
    pub accelerator_observation_sha256: String,
    /// Closed target or coverage records.
    pub records: TrajectoryRecordsV1,
}

/// CPU or GPU checkpoint producer identity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "backend", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProducerBackendV1 {
    /// CPU producer.
    Cpu {},
    /// GPU producer linked to the start receipt.
    Gpu {
        /// Runtime GPU UUID.
        device_uuid: String,
        /// Producing kernel name.
        kernel_name: String,
        /// Loaded code-object SHA-256.
        code_object_sha256: String,
    },
}

/// Closed target or coverage trajectory-record union.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "record_kind",
    content = "items",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum TrajectoryRecordsV1 {
    /// Target records.
    Target(Vec<TargetTrajectoryRecordV1>),
    /// Coverage records.
    Coverage(Vec<CoverageTrajectoryRecordV1>),
}

/// One target trajectory record `(r,j,s,E)`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetTrajectoryRecordV1 {
    /// Run index.
    pub run: u16,
    /// Trajectory index.
    pub trajectory: u32,
    /// Recomputed stream index.
    pub stream_index: u64,
    /// Final likelihood exponent.
    pub exponent: u32,
}

/// One coverage trajectory record `(q,b,r,j,s,E)`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoverageTrajectoryRecordV1 {
    /// Field order.
    pub q: u8,
    /// Replicate index.
    pub replicate: u16,
    /// Run index.
    pub run: u16,
    /// Trajectory index.
    pub trajectory: u32,
    /// Recomputed stream index.
    pub stream_index: u64,
    /// Final likelihood exponent.
    pub exponent: u32,
}

/// Immutable start or terminal phase for one execution attempt.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ExecutionAttemptReceiptV1 {
    /// Complete dataset identity.
    pub dataset_identity: RareEventDatasetIdentityV1,
    /// Recomputed dataset ID.
    pub dataset_id: String,
    /// Recomputed attempt ID.
    pub attempt_id: String,
    /// Gap-free zero-based ordinal.
    pub attempt_ordinal: u64,
    /// Closed predecessor tag.
    pub predecessor: AttemptPredecessorV1,
    /// Start or terminal fields.
    #[serde(flatten)]
    pub phase: AttemptPhaseV1,
}

/// Attempt predecessor tag.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "predecessor", rename_all = "snake_case", deny_unknown_fields)]
pub enum AttemptPredecessorV1 {
    /// First attempt has no predecessor.
    None {},
    /// Later attempt binds the prior terminal artifact.
    Terminal {
        /// Prior terminal SHA-256.
        terminal_sha256: String,
    },
}

/// Closed start/terminal phase fields.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "phase", rename_all = "snake_case", deny_unknown_fields)]
pub enum AttemptPhaseV1 {
    /// Launcher-observed start phase.
    Start {
        /// Ordered accepted checkpoint refs at process start.
        resume_checkpoint_refs: Vec<CheckpointRefV1>,
        /// OS-successful child creation time.
        start_utc: String,
        /// Time immediately before start publication.
        start_receipt_utc: String,
        /// Exact invocation/effective-input record.
        invocation: Box<InvocationV1>,
        /// Sorted declared environment inputs.
        environment_inputs: Vec<EnvironmentInputV1>,
        /// Requested/effective worker settings.
        worker_configuration: WorkerConfigurationV1,
        /// Runtime-observed host.
        host_observation: Box<HostObservationV1>,
        /// Runtime-observed accelerator selection.
        accelerator_observation: AcceleratorObservationV1,
    },
    /// Launcher- or resumer-observed terminal phase.
    Terminal {
        /// Digest of the immutable start artifact.
        attempt_start_sha256: String,
        /// Echoed start time.
        start_utc: String,
        /// Observed terminal/resume time.
        end_utc: String,
        /// Meaning of `end_utc`.
        end_time_meaning: EndTimeMeaningV1,
        /// Optional monotonic elapsed nanoseconds.
        monotonic_elapsed_ns: Option<u64>,
        /// Digest of start host observation canonical bytes.
        host_observation_sha256: String,
        /// Digest of start accelerator observation canonical bytes.
        accelerator_observation_sha256: String,
        /// Launcher/resumer observer identity.
        outcome_observer: OutcomeObserverV1,
        /// Closed outcome.
        outcome: AttemptOutcomeV1,
        /// Ordered checkpoint refs produced by this attempt.
        checkpoint_refs: Vec<CheckpointRefV1>,
    },
}

/// Exact invocation and effective input resolution.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InvocationV1 {
    /// Exact UTF-8 argument vector including argument zero.
    pub argv: Vec<String>,
    /// Resolved executable path.
    pub executable_path: String,
    /// Runtime executable SHA-256.
    pub executable_sha256: String,
    /// Runtime child PID.
    pub process_id: u32,
    /// OS process-start token.
    pub process_start_token: String,
    /// Boot/container identity.
    pub boot_identity: String,
    /// Immutable configuration path.
    pub configuration_path: String,
    /// Configuration digest.
    pub configuration_sha256: String,
    /// Canonical effective-configuration bytes as lowercase hex.
    pub effective_configuration_hex: String,
    /// Sorted field-by-field resolution.
    pub input_resolution: Vec<InputResolutionV1>,
}

/// One effective input and its sole source.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputResolutionV1 {
    /// Schema field name.
    pub field: String,
    /// Closed input origin.
    pub origin: InputOriginV1,
    /// Exact UTF-8 effective value.
    pub value: String,
}

/// Closed input origin.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputOriginV1 {
    /// Configuration file.
    Configuration,
    /// Argument token.
    Argument,
    /// Declared environment input.
    Environment,
    /// Schema default.
    SchemaDefault,
}

/// One declared behavior-affecting environment input.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnvironmentInputV1 {
    /// Exact environment name.
    pub name: String,
    /// Unset or exact UTF-8 value.
    pub value: EnvironmentValueV1,
}

/// Closed environment-value tag.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "state",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum EnvironmentValueV1 {
    /// Name was unset.
    Unset {},
    /// Exact UTF-8 value.
    Set(String),
}

/// Requested and effective execution configuration.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerConfigurationV1 {
    /// Requested worker count.
    pub requested_workers: usize,
    /// Effective worker count.
    pub effective_workers: usize,
    /// Executor/backend mode.
    pub executor_mode: String,
    /// CPU affinity or exact `unpinned` tag.
    pub cpu_affinity: String,
    /// Work-queue policy identifier.
    pub work_queue_policy: String,
    /// Block-assignment policy identifier.
    pub block_assignment_policy: String,
    /// Accelerator-selection policy.
    pub accelerator_selection: String,
    /// Fallback policy.
    pub fallback_policy: String,
    /// Ordered effective device selection.
    pub effective_devices: Vec<String>,
}

/// Exact observed evidence bytes and digest.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservationEvidenceV1 {
    /// Runtime observation source identifier.
    pub source: String,
    /// Exact returned bytes as lowercase hex.
    pub evidence_hex: String,
    /// SHA-256 of decoded evidence bytes.
    pub evidence_sha256: String,
}

/// Runtime-observed CPU/RAM/OS facts.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostObservationV1 {
    /// UTC collection time.
    pub observation_utc: String,
    /// Runtime architecture.
    pub cpu_architecture: String,
    /// Runtime vendor.
    pub cpu_vendor: String,
    /// Runtime model identity.
    pub cpu_model: String,
    /// Observed socket count.
    pub sockets: u32,
    /// Observed NUMA-node count.
    pub numa_nodes: u32,
    /// Observed physical-core count.
    pub physical_cores: u32,
    /// Observed logical CPU count.
    pub logical_cpus: u32,
    /// Sorted online logical identifiers.
    pub online_cpus: Vec<u32>,
    /// Total RAM bytes.
    pub total_ram_bytes: u64,
    /// Available RAM bytes.
    pub available_ram_bytes: u64,
    /// Runtime OS name.
    pub os_name: String,
    /// Runtime OS version.
    pub os_version: String,
    /// Kernel release.
    pub kernel_release: String,
    /// Kernel version.
    pub kernel_version: String,
    /// Ordered evidence groups used to derive normalized fields.
    pub evidence: Vec<ObservationEvidenceV1>,
}

#[derive(Serialize, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct HostNormalizedEvidenceV1 {
    observation_utc: String,
    cpu_architecture: String,
    cpu_vendor: String,
    cpu_model: String,
    sockets: u32,
    numa_nodes: u32,
    physical_cores: u32,
    logical_cpus: u32,
    online_cpus: Vec<u32>,
    total_ram_bytes: u64,
    available_ram_bytes: u64,
    os_name: String,
    os_version: String,
    kernel_release: String,
    kernel_version: String,
}

impl From<&HostObservationV1> for HostNormalizedEvidenceV1 {
    fn from(host: &HostObservationV1) -> Self {
        Self {
            observation_utc: host.observation_utc.clone(),
            cpu_architecture: host.cpu_architecture.clone(),
            cpu_vendor: host.cpu_vendor.clone(),
            cpu_model: host.cpu_model.clone(),
            sockets: host.sockets,
            numa_nodes: host.numa_nodes,
            physical_cores: host.physical_cores,
            logical_cpus: host.logical_cpus,
            online_cpus: host.online_cpus.clone(),
            total_ram_bytes: host.total_ram_bytes,
            available_ram_bytes: host.available_ram_bytes,
            os_name: host.os_name.clone(),
            os_version: host.os_version.clone(),
            kernel_release: host.kernel_release.clone(),
            kernel_version: host.kernel_version.clone(),
        }
    }
}

/// Runtime accelerator observation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "selection", rename_all = "snake_case", deny_unknown_fields)]
pub enum AcceleratorObservationV1 {
    /// No accelerator selected or loaded.
    NotUsed {
        /// Runtime-observed selection reason.
        reason: String,
        /// Exact source evidence.
        evidence: Vec<ObservationEvidenceV1>,
    },
    /// One or more selected/loaded GPUs.
    Used {
        /// Sorted observations by UUID.
        devices: Vec<GpuObservationV1>,
    },
}

/// One runtime-observed GPU and loaded kernel.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GpuObservationV1 {
    /// Model name.
    pub model: String,
    /// Runtime UUID.
    pub uuid: String,
    /// PCI address.
    pub pci_address: String,
    /// Architecture identifier.
    pub architecture: String,
    /// Driver version.
    pub driver_version: String,
    /// ROCm version.
    pub rocm_version: String,
    /// HIP runtime version.
    pub hip_version: String,
    /// Producing kernel name.
    pub kernel_name: String,
    /// Loaded code-object SHA-256.
    pub code_object_sha256: String,
    /// Exact runtime evidence.
    pub evidence: Vec<ObservationEvidenceV1>,
}

#[derive(Serialize, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct GpuNormalizedEvidenceV1 {
    model: String,
    uuid: String,
    pci_address: String,
    architecture: String,
    driver_version: String,
    rocm_version: String,
    hip_version: String,
    kernel_name: String,
    code_object_sha256: String,
}

impl From<&GpuObservationV1> for GpuNormalizedEvidenceV1 {
    fn from(device: &GpuObservationV1) -> Self {
        Self {
            model: device.model.clone(),
            uuid: device.uuid.clone(),
            pci_address: device.pci_address.clone(),
            architecture: device.architecture.clone(),
            driver_version: device.driver_version.clone(),
            rocm_version: device.rocm_version.clone(),
            hip_version: device.hip_version.clone(),
            kernel_name: device.kernel_name.clone(),
            code_object_sha256: device.code_object_sha256.clone(),
        }
    }
}

#[derive(Serialize, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct AcceleratorNotUsedEvidenceV1 {
    reason: String,
}

/// Meaning of a terminal time.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EndTimeMeaningV1 {
    /// Launcher observed process termination.
    ProcessObserved,
    /// Resumer observed abandoned identity absent.
    ResumeObservation,
}

/// Terminal outcome observer.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "observer", rename_all = "snake_case", deny_unknown_fields)]
pub enum OutcomeObserverV1 {
    /// Supervising launcher.
    SupervisingLauncher {
        /// Launcher executable SHA-256.
        launcher_sha256: String,
    },
    /// Resuming launcher.
    ResumingLauncher {
        /// Launcher executable SHA-256.
        launcher_sha256: String,
        /// Liveness observation evidence.
        liveness_evidence: ObservationEvidenceV1,
    },
}

/// Closed terminal outcome.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
pub enum AttemptOutcomeV1 {
    /// Successful observed exit and completed contract.
    Completed {},
    /// Observed error exit or structured failure.
    Failed {
        /// Exact wait-status token.
        wait_status: String,
        /// Optional structured failure category.
        failure_category: Option<String>,
    },
    /// Observed cancellation or termination signal.
    Interrupted {
        /// Exact signal/status token.
        wait_status: String,
    },
    /// Prior child identity was proved not live during resume.
    TerminationUnobservedOnResume {},
}

/// One immutable checkpoint reference.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckpointRefV1 {
    /// Canonical block address string.
    pub block_address: String,
    /// Checkpoint artifact SHA-256.
    pub checkpoint_sha256: String,
}

/// One transitive attempt reference in a final receipt.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttemptRefV1 {
    /// Recomputed attempt ID.
    pub attempt_id: String,
    /// Start artifact SHA-256.
    pub attempt_start_sha256: String,
    /// Terminal artifact SHA-256.
    pub attempt_terminal_sha256: String,
}

/// Serialized execution provenance embedded in a final receipt.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionProvenanceV1 {
    /// Gap-free ordinal-ordered attempt triples.
    pub attempts: Vec<AttemptRefV1>,
    /// Complete ordered checkpoint partition.
    pub checkpoint_refs: Vec<CheckpointRefV1>,
}

/// Opaque complete checkpoint set accepted at the artifact boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedCheckpointSet {
    dataset_id: String,
    checkpoints: Vec<ValidatedCheckpoint>,
    references: Vec<CheckpointRefV1>,
}

impl ValidatedCheckpointSet {
    /// Returns canonical references in exact semantic block order.
    #[must_use]
    pub fn checkpoint_refs(&self) -> &[CheckpointRefV1] {
        &self.references
    }

    /// Returns the number of complete immutable blocks.
    #[must_use]
    pub fn len(&self) -> usize {
        self.references.len()
    }

    /// Returns whether this complete set has no blocks.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.references.is_empty()
    }
}

/// Opaque checkpoint handle created only after full schema/address validation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedCheckpoint {
    dataset_id: String,
    reference: CheckpointRefV1,
    attempt_id: String,
    attempt_start_sha256: String,
    producer: ProducerBackendV1,
    accelerator_observation_sha256: String,
    exponent_histogram: Vec<ExponentBinV1>,
}

impl ValidatedCheckpoint {
    /// Returns the canonical immutable checkpoint reference.
    #[must_use]
    pub fn checkpoint_ref(&self) -> &CheckpointRefV1 {
        &self.reference
    }
}

/// Opaque validated start or terminal attempt artifact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedAttemptArtifact {
    digest: String,
    payload: ExecutionAttemptReceiptV1,
}

impl ValidatedAttemptArtifact {
    /// Returns the immutable artifact digest.
    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

/// Opaque gap-free execution lineage built from verified start/terminal phases.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedExecutionLineage {
    dataset_id: String,
    provenance: ExecutionProvenanceV1,
}

/// Opaque final receipt linked to validated checkpoints and attempt phases.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedFinalReceipt {
    digest: String,
    envelope: RareEventArtifactEnvelopeV1,
}

impl ValidatedFinalReceipt {
    /// Returns the immutable final artifact digest.
    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }

    /// Returns the fully linked final envelope.
    #[must_use]
    pub fn envelope(&self) -> &RareEventArtifactEnvelopeV1 {
        &self.envelope
    }
}

impl ValidatedExecutionLineage {
    /// Returns the canonical serialized execution provenance.
    #[must_use]
    pub fn provenance(&self) -> &ExecutionProvenanceV1 {
        &self.provenance
    }
}

/// Canonical target scientific result, separate from execution provenance.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetResultPayloadV1 {
    /// Exact expected final-weight count.
    pub expected_trajectory_count: u64,
    /// Primary exact-result repository path.
    pub exact_result_path: String,
    /// Primary exact-result content SHA-256.
    pub exact_result_sha256: String,
    /// Raw exact deficient count.
    pub exact_raw_count: String,
    /// Raw exact total.
    pub exact_total: String,
    /// Reduced primary probability.
    pub exact_probability: ExactDecimalV1,
    /// Exact cross-check estimate.
    pub cross_check_estimate: ExactDecimalV1,
    /// Exact independent-run variance.
    pub independent_run_variance: ExactDecimalV1,
    /// Outward-rendered lower endpoint.
    pub interval_lower: String,
    /// Outward-rendered upper endpoint.
    pub interval_upper: String,
    /// Exact final-weight ESS.
    pub final_weight_ess: ExactDecimalV1,
    /// Exact final-weight ESS for every independent run.
    pub per_run_ess: Vec<ExactDecimalV1>,
    /// Exact ESS/sample fraction.
    pub ess_fraction: ExactDecimalV1,
    /// Largest normalized single final-weight share.
    pub largest_weight_share: ExactDecimalV1,
    /// Largest independent-run mean share.
    pub largest_run_mean_share: ExactDecimalV1,
    /// Complete pooled exponent histogram.
    pub exponent_histogram: Vec<ExponentBinV1>,
    /// Minimum observed final exponent.
    pub minimum_exponent: u32,
    /// Maximum observed final exponent.
    pub maximum_exponent: u32,
    /// Complete exact run means.
    pub run_means: Vec<ExactDecimalV1>,
    /// Every extinction diagnostic.
    pub extinction_reasons: Vec<String>,
    /// Fixed degeneracy verdict.
    pub degeneracy: bool,
    /// Closed usable/contradiction/unusable verdict.
    pub verdict: CrossCheckVerdictV1,
}

/// One complete exponent-histogram bin.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExponentBinV1 {
    /// Exponent.
    pub exponent: u32,
    /// Count.
    pub count: u64,
}

/// Target cross-check verdict.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CrossCheckVerdictV1 {
    /// Exact value lies in a usable interval.
    Agreement,
    /// Usable interval excludes the exact value.
    Contradiction,
    /// Extinction or below-threshold ESS.
    Unusable,
}

/// Target final receipt.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetCrossCheckReceiptV1 {
    /// Complete target identity.
    pub dataset_identity: RareEventDatasetIdentityV1,
    /// Recomputed dataset ID.
    pub dataset_id: String,
    /// Canonical scientific result.
    pub result_payload: TargetResultPayloadV1,
    /// SHA-256 over canonical result-payload bytes only.
    pub result_sha256: String,
    /// Complete execution provenance.
    pub execution_provenance: ExecutionProvenanceV1,
}

/// One coverage replicate result.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoverageReplicateV1 {
    /// Field order.
    pub q: u8,
    /// Replicate index.
    pub replicate: u16,
    /// Exact anchor.
    pub exact_anchor: ExactDecimalV1,
    /// Exact point estimate.
    pub estimate: ExactDecimalV1,
    /// Exact independent-run variance.
    pub independent_run_variance: ExactDecimalV1,
    /// Outward-rendered lower endpoint.
    pub interval_lower: String,
    /// Outward-rendered upper endpoint.
    pub interval_upper: String,
    /// Exact containment verdict.
    pub contains_anchor: bool,
    /// Exact ESS fraction diagnostic.
    pub ess_fraction: ExactDecimalV1,
    /// Exact pooled final-weight ESS diagnostic.
    pub final_weight_ess: ExactDecimalV1,
    /// Complete pooled exponent histogram.
    pub exponent_histogram: Vec<ExponentBinV1>,
    /// Complete exact independent-run means.
    pub run_means: Vec<ExactDecimalV1>,
    /// Extinction diagnostics.
    pub extinction_reasons: Vec<String>,
    /// Fixed below-threshold degeneracy diagnostic.
    pub degeneracy: bool,
}

/// Canonical coverage scientific result.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoverageResultPayloadV1 {
    /// Exact expected final-weight count.
    pub expected_trajectory_count: u64,
    /// All replicates in increasing `(q,b)` order.
    pub replicates: Vec<CoverageReplicateV1>,
    /// Per-field containment counts.
    pub coverage_counts: Vec<CoverageCountV1>,
    /// Closed validation verdict.
    pub verdict: CoverageVerdictV1,
}

/// One per-field empirical coverage count.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoverageCountV1 {
    /// Field order.
    pub q: u8,
    /// Containing intervals out of 200.
    pub count: u16,
}

/// Coverage-validation verdict.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoverageVerdictV1 {
    /// Every field has at least 180/200.
    Adequate,
    /// At least one field fails 180/200 or has extinction.
    Unusable,
}

/// Coverage final receipt.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoverageValidationReceiptV1 {
    /// Complete coverage identity.
    pub dataset_identity: RareEventDatasetIdentityV1,
    /// Recomputed dataset ID.
    pub dataset_id: String,
    /// Canonical scientific result.
    pub result_payload: CoverageResultPayloadV1,
    /// SHA-256 over canonical result-payload bytes only.
    pub result_sha256: String,
    /// Complete execution provenance.
    pub execution_provenance: ExecutionProvenanceV1,
}

/// A schema, identity, integrity, lineage, or publication refusal.
#[derive(Debug)]
pub enum ArtifactError {
    /// JSON serialization/deserialization failed.
    Json(serde_json::Error),
    /// Schema identifier, enum tag, field, or canonical encoding is invalid.
    Schema(String),
    /// SHA-256 sidecar or linked digest is invalid.
    Integrity(String),
    /// Dataset, run, attempt, or source identity is invalid.
    Identity(String),
    /// Semantic address set is incomplete, extra, duplicated, or reordered.
    AddressSet(String),
    /// Attempt/checkpoint lineage is invalid.
    Lineage(String),
}

impl fmt::Display for ArtifactError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(error) => write!(formatter, "artifact JSON error: {error}"),
            Self::Schema(message) => write!(formatter, "artifact schema refusal: {message}"),
            Self::Integrity(message) => write!(formatter, "artifact integrity refusal: {message}"),
            Self::Identity(message) => write!(formatter, "artifact identity refusal: {message}"),
            Self::AddressSet(message) => {
                write!(formatter, "artifact address-set refusal: {message}")
            }
            Self::Lineage(message) => write!(formatter, "artifact lineage refusal: {message}"),
        }
    }
}

impl std::error::Error for ArtifactError {}

impl From<serde_json::Error> for ArtifactError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

/// Serializes a schema value to canonical UTF-8 JSON plus one final LF.
pub fn canonical_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, ArtifactError> {
    let value = serde_json::to_value(value)?;
    let mut bytes = Vec::new();
    write_canonical_value(&value, &mut bytes)?;
    bytes.push(b'\n');
    Ok(bytes)
}

/// Computes lowercase SHA-256 over exact bytes.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Encodes the canonical raw collector record behind normalized host fields.
pub fn normalized_host_evidence(
    host: &HostObservationV1,
) -> Result<ObservationEvidenceV1, ArtifactError> {
    evidence_from_value(
        "gf2.host-observation-normalized-json/v1",
        &HostNormalizedEvidenceV1::from(host),
    )
}

/// Encodes the canonical raw collector record behind one normalized GPU.
pub fn normalized_gpu_evidence(
    device: &GpuObservationV1,
) -> Result<ObservationEvidenceV1, ArtifactError> {
    evidence_from_value(
        "gf2.gpu-observation-normalized-json/v1",
        &GpuNormalizedEvidenceV1::from(device),
    )
}

/// Encodes the canonical raw collector record behind a no-accelerator reason.
pub fn normalized_accelerator_not_used_evidence(
    reason: &str,
) -> Result<ObservationEvidenceV1, ArtifactError> {
    evidence_from_value(
        "gf2.accelerator-not-used-normalized-json/v1",
        &AcceleratorNotUsedEvidenceV1 {
            reason: reason.to_owned(),
        },
    )
}

fn evidence_from_value(
    source: &str,
    value: &impl Serialize,
) -> Result<ObservationEvidenceV1, ArtifactError> {
    let bytes = canonical_bytes(value)?;
    Ok(ObservationEvidenceV1 {
        source: source.into(),
        evidence_hex: bytes.iter().map(|byte| format!("{byte:02x}")).collect(),
        evidence_sha256: sha256_hex(&bytes),
    })
}

/// Recomputes a dataset ID from canonical identity bytes.
pub fn dataset_id(identity: &RareEventDatasetIdentityV1) -> Result<String, ArtifactError> {
    domain_digest(
        b"gf2-rare-event-dataset-identity-v1",
        &canonical_bytes(identity)?,
    )
}

/// Recomputes a logical run ID from the dataset and semantic run address.
pub fn run_id(
    identity: &RareEventDatasetIdentityV1,
    address: &RunAddressV1,
) -> Result<String, ArtifactError> {
    let dataset = decode_digest(&dataset_id(identity)?)?;
    let address = canonical_bytes(address)?;
    let mut input = Vec::with_capacity(dataset.len() + address.len());
    input.extend(dataset);
    input.extend(address);
    domain_digest(b"gf2-rare-event-run-identity-v1", &input)
}

/// Recomputes an attempt ID from its dataset, ordinal, and predecessor.
pub fn attempt_id(
    identity: &RareEventDatasetIdentityV1,
    ordinal: u64,
    predecessor: &AttemptPredecessorV1,
) -> Result<String, ArtifactError> {
    if ordinal >= 1_000_000_000_000 {
        return Err(ArtifactError::Identity(
            "attempt ordinal is outside twelve digits".into(),
        ));
    }
    let mut input = decode_digest(&dataset_id(identity)?)?;
    input.extend(ordinal.to_le_bytes());
    match predecessor {
        AttemptPredecessorV1::None {} if ordinal == 0 => input.push(0),
        AttemptPredecessorV1::Terminal { terminal_sha256 } if ordinal > 0 => {
            input.push(1);
            input.extend(decode_digest(terminal_sha256)?);
        }
        _ => {
            return Err(ArtifactError::Identity(
                "attempt predecessor tag disagrees with ordinal".into(),
            ))
        }
    }
    domain_digest(b"gf2-rare-event-attempt-identity-v1", &input)
}

/// Parses and validates exact canonical envelope bytes at the schema level.
///
/// For a final receipt this checks closed fields, identity, result arithmetic,
/// and embedded reference shape, but deliberately does not confer scientific
/// validity. Scientific acceptance requires opaque checkpoint handles and a
/// [`ValidatedExecutionLineage`] through the final reducers or strict
/// directory verification.
pub fn decode_envelope(bytes: &[u8]) -> Result<RareEventArtifactEnvelopeV1, ArtifactError> {
    let envelope: RareEventArtifactEnvelopeV1 = serde_json::from_slice(bytes)?;
    if canonical_bytes(&envelope)? != bytes {
        return Err(ArtifactError::Schema(
            "artifact JSON is not canonical".into(),
        ));
    }
    validate_envelope(&envelope)?;
    Ok(envelope)
}

/// Parses a closed canonical configuration from exact bytes.
pub fn decode_configuration(bytes: &[u8]) -> Result<RareEventConfigurationV1, ArtifactError> {
    let configuration: RareEventConfigurationV1 = serde_json::from_slice(bytes)?;
    if canonical_bytes(&configuration)? != bytes {
        return Err(ArtifactError::Schema(
            "configuration JSON is not canonical".into(),
        ));
    }
    if configuration.configuration_schema != CONFIGURATION_SCHEMA_V1 {
        return Err(ArtifactError::Schema("unknown configuration schema".into()));
    }
    validate_configuration(&configuration)?;
    Ok(configuration)
}

/// Exact JSON, sidecar, and digest bytes ready for later immutable publication.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactFileBytes {
    /// Canonical `artifact.json` bytes.
    pub artifact_json: Vec<u8>,
    /// Exact `artifact.sha256` bytes.
    pub artifact_sha256: Vec<u8>,
    /// Lowercase SHA-256 of `artifact_json`.
    pub digest: String,
}

/// Encodes and validates the two files of one artifact directory.
pub fn encode_artifact_files(
    envelope: &RareEventArtifactEnvelopeV1,
) -> Result<ArtifactFileBytes, ArtifactError> {
    validate_envelope(envelope)?;
    let artifact_json = canonical_bytes(envelope)?;
    let digest = sha256_hex(&artifact_json);
    let artifact_sha256 = format!("{digest}  {ARTIFACT_JSON}\n").into_bytes();
    Ok(ArtifactFileBytes {
        artifact_json,
        artifact_sha256,
        digest,
    })
}

/// Verifies exact JSON/sidecar bytes against an externally expected identity.
pub fn verify_artifact_files(
    artifact_json: &[u8],
    artifact_sha256: &[u8],
    expected_identity: &RareEventDatasetIdentityV1,
) -> Result<RareEventArtifactEnvelopeV1, ArtifactError> {
    let digest = sha256_hex(artifact_json);
    if artifact_sha256 != format!("{digest}  {ARTIFACT_JSON}\n").as_bytes() {
        return Err(ArtifactError::Integrity(
            "artifact sidecar does not match canonical JSON bytes".into(),
        ));
    }
    let envelope = decode_envelope(artifact_json)?;
    if payload_identity(&envelope.payload) != expected_identity {
        return Err(ArtifactError::Identity(
            "embedded dataset identity differs from expected identity".into(),
        ));
    }
    Ok(envelope)
}

/// Reconstructs the complete canonical block-address set from a fixed identity.
pub fn expected_checkpoint_block_addresses(
    identity: &RareEventDatasetIdentityV1,
) -> Result<Vec<String>, ArtifactError> {
    validate_identity(identity)?;
    let mut addresses = Vec::new();
    match &identity.scientific {
        ScientificIdentityV1::Target { .. } => {
            addresses.reserve(2_048);
            for run in 0..TARGET_RUNS {
                for block in 0..64_u16 {
                    addresses.push(format!("target/{run:02}/{block:04}"));
                }
            }
        }
        ScientificIdentityV1::Coverage { .. } => {
            addresses.reserve(19_200);
            for q in [3_u8, 5, 7] {
                for replicate in 0..COVERAGE_REPLICATES {
                    for run in 0..COVERAGE_RUNS {
                        addresses.push(format!("coverage/q{q}/b{replicate:03}/r{run:02}/0000"));
                    }
                }
            }
        }
    }
    Ok(addresses)
}

/// Validates exact checkpoint bytes and returns an opaque immutable handle.
pub fn validate_checkpoint_artifact_files(
    artifact_json: &[u8],
    artifact_sha256: &[u8],
    expected_identity: &RareEventDatasetIdentityV1,
) -> Result<ValidatedCheckpoint, ArtifactError> {
    let digest = sha256_hex(artifact_json);
    let envelope = verify_artifact_files(artifact_json, artifact_sha256, expected_identity)?;
    let RareEventPayloadV1::TrajectoryCheckpoint(payload) = envelope.payload else {
        return Err(ArtifactError::Schema(
            "expected a trajectory checkpoint artifact".into(),
        ));
    };
    Ok(ValidatedCheckpoint {
        dataset_id: payload.dataset_id.clone(),
        reference: CheckpointRefV1 {
            block_address: checkpoint_block_address(&payload),
            checkpoint_sha256: digest,
        },
        attempt_id: payload.attempt_id.clone(),
        attempt_start_sha256: payload.attempt_start_sha256.clone(),
        producer: payload.producer.clone(),
        accelerator_observation_sha256: payload.accelerator_observation_sha256.clone(),
        exponent_histogram: checkpoint_exponent_histogram(&payload),
    })
}

/// Validates exact attempt-phase bytes and returns an opaque immutable handle.
pub fn validate_attempt_artifact_files(
    artifact_json: &[u8],
    artifact_sha256: &[u8],
    expected_identity: &RareEventDatasetIdentityV1,
) -> Result<ValidatedAttemptArtifact, ArtifactError> {
    let digest = sha256_hex(artifact_json);
    let envelope = verify_artifact_files(artifact_json, artifact_sha256, expected_identity)?;
    let RareEventPayloadV1::ExecutionAttempt(payload) = envelope.payload else {
        return Err(ArtifactError::Schema(
            "expected an execution-attempt artifact".into(),
        ));
    };
    Ok(ValidatedAttemptArtifact {
        digest,
        payload: *payload,
    })
}

/// Validates the exact closed checkpoint block set in canonical order.
pub fn validate_checkpoint_set(
    identity: &RareEventDatasetIdentityV1,
    checkpoints: Vec<ValidatedCheckpoint>,
) -> Result<ValidatedCheckpointSet, ArtifactError> {
    let expected_dataset_id = dataset_id(identity)?;
    let expected_addresses = expected_checkpoint_block_addresses(identity)?;
    if checkpoints.len() != expected_addresses.len() {
        return Err(ArtifactError::AddressSet(format!(
            "checkpoint set needs exactly {} blocks",
            expected_addresses.len()
        )));
    }
    let mut references = Vec::with_capacity(checkpoints.len());
    for (index, (checkpoint, expected_address)) in
        checkpoints.iter().zip(expected_addresses).enumerate()
    {
        if checkpoint.dataset_id != expected_dataset_id {
            return Err(ArtifactError::Identity(
                "checkpoint handle belongs to a different dataset".into(),
            ));
        }
        if checkpoint.reference.block_address != expected_address {
            return Err(ArtifactError::AddressSet(
                "checkpoint handles are missing, extra, duplicated, or reordered".into(),
            ));
        }
        validate_digest(&checkpoint.reference.checkpoint_sha256)?;
        match identity.scientific {
            ScientificIdentityV1::Target { .. } => {
                validate_histogram(&checkpoint.exponent_histogram, 3, 3 * 1_024, 256)?;
            }
            ScientificIdentityV1::Coverage { .. } => {
                let q = [3_u8, 5, 7][index / (200 * 32)];
                validate_histogram(&checkpoint.exponent_histogram, q, 9, 4_096)?;
            }
        }
        references.push(checkpoint.reference.clone());
    }
    Ok(ValidatedCheckpointSet {
        dataset_id: expected_dataset_id,
        checkpoints,
        references,
    })
}

/// Constructs deterministic block-reference handles for schema conformance fixtures.
///
/// This does not read trajectory records and is unavailable to production builds.
#[cfg(feature = "test-support")]
pub fn validated_checkpoint_set_fixture(
    identity: &RareEventDatasetIdentityV1,
    references: Vec<CheckpointRefV1>,
    attempt_id: &str,
    attempt_start_sha256: &str,
    producer: ProducerBackendV1,
    accelerator_observation_sha256: &str,
    exponent_histograms: Vec<Vec<ExponentBinV1>>,
) -> Result<ValidatedCheckpointSet, ArtifactError> {
    validate_digest(attempt_id)?;
    validate_digest(attempt_start_sha256)?;
    validate_digest(accelerator_observation_sha256)?;
    let dataset_id = dataset_id(identity)?;
    if references.len() != exponent_histograms.len() {
        return Err(ArtifactError::AddressSet(
            "fixture checkpoint references and summaries differ in length".into(),
        ));
    }
    let checkpoints = references
        .into_iter()
        .zip(exponent_histograms)
        .map(|(reference, exponent_histogram)| ValidatedCheckpoint {
            dataset_id: dataset_id.clone(),
            reference,
            attempt_id: attempt_id.to_owned(),
            attempt_start_sha256: attempt_start_sha256.to_owned(),
            producer: producer.clone(),
            accelerator_observation_sha256: accelerator_observation_sha256.to_owned(),
            exponent_histogram,
        })
        .collect();
    validate_checkpoint_set(identity, checkpoints)
}

/// Verifies a gap-free start/terminal chain and exact checkpoint partition.
pub fn validate_execution_lineage(
    identity: &RareEventDatasetIdentityV1,
    phases: &[ValidatedAttemptArtifact],
    checkpoints: &ValidatedCheckpointSet,
) -> Result<ValidatedExecutionLineage, ArtifactError> {
    let expected_dataset_id = dataset_id(identity)?;
    if checkpoints.dataset_id != expected_dataset_id {
        return Err(ArtifactError::Identity(
            "checkpoint set and lineage dataset differ".into(),
        ));
    }
    if phases.is_empty() || !phases.len().is_multiple_of(2) {
        return Err(ArtifactError::Lineage(
            "lineage needs one start and one terminal for every attempt".into(),
        ));
    }

    let mut attempts = Vec::with_capacity(phases.len() / 2);
    let mut claimed_refs = Vec::new();
    let mut prior_terminal_digest: Option<String> = None;
    for (ordinal, pair) in phases.chunks_exact(2).enumerate() {
        let start = &pair[0];
        let terminal = &pair[1];
        let expected_predecessor = match &prior_terminal_digest {
            None => AttemptPredecessorV1::None {},
            Some(terminal_sha256) => AttemptPredecessorV1::Terminal {
                terminal_sha256: terminal_sha256.clone(),
            },
        };
        let expected_attempt_id = attempt_id(identity, ordinal as u64, &expected_predecessor)?;
        if start.payload.dataset_id != expected_dataset_id
            || terminal.payload.dataset_id != expected_dataset_id
            || start.payload.attempt_ordinal != ordinal as u64
            || terminal.payload.attempt_ordinal != ordinal as u64
            || start.payload.predecessor != expected_predecessor
            || terminal.payload.predecessor != expected_predecessor
            || start.payload.attempt_id != expected_attempt_id
            || terminal.payload.attempt_id != expected_attempt_id
        {
            return Err(ArtifactError::Lineage(
                "attempt phases are gapped, reordered, forked, or replaced".into(),
            ));
        }
        let AttemptPhaseV1::Start {
            resume_checkpoint_refs,
            start_utc,
            host_observation,
            accelerator_observation,
            ..
        } = &start.payload.phase
        else {
            return Err(ArtifactError::Lineage(
                "attempt pair does not begin with a start phase".into(),
            ));
        };
        let AttemptPhaseV1::Terminal {
            attempt_start_sha256,
            start_utc: terminal_start_utc,
            host_observation_sha256,
            accelerator_observation_sha256,
            checkpoint_refs,
            ..
        } = &terminal.payload.phase
        else {
            return Err(ArtifactError::Lineage(
                "attempt pair does not end with a terminal phase".into(),
            ));
        };
        if attempt_start_sha256 != &start.digest
            || terminal_start_utc != start_utc
            || host_observation_sha256 != &sha256_hex(&canonical_bytes(host_observation)?)
            || accelerator_observation_sha256
                != &sha256_hex(&canonical_bytes(accelerator_observation)?)
            || resume_checkpoint_refs != &claimed_refs
        {
            return Err(ArtifactError::Lineage(
                "terminal/start link or accepted resume set mismatch".into(),
            ));
        }

        let produced: Vec<_> = checkpoints
            .checkpoints
            .iter()
            .filter(|checkpoint| checkpoint.attempt_id == expected_attempt_id)
            .collect();
        let produced_refs: Vec<_> = produced
            .iter()
            .map(|checkpoint| checkpoint.reference.clone())
            .collect();
        if &produced_refs != checkpoint_refs {
            return Err(ArtifactError::Lineage(
                "terminal checkpoint partition is missing, duplicated, or reordered".into(),
            ));
        }
        for checkpoint in produced {
            if checkpoint.attempt_start_sha256 != start.digest
                || checkpoint.accelerator_observation_sha256 != *accelerator_observation_sha256
            {
                return Err(ArtifactError::Lineage(
                    "checkpoint does not link to its producing start observation".into(),
                ));
            }
            validate_producer(&checkpoint.producer, accelerator_observation)?;
        }
        claimed_refs.extend(produced_refs);
        attempts.push(AttemptRefV1 {
            attempt_id: expected_attempt_id,
            attempt_start_sha256: start.digest.clone(),
            attempt_terminal_sha256: terminal.digest.clone(),
        });
        prior_terminal_digest = Some(terminal.digest.clone());
    }
    if claimed_refs != checkpoints.references {
        return Err(ArtifactError::Lineage(
            "attempt terminals do not partition the exact checkpoint set".into(),
        ));
    }
    Ok(ValidatedExecutionLineage {
        dataset_id: expected_dataset_id,
        provenance: ExecutionProvenanceV1 {
            attempts,
            checkpoint_refs: claimed_refs,
        },
    })
}

/// Validates final bytes against opaque checkpoint and execution handles.
pub fn validate_final_artifact_files(
    artifact_json: &[u8],
    artifact_sha256: &[u8],
    expected_identity: &RareEventDatasetIdentityV1,
    checkpoints: &ValidatedCheckpointSet,
    execution_lineage: &ValidatedExecutionLineage,
) -> Result<ValidatedFinalReceipt, ArtifactError> {
    let envelope = verify_artifact_files(artifact_json, artifact_sha256, expected_identity)?;
    if checkpoints.dataset_id != dataset_id(expected_identity)?
        || execution_lineage.dataset_id != checkpoints.dataset_id
        || execution_lineage.provenance.checkpoint_refs != checkpoints.references
    {
        return Err(ArtifactError::Lineage(
            "final verifier inputs do not share one dataset/checkpoint partition".into(),
        ));
    }
    match &envelope.payload {
        RareEventPayloadV1::TargetCrossCheck(receipt) => {
            if receipt.execution_provenance != execution_lineage.provenance {
                return Err(ArtifactError::Lineage(
                    "target final provenance differs from validated execution lineage".into(),
                ));
            }
            validate_target_result_against_checkpoints(&receipt.result_payload, checkpoints)?;
        }
        RareEventPayloadV1::CoverageValidation(receipt) => {
            if receipt.execution_provenance != execution_lineage.provenance {
                return Err(ArtifactError::Lineage(
                    "coverage final provenance differs from validated execution lineage".into(),
                ));
            }
            validate_coverage_result_against_checkpoints(&receipt.result_payload, checkpoints)?;
        }
        _ => {
            return Err(ArtifactError::Schema(
                "expected a target or coverage final receipt".into(),
            ))
        }
    }
    Ok(ValidatedFinalReceipt {
        digest: sha256_hex(artifact_json),
        envelope,
    })
}

/// Wraps and validates a target final receipt from checked inputs.
pub fn target_final_envelope(
    identity: RareEventDatasetIdentityV1,
    result_payload: TargetResultPayloadV1,
    checkpoints: &ValidatedCheckpointSet,
    execution_lineage: &ValidatedExecutionLineage,
) -> Result<RareEventArtifactEnvelopeV1, ArtifactError> {
    let expected_dataset_id = dataset_id(&identity)?;
    if checkpoints.dataset_id != expected_dataset_id
        || execution_lineage.dataset_id != expected_dataset_id
        || checkpoints.references != execution_lineage.provenance.checkpoint_refs
    {
        return Err(ArtifactError::Lineage(
            "target final inputs do not share one validated checkpoint lineage".into(),
        ));
    }
    validate_target_result_against_checkpoints(&result_payload, checkpoints)?;
    let result_sha256 = sha256_hex(&canonical_bytes(&result_payload)?);
    let receipt = TargetCrossCheckReceiptV1 {
        dataset_id: dataset_id(&identity)?,
        dataset_identity: identity,
        result_payload,
        result_sha256,
        execution_provenance: execution_lineage.provenance.clone(),
    };
    let envelope = RareEventArtifactEnvelopeV1 {
        envelope_schema: ENVELOPE_SCHEMA_V1.into(),
        artifact_kind: ArtifactKindV1::TargetCrossCheck,
        payload: RareEventPayloadV1::TargetCrossCheck(Box::new(receipt)),
    };
    validate_envelope(&envelope)?;
    Ok(envelope)
}

/// Wraps and validates a coverage final receipt from checked inputs.
pub fn coverage_final_envelope(
    identity: RareEventDatasetIdentityV1,
    result_payload: CoverageResultPayloadV1,
    checkpoints: &ValidatedCheckpointSet,
    execution_lineage: &ValidatedExecutionLineage,
) -> Result<RareEventArtifactEnvelopeV1, ArtifactError> {
    let expected_dataset_id = dataset_id(&identity)?;
    if checkpoints.dataset_id != expected_dataset_id
        || execution_lineage.dataset_id != expected_dataset_id
        || checkpoints.references != execution_lineage.provenance.checkpoint_refs
    {
        return Err(ArtifactError::Lineage(
            "coverage final inputs do not share one validated checkpoint lineage".into(),
        ));
    }
    validate_coverage_result_against_checkpoints(&result_payload, checkpoints)?;
    let result_sha256 = sha256_hex(&canonical_bytes(&result_payload)?);
    let receipt = CoverageValidationReceiptV1 {
        dataset_id: dataset_id(&identity)?,
        dataset_identity: identity,
        result_payload,
        result_sha256,
        execution_provenance: execution_lineage.provenance.clone(),
    };
    let envelope = RareEventArtifactEnvelopeV1 {
        envelope_schema: ENVELOPE_SCHEMA_V1.into(),
        artifact_kind: ArtifactKindV1::CoverageValidation,
        payload: RareEventPayloadV1::CoverageValidation(Box::new(receipt)),
    };
    validate_envelope(&envelope)?;
    Ok(envelope)
}

fn validate_configuration(configuration: &RareEventConfigurationV1) -> Result<(), ArtifactError> {
    validate_relative_path(&configuration.artifact_root)?;
    if configuration.artifact_root == RAW_CAMPAIGN_ROOT
        || configuration
            .artifact_root
            .starts_with(&format!("{RAW_CAMPAIGN_ROOT}/"))
    {
        return Err(ArtifactError::Schema(
            "artifact root must lie outside raw campaign samples".into(),
        ));
    }
    validate_design(&configuration.design_identity)?;
    validate_scientific(&configuration.scientific_identity)?;
    validate_behavior(&configuration.behavior)
}

fn validate_envelope(envelope: &RareEventArtifactEnvelopeV1) -> Result<(), ArtifactError> {
    if envelope.envelope_schema != ENVELOPE_SCHEMA_V1 {
        return Err(ArtifactError::Schema("unknown envelope schema".into()));
    }
    let expected_kind = match &envelope.payload {
        RareEventPayloadV1::TrajectoryCheckpoint(payload) => {
            validate_common(&payload.dataset_identity, &payload.dataset_id)?;
            validate_checkpoint(payload)?;
            ArtifactKindV1::TrajectoryCheckpoint
        }
        RareEventPayloadV1::ExecutionAttempt(payload) => {
            validate_common(&payload.dataset_identity, &payload.dataset_id)?;
            validate_attempt(payload)?;
            ArtifactKindV1::ExecutionAttempt
        }
        RareEventPayloadV1::TargetCrossCheck(payload) => {
            validate_common(&payload.dataset_identity, &payload.dataset_id)?;
            if payload.dataset_identity.scientific != ScientificIdentityV1::target() {
                return Err(ArtifactError::Identity(
                    "target receipt embeds a non-target dataset identity".into(),
                ));
            }
            if payload.result_sha256 != sha256_hex(&canonical_bytes(&payload.result_payload)?) {
                return Err(ArtifactError::Integrity(
                    "target result payload digest mismatch".into(),
                ));
            }
            validate_target_result(&payload.result_payload)?;
            validate_final_refs(
                &payload.dataset_identity,
                &payload.execution_provenance,
                2_048,
            )?;
            ArtifactKindV1::TargetCrossCheck
        }
        RareEventPayloadV1::CoverageValidation(payload) => {
            validate_common(&payload.dataset_identity, &payload.dataset_id)?;
            if payload.dataset_identity.scientific != ScientificIdentityV1::coverage() {
                return Err(ArtifactError::Identity(
                    "coverage receipt embeds a non-coverage dataset identity".into(),
                ));
            }
            if payload.result_sha256 != sha256_hex(&canonical_bytes(&payload.result_payload)?) {
                return Err(ArtifactError::Integrity(
                    "coverage result payload digest mismatch".into(),
                ));
            }
            validate_coverage_result(&payload.result_payload)?;
            validate_final_refs(
                &payload.dataset_identity,
                &payload.execution_provenance,
                19_200,
            )?;
            ArtifactKindV1::CoverageValidation
        }
    };
    if envelope.artifact_kind != expected_kind {
        return Err(ArtifactError::Schema(
            "artifact kind does not select payload schema".into(),
        ));
    }
    Ok(())
}

fn validate_common(
    identity: &RareEventDatasetIdentityV1,
    embedded_id: &str,
) -> Result<(), ArtifactError> {
    validate_identity(identity)?;
    if embedded_id != dataset_id(identity)? {
        return Err(ArtifactError::Identity(
            "dataset ID recomputation mismatch".into(),
        ));
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ExactValue {
    numerator: BigUint,
    denominator: BigUint,
}

impl ExactValue {
    fn new(numerator: BigUint, denominator: BigUint) -> Result<Self, ArtifactError> {
        if denominator == BigUint::from(0_u8) {
            return Err(ArtifactError::Schema(
                "exact denominator must be positive".into(),
            ));
        }
        let divisor = gcd(numerator.clone(), denominator.clone());
        Ok(Self {
            numerator: numerator / &divisor,
            denominator: denominator / divisor,
        })
    }

    fn zero() -> Self {
        Self::from_integer(0_u8)
    }

    fn one() -> Self {
        Self::from_integer(1_u8)
    }

    fn from_integer(value: impl Into<BigUint>) -> Self {
        Self {
            numerator: value.into(),
            denominator: BigUint::from(1_u8),
        }
    }

    fn add(&self, other: &Self) -> Self {
        Self::new(
            &self.numerator * &other.denominator + &other.numerator * &self.denominator,
            &self.denominator * &other.denominator,
        )
        .expect("product of positive exact denominators is positive")
    }

    fn multiply(&self, other: &Self) -> Self {
        Self::new(
            &self.numerator * &other.numerator,
            &self.denominator * &other.denominator,
        )
        .expect("product of positive exact denominators is positive")
    }

    fn divide(&self, other: &Self) -> Result<Self, ArtifactError> {
        Self::new(
            &self.numerator * &other.denominator,
            &self.denominator * &other.numerator,
        )
    }

    fn abs_diff(&self, other: &Self) -> Self {
        let left = &self.numerator * &other.denominator;
        let right = &other.numerator * &self.denominator;
        let numerator = if left >= right {
            left - right
        } else {
            right - left
        };
        Self::new(numerator, &self.denominator * &other.denominator)
            .expect("product of positive exact denominators is positive")
    }
}

impl Ord for ExactValue {
    fn cmp(&self, other: &Self) -> Ordering {
        (&self.numerator * &other.denominator).cmp(&(&other.numerator * &self.denominator))
    }
}

impl PartialOrd for ExactValue {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn gcd(mut left: BigUint, mut right: BigUint) -> BigUint {
    while right != BigUint::from(0_u8) {
        let remainder = &left % &right;
        left = right;
        right = remainder;
    }
    left
}

fn parse_canonical_integer(value: &str, name: &str) -> Result<BigUint, ArtifactError> {
    if value.is_empty()
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || (value.len() > 1 && value.starts_with('0'))
    {
        return Err(ArtifactError::Schema(format!(
            "{name} is not a canonical nonnegative decimal integer"
        )));
    }
    BigUint::parse_bytes(value.as_bytes(), 10)
        .ok_or_else(|| ArtifactError::Schema(format!("{name} is not a decimal integer")))
}

fn parse_exact(value: &ExactDecimalV1, name: &str) -> Result<ExactValue, ArtifactError> {
    let numerator = parse_canonical_integer(&value.numerator, name)?;
    let denominator = parse_canonical_integer(&value.denominator, name)?;
    let exact = ExactValue::new(numerator.clone(), denominator.clone())?;
    if exact.numerator != numerator || exact.denominator != denominator {
        return Err(ArtifactError::Schema(format!(
            "{name} is not a reduced exact decimal pair"
        )));
    }
    Ok(exact)
}

fn parse_probability(value: &ExactDecimalV1, name: &str) -> Result<ExactValue, ArtifactError> {
    let value = parse_exact(value, name)?;
    if value > ExactValue::one() {
        return Err(ArtifactError::Schema(format!(
            "{name} lies outside the closed unit interval"
        )));
    }
    Ok(value)
}

fn exact_mean(values: &[ExactValue]) -> ExactValue {
    values
        .iter()
        .fold(ExactValue::zero(), |sum, value| sum.add(value))
        .divide(&ExactValue::from_integer(values.len()))
        .expect("a nonempty exact mean has a positive divisor")
}

fn exact_sample_variance(values: &[ExactValue]) -> ExactValue {
    let mean = exact_mean(values);
    values
        .iter()
        .map(|value| value.abs_diff(&mean))
        .map(|difference| difference.multiply(&difference))
        .fold(ExactValue::zero(), |sum, square| sum.add(&square))
        .divide(&ExactValue::from_integer(values.len() - 1))
        .expect("multiple independent runs have a positive variance divisor")
}

fn interval_radius_squared(
    independent_run_variance: &ExactValue,
) -> Result<ExactValue, ArtifactError> {
    let critical = ExactValue::new(
        BigUint::from(1_019_756_723_u64),
        BigUint::from(500_000_000_u64),
    )?;
    critical
        .multiply(&critical)
        .multiply(independent_run_variance)
        .divide(&ExactValue::from_integer(TARGET_RUNS))
}

fn validate_interval(
    lower: &str,
    upper: &str,
    center: &ExactValue,
    independent_run_variance: &ExactValue,
    scale_base: u32,
    scale_exponent: u32,
) -> Result<(), ArtifactError> {
    let interval = ScaledStudentInterval::from_exact_independent_runs(
        center.numerator.clone(),
        center.denominator.clone(),
        independent_run_variance.numerator.clone(),
        independent_run_variance.denominator.clone(),
        BigUint::from(1_019_756_723_u64),
        BigUint::from(500_000_000_u64),
        usize::from(TARGET_RUNS),
        scale_base,
        scale_exponent,
    )
    .map_err(|error| ArtifactError::Schema(error.to_string()))?;
    let rendered = interval
        .render_outward(18)
        .map_err(|error| ArtifactError::Schema(error.to_string()))?;
    if lower != rendered.lower || upper != rendered.upper {
        return Err(ArtifactError::Schema(
            "interval endpoints differ from adjacent 18-digit outward rendering".into(),
        ));
    }
    Ok(())
}

fn interval_contains(
    value: &ExactValue,
    center: &ExactValue,
    independent_run_variance: &ExactValue,
) -> Result<bool, ArtifactError> {
    let difference = value.abs_diff(center);
    Ok(difference.multiply(&difference) <= interval_radius_squared(independent_run_variance)?)
}

fn validate_histogram(
    bins: &[ExponentBinV1],
    base: u8,
    maximum_exponent: u32,
    expected_count: u64,
) -> Result<(ExactValue, ExactValue, ExactValue), ArtifactError> {
    let observed_count = bins.iter().try_fold(0_u64, |sum, bin| {
        sum.checked_add(bin.count)
            .ok_or_else(|| ArtifactError::AddressSet("exponent histogram count overflow".into()))
    })?;
    if bins.is_empty()
        || bins.iter().any(|bin| bin.count == 0)
        || bins
            .windows(2)
            .any(|window| window[0].exponent >= window[1].exponent)
        || bins.last().expect("nonempty histogram").exponent > maximum_exponent
        || observed_count != expected_count
    {
        return Err(ArtifactError::AddressSet(
            "exponent histogram is incomplete, unordered, or out of range".into(),
        ));
    }
    let maximum_observed = bins.last().expect("nonempty histogram").exponent;
    let base = BigUint::from(base);
    let weight_sum = bins.iter().fold(BigUint::from(0_u8), |sum, bin| {
        sum + BigUint::from(bin.count) * base.pow(maximum_observed - bin.exponent)
    });
    let squared_weight_sum = bins.iter().fold(BigUint::from(0_u8), |sum, bin| {
        sum + BigUint::from(bin.count) * base.pow(2 * (maximum_observed - bin.exponent))
    });
    let minimum_observed = bins.first().expect("nonempty histogram").exponent;
    let largest_scaled_weight = base.pow(maximum_observed - minimum_observed);
    Ok((
        ExactValue::new(&weight_sum * &weight_sum, squared_weight_sum)?,
        ExactValue::new(largest_scaled_weight, weight_sum.clone())?,
        ExactValue::new(
            weight_sum,
            BigUint::from(expected_count) * base.pow(maximum_observed),
        )?,
    ))
}

fn merge_histograms<'a>(
    histograms: impl IntoIterator<Item = &'a [ExponentBinV1]>,
    maximum_exponent: u32,
) -> Result<Vec<ExponentBinV1>, ArtifactError> {
    let mut counts = vec![0_u64; maximum_exponent as usize + 1];
    for histogram in histograms {
        for bin in histogram {
            counts[bin.exponent as usize] = counts[bin.exponent as usize]
                .checked_add(bin.count)
                .ok_or_else(|| ArtifactError::AddressSet("histogram count overflow".into()))?;
        }
    }
    Ok(counts
        .into_iter()
        .enumerate()
        .filter_map(|(exponent, count)| {
            (count != 0).then_some(ExponentBinV1 {
                exponent: exponent as u32,
                count,
            })
        })
        .collect())
}

fn validate_target_result_against_checkpoints(
    payload: &TargetResultPayloadV1,
    checkpoints: &ValidatedCheckpointSet,
) -> Result<(), ArtifactError> {
    if checkpoints.checkpoints.len() != 2_048 {
        return Err(ArtifactError::AddressSet(
            "target reducer needs every validated checkpoint".into(),
        ));
    }
    for run in 0..usize::from(TARGET_RUNS) {
        let blocks = &checkpoints.checkpoints[run * 64..(run + 1) * 64];
        let histogram = merge_histograms(
            blocks
                .iter()
                .map(|checkpoint| checkpoint.exponent_histogram.as_slice()),
            3 * 1_024,
        )?;
        let (ess, _, mean) = validate_histogram(
            &histogram,
            3,
            3 * 1_024,
            u64::from(TARGET_TRAJECTORIES_PER_RUN),
        )?;
        if parse_exact(&payload.per_run_ess[run], "per-run checkpoint-backed ESS")? != ess
            || parse_probability(&payload.run_means[run], "checkpoint-backed run mean")? != mean
        {
            return Err(ArtifactError::AddressSet(
                "target per-run statistics disagree with validated checkpoints".into(),
            ));
        }
    }
    let pooled = merge_histograms(
        checkpoints
            .checkpoints
            .iter()
            .map(|checkpoint| checkpoint.exponent_histogram.as_slice()),
        3 * 1_024,
    )?;
    if pooled != payload.exponent_histogram {
        return Err(ArtifactError::AddressSet(
            "target pooled histogram disagrees with validated checkpoints".into(),
        ));
    }
    Ok(())
}

fn validate_coverage_result_against_checkpoints(
    payload: &CoverageResultPayloadV1,
    checkpoints: &ValidatedCheckpointSet,
) -> Result<(), ArtifactError> {
    if checkpoints.checkpoints.len() != 19_200 {
        return Err(ArtifactError::AddressSet(
            "coverage reducer needs every validated checkpoint".into(),
        ));
    }
    for (case_slot, q) in [3_u8, 5, 7].into_iter().enumerate() {
        for replicate in 0..usize::from(COVERAGE_REPLICATES) {
            let result =
                &payload.replicates[case_slot * usize::from(COVERAGE_REPLICATES) + replicate];
            let offset = (case_slot * usize::from(COVERAGE_REPLICATES) + replicate)
                * usize::from(COVERAGE_RUNS);
            let runs = &checkpoints.checkpoints[offset..offset + usize::from(COVERAGE_RUNS)];
            for (run, checkpoint) in runs.iter().enumerate() {
                let (_, _, mean) = validate_histogram(&checkpoint.exponent_histogram, q, 9, 4_096)?;
                if parse_probability(&result.run_means[run], "coverage checkpoint run mean")?
                    != mean
                {
                    return Err(ArtifactError::AddressSet(
                        "coverage run mean disagrees with validated checkpoint".into(),
                    ));
                }
            }
            let pooled = merge_histograms(
                runs.iter()
                    .map(|checkpoint| checkpoint.exponent_histogram.as_slice()),
                9,
            )?;
            if pooled != result.exponent_histogram {
                return Err(ArtifactError::AddressSet(
                    "coverage replicate histogram disagrees with validated checkpoints".into(),
                ));
            }
        }
    }
    Ok(())
}

fn validate_extinction_reasons(reasons: &[String]) -> Result<(), ArtifactError> {
    if reasons.iter().any(String::is_empty) {
        return Err(ArtifactError::Schema(
            "extinction diagnostics may not contain an empty reason".into(),
        ));
    }
    ensure_sorted_unique(reasons, "extinction diagnostics")
}

fn validate_target_result(payload: &TargetResultPayloadV1) -> Result<(), ArtifactError> {
    if payload.expected_trajectory_count != 524_288 {
        return Err(ArtifactError::AddressSet(
            "target result count must be 524288".into(),
        ));
    }
    validate_relative_path(&payload.exact_result_path)?;
    validate_digest(&payload.exact_result_sha256)?;
    let raw_count = parse_canonical_integer(&payload.exact_raw_count, "exact raw count")?;
    let raw_total = parse_canonical_integer(&payload.exact_total, "exact total")?;
    let exact_probability = parse_probability(&payload.exact_probability, "exact probability")?;
    if raw_count > raw_total
        || raw_total != BigUint::from(3_u8).pow(3_072)
        || ExactValue::new(raw_count, raw_total)? != exact_probability
    {
        return Err(ArtifactError::Schema(
            "raw exact counts disagree with the reduced exact probability".into(),
        ));
    }

    if payload.run_means.len() != usize::from(TARGET_RUNS)
        || payload.per_run_ess.len() != usize::from(TARGET_RUNS)
    {
        return Err(ArtifactError::AddressSet(
            "target diagnostics need exactly 32 independent runs".into(),
        ));
    }
    let run_means: Vec<_> = payload
        .run_means
        .iter()
        .map(|value| parse_probability(value, "target run mean"))
        .collect::<Result<_, _>>()?;
    let estimate = parse_probability(&payload.cross_check_estimate, "target estimate")?;
    if estimate != exact_mean(&run_means) {
        return Err(ArtifactError::Schema(
            "target estimate differs from exact independent-run mean".into(),
        ));
    }
    let variance = parse_exact(
        &payload.independent_run_variance,
        "target independent-run variance",
    )?;
    if variance != exact_sample_variance(&run_means) {
        return Err(ArtifactError::Schema(
            "target variance differs from exact independent-run variance".into(),
        ));
    }
    validate_interval(
        &payload.interval_lower,
        &payload.interval_upper,
        &estimate,
        &variance,
        3,
        payload.minimum_exponent,
    )?;

    let (computed_ess, computed_largest_weight_share, histogram_mean) =
        validate_histogram(&payload.exponent_histogram, 3, 3 * 1_024, 524_288)?;
    if payload.minimum_exponent
        != payload
            .exponent_histogram
            .first()
            .expect("validated nonempty histogram")
            .exponent
        || payload.maximum_exponent
            != payload
                .exponent_histogram
                .last()
                .expect("validated nonempty histogram")
                .exponent
    {
        return Err(ArtifactError::Schema(
            "target exponent extrema disagree with histogram".into(),
        ));
    }
    if estimate != histogram_mean {
        return Err(ArtifactError::Schema(
            "target estimate disagrees with exponent histogram".into(),
        ));
    }
    let final_ess = parse_exact(&payload.final_weight_ess, "target final-weight ESS")?;
    if final_ess != computed_ess {
        return Err(ArtifactError::Schema(
            "target ESS disagrees with its exponent histogram".into(),
        ));
    }
    let ess_fraction = parse_probability(&payload.ess_fraction, "target ESS fraction")?;
    if ess_fraction
        != final_ess.divide(&ExactValue::from_integer(payload.expected_trajectory_count))?
    {
        return Err(ArtifactError::Schema(
            "target ESS fraction disagrees with final-weight ESS".into(),
        ));
    }
    for per_run in &payload.per_run_ess {
        let per_run = parse_exact(per_run, "per-run final-weight ESS")?;
        if per_run == ExactValue::zero()
            || per_run > ExactValue::from_integer(TARGET_TRAJECTORIES_PER_RUN)
        {
            return Err(ArtifactError::Schema(
                "per-run ESS lies outside its fixed allocation".into(),
            ));
        }
    }
    if parse_probability(
        &payload.largest_weight_share,
        "largest normalized weight share",
    )? != computed_largest_weight_share
    {
        return Err(ArtifactError::Schema(
            "largest weight share disagrees with exponent histogram".into(),
        ));
    }
    let run_sum = run_means
        .iter()
        .fold(ExactValue::zero(), |sum, value| sum.add(value));
    if run_sum == ExactValue::zero() {
        return Err(ArtifactError::Schema("target run-mean sum is zero".into()));
    }
    let largest_run_share = run_means
        .iter()
        .max()
        .expect("32 target run means")
        .divide(&run_sum)?;
    if parse_probability(&payload.largest_run_mean_share, "largest run-mean share")?
        != largest_run_share
    {
        return Err(ArtifactError::Schema(
            "largest run-mean share disagrees with run means".into(),
        ));
    }

    validate_extinction_reasons(&payload.extinction_reasons)?;
    let degeneracy = ess_fraction < ExactValue::new(1_u8.into(), 100_u8.into())?;
    if payload.degeneracy != degeneracy {
        return Err(ArtifactError::Schema(
            "target degeneracy disagrees with fixed ESS threshold".into(),
        ));
    }
    let expected_verdict = if !payload.extinction_reasons.is_empty() || degeneracy {
        CrossCheckVerdictV1::Unusable
    } else if interval_contains(&exact_probability, &estimate, &variance)? {
        CrossCheckVerdictV1::Agreement
    } else {
        CrossCheckVerdictV1::Contradiction
    };
    if payload.verdict != expected_verdict {
        return Err(ArtifactError::Schema(
            "target verdict disagrees with exact containment/usability".into(),
        ));
    }
    Ok(())
}

fn validate_coverage_result(payload: &CoverageResultPayloadV1) -> Result<(), ArtifactError> {
    const REPLICATE_TRAJECTORIES: u64 = 32 * 4_096;
    if payload.expected_trajectory_count != 78_643_200 || payload.replicates.len() != 600 {
        return Err(ArtifactError::AddressSet(
            "coverage result must contain exactly 78643200 addressed weights".into(),
        ));
    }
    let cases = [
        (3_u8, "907", "2187"),
        (5, "17581", "78125"),
        (7, "126295", "823543"),
    ];
    let mut recomputed_counts = Vec::with_capacity(3);
    let mut has_extinction = false;
    for (case_index, &(q, numerator, denominator)) in cases.iter().enumerate() {
        let anchor = ExactValue::new(
            parse_canonical_integer(numerator, "coverage anchor")?,
            parse_canonical_integer(denominator, "coverage anchor")?,
        )?;
        let mut containing = 0_u16;
        for replicate in 0..COVERAGE_REPLICATES {
            let item = &payload.replicates
                [case_index * usize::from(COVERAGE_REPLICATES) + usize::from(replicate)];
            if item.q != q || item.replicate != replicate {
                return Err(ArtifactError::AddressSet(
                    "coverage replicates are not in exact (q,b) order".into(),
                ));
            }
            if parse_probability(&item.exact_anchor, "coverage exact anchor")? != anchor {
                return Err(ArtifactError::Schema(
                    "coverage replicate anchor differs from preregistration".into(),
                ));
            }
            if item.run_means.len() != usize::from(COVERAGE_RUNS) {
                return Err(ArtifactError::AddressSet(
                    "coverage replicate needs exactly 32 independent runs".into(),
                ));
            }
            let run_means: Vec<_> = item
                .run_means
                .iter()
                .map(|value| parse_probability(value, "coverage run mean"))
                .collect::<Result<_, _>>()?;
            let estimate = parse_probability(&item.estimate, "coverage estimate")?;
            if estimate != exact_mean(&run_means) {
                return Err(ArtifactError::Schema(
                    "coverage estimate differs from exact independent-run mean".into(),
                ));
            }
            let variance = parse_exact(
                &item.independent_run_variance,
                "coverage independent-run variance",
            )?;
            if variance != exact_sample_variance(&run_means) {
                return Err(ArtifactError::Schema(
                    "coverage variance differs from exact independent-run variance".into(),
                ));
            }
            let (computed_ess, _, histogram_mean) =
                validate_histogram(&item.exponent_histogram, q, 9, REPLICATE_TRAJECTORIES)?;
            validate_interval(
                &item.interval_lower,
                &item.interval_upper,
                &estimate,
                &variance,
                u32::from(q),
                item.exponent_histogram
                    .first()
                    .expect("validated nonempty histogram")
                    .exponent,
            )?;
            let contains = interval_contains(&anchor, &estimate, &variance)?;
            if item.contains_anchor != contains {
                return Err(ArtifactError::Schema(
                    "coverage containment bit disagrees with exact interval".into(),
                ));
            }
            containing += u16::from(contains);

            if estimate != histogram_mean {
                return Err(ArtifactError::Schema(
                    "coverage estimate disagrees with exponent histogram".into(),
                ));
            }
            let final_ess = parse_exact(&item.final_weight_ess, "coverage final-weight ESS")?;
            if final_ess != computed_ess {
                return Err(ArtifactError::Schema(
                    "coverage ESS disagrees with exponent histogram".into(),
                ));
            }
            let ess_fraction = parse_probability(&item.ess_fraction, "coverage ESS fraction")?;
            if ess_fraction
                != final_ess.divide(&ExactValue::from_integer(REPLICATE_TRAJECTORIES))?
            {
                return Err(ArtifactError::Schema(
                    "coverage ESS fraction disagrees with final-weight ESS".into(),
                ));
            }
            let degeneracy = ess_fraction < ExactValue::new(1_u8.into(), 100_u8.into())?;
            if item.degeneracy != degeneracy {
                return Err(ArtifactError::Schema(
                    "coverage degeneracy disagrees with fixed ESS threshold".into(),
                ));
            }
            validate_extinction_reasons(&item.extinction_reasons)?;
            has_extinction |= !item.extinction_reasons.is_empty();
        }
        recomputed_counts.push(CoverageCountV1 {
            q,
            count: containing,
        });
    }
    if payload.coverage_counts != recomputed_counts {
        return Err(ArtifactError::Schema(
            "coverage counts disagree with exact containment bits".into(),
        ));
    }
    let adequate = !has_extinction && payload.coverage_counts.iter().all(|item| item.count >= 180);
    if payload.verdict
        != if adequate {
            CoverageVerdictV1::Adequate
        } else {
            CoverageVerdictV1::Unusable
        }
    {
        return Err(ArtifactError::Schema(
            "coverage verdict disagrees with fixed 180/200 rule".into(),
        ));
    }
    Ok(())
}

fn validate_identity(identity: &RareEventDatasetIdentityV1) -> Result<(), ArtifactError> {
    if identity.campaign != CampaignAuthorityV1::default() {
        return Err(ArtifactError::Identity(
            "campaign authority differs from frozen constants".into(),
        ));
    }
    validate_design(&identity.preregistration.design)?;
    validate_relative_path(&identity.preregistration.configuration_path)?;
    if identity.preregistration.configuration_schema != CONFIGURATION_SCHEMA_V1 {
        return Err(ArtifactError::Identity(
            "unknown configuration identity".into(),
        ));
    }
    validate_digest(&identity.preregistration.configuration_sha256)?;
    validate_scientific(&identity.scientific)?;
    if identity.statistical != StatisticalConstantsV1::default() {
        return Err(ArtifactError::Identity(
            "statistical constants differ from preregistration".into(),
        ));
    }
    validate_behavior(&identity.behavior)
}

fn validate_design(design: &DesignIdentityV1) -> Result<(), ArtifactError> {
    if design.path != "dev/active/3f664839/design.md" {
        return Err(ArtifactError::Identity(
            "design path is not the preregistration path".into(),
        ));
    }
    validate_hex(&design.git_revision, 40)?;
    validate_hex(&design.blob_id, 40)?;
    validate_digest(&design.content_sha256)
}

fn validate_scientific(scientific: &ScientificIdentityV1) -> Result<(), ArtifactError> {
    if scientific != &ScientificIdentityV1::target()
        && scientific != &ScientificIdentityV1::coverage()
    {
        return Err(ArtifactError::Identity(
            "scientific allocation differs from registered target/coverage".into(),
        ));
    }
    Ok(())
}

fn validate_behavior(behavior: &BehaviorIdentityV1) -> Result<(), ArtifactError> {
    if behavior.sampler != SAMPLER_V1
        || behavior.address_to_seed != ADDRESS_TO_SEED_V1
        || behavior.rng_algorithm != RNG_V1
        || behavior.rng_crate_version != "0.9.0"
        || behavior.serializer != SERIALIZER_V1
    {
        return Err(ArtifactError::Identity(
            "unknown sampler/RNG/serializer behavior".into(),
        ));
    }
    if behavior.source_dirty {
        return Err(ArtifactError::Identity(
            "dirty producer is forbidden".into(),
        ));
    }
    for digest in [
        &behavior.estimator_behavior_sha256,
        &behavior.executable_sha256,
        &behavior.closure.cargo_lock_sha256,
    ] {
        validate_digest(digest)?;
    }
    if behavior.closure.behavior_schema != BEHAVIOR_CLOSURE_SCHEMA_V1 {
        return Err(ArtifactError::Identity(
            "unknown behavior-closure schema".into(),
        ));
    }
    ensure_sorted_unique(&behavior.closure.enabled_features, "enabled features")?;
    ensure_sorted_unique(&behavior.closure.repository_inputs, "repository inputs")?;
    ensure_sorted_unique(&behavior.closure.packages, "package closure")?;
    ensure_sorted_unique(
        &behavior.closure.environment_input_names,
        "environment declarations",
    )?;
    for input in &behavior.closure.repository_inputs {
        validate_relative_path(&input.path)?;
        validate_hex(&input.blob_id, 40)?;
        validate_digest(&input.content_sha256)?;
    }
    let closure_digest = sha256_hex(&canonical_bytes(&behavior.closure)?);
    if closure_digest != behavior.estimator_behavior_sha256 {
        return Err(ArtifactError::Identity(
            "behavior closure digest mismatch".into(),
        ));
    }
    validate_hex(&behavior.source_revision, 40)
}

fn validate_worker(worker: &WorkerConfigurationV1) -> Result<(), ArtifactError> {
    if worker.requested_workers == 0 || worker.effective_workers == 0 {
        return Err(ArtifactError::Schema(
            "worker counts must be positive".into(),
        ));
    }
    ensure_sorted_unique(&worker.effective_devices, "effective devices")
}

fn validate_checkpoint(payload: &TrajectoryCheckpointV1) -> Result<(), ArtifactError> {
    if payload.run_id != run_id(&payload.dataset_identity, &payload.run_address)? {
        return Err(ArtifactError::Identity(
            "run ID recomputation mismatch".into(),
        ));
    }
    validate_digest(&payload.attempt_start_sha256)?;
    validate_digest(&payload.accelerator_observation_sha256)?;
    validate_digest(&payload.attempt_id)?;
    match (
        &payload.run_address,
        &payload.records,
        &payload.dataset_identity.scientific,
    ) {
        (
            RunAddressV1::Target { run },
            TrajectoryRecordsV1::Target(records),
            ScientificIdentityV1::Target { .. },
        ) => {
            if *run >= TARGET_RUNS
                || payload.block_index >= 64
                || payload.trajectory_start != u32::from(payload.block_index) * TARGET_BLOCK_SIZE
                || payload.trajectory_end != payload.trajectory_start + TARGET_BLOCK_SIZE
                || records.len() != TARGET_BLOCK_SIZE as usize
            {
                return Err(ArtifactError::AddressSet(
                    "target checkpoint block shape mismatch".into(),
                ));
            }
            for (offset, record) in records.iter().enumerate() {
                let trajectory = payload.trajectory_start + offset as u32;
                if record.run != *run
                    || record.trajectory != trajectory
                    || record.stream_index
                        != target_address(*run, trajectory)
                            .map_err(|error| ArtifactError::AddressSet(error.to_string()))?
                            .stream()
                            .get()
                    || record.exponent > 3 * 1_024
                {
                    return Err(ArtifactError::AddressSet(
                        "target record address/order/exponent mismatch".into(),
                    ));
                }
            }
        }
        (
            RunAddressV1::Coverage { q, replicate, run },
            TrajectoryRecordsV1::Coverage(records),
            ScientificIdentityV1::Coverage { .. },
        ) => {
            if ![3, 5, 7].contains(q)
                || *replicate >= COVERAGE_REPLICATES
                || *run >= COVERAGE_RUNS
                || payload.block_index != 0
                || payload.trajectory_start != 0
                || payload.trajectory_end != COVERAGE_TRAJECTORIES_PER_RUN
                || records.len() != COVERAGE_TRAJECTORIES_PER_RUN as usize
            {
                return Err(ArtifactError::AddressSet(
                    "coverage checkpoint block shape mismatch".into(),
                ));
            }
            for (trajectory, record) in records.iter().enumerate() {
                if record.q != *q
                    || record.replicate != *replicate
                    || record.run != *run
                    || record.trajectory != trajectory as u32
                    || record.stream_index
                        != coverage_address(*q, *replicate, *run, trajectory as u32)
                            .map_err(|error| ArtifactError::AddressSet(error.to_string()))?
                            .stream()
                            .get()
                    || record.exponent > 9
                {
                    return Err(ArtifactError::AddressSet(
                        "coverage record address/order/exponent mismatch".into(),
                    ));
                }
            }
        }
        _ => {
            return Err(ArtifactError::Schema(
                "checkpoint mode/records/identity mismatch".into(),
            ))
        }
    }
    Ok(())
}

fn checkpoint_block_address(payload: &TrajectoryCheckpointV1) -> String {
    match payload.run_address {
        RunAddressV1::Target { run } => {
            format!("target/{run:02}/{:04}", payload.block_index)
        }
        RunAddressV1::Coverage { q, replicate, run } => {
            format!(
                "coverage/q{q}/b{replicate:03}/r{run:02}/{:04}",
                payload.block_index
            )
        }
    }
}

fn checkpoint_exponent_histogram(payload: &TrajectoryCheckpointV1) -> Vec<ExponentBinV1> {
    let maximum = match &payload.records {
        TrajectoryRecordsV1::Target(_) => 3 * 1_024,
        TrajectoryRecordsV1::Coverage(_) => 9,
    };
    let mut counts = vec![0_u64; maximum as usize + 1];
    match &payload.records {
        TrajectoryRecordsV1::Target(records) => {
            for record in records {
                counts[record.exponent as usize] += 1;
            }
        }
        TrajectoryRecordsV1::Coverage(records) => {
            for record in records {
                counts[record.exponent as usize] += 1;
            }
        }
    }
    counts
        .into_iter()
        .enumerate()
        .filter_map(|(exponent, count)| {
            (count != 0).then_some(ExponentBinV1 {
                exponent: exponent as u32,
                count,
            })
        })
        .collect()
}

fn validate_producer(
    producer: &ProducerBackendV1,
    accelerator: &AcceleratorObservationV1,
) -> Result<(), ArtifactError> {
    match (producer, accelerator) {
        (ProducerBackendV1::Cpu {}, AcceleratorObservationV1::NotUsed { .. }) => Ok(()),
        (
            ProducerBackendV1::Gpu {
                device_uuid,
                kernel_name,
                code_object_sha256,
            },
            AcceleratorObservationV1::Used { devices },
        ) if devices.iter().any(|device| {
            device.uuid == *device_uuid
                && device.kernel_name == *kernel_name
                && device.code_object_sha256 == *code_object_sha256
        }) =>
        {
            Ok(())
        }
        _ => Err(ArtifactError::Lineage(
            "checkpoint producer disagrees with start accelerator observation".into(),
        )),
    }
}

fn validate_attempt(payload: &ExecutionAttemptReceiptV1) -> Result<(), ArtifactError> {
    if payload.attempt_id
        != attempt_id(
            &payload.dataset_identity,
            payload.attempt_ordinal,
            &payload.predecessor,
        )?
    {
        return Err(ArtifactError::Identity(
            "attempt ID recomputation mismatch".into(),
        ));
    }
    match &payload.phase {
        AttemptPhaseV1::Start {
            resume_checkpoint_refs,
            start_utc,
            start_receipt_utc,
            invocation,
            environment_inputs,
            worker_configuration,
            host_observation,
            accelerator_observation,
        } => {
            validate_timestamp(start_utc)?;
            validate_timestamp(start_receipt_utc)?;
            if invocation.executable_sha256 != payload.dataset_identity.behavior.executable_sha256 {
                return Err(ArtifactError::Identity(
                    "invoked executable digest differs from dataset behavior".into(),
                ));
            }
            if invocation.configuration_path
                != payload.dataset_identity.preregistration.configuration_path
                || invocation.configuration_sha256
                    != payload
                        .dataset_identity
                        .preregistration
                        .configuration_sha256
            {
                return Err(ArtifactError::Identity(
                    "invoked configuration identity differs from dataset identity".into(),
                ));
            }
            let effective_configuration = decode_hex(&invocation.effective_configuration_hex)?;
            if sha256_hex(&effective_configuration) != invocation.configuration_sha256 {
                return Err(ArtifactError::Identity(
                    "effective configuration bytes differ from configuration digest".into(),
                ));
            }
            let configuration = decode_configuration(&effective_configuration)?;
            if configuration.design_identity != payload.dataset_identity.preregistration.design
                || configuration.scientific_identity != payload.dataset_identity.scientific
                || configuration.behavior != payload.dataset_identity.behavior
            {
                return Err(ArtifactError::Identity(
                    "effective configuration differs from dataset scientific/source inputs".into(),
                ));
            }
            if invocation.argv.is_empty()
                || invocation.process_start_token.is_empty()
                || invocation.boot_identity.is_empty()
            {
                return Err(ArtifactError::Schema(
                    "invocation lacks argv or PID-reuse identity".into(),
                ));
            }
            ensure_sorted_by(
                environment_inputs,
                |entry| &entry.name,
                "environment inputs",
            )?;
            let declared: Vec<_> = environment_inputs
                .iter()
                .map(|entry| entry.name.clone())
                .collect();
            if declared
                != payload
                    .dataset_identity
                    .behavior
                    .closure
                    .environment_input_names
            {
                return Err(ArtifactError::Identity(
                    "environment declarations differ from behavior closure".into(),
                ));
            }
            validate_worker(worker_configuration)?;
            validate_input_resolution(
                invocation,
                environment_inputs,
                worker_configuration,
                &configuration,
            )?;
            validate_host(host_observation)?;
            validate_accelerator(accelerator_observation)?;
            ensure_sorted_unique(resume_checkpoint_refs, "resume checkpoint refs")?;
        }
        AttemptPhaseV1::Terminal {
            attempt_start_sha256,
            start_utc,
            end_utc,
            end_time_meaning,
            host_observation_sha256,
            accelerator_observation_sha256,
            outcome_observer,
            outcome,
            checkpoint_refs,
            ..
        } => {
            validate_digest(attempt_start_sha256)?;
            validate_digest(host_observation_sha256)?;
            validate_digest(accelerator_observation_sha256)?;
            validate_timestamp(start_utc)?;
            validate_timestamp(end_utc)?;
            ensure_sorted_unique(checkpoint_refs, "terminal checkpoint refs")?;
            match (end_time_meaning, outcome_observer, outcome) {
                (
                    EndTimeMeaningV1::ProcessObserved,
                    OutcomeObserverV1::SupervisingLauncher { launcher_sha256 },
                    AttemptOutcomeV1::Completed {}
                    | AttemptOutcomeV1::Failed { .. }
                    | AttemptOutcomeV1::Interrupted { .. },
                ) => validate_digest(launcher_sha256)?,
                (
                    EndTimeMeaningV1::ResumeObservation,
                    OutcomeObserverV1::ResumingLauncher {
                        launcher_sha256,
                        liveness_evidence,
                    },
                    AttemptOutcomeV1::TerminationUnobservedOnResume {},
                ) => {
                    validate_digest(launcher_sha256)?;
                    validate_evidence(liveness_evidence)?;
                }
                _ => {
                    return Err(ArtifactError::Lineage(
                        "terminal outcome is not launcher-observed with matching time meaning"
                            .into(),
                    ))
                }
            }
        }
    }
    Ok(())
}

fn validate_input_resolution(
    invocation: &InvocationV1,
    environment_inputs: &[EnvironmentInputV1],
    worker: &WorkerConfigurationV1,
    configuration: &RareEventConfigurationV1,
) -> Result<(), ArtifactError> {
    let scientific_mode = match configuration.scientific_identity {
        ScientificIdentityV1::Target { .. } => "target",
        ScientificIdentityV1::Coverage { .. } => "coverage",
    };
    let expected = [
        (
            "accelerator_selection",
            worker.accelerator_selection.clone(),
            false,
        ),
        ("artifact_root", configuration.artifact_root.clone(), true),
        (
            "block_assignment_policy",
            worker.block_assignment_policy.clone(),
            false,
        ),
        ("cpu_affinity", worker.cpu_affinity.clone(), false),
        (
            "effective_devices",
            worker.effective_devices.join(","),
            false,
        ),
        (
            "effective_workers",
            worker.effective_workers.to_string(),
            false,
        ),
        ("executor_mode", worker.executor_mode.clone(), false),
        ("fallback_policy", worker.fallback_policy.clone(), false),
        (
            "requested_workers",
            worker.requested_workers.to_string(),
            false,
        ),
        ("scientific_identity", scientific_mode.into(), true),
        ("work_queue_policy", worker.work_queue_policy.clone(), false),
    ];
    if invocation.input_resolution.len() != expected.len() {
        return Err(ArtifactError::Identity(
            "input resolution does not contain the closed field inventory".into(),
        ));
    }
    for (entry, (field, effective_value, configuration_only)) in
        invocation.input_resolution.iter().zip(expected)
    {
        if entry.field != field || entry.value != effective_value {
            return Err(ArtifactError::Identity(
                "input-resolution field/value differs from effective inputs".into(),
            ));
        }
        if configuration_only {
            if entry.origin != InputOriginV1::Configuration {
                return Err(ArtifactError::Identity(
                    "configuration-owned input has a different claimed origin".into(),
                ));
            }
            continue;
        }
        match entry.origin {
            InputOriginV1::Argument => {
                let flag = format!("--{}={}", field.replace('_', "-"), entry.value);
                if invocation
                    .argv
                    .iter()
                    .filter(|token| *token == &flag)
                    .count()
                    != 1
                {
                    return Err(ArtifactError::Identity(
                        "argument-origin input lacks one exact argv token".into(),
                    ));
                }
            }
            InputOriginV1::Environment => {
                let environment_name = if matches!(field, "requested_workers" | "effective_workers")
                {
                    "RAYON_NUM_THREADS".to_owned()
                } else {
                    format!("GF2_{}", field.to_ascii_uppercase())
                };
                let observed = environment_inputs
                    .iter()
                    .find(|item| item.name == environment_name);
                if !matches!(
                    observed,
                    Some(EnvironmentInputV1 {
                        value: EnvironmentValueV1::Set(value),
                        ..
                    }) if value == &entry.value
                ) {
                    return Err(ArtifactError::Identity(
                        "environment-origin input differs from observed environment".into(),
                    ));
                }
            }
            InputOriginV1::SchemaDefault => {
                if schema_default(field) != Some(entry.value.as_str()) {
                    return Err(ArtifactError::Identity(
                        "default-origin input differs from the closed schema default".into(),
                    ));
                }
            }
            InputOriginV1::Configuration => {
                return Err(ArtifactError::Identity(
                    "execution input is not owned by immutable scientific configuration".into(),
                ));
            }
        }
    }
    Ok(())
}

fn schema_default(field: &str) -> Option<&'static str> {
    match field {
        "accelerator_selection" => Some("none"),
        "block_assignment_policy" => Some("round-robin/v1"),
        "cpu_affinity" => Some("unpinned"),
        "effective_devices" => Some(""),
        "executor_mode" => Some("cpu"),
        "fallback_policy" => Some("safe-cpu/v1"),
        "work_queue_policy" => Some("canonical-block-queue/v1"),
        _ => None,
    }
}

fn validate_host(host: &HostObservationV1) -> Result<(), ArtifactError> {
    validate_timestamp(&host.observation_utc)?;
    if host.logical_cpus == 0
        || host.online_cpus.len() != host.logical_cpus as usize
        || host.available_ram_bytes > host.total_ram_bytes
    {
        return Err(ArtifactError::Identity(
            "normalized host fields are inconsistent".into(),
        ));
    }
    if !host
        .online_cpus
        .windows(2)
        .all(|window| window[0] < window[1])
    {
        return Err(ArtifactError::Identity(
            "online CPUs are not strictly sorted".into(),
        ));
    }
    for evidence in &host.evidence {
        validate_evidence(evidence)?;
    }
    let normalized: Vec<_> = host
        .evidence
        .iter()
        .filter(|evidence| evidence.source == "gf2.host-observation-normalized-json/v1")
        .collect();
    if normalized.len() != 1 {
        return Err(ArtifactError::Identity(
            "host observation needs exactly one canonical normalized collector record".into(),
        ));
    }
    let bytes = decode_hex(&normalized[0].evidence_hex)?;
    let observed: HostNormalizedEvidenceV1 = serde_json::from_slice(&bytes)?;
    if canonical_bytes(&observed)? != bytes || observed != HostNormalizedEvidenceV1::from(host) {
        return Err(ArtifactError::Identity(
            "normalized host fields differ from immutable collector evidence".into(),
        ));
    }
    Ok(())
}

fn validate_accelerator(accelerator: &AcceleratorObservationV1) -> Result<(), ArtifactError> {
    match accelerator {
        AcceleratorObservationV1::NotUsed { reason, evidence } => {
            if reason.is_empty() {
                return Err(ArtifactError::Identity(
                    "not-used accelerator needs observed reason".into(),
                ));
            }
            for item in evidence {
                validate_evidence(item)?;
            }
            let normalized: Vec<_> = evidence
                .iter()
                .filter(|item| item.source == "gf2.accelerator-not-used-normalized-json/v1")
                .collect();
            if normalized.len() != 1 {
                return Err(ArtifactError::Identity(
                    "no-accelerator observation needs one canonical collector record".into(),
                ));
            }
            let bytes = decode_hex(&normalized[0].evidence_hex)?;
            let observed: AcceleratorNotUsedEvidenceV1 = serde_json::from_slice(&bytes)?;
            if canonical_bytes(&observed)? != bytes || observed.reason != *reason {
                return Err(ArtifactError::Identity(
                    "no-accelerator reason differs from immutable collector evidence".into(),
                ));
            }
        }
        AcceleratorObservationV1::Used { devices } => {
            if devices.is_empty() {
                return Err(ArtifactError::Identity(
                    "used accelerator needs a device".into(),
                ));
            }
            ensure_sorted_by(devices, |device| &device.uuid, "GPU observations")?;
            for device in devices {
                validate_digest(&device.code_object_sha256)?;
                for item in &device.evidence {
                    validate_evidence(item)?;
                }
                let normalized: Vec<_> = device
                    .evidence
                    .iter()
                    .filter(|item| item.source == "gf2.gpu-observation-normalized-json/v1")
                    .collect();
                if normalized.len() != 1 {
                    return Err(ArtifactError::Identity(
                        "GPU observation needs one canonical collector record".into(),
                    ));
                }
                let bytes = decode_hex(&normalized[0].evidence_hex)?;
                let observed: GpuNormalizedEvidenceV1 = serde_json::from_slice(&bytes)?;
                if canonical_bytes(&observed)? != bytes
                    || observed != GpuNormalizedEvidenceV1::from(device)
                {
                    return Err(ArtifactError::Identity(
                        "normalized GPU fields differ from immutable collector evidence".into(),
                    ));
                }
            }
        }
    }
    Ok(())
}

fn validate_evidence(evidence: &ObservationEvidenceV1) -> Result<(), ArtifactError> {
    let bytes = decode_hex(&evidence.evidence_hex)?;
    if evidence.evidence_sha256 != sha256_hex(&bytes) {
        return Err(ArtifactError::Identity(
            "runtime evidence digest mismatch".into(),
        ));
    }
    Ok(())
}

fn validate_final_refs(
    identity: &RareEventDatasetIdentityV1,
    provenance: &ExecutionProvenanceV1,
    expected_blocks: usize,
) -> Result<(), ArtifactError> {
    if provenance.checkpoint_refs.len() != expected_blocks {
        return Err(ArtifactError::AddressSet(format!(
            "final receipt needs {expected_blocks} checkpoint refs"
        )));
    }
    let expected_addresses = expected_checkpoint_block_addresses(identity)?;
    for (reference, expected_address) in provenance.checkpoint_refs.iter().zip(expected_addresses) {
        if reference.block_address != expected_address {
            return Err(ArtifactError::AddressSet(
                "final checkpoint refs are missing, extra, duplicated, or reordered".into(),
            ));
        }
        validate_digest(&reference.checkpoint_sha256)?;
    }
    if provenance.attempts.is_empty() {
        return Err(ArtifactError::Lineage(
            "final receipt omits its execution attempts".into(),
        ));
    }
    for (ordinal, attempt) in provenance.attempts.iter().enumerate() {
        validate_digest(&attempt.attempt_start_sha256)?;
        validate_digest(&attempt.attempt_terminal_sha256)?;
        let predecessor = if ordinal == 0 {
            AttemptPredecessorV1::None {}
        } else {
            AttemptPredecessorV1::Terminal {
                terminal_sha256: provenance.attempts[ordinal - 1]
                    .attempt_terminal_sha256
                    .clone(),
            }
        };
        if attempt.attempt_id != attempt_id(identity, ordinal as u64, &predecessor)? {
            return Err(ArtifactError::Lineage(
                "final attempt ID chain mismatch".into(),
            ));
        }
    }
    Ok(())
}

fn payload_identity(payload: &RareEventPayloadV1) -> &RareEventDatasetIdentityV1 {
    match payload {
        RareEventPayloadV1::TrajectoryCheckpoint(payload) => &payload.dataset_identity,
        RareEventPayloadV1::ExecutionAttempt(payload) => &payload.dataset_identity,
        RareEventPayloadV1::TargetCrossCheck(payload) => &payload.dataset_identity,
        RareEventPayloadV1::CoverageValidation(payload) => &payload.dataset_identity,
    }
}

fn write_canonical_value(value: &Value, bytes: &mut Vec<u8>) -> Result<(), ArtifactError> {
    match value {
        Value::Null => bytes.extend(b"null"),
        Value::Bool(value) => bytes.extend(if *value {
            b"true".as_slice()
        } else {
            b"false".as_slice()
        }),
        Value::Number(number) => {
            if number.is_f64() {
                return Err(ArtifactError::Schema(
                    "floating-point JSON numbers are forbidden".into(),
                ));
            }
            bytes.extend(number.to_string().as_bytes());
        }
        Value::String(string) => bytes.extend(serde_json::to_string(string)?.as_bytes()),
        Value::Array(values) => {
            bytes.push(b'[');
            for (index, value) in values.iter().enumerate() {
                if index != 0 {
                    bytes.push(b',');
                }
                write_canonical_value(value, bytes)?;
            }
            bytes.push(b']');
        }
        Value::Object(object) => {
            bytes.push(b'{');
            let mut keys: Vec<_> = object.keys().collect();
            keys.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
            for (index, key) in keys.into_iter().enumerate() {
                if index != 0 {
                    bytes.push(b',');
                }
                bytes.extend(serde_json::to_string(key)?.as_bytes());
                bytes.push(b':');
                write_canonical_value(&object[key], bytes)?;
            }
            bytes.push(b'}');
        }
    }
    Ok(())
}

fn domain_digest(domain: &[u8], input: &[u8]) -> Result<String, ArtifactError> {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update([0]);
    hasher.update(input);
    Ok(format!("{:x}", hasher.finalize()))
}

fn decode_digest(value: &str) -> Result<Vec<u8>, ArtifactError> {
    validate_digest(value)?;
    decode_hex(value)
}

fn validate_digest(value: &str) -> Result<(), ArtifactError> {
    validate_hex(value, 64)
}

fn validate_hex(value: &str, length: usize) -> Result<(), ArtifactError> {
    if value.len() != length
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(ArtifactError::Identity(format!(
            "expected {length} lowercase hexadecimal characters"
        )));
    }
    Ok(())
}

fn validate_hex_bytes(value: &str) -> Result<(), ArtifactError> {
    if !value.len().is_multiple_of(2)
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(ArtifactError::Schema(
            "evidence bytes are not lowercase even-length hex".into(),
        ));
    }
    Ok(())
}

fn decode_hex(value: &str) -> Result<Vec<u8>, ArtifactError> {
    validate_hex_bytes(value)?;
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let digits = std::str::from_utf8(pair).expect("validated ASCII hex");
            u8::from_str_radix(digits, 16).map_err(|_| ArtifactError::Schema("invalid hex".into()))
        })
        .collect()
}

fn validate_relative_path(value: &str) -> Result<(), ArtifactError> {
    if value.is_empty()
        || value.starts_with('/')
        || value.contains('\\')
        || value
            .split('/')
            .any(|component| component.is_empty() || component == "." || component == "..")
    {
        return Err(ArtifactError::Schema(
            "path is not normalized repository-relative".into(),
        ));
    }
    Ok(())
}

fn validate_timestamp(value: &str) -> Result<(), ArtifactError> {
    let bytes = value.as_bytes();
    let separators = [
        (4, b'-'),
        (7, b'-'),
        (10, b'T'),
        (13, b':'),
        (16, b':'),
        (19, b'.'),
        (29, b'Z'),
    ];
    if bytes.len() != 30
        || separators
            .iter()
            .any(|&(index, expected)| bytes[index] != expected)
        || bytes.iter().enumerate().any(|(index, byte)| {
            !separators.iter().any(|&(position, _)| position == index) && !byte.is_ascii_digit()
        })
    {
        return Err(ArtifactError::Schema(
            "UTC timestamp is not YYYY-MM-DDTHH:MM:SS.nnnnnnnnnZ".into(),
        ));
    }
    Ok(())
}

fn ensure_sorted_unique<T: Ord>(values: &[T], name: &str) -> Result<(), ArtifactError> {
    if values.windows(2).any(|window| window[0] >= window[1]) {
        return Err(ArtifactError::Schema(format!(
            "{name} must be strictly sorted and unique"
        )));
    }
    Ok(())
}

fn ensure_sorted_by<T, K: Ord>(
    values: &[T],
    key: impl Fn(&T) -> &K,
    name: &str,
) -> Result<(), ArtifactError> {
    if values
        .windows(2)
        .any(|window| key(&window[0]) >= key(&window[1]))
    {
        return Err(ArtifactError::Schema(format!(
            "{name} must be strictly sorted and unique"
        )));
    }
    Ok(())
}
