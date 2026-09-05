//! Neutral, durable campaign contracts shared by owners and the driver.
//!
//! Owners supply ordered manifests and validate the mathematical meaning of
//! opaque case/result payloads. This layer validates identities, progress and
//! durability. The binary owns OS process launch, concurrent pipe draining,
//! process-tree reaping and independent flock observations: those observations
//! are explicit inputs here, never inferred from a wrapper's exit alone.

use crate::journal::{
    atomic_write_new, CheckpointStore, CompletedUnit, ExecutionLog, JournalEvent, JournalRecord,
    PendingRecovery, ResumeIdentity, TerminalState,
};
use crate::timing::{self, TimingSample, EXECUTIONS, MAX_CALLS, WINDOWS};
use crate::transport::{self, FRESH_CASE_VALUE, FRESH_CASE_VAR};
use serde::de::{self, DeserializeOwned, MapAccess, SeqAccess, Visitor};
use serde::ser::{SerializeMap, SerializeSeq};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};

/// Canonical owner work manifest schema.
pub const MANIFEST_SCHEMA: &str = "tuning-campaign-owner-manifest-v1";
/// Canonical neutral child envelope schema.
pub const RESULT_SCHEMA: &str = "tuning-campaign-child-result-v1";
/// Canonical progress record schema.
pub const PROGRESS_SCHEMA: &str = "tuning-campaign-progress-v1";
/// Stderr prefix for progress; other stderr lines are retained diagnostics.
pub const PROGRESS_PREFIX: &str = "GF2_TUNING_PROGRESS=";
/// Canonical session descriptor and lifecycle schema.
pub const LIFECYCLE_SCHEMA: &str = "tuning-campaign-session-v1";
/// Exact feature set of the a835 measurement build.
pub const FEATURE_CONTRACT: &str = "parallel,simd,tuning-profile,test-support";
/// Exact inherited Rayon and dedicated-pool measurement contract.
pub const THREAD_CONTRACT: &str = "RAYON_NUM_THREADS=4;dedicated_pool_width=4";
/// Exact direct-binary toolchain environment required by a835.
pub const TOOLCHAIN_CONTRACT: &str = "1.95.0";

/// The complete declared child environment. Runners use `env_clear()` then
/// install this map, so inherited agent/tool settings cannot alter a campaign.
/// An additional variable requires a reviewed protocol/behavior declaration.
pub fn measurement_environment() -> BTreeMap<String, String> {
    BTreeMap::from([
        (FRESH_CASE_VAR.into(), FRESH_CASE_VALUE.into()),
        ("RAYON_NUM_THREADS".into(), "4".into()),
        ("GF2_BENCH".into(), "1".into()),
        ("RUSTUP_TOOLCHAIN".into(), TOOLCHAIN_CONTRACT.into()),
    ])
}

/// Hard wall limit for a child, excluding its termination grace.
pub const CHILD_TIMEOUT_SECONDS: u64 = 120;
/// Maximum additional termination/reaping grace.
pub const CHILD_KILL_GRACE_SECONDS: u64 = 5;
/// Active budget of each independently locked session.
pub const SESSION_BUDGET_SECONDS: u64 = 10_800;

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}
fn wire_error(error: impl fmt::Display) -> io::Error {
    invalid(error.to_string())
}
fn bytes<T: Serialize>(value: &T) -> io::Result<Vec<u8>> {
    serde_json::to_vec(value).map_err(wire_error)
}
fn decode<T: DeserializeOwned + Serialize>(input: &[u8]) -> io::Result<T> {
    let value: T = serde_json::from_slice(input).map_err(wire_error)?;
    if bytes(&value)? != input {
        return Err(invalid("noncanonical JSON encoding"));
    }
    Ok(value)
}

/// Nonempty portable identity component (ASCII letters, digits, `.`, `_`, `-`).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct Token(String);
impl Token {
    /// Rejects whitespace, separators, controls and ambiguous path components.
    pub fn new(value: impl Into<String>) -> io::Result<Self> {
        let value = value.into();
        if value.is_empty()
            || matches!(value.as_str(), "." | "..")
            || !value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        {
            return Err(invalid("invalid identity token"));
        }
        Ok(Self(value))
    }
    /// Exact validated identity spelling.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl<'de> Deserialize<'de> for Token {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::new(String::deserialize(d)?).map_err(de::Error::custom)
    }
}

/// Exactly 64 lowercase hexadecimal SHA-256 digits.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct Sha256Digest(String);
impl Sha256Digest {
    /// Validates a digest supplied at a serialization boundary.
    pub fn new(value: impl Into<String>) -> io::Result<Self> {
        let value = value.into();
        if value.len() != 64
            || !value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(invalid("invalid SHA-256 digest"));
        }
        Ok(Self(value))
    }
    /// Hashes exact bytes without normalization.
    pub fn of(input: &[u8]) -> Self {
        Self(format!("{:x}", Sha256::digest(input)))
    }
    /// Exact lowercase hexadecimal spelling.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl<'de> Deserialize<'de> for Sha256Digest {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::new(String::deserialize(d)?).map_err(de::Error::custom)
    }
}

// Preserve owner struct key order while detecting duplicate keys. A Value
// roundtrip would reorder owner fields and change the canonical stdin bytes.
#[derive(Debug)]
enum OrderedJson {
    Null,
    Bool(bool),
    Number(serde_json::Number),
    String(String),
    Array(Vec<Self>),
    Object(Vec<(String, Self)>),
}
impl Serialize for OrderedJson {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Null => s.serialize_unit(),
            Self::Bool(v) => s.serialize_bool(*v),
            Self::Number(v) => v.serialize(s),
            Self::String(v) => s.serialize_str(v),
            Self::Array(v) => {
                let mut seq = s.serialize_seq(Some(v.len()))?;
                for item in v {
                    seq.serialize_element(item)?;
                }
                seq.end()
            }
            Self::Object(v) => {
                let mut map = s.serialize_map(Some(v.len()))?;
                for (k, item) in v {
                    map.serialize_entry(k, item)?;
                }
                map.end()
            }
        }
    }
}
impl<'de> Deserialize<'de> for OrderedJson {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct JsonVisitor;
        impl<'de> Visitor<'de> for JsonVisitor {
            type Value = OrderedJson;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("JSON with unique object keys")
            }
            fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
                Ok(OrderedJson::Null)
            }
            fn visit_bool<E: de::Error>(self, v: bool) -> Result<Self::Value, E> {
                Ok(OrderedJson::Bool(v))
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Self::Value, E> {
                Ok(OrderedJson::Number(v.into()))
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Self::Value, E> {
                Ok(OrderedJson::Number(v.into()))
            }
            fn visit_f64<E: de::Error>(self, v: f64) -> Result<Self::Value, E> {
                serde_json::Number::from_f64(v)
                    .map(OrderedJson::Number)
                    .ok_or_else(|| E::custom("nonfinite JSON number"))
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
                Ok(OrderedJson::String(v.into()))
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let mut v = Vec::new();
                while let Some(item) = seq.next_element()? {
                    v.push(item);
                }
                Ok(OrderedJson::Array(v))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut v = Vec::new();
                let mut keys = BTreeSet::new();
                while let Some((k, item)) = map.next_entry::<String, OrderedJson>()? {
                    if !keys.insert(k.clone()) {
                        return Err(de::Error::custom("duplicate JSON key"));
                    }
                    v.push((k, item));
                }
                Ok(OrderedJson::Object(v))
            }
        }
        d.deserialize_any(JsonVisitor)
    }
}

/// Opaque canonical compact JSON bytes, serialized as a JSON string.
///
/// Object order remains the owner's order. Whitespace, duplicate keys and
/// noncanonical numeric/string spellings reject. Unknown *owner* fields are
/// checked by the owner's typed codec, not interpreted by this neutral layer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct CanonicalJson(String);
impl CanonicalJson {
    /// Validates bytes without changing the owner's field order.
    pub fn new(value: impl Into<String>) -> io::Result<Self> {
        let value = value.into();
        let _: OrderedJson = decode(value.as_bytes())?;
        Ok(Self(value))
    }
    /// Encodes an owner wire type once, preserving its canonical field order.
    pub fn from_serializable<T: Serialize>(value: &T) -> io::Result<Self> {
        Self::new(String::from_utf8(bytes(value)?).map_err(wire_error)?)
    }
    /// Exact bytes for stdin or owner semantic decoding.
    pub fn as_str(&self) -> &str {
        &self.0
    }
    /// Digest of the exact canonical owner bytes.
    pub fn digest(&self) -> Sha256Digest {
        Sha256Digest::of(self.0.as_bytes())
    }
    /// Strict owner decoding, including canonical re-encoding and unknown fields.
    pub fn decode<T: DeserializeOwned + Serialize>(&self) -> io::Result<T> {
        decode(self.0.as_bytes())
    }
}
impl<'de> Deserialize<'de> for CanonicalJson {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::new(String::deserialize(d)?).map_err(de::Error::custom)
    }
}

/// One accepted unit is either a semantic probe or one full timed execution.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Task {
    Probe,
    Measure { execution: u64 },
}
impl Task {
    /// Validates the fixed five-execution protocol.
    pub fn validate(self) -> io::Result<()> {
        if matches!(self,Self::Measure{execution} if execution>=EXECUTIONS) {
            Err(invalid("execution outside protocol"))
        } else {
            Ok(())
        }
    }
    /// Number of accepted calibration/window records (zero for a probe).
    pub fn expected_progress(self) -> u64 {
        match self {
            Self::Probe => 0,
            Self::Measure { .. } => WINDOWS + 1,
        }
    }
    fn index(self) -> u64 {
        match self {
            Self::Probe => 0,
            Self::Measure { execution } => execution + 1,
        }
    }
}

/// Every coordinate that determines one immutable accepted child result.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UnitIdentity {
    pub protocol: Token,
    pub owner: Token,
    pub campaign_id: Token,
    pub phase: Token,
    pub field: Token,
    pub stratum: Token,
    pub candidate: Token,
    pub task: Task,
}
impl UnitIdentity {
    /// Stable exact unit key, independent of process attempts or session IDs.
    pub fn key(&self) -> io::Result<Sha256Digest> {
        self.task.validate()?;
        Ok(Sha256Digest::of(&bytes(self)?))
    }
    fn cell_key(&self) -> io::Result<Sha256Digest> {
        let mut cell = self.clone();
        cell.task = Task::Probe;
        cell.key()
    }
}

/// Staged executable and exact child environment; the runner clears inherited
/// environment before installing this map. No shell command is represented.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProcessDescriptor {
    pub id: Token,
    pub executable: PathBuf,
    pub executable_sha256: Sha256Digest,
    pub arguments: Vec<String>,
    pub environment: BTreeMap<String, String>,
    pub working_directory: PathBuf,
}
impl ProcessDescriptor {
    /// Checks canonical paths and closed child-entry environment contract.
    /// File existence/digests are checked separately by `verify_staged`.
    pub fn validate(&self) -> io::Result<()> {
        absolute_path(&self.executable)?;
        absolute_path(&self.working_directory)?;
        if self.arguments.iter().any(|s| s.contains('\0')) {
            return Err(invalid("NUL in process argument"));
        }
        for (k, v) in &self.environment {
            if k.is_empty()
                || !k.bytes().enumerate().all(|(i, b)| {
                    b == b'_' || b.is_ascii_alphabetic() || (i > 0 && b.is_ascii_digit())
                })
                || v.contains('\0')
            {
                return Err(invalid("invalid environment entry"));
            }
        }
        if self.environment != measurement_environment() {
            return Err(invalid(
                "staged child environment differs from its declared protocol",
            ));
        }
        Ok(())
    }
    /// Reopens staged paths and verifies their canonical identity and bytes.
    pub fn verify_staged(&self) -> io::Result<()> {
        self.validate()?;
        if fs::canonicalize(&self.executable)? != self.executable
            || fs::canonicalize(&self.working_directory)? != self.working_directory
            || !self.working_directory.is_dir()
            || !self.executable.is_file()
            || Sha256Digest::of(&fs::read(&self.executable)?) != self.executable_sha256
        {
            return Err(invalid("staged process identity mismatch"));
        }
        Ok(())
    }
}
fn absolute_path(path: &Path) -> io::Result<()> {
    let reconstructed: PathBuf = path.components().collect();
    if !path.is_absolute()
        || path.to_str().is_none()
        || path
            .components()
            .any(|c| matches!(c, Component::CurDir | Component::ParentDir))
        || path.as_os_str() != reconstructed.as_os_str()
        || path.to_string_lossy().contains('\0')
    {
        Err(invalid("path must be absolute, UTF-8 and normalized"))
    } else {
        Ok(())
    }
}

/// Owner-supplied bounded action in the immutable global acquisition order.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LaunchUnit {
    pub ordinal: u64,
    pub identity: UnitIdentity,
    pub key: Sha256Digest,
    pub process: Token,
    pub case: CanonicalJson,
    pub expected_progress: u64,
}
impl LaunchUnit {
    /// Constructs an internally consistent launch unit.
    pub fn new(
        ordinal: u64,
        identity: UnitIdentity,
        process: Token,
        case: CanonicalJson,
    ) -> io::Result<Self> {
        let key = identity.key()?;
        let expected_progress = identity.task.expected_progress();
        Ok(Self {
            ordinal,
            identity,
            key,
            process,
            case,
            expected_progress,
        })
    }
    /// Rejects changed keys or progress counts.
    pub fn validate(&self) -> io::Result<()> {
        if self.key != self.identity.key()?
            || self.expected_progress != self.identity.task.expected_progress()
        {
            return Err(invalid("launch identity/key/progress mismatch"));
        }
        Ok(())
    }
}

/// Declared accepted accounting; failed attempts and orchestration are separate.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DeclaredCounts {
    pub cells: u64,
    pub probes: u64,
    pub timed_children: u64,
    pub accepted_results: u64,
    pub windows: u64,
    pub progress_records: u64,
}
impl DeclaredCounts {
    /// Computes counts from a complete six-unit-per-cell manifest.
    pub fn for_cells(cells: u64) -> io::Result<Self> {
        let timed_children = cells
            .checked_mul(EXECUTIONS)
            .ok_or_else(|| invalid("count overflow"))?;
        Ok(Self {
            cells,
            probes: cells,
            timed_children,
            accepted_results: cells
                .checked_add(timed_children)
                .ok_or_else(|| invalid("count overflow"))?,
            windows: timed_children
                .checked_mul(WINDOWS)
                .ok_or_else(|| invalid("count overflow"))?,
            progress_records: timed_children
                .checked_mul(WINDOWS + 1)
                .ok_or_else(|| invalid("count overflow"))?,
        })
    }
}

/// Preregistered acquisition group. Every candidate is probed once in base
/// order; each subsequent execution block visits exactly those candidates in
/// rotate-left(execution % count), then odd-execution reverse order.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateBlock {
    pub phase: Token,
    pub field: Token,
    pub stratum: Token,
    pub base_candidates: Vec<Token>,
}
impl CandidateBlock {
    /// Returns the exact preregistered candidate order for one task.
    pub fn candidates_for(&self, task: Task) -> io::Result<Vec<Token>> {
        task.validate()?;
        if self.base_candidates.is_empty()
            || self.base_candidates.iter().collect::<BTreeSet<_>>().len()
                != self.base_candidates.len()
        {
            return Err(invalid(
                "candidate block requires distinct nonempty base candidates",
            ));
        }
        let mut candidates = self.base_candidates.clone();
        if let Task::Measure { execution } = task {
            let count = candidates.len();
            candidates.rotate_left(execution as usize % count);
            if execution % 2 == 1 {
                candidates.reverse();
            }
        }
        Ok(candidates)
    }
}

/// Strict owner manifest. `manifest_sha256` hashes all fields preceding it in
/// their canonical encoding with that digest field omitted.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OwnerManifest {
    pub schema: String,
    pub owner: Token,
    pub owner_protocol: Token,
    pub behavior_token: Token,
    pub campaign_id: Token,
    pub phases: Vec<Token>,
    pub candidate_blocks: Vec<CandidateBlock>,
    pub counts: DeclaredCounts,
    pub processes: Vec<ProcessDescriptor>,
    pub ordered_units: Vec<LaunchUnit>,
    pub manifest_sha256: Sha256Digest,
}
#[derive(Serialize)]
struct ManifestBody<'a> {
    schema: &'a str,
    owner: &'a Token,
    owner_protocol: &'a Token,
    behavior_token: &'a Token,
    campaign_id: &'a Token,
    phases: &'a [Token],
    candidate_blocks: &'a [CandidateBlock],
    counts: DeclaredCounts,
    processes: &'a [ProcessDescriptor],
    ordered_units: &'a [LaunchUnit],
}
impl OwnerManifest {
    fn body(&self) -> ManifestBody<'_> {
        ManifestBody {
            schema: &self.schema,
            owner: &self.owner,
            owner_protocol: &self.owner_protocol,
            behavior_token: &self.behavior_token,
            campaign_id: &self.campaign_id,
            phases: &self.phases,
            candidate_blocks: &self.candidate_blocks,
            counts: self.counts,
            processes: &self.processes,
            ordered_units: &self.ordered_units,
        }
    }
    /// Recomputes the manifest digest. Call before publishing a constructed
    /// manifest; externally supplied manifests must use `decode`/`validate`.
    pub fn seal(&mut self) -> io::Result<()> {
        self.manifest_sha256 = Sha256Digest::of(&bytes(&self.body())?);
        self.validate()
    }
    /// Decodes strict canonical bytes and checks every neutral contract.
    pub fn decode(input: &[u8]) -> io::Result<Self> {
        let manifest: Self = decode(input)?;
        manifest.validate()?;
        Ok(manifest)
    }
    /// Validates phases, acquisition order, task completeness, counts and digests.
    /// Candidate blocks must exactly match the preregistered base orders and
    /// fixed rotation rule; owners additionally validate their semantic grids.
    pub fn validate(&self) -> io::Result<()> {
        if self.schema != MANIFEST_SCHEMA
            || self.phases.is_empty()
            || self.processes.is_empty()
            || self.ordered_units.is_empty()
            || self.manifest_sha256 != Sha256Digest::of(&bytes(&self.body())?)
        {
            return Err(invalid("manifest schema/content digest mismatch"));
        }
        let phases: BTreeMap<_, _> = self
            .phases
            .iter()
            .enumerate()
            .map(|(i, p)| (p, i))
            .collect();
        if phases.len() != self.phases.len() {
            return Err(invalid("duplicate phase"));
        }
        let mut processes = BTreeSet::new();
        for process in &self.processes {
            process.validate()?;
            if !processes.insert(&process.id) {
                return Err(invalid("duplicate process ID"));
            }
        }
        if self.candidate_blocks.is_empty() {
            return Err(invalid("manifest has no preregistered candidate blocks"));
        }
        let mut groups = BTreeSet::new();
        let mut ordinal = 0usize;
        for block in &self.candidate_blocks {
            if !groups.insert((&block.phase, &block.field, &block.stratum)) {
                return Err(invalid("duplicate candidate block"));
            }
            for task in std::iter::once(Task::Probe)
                .chain((0..EXECUTIONS).map(|execution| Task::Measure { execution }))
            {
                for candidate in block.candidates_for(task)? {
                    let unit = self
                        .ordered_units
                        .get(ordinal)
                        .ok_or_else(|| invalid("incomplete execution block"))?;
                    let identity = &unit.identity;
                    if identity.phase != block.phase
                        || identity.field != block.field
                        || identity.stratum != block.stratum
                        || identity.candidate != candidate
                        || identity.task != task
                    {
                        return Err(invalid(
                            "unit order differs from preregistered execution block",
                        ));
                    }
                    ordinal += 1;
                }
            }
        }
        if ordinal != self.ordered_units.len() {
            return Err(invalid("undeclared trailing execution block"));
        }
        let mut keys = BTreeSet::new();
        let mut cells = BTreeMap::new();
        let mut phase_seen = BTreeSet::new();
        let mut previous_phase = 0;
        for (index, unit) in self.ordered_units.iter().enumerate() {
            unit.validate()?;
            let id = &unit.identity;
            let phase = *phases
                .get(&id.phase)
                .ok_or_else(|| invalid("unknown phase"))?;
            if unit.ordinal != index as u64
                || id.owner != self.owner
                || id.protocol != self.owner_protocol
                || id.campaign_id != self.campaign_id
                || phase < previous_phase
                || !processes.contains(&unit.process)
                || !keys.insert(&unit.key)
            {
                return Err(invalid(
                    "unit ordinal/identity/order/process/duplicate mismatch",
                ));
            }
            previous_phase = phase;
            phase_seen.insert(phase);
            let next = cells.entry(id.cell_key()?).or_insert(0);
            if id.task.index() != *next {
                return Err(invalid(
                    "cell tasks must be probe then executions 0 through 4",
                ));
            }
            *next += 1;
        }
        if phase_seen.len() != phases.len()
            || cells.values().any(|n| *n != EXECUTIONS + 1)
            || self.counts != DeclaredCounts::for_cells(cells.len() as u64)?
            || self.counts.accepted_results != self.ordered_units.len() as u64
        {
            return Err(invalid(
                "manifest declared count or task completeness mismatch",
            ));
        }
        Ok(())
    }
    /// Ordered work digest used verbatim in resume identity.
    pub fn ordered_work_digest(&self) -> io::Result<Sha256Digest> {
        Ok(Sha256Digest::of(&bytes(&self.ordered_units)?))
    }
    /// Staged process descriptor digest used verbatim in resume identity.
    pub fn process_digest(&self) -> io::Result<Sha256Digest> {
        Ok(Sha256Digest::of(&bytes(&self.processes)?))
    }
}

/// Canonical filesystem channel identities shared by all sessions of a campaign.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SessionChannels {
    pub stage: PathBuf,
    pub execution_log: PathBuf,
    pub checkpoints: PathBuf,
}
impl SessionChannels {
    /// Resolves an existing stage and derives the canonical channel names.
    pub fn for_stage(stage: impl AsRef<Path>) -> io::Result<Self> {
        let stage = fs::canonicalize(stage)?;
        Ok(Self {
            execution_log: stage.join("execution.log"),
            checkpoints: stage.join("checkpoints"),
            stage,
        })
    }
    /// Rejects redirected, relative, or aliased channel names.
    pub fn validate(&self) -> io::Result<()> {
        absolute_path(&self.stage)?;
        if self.execution_log != self.stage.join("execution.log")
            || self.checkpoints != self.stage.join("checkpoints")
        {
            return Err(invalid("campaign channel path mismatch"));
        }
        Ok(())
    }
}

/// Exact canonical file path and bytes bound to an owner operation.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactIdentity {
    pub path: PathBuf,
    pub sha256: Sha256Digest,
}
impl ArtifactIdentity {
    /// Strictly reopens a canonical artifact and checks its byte digest.
    pub fn read<T: DeserializeOwned + Serialize>(&self) -> io::Result<T> {
        absolute_path(&self.path)?;
        if fs::canonicalize(&self.path)? != self.path {
            return Err(invalid("artifact path is not canonical"));
        }
        let data = fs::read(&self.path)?;
        if Sha256Digest::of(&data) != self.sha256 {
            return Err(invalid("artifact digest mismatch"));
        }
        decode(&data)
    }
}

/// Driver-observed provenance, with runtime facts opaque to the neutral layer.
/// `identity` pins source, behavioral, host, feature, thread and process facts;
/// `runtime` contains the owner's strict observed facts, never copied claims.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ObservedProvenance {
    pub identity: ResumeIdentity,
    pub process: Token,
    pub observed_utc: String,
    pub runtime: CanonicalJson,
}

/// Request for an owner to declare its exact work against staged identities.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestRequest {
    pub campaign_id: Token,
    pub protocol_sha256: Sha256Digest,
    pub channels: SessionChannels,
    pub processes: Vec<ProcessDescriptor>,
}

/// One driver-validated immutable unit exported for owner semantic analysis.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AcceptedResult {
    pub unit: LaunchUnit,
    pub result: ChildResult,
    pub checkpoint_sha256: Sha256Digest,
}
/// Canonical exported input layout. Owners use this shared schema, never parse
/// the private checkpoint on-disk representation. Entries follow manifest order.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AcceptedResultsBundle {
    pub schema: String,
    pub manifest_sha256: Sha256Digest,
    pub accepted: Vec<AcceptedResult>,
}
/// Schema for owner input exports.
pub const ACCEPTED_RESULTS_SCHEMA: &str = "tuning-campaign-owner-input-v1";

/// Owner analysis/emission inputs. Analysis is performed by this operation;
/// the driver supplies separate observed measurement and assembly provenance.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EmitOwnerRequest {
    pub campaign_id: Token,
    pub manifest_sha256: Sha256Digest,
    pub accepted_results: ArtifactIdentity,
    pub measurement: ObservedProvenance,
    pub assembly: ObservedProvenance,
    pub output: PathBuf,
}
/// Conditional manifest derivation binds fixed reserved slots and exact accepted
/// one-factor inputs. The owner may replace opaque cases, never slot identities.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DeriveManifestRequest {
    pub campaign_id: Token,
    pub original_manifest_sha256: Sha256Digest,
    pub reserved_units: Vec<LaunchUnit>,
    pub accepted_inputs: ArtifactIdentity,
}
/// Derived conditional cases and their immutable input binding.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DerivedManifest {
    pub original_manifest_sha256: Sha256Digest,
    pub accepted_inputs_sha256: Sha256Digest,
    pub units: Vec<LaunchUnit>,
    pub derivation: CanonicalJson,
}
impl DerivedManifest {
    /// Verifies that a derivation only fills the originally reserved cases.
    /// The owner additionally validates its fixed vector semantics and counts.
    pub fn validate(&self, request: &DeriveManifestRequest) -> io::Result<()> {
        if self.original_manifest_sha256 != request.original_manifest_sha256
            || self.accepted_inputs_sha256 != request.accepted_inputs.sha256
            || self.units.len() != request.reserved_units.len()
            || self.units.is_empty()
        {
            return Err(invalid("derived manifest binding mismatch"));
        }
        let mut keys = BTreeSet::new();
        for (derived, reserved) in self.units.iter().zip(&request.reserved_units) {
            derived.validate()?;
            reserved.validate()?;
            if derived.identity.campaign_id != request.campaign_id
                || derived.identity != reserved.identity
                || derived.ordinal != reserved.ordinal
                || derived.process != reserved.process
                || !keys.insert(&derived.key)
            {
                return Err(invalid("derived manifest changed a reserved slot"));
            }
        }
        Ok(())
    }
}

/// Shared explicit producer CLI is `--owner-operation`: one canonical request
/// on stdin and one `GF2_TUNING_RESULT=` OwnerResponse on stdout. These actions
/// do not install tuning or count as additional probe/timed cells.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "operation", rename_all = "kebab-case", deny_unknown_fields)]
pub enum OwnerOperation {
    SelfCheck,
    ListGrid,
    CapabilityReport,
    CampaignManifest {
        request: ManifestRequest,
    },
    ValidateResult {
        unit: Box<LaunchUnit>,
        result: Box<ChildResult>,
    },
    DeriveManifest {
        request: DeriveManifestRequest,
    },
    EmitOwner {
        request: Box<EmitOwnerRequest>,
    },
}
/// Strict neutral owner-operation response; owner-specific evidence stays opaque.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "operation", rename_all = "kebab-case", deny_unknown_fields)]
pub enum OwnerResponse {
    SelfCheck {
        evidence: CanonicalJson,
    },
    ListGrid {
        evidence: CanonicalJson,
    },
    CapabilityReport {
        evidence: CanonicalJson,
    },
    CampaignManifest {
        manifest: Box<OwnerManifest>,
    },
    ValidateResult {
        unit_key: Sha256Digest,
        result_sha256: Sha256Digest,
    },
    DeriveManifest {
        manifest: DerivedManifest,
    },
    EmitOwner {
        artifact: ArtifactIdentity,
        decisions: CanonicalJson,
    },
}

/// Exact child identity and opaque owner evidence inside one stdout result.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ChildResult {
    pub schema: String,
    pub identity: UnitIdentity,
    pub case_sha256: Sha256Digest,
    pub outcome: ChildOutcome,
    pub samples: Vec<TimingSample>,
    pub payload: CanonicalJson,
}
impl ChildResult {
    /// SHA-256 of the canonical compact typed result object only. The stdout
    /// prefix and trailing newline are excluded from semantic validation and
    /// checkpoint identity; their exact stream hash is separate exit evidence.
    pub fn digest(&self) -> io::Result<Sha256Digest> {
        Ok(Sha256Digest::of(&bytes(self)?))
    }
}

/// Unavailable results carry owner-defined diagnostic evidence and are never
/// accepted as complete campaign cells.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ChildOutcome {
    Complete,
    Unavailable,
}

/// One post-interval event; the task identity carries the execution index.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "event", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ProgressKind {
    CalibrationComplete {
        calls: u64,
    },
    WindowComplete {
        repetition: u64,
        calls: u64,
        elapsed_ns: u64,
    },
}
/// Canonical stderr progress record.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProgressRecord {
    pub schema: String,
    pub identity: UnitIdentity,
    pub case_sha256: Sha256Digest,
    pub progress: ProgressKind,
}
impl ProgressRecord {
    /// Builds a validated event for a timed case.
    pub fn new(
        identity: UnitIdentity,
        case_sha256: Sha256Digest,
        progress: ProgressKind,
    ) -> io::Result<Self> {
        let record = Self {
            schema: PROGRESS_SCHEMA.into(),
            identity,
            case_sha256,
            progress,
        };
        record.validate()?;
        Ok(record)
    }
    /// Checks schema, positive measurements and fixed execution coordinates.
    pub fn validate(&self) -> io::Result<()> {
        self.identity.task.validate()?;
        let Task::Measure { execution } = self.identity.task else {
            return Err(invalid("probe emitted timing progress"));
        };
        if self.schema != PROGRESS_SCHEMA {
            return Err(invalid("progress schema mismatch"));
        }
        match self.progress {
            ProgressKind::CalibrationComplete { calls } => {
                if calls == 0 || calls > MAX_CALLS {
                    return Err(invalid("invalid calibration calls"));
                }
            }
            ProgressKind::WindowComplete {
                repetition,
                calls,
                elapsed_ns,
            } => TimingSample::new(execution, repetition, calls, elapsed_ns)
                .map_err(wire_error)
                .map(|_| ())?,
        }
        Ok(())
    }
    /// Parses one prefixed line without its newline; diagnostics return `None`.
    pub fn parse_line(line: &str) -> io::Result<Option<Self>> {
        let Some(payload) = line.strip_prefix(PROGRESS_PREFIX) else {
            return Ok(None);
        };
        let record: Self = decode(payload.as_bytes())?;
        record.validate()?;
        Ok(Some(record))
    }
    /// Writes and flushes a complete buffered stderr line after its interval.
    pub fn write_line(&self, mut writer: impl Write) -> io::Result<()> {
        self.validate()?;
        let mut line = PROGRESS_PREFIX.as_bytes().to_vec();
        line.extend(bytes(self)?);
        line.push(b'\n');
        writer.write_all(&line)?;
        writer.flush()
    }
    /// Maps directly to an authoritative journal event.
    pub fn journal_event(&self) -> JournalEvent {
        match self.progress {
            ProgressKind::CalibrationComplete { .. } => JournalEvent::ExecutionProgress,
            ProgressKind::WindowComplete { .. } => JournalEvent::WindowProgress,
        }
    }
}

/// Poison-on-error child progress validator. Partial or rejected evidence can
/// never later become acceptable by supplying extra records.
pub struct ProgressTracker {
    identity: UnitIdentity,
    case_sha256: Sha256Digest,
    calls: Option<u64>,
    samples: Vec<TimingSample>,
    poisoned: bool,
}
impl ProgressTracker {
    /// Starts an empty stream bound to one immutable child request.
    pub fn new(identity: UnitIdentity, case_sha256: Sha256Digest) -> io::Result<Self> {
        identity.task.validate()?;
        Ok(Self {
            identity,
            case_sha256,
            calls: None,
            samples: Vec::new(),
            poisoned: false,
        })
    }
    /// Validates the next event and returns it for immediate journal append.
    pub fn accept(&mut self, record: ProgressRecord) -> io::Result<ProgressRecord> {
        let result = self.accept_inner(&record);
        if let Err(e) = result {
            self.poisoned = true;
            return Err(e);
        }
        Ok(record)
    }
    fn accept_inner(&mut self, record: &ProgressRecord) -> io::Result<()> {
        if self.poisoned {
            return Err(invalid("progress stream already rejected"));
        }
        record.validate()?;
        if record.identity != self.identity || record.case_sha256 != self.case_sha256 {
            return Err(invalid("wrong progress identity"));
        }
        match record.progress {
            ProgressKind::CalibrationComplete { calls } => {
                if self.calls.replace(calls).is_some() {
                    return Err(invalid("duplicate calibration"));
                }
            }
            ProgressKind::WindowComplete {
                repetition,
                calls,
                elapsed_ns,
            } => {
                if self.calls != Some(calls) || repetition != self.samples.len() as u64 {
                    return Err(invalid("window order/calls mismatch"));
                }
                let Task::Measure { execution } = self.identity.task else {
                    return Err(invalid("probe progress"));
                };
                self.samples.push(
                    TimingSample::new(execution, repetition, calls, elapsed_ns)
                        .map_err(wire_error)?,
                );
            }
        }
        Ok(())
    }
    /// Requires one canonical stdout result whose windows exactly equal the
    /// streamed evidence. Owner semantics must pass before checkpointing.
    pub fn finish(&mut self, stdout: &str) -> io::Result<ChildResult> {
        let result = self.finish_inner(stdout);
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }
    fn finish_inner(&self, stdout: &str) -> io::Result<ChildResult> {
        if self.poisoned {
            return Err(invalid("progress stream already rejected"));
        }
        if stdout.contains('\r') {
            return Err(invalid("noncanonical result newline"));
        }
        let result: ChildResult = transport::parse_result(stdout).map_err(wire_error)?;
        if result.schema != RESULT_SCHEMA
            || result.identity != self.identity
            || result.case_sha256 != self.case_sha256
            || result.outcome != ChildOutcome::Complete
            || result.samples != self.samples
        {
            return Err(invalid("result/progress identity or outcome mismatch"));
        }
        match self.identity.task {
            Task::Probe => {
                if self.calls.is_some() || !self.samples.is_empty() {
                    return Err(invalid("probe timing evidence"));
                }
            }
            Task::Measure { .. } => {
                if self.calls.is_none() || self.samples.len() != WINDOWS as usize {
                    return Err(invalid("incomplete timed progress"));
                }
            }
        }
        Ok(result)
    }
}

/// Measures with the shared timing engine and emits canonical progress through
/// a caller writer. Callback writes occur only after calibration/windows end.
pub fn execution_with_progress(
    identity: &UnitIdentity,
    case_sha256: &Sha256Digest,
    body: &mut impl FnMut(usize),
    writer: &mut impl Write,
) -> io::Result<Vec<TimingSample>> {
    let Task::Measure { execution } = identity.task else {
        return Err(invalid("probe cannot execute timing windows"));
    };
    identity.task.validate()?;
    timing::execution_windows_with_progress(execution, body, |event| {
        let progress = match event {
            timing::TimingProgress::CalibrationComplete { calls } => {
                ProgressKind::CalibrationComplete { calls }
            }
            timing::TimingProgress::WindowComplete(sample) => ProgressKind::WindowComplete {
                repetition: sample.repetition,
                calls: sample.calls,
                elapsed_ns: sample.elapsed_ns,
            },
        };
        ProgressRecord::new(identity.clone(), case_sha256.clone(), progress)?
            .write_line(&mut *writer)
    })
}

/// Proposed outcome, finalized only after truthful wrapper/release evidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SessionOutcome {
    Complete,
    Failed,
    Paused,
    BudgetExhausted,
}
impl From<SessionOutcome> for TerminalState {
    fn from(v: SessionOutcome) -> Self {
        match v {
            SessionOutcome::Complete => Self::Complete,
            SessionOutcome::Failed => Self::Failed,
            SessionOutcome::Paused => Self::Paused,
            SessionOutcome::BudgetExhausted => Self::BudgetExhausted,
        }
    }
}
/// Actual inherited full-host lock observation supplied by the runner.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LockEvidence {
    pub lock_path: PathBuf,
    pub holder_pid: u32,
    pub observation: Token,
}
/// Synced work outcome; clean completion requires all descendants reaped.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkEvidence {
    pub outcome: SessionOutcome,
    pub all_descendants_reaped: bool,
    pub active_elapsed_ns: u64,
}
/// Observed wrapper termination. Exactly one of exit code or signal is present.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WrapperEvidence {
    pub exit_code: Option<i32>,
    pub signal: Option<i32>,
}
impl WrapperEvidence {
    fn validate(&self) -> io::Result<()> {
        if self.exit_code.is_some() == self.signal.is_some()
            || self.exit_code.is_some_and(|v| !(0..=255).contains(&v))
            || self.signal.is_some_and(|v| v <= 0)
        {
            Err(invalid("invalid wrapper termination evidence"))
        } else {
            Ok(())
        }
    }
}
/// Independent observation proving the prior hold has ended. PID death alone
/// is insufficient; the finalizer must observe lock availability as well.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IndependentReleaseEvidence {
    pub holder_dead: bool,
    pub descendants_dead: bool,
    pub lock_path: PathBuf,
    pub observed_utc: String,
    pub lock_available: bool,
}
impl IndependentReleaseEvidence {
    fn validate(&self, lock: &LockEvidence) -> io::Result<()> {
        absolute_path(&self.lock_path)?;
        if !self.holder_dead
            || !self.descendants_dead
            || !self.lock_available
            || self.lock_path != lock.lock_path
            || self.observed_utc.is_empty()
        {
            Err(invalid(
                "release lacks independent lock and process-tree proof",
            ))
        } else {
            Ok(())
        }
    }
}
/// Clean wrapper return or independent observation after an unclean return.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ReleaseEvidence {
    CleanReaped,
    Independent {
        evidence: IndependentReleaseEvidence,
    },
}
/// Current mode-visible lifecycle state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SessionState {
    Prepared,
    LockHeld,
    WorkFinished,
    WrapperReturned,
    ReleaseUnobserved,
    LockReleased,
    Interrupted,
    Terminal,
}
/// State machine shared by prepare/run/finalize. It consumes observed evidence,
/// not PID guesses; the driver must actually obtain the supplied OS facts.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SessionLifecycle {
    state: SessionState,
    lock: Option<LockEvidence>,
    work: Option<WorkEvidence>,
    wrapper: Option<WrapperEvidence>,
    release: Option<ReleaseEvidence>,
    outcome: Option<SessionOutcome>,
}
impl SessionLifecycle {
    /// Initial state before a lock wrapper is invoked.
    pub fn prepared() -> Self {
        Self {
            state: SessionState::Prepared,
            lock: None,
            work: None,
            wrapper: None,
            release: None,
            outcome: None,
        }
    }
    /// Current validated transition state.
    pub fn state(&self) -> SessionState {
        self.state
    }
    /// Records inherited held-lock evidence once.
    pub fn lock_held(&mut self, evidence: LockEvidence) -> io::Result<()> {
        if self.state != SessionState::Prepared || evidence.holder_pid == 0 {
            return Err(invalid("illegal LockHeld transition"));
        }
        absolute_path(&evidence.lock_path)?;
        self.lock = Some(evidence);
        self.state = SessionState::LockHeld;
        Ok(())
    }
    /// Ends work while still inside the wrapper; cannot publish a terminal.
    pub fn work_finished(&mut self, evidence: WorkEvidence) -> io::Result<()> {
        if self.state != SessionState::LockHeld
            || !evidence.all_descendants_reaped
            || evidence.active_elapsed_ns == 0
        {
            return Err(invalid(
                "WorkFinished requires held lock and reaped descendants",
            ));
        }
        self.work = Some(evidence);
        self.state = SessionState::WorkFinished;
        Ok(())
    }
    /// Records observed wrapper return, including failures before LockHeld.
    pub fn wrapper_returned(&mut self, evidence: WrapperEvidence) -> io::Result<()> {
        if !matches!(
            self.state,
            SessionState::Prepared | SessionState::LockHeld | SessionState::WorkFinished
        ) {
            return Err(invalid("illegal WrapperReturned transition"));
        }
        evidence.validate()?;
        self.wrapper = Some(evidence);
        self.state = SessionState::WrapperReturned;
        Ok(())
    }
    /// Preserves a nonterminal unproven release. No resume or publication is
    /// legal until an independent observation establishes that hold's end.
    pub fn release_unobserved(&mut self) -> io::Result<()> {
        if !matches!(
            self.state,
            SessionState::WrapperReturned | SessionState::LockHeld | SessionState::WorkFinished
        ) || self.lock.is_none()
        {
            return Err(invalid("illegal release-unobserved transition"));
        }
        self.state = SessionState::ReleaseUnobserved;
        Ok(())
    }
    /// Records actual release. Independent proof also permits recovery after
    /// process death without fabricating a historical wrapper-return event.
    pub fn observe_release(&mut self, evidence: ReleaseEvidence) -> io::Result<()> {
        if !matches!(
            self.state,
            SessionState::WrapperReturned
                | SessionState::ReleaseUnobserved
                | SessionState::LockHeld
                | SessionState::WorkFinished
        ) {
            return Err(invalid("illegal LockRelease transition"));
        }
        let lock = self
            .lock
            .as_ref()
            .ok_or_else(|| invalid("cannot release a hold that never started"))?;
        match &evidence {
            ReleaseEvidence::CleanReaped => {
                if self.wrapper.is_none()
                    || self.work.as_ref().is_none_or(|w| !w.all_descendants_reaped)
                {
                    return Err(invalid("clean release requires work and wrapper return"));
                }
            }
            ReleaseEvidence::Independent { evidence } => evidence.validate(lock)?,
        }
        self.release = Some(evidence);
        self.state = SessionState::LockReleased;
        Ok(())
    }
    /// Closes an interrupted session after independently observed release,
    /// retaining a censored active duration instead of fabricating an end time.
    pub fn interrupted(&mut self, active_elapsed_censored: bool) -> io::Result<()> {
        if self.state != SessionState::LockReleased
            || !active_elapsed_censored
            || !matches!(self.release, Some(ReleaseEvidence::Independent { .. }))
        {
            return Err(invalid(
                "interruption requires independent release and censored duration",
            ));
        }
        self.state = SessionState::Interrupted;
        Ok(())
    }

    /// Finalizes only a release-observed session, or a failed wrapper that
    /// never acquired the lock. Complete requires matching successful work
    /// and wrapper exit, so an interrupted run cannot manufacture success.
    pub fn terminal(&mut self, outcome: SessionOutcome) -> io::Result<()> {
        let prelock_failure = self.state == SessionState::WrapperReturned
            && self.lock.is_none()
            && outcome == SessionOutcome::Failed
            && self
                .wrapper
                .as_ref()
                .is_some_and(|w| w.exit_code != Some(0));
        if self.state != SessionState::LockReleased && !prelock_failure {
            return Err(invalid("terminal before observed release"));
        }
        if let Some(work) = &self.work {
            if outcome != work.outcome && outcome != SessionOutcome::Failed {
                return Err(invalid("terminal contradicts proposed work outcome"));
            }
        }
        if outcome != SessionOutcome::Failed && self.work.is_none() {
            return Err(invalid("unclean run cannot claim clean terminal"));
        }
        if outcome == SessionOutcome::Complete
            && self.wrapper.as_ref().is_none_or(|w| w.exit_code != Some(0))
        {
            return Err(invalid("complete contradicts wrapper exit"));
        }
        self.outcome = Some(outcome);
        self.state = SessionState::Terminal;
        Ok(())
    }
}

/// Exactly the three external driver entry modes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SessionMode {
    PrepareSession,
    RunSession,
    FinalizeSession,
}
/// Immutable hand-off descriptor. Its canonical digest is bound into every
/// create-new mode claim and transition; replacing a descriptor rejects.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SessionDescriptor {
    pub schema: String,
    pub campaign_id: Token,
    pub session_id: Token,
    pub channels: SessionChannels,
    pub identity: ResumeIdentity,
    pub counts: DeclaredCounts,
    pub lock_path: PathBuf,
}
impl SessionDescriptor {
    /// Validates immutable identity and the fixed active budget contract.
    pub fn validate(&self) -> io::Result<()> {
        self.channels.validate()?;
        absolute_path(&self.lock_path)?;
        if self.schema != LIFECYCLE_SCHEMA
            || self.identity.lifecycle_schema != LIFECYCLE_SCHEMA
            || self.identity.feature_contract != FEATURE_CONTRACT
            || self.identity.thread_contract != THREAD_CONTRACT
            || self.counts != DeclaredCounts::for_cells(self.counts.cells)?
        {
            return Err(invalid("session descriptor schema or counts mismatch"));
        }
        crate::journal::validate_resume_identity(&self.identity)
    }
    /// Digest consumed by all mode claims and transition records.
    pub fn digest(&self) -> io::Result<Sha256Digest> {
        self.validate()?;
        Ok(Sha256Digest::of(&bytes(self)?))
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ModeClaim {
    schema: String,
    descriptor_sha256: Sha256Digest,
    mode: SessionMode,
}
/// Immutable lifecycle transition evidence, replayed strictly after a crash.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "transition", rename_all = "kebab-case", deny_unknown_fields)]
pub enum SessionTransition {
    LockHeld { evidence: LockEvidence },
    WorkFinished { evidence: WorkEvidence },
    WrapperReturned { evidence: WrapperEvidence },
    ReleaseUnobserved,
    LockRelease { evidence: ReleaseEvidence },
    Terminal { outcome: SessionOutcome },
    Interrupted { active_elapsed_censored: bool },
}
impl SessionTransition {
    fn apply(&self, lifecycle: &mut SessionLifecycle) -> io::Result<()> {
        match self {
            Self::LockHeld { evidence } => lifecycle.lock_held(evidence.clone()),
            Self::WorkFinished { evidence } => lifecycle.work_finished(evidence.clone()),
            Self::WrapperReturned { evidence } => lifecycle.wrapper_returned(evidence.clone()),
            Self::ReleaseUnobserved => lifecycle.release_unobserved(),
            Self::LockRelease { evidence } => lifecycle.observe_release(evidence.clone()),
            Self::Terminal { outcome } => lifecycle.terminal(*outcome),
            Self::Interrupted {
                active_elapsed_censored,
            } => lifecycle.interrupted(*active_elapsed_censored),
        }
    }
    fn event(&self) -> JournalEvent {
        match self {
            Self::LockHeld { .. } => JournalEvent::LockHold,
            Self::WorkFinished { .. } => JournalEvent::WorkFinished,
            Self::WrapperReturned { .. } => JournalEvent::WrapperReturned,
            Self::ReleaseUnobserved => JournalEvent::ReleaseUnobserved,
            Self::LockRelease { .. } => JournalEvent::LockRelease,
            Self::Terminal { outcome } => TerminalState::from(*outcome).into(),
            Self::Interrupted { .. } => JournalEvent::Interrupted,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct TransitionRecord {
    descriptor_sha256: Sha256Digest,
    sequence: u64,
    transition: SessionTransition,
    recovery: bool,
}
/// Exclusive per-mode writer guard plus an immutable session descriptor.
/// The OS lock is held for this value's lifetime; a durable active-session
/// claim remains between modes and after process death. Recovery must reopen
/// that exact descriptor and establish release before another session starts.
pub struct SessionStore {
    _writer: File,
    stage: PathBuf,
    root: PathBuf,
    descriptor: SessionDescriptor,
    digest: Sha256Digest,
    lifecycle: SessionLifecycle,
    transitions: Vec<TransitionRecord>,
    mode: Option<SessionMode>,
}
impl SessionStore {
    fn writer(stage: &Path) -> io::Result<File> {
        let path = stage.join("session-writer.lock");
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;
        file.try_lock().map_err(|e| {
            io::Error::new(
                io::ErrorKind::WouldBlock,
                format!("another session writer is active: {e}"),
            )
        })?;
        Ok(file)
    }
    /// Creates a session and sole active claim. Existing active claims reject,
    /// including dead sessions; recover and retire their exact descriptor first.
    pub fn prepare(descriptor: SessionDescriptor) -> io::Result<Self> {
        descriptor.validate()?;
        let stage = fs::canonicalize(&descriptor.channels.stage)?;
        if stage != descriptor.channels.stage {
            return Err(invalid("stage is not canonical"));
        }
        let writer = Self::writer(&stage)?;
        let active = stage.join("active-session.json");
        if active.try_exists()? {
            return Err(invalid("prior active session requires strict recovery"));
        }
        let sessions = stage.join("sessions");
        fs::create_dir_all(&sessions)?;
        File::open(&stage)?.sync_all()?;
        let root = sessions.join(descriptor.session_id.as_str());
        fs::create_dir(&root)?;
        File::open(&sessions)?.sync_all()?;
        let encoded = bytes(&descriptor)?;
        publish_session_control(&stage, &root, &descriptor, "descriptor.json", &encoded)?;
        let digest = descriptor.digest()?;
        let claim = ModeClaim {
            schema: LIFECYCLE_SCHEMA.into(),
            descriptor_sha256: digest.clone(),
            mode: SessionMode::PrepareSession,
        };
        publish_session_control(
            &stage,
            &root,
            &descriptor,
            "prepare-session.json",
            &bytes(&claim)?,
        )?;
        publish_session_control(&stage, &root, &descriptor, "active-session.json", &encoded)?;
        Ok(Self {
            _writer: writer,
            stage,
            root,
            descriptor,
            digest,
            lifecycle: SessionLifecycle::prepared(),
            transitions: Vec::new(),
            mode: None,
        })
    }
    /// Strictly reopens the current session; this does not consume a run/finalize
    /// mode. Those descriptors are consumed once by `consume_mode`.
    pub fn reopen(expected: SessionDescriptor) -> io::Result<Self> {
        expected.validate()?;
        let stage = fs::canonicalize(&expected.channels.stage)?;
        if stage != expected.channels.stage {
            return Err(invalid("stage is not canonical"));
        }
        let writer = Self::writer(&stage)?;
        if stage.join("active-session.json").try_exists()? {
            let active: SessionDescriptor = decode(&fs::read(stage.join("active-session.json"))?)?;
            if active != expected {
                return Err(invalid("active descriptor mismatch before recovery"));
            }
        }
        let root = stage.join("sessions").join(expected.session_id.as_str());
        if root.join("descriptor.json").try_exists()? {
            let stored: SessionDescriptor = decode(&fs::read(root.join("descriptor.json"))?)?;
            if stored != expected {
                return Err(invalid("stored descriptor mismatch before recovery"));
            }
        }
        if !root.is_dir() {
            return Err(invalid("no session preparation exists to recover"));
        }
        recover_session_publication(&stage, &root, &expected)?;
        let descriptor_path = root.join("descriptor.json");
        if !descriptor_path.try_exists()? {
            // Before the first immutable descriptor commit no mode could have
            // returned or launched work; supplied preparation identity is the
            // sole allowed completion after preserving uncommitted bytes.
            if fs::read_dir(&root)?
                .any(|entry| entry.is_ok_and(|entry| entry.file_name() != "publication"))
            {
                return Err(invalid(
                    "missing descriptor has conflicting session evidence",
                ));
            }
            publish_session_control(
                &stage,
                &root,
                &expected,
                "descriptor.json",
                &bytes(&expected)?,
            )?;
        }
        let stored: SessionDescriptor = decode(&fs::read(&descriptor_path)?)?;
        if stored != expected {
            return Err(invalid("session descriptor changed"));
        }
        let digest = expected.digest()?;
        if !root.join("prepare-session.json").try_exists()? {
            if stage.join("active-session.json").try_exists()?
                || root.join("run-session.json").try_exists()?
                || root.join("finalize-session.json").try_exists()?
            {
                return Err(invalid(
                    "published/executed session lost its preparation descriptor",
                ));
            }
            let claim = ModeClaim {
                schema: LIFECYCLE_SCHEMA.into(),
                descriptor_sha256: digest.clone(),
                mode: SessionMode::PrepareSession,
            };
            publish_session_control(
                &stage,
                &root,
                &expected,
                "prepare-session.json",
                &bytes(&claim)?,
            )?;
        }
        if !stage.join("active-session.json").try_exists()? {
            if root.join("retired.json").try_exists()?
                || root.join("run-session.json").try_exists()?
                || root.join("finalize-session.json").try_exists()?
            {
                return Err(invalid(
                    "retired or executed session lacks its active claim",
                ));
            }
            publish_session_control(
                &stage,
                &root,
                &expected,
                "active-session.json",
                &bytes(&expected)?,
            )?;
        }
        let active: SessionDescriptor = decode(&fs::read(stage.join("active-session.json"))?)?;
        if active != expected {
            return Err(invalid("active descriptor mismatch"));
        }
        session_publication_recoveries(&root, &expected)?;
        let mut records = BTreeMap::new();
        for entry in fs::read_dir(&root)? {
            let entry = entry?;
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| invalid("non-UTF-8 session filename"))?;
            if name == "publication" {
                if !entry.file_type()?.is_dir() {
                    return Err(invalid("publication state is not a directory"));
                }
                continue;
            }
            if !entry.file_type()?.is_file() {
                return Err(invalid("session evidence is not a regular file"));
            }
            if name == "descriptor.json" {
                continue;
            }
            if name == "retired.json" {
                let retired: RetirementRecord = decode(&fs::read(entry.path())?)?;
                if retired.descriptor_sha256 != digest {
                    return Err(invalid("retirement descriptor mismatch"));
                }
                continue;
            }
            if let Some(mode) = match name.as_str() {
                "prepare-session.json" => Some(SessionMode::PrepareSession),
                "run-session.json" => Some(SessionMode::RunSession),
                "finalize-session.json" => Some(SessionMode::FinalizeSession),
                _ => None,
            } {
                let claim: ModeClaim = decode(&fs::read(entry.path())?)?;
                if claim.schema != LIFECYCLE_SCHEMA
                    || claim.descriptor_sha256 != digest
                    || claim.mode != mode
                {
                    return Err(invalid("mode descriptor mismatch"));
                }
                continue;
            }
            let record: TransitionRecord = decode(&fs::read(entry.path())?)?;
            if name != format!("transition-{:06}.json", record.sequence)
                || record.descriptor_sha256 != digest
                || records.insert(record.sequence, record).is_some()
            {
                return Err(invalid("unrecognized session transition file"));
            }
        }
        let mut lifecycle = SessionLifecycle::prepared();
        let mut transitions = Vec::new();
        for (index, (sequence, record)) in records.into_iter().enumerate() {
            if sequence != index as u64 {
                return Err(invalid("session transition sequence gap"));
            }
            if record.recovery {
                if !matches!(
                    record.transition,
                    SessionTransition::ReleaseUnobserved
                        | SessionTransition::LockRelease {
                            evidence: ReleaseEvidence::Independent { .. }
                        }
                        | SessionTransition::Terminal {
                            outcome: SessionOutcome::Failed
                        }
                        | SessionTransition::Interrupted {
                            active_elapsed_censored: true
                        }
                ) {
                    return Err(invalid("invalid persisted recovery transition"));
                }
            } else {
                let claim = match record.transition {
                    SessionTransition::LockHeld { .. } | SessionTransition::WorkFinished { .. } => {
                        "run-session.json"
                    }
                    _ => "finalize-session.json",
                };
                if !root.join(claim).try_exists()? {
                    return Err(invalid("transition lacks consumed mode descriptor"));
                }
            }
            if let SessionTransition::LockHeld { evidence } = &record.transition {
                if evidence.lock_path != expected.lock_path {
                    return Err(invalid("persisted held-lock path mismatch"));
                }
            }
            record.transition.apply(&mut lifecycle)?;
            transitions.push(record);
        }
        Ok(Self {
            _writer: writer,
            stage,
            root,
            descriptor: expected,
            digest,
            lifecycle,
            transitions,
            mode: None,
        })
    }
    /// Repairs a torn journal in this same claimed session, then projects
    /// immutable transitions. Only independently observed release can permit
    /// recovery interruption and retirement; no replacement session is opened.
    pub fn repair_log(&self) -> io::Result<ExecutionLog> {
        let mut log = ExecutionLog::repair_active(
            &self.descriptor.channels.execution_log,
            self.descriptor.campaign_id.as_str(),
            self.descriptor.session_id.as_str(),
        )?;
        self.reconcile_journal(&mut log)?;
        Ok(log)
    }

    /// Current replayed state.
    pub fn lifecycle(&self) -> &SessionLifecycle {
        &self.lifecycle
    }
    /// Immutable session descriptor.
    pub fn descriptor(&self) -> &SessionDescriptor {
        &self.descriptor
    }
    /// Prints and flushes the canonical path, then durably records preparation.
    /// The prepare-mode caller invokes this before handing the descriptor to
    /// the lock wrapper; repeated calls never duplicate prepared evidence.
    pub fn announce_prepared(
        &self,
        log: &mut ExecutionLog,
        writer: &mut impl Write,
    ) -> io::Result<()> {
        if log.path() != self.descriptor.channels.execution_log {
            return Err(invalid("prepared log path mismatch"));
        }
        log.announce(writer)?;
        self.reconcile_journal(log)
    }

    /// Consumes a run/finalize handoff exactly once with a durable create-new
    /// record. An interrupted mode uses recovery, never repeats the launch.
    pub fn consume_mode(&mut self, mode: SessionMode) -> io::Result<()> {
        let (name, legal) = match mode {
            SessionMode::PrepareSession => ("prepare-session.json", false),
            SessionMode::RunSession => (
                "run-session.json",
                self.lifecycle.state == SessionState::Prepared,
            ),
            SessionMode::FinalizeSession => (
                "finalize-session.json",
                matches!(
                    self.lifecycle.state,
                    SessionState::Prepared | SessionState::LockHeld | SessionState::WorkFinished
                ),
            ),
        };
        if !legal {
            return Err(invalid("illegal or duplicate session mode"));
        }
        let claim = ModeClaim {
            schema: LIFECYCLE_SCHEMA.into(),
            descriptor_sha256: self.digest.clone(),
            mode,
        };
        publish_session_control(
            &self.stage,
            &self.root,
            &self.descriptor,
            name,
            &bytes(&claim)?,
        )?;
        self.mode = Some(mode);
        Ok(())
    }
    /// Commits one immutable transition before its journal projection. Reopen
    /// plus `reconcile_journal` repairs a killed projection without repeating
    /// the transition or fabricating missing observed OS evidence.
    pub fn transition(
        &mut self,
        log: &mut ExecutionLog,
        transition: SessionTransition,
    ) -> io::Result<()> {
        let expected = match transition {
            SessionTransition::LockHeld { .. } | SessionTransition::WorkFinished { .. } => {
                SessionMode::RunSession
            }
            _ => SessionMode::FinalizeSession,
        };
        if self.mode != Some(expected) {
            return Err(invalid(
                "transition requires this process to consume its mode descriptor",
            ));
        }
        self.commit_transition(log, transition, false)
    }

    /// Records independently observed recovery after a killed mode. This
    /// never repeats a run or fabricates WorkFinished/WrapperReturned. Only
    /// unobserved release, independently proven release and failed closure
    /// are allowed; original mode descriptors remain consumed.
    pub fn recover_transition(
        &mut self,
        log: &mut ExecutionLog,
        transition: SessionTransition,
    ) -> io::Result<()> {
        if !matches!(
            transition,
            SessionTransition::ReleaseUnobserved
                | SessionTransition::LockRelease {
                    evidence: ReleaseEvidence::Independent { .. }
                }
                | SessionTransition::Terminal {
                    outcome: SessionOutcome::Failed
                }
                | SessionTransition::Interrupted {
                    active_elapsed_censored: true
                }
        ) {
            return Err(invalid("illegal recovery transition"));
        }
        self.commit_transition(log, transition, true)
    }

    fn commit_transition(
        &mut self,
        log: &mut ExecutionLog,
        transition: SessionTransition,
        recovery: bool,
    ) -> io::Result<()> {
        self.reconcile_journal(log)?;
        let mut next = self.lifecycle.clone();
        transition.apply(&mut next)?;
        if let SessionTransition::LockHeld { evidence } = &transition {
            if evidence.lock_path != self.descriptor.lock_path {
                return Err(invalid("held lock path differs from descriptor"));
            }
        }
        let record = TransitionRecord {
            descriptor_sha256: self.digest.clone(),
            sequence: self.transitions.len() as u64,
            transition,
            recovery,
        };
        publish_session_control(
            &self.stage,
            &self.root,
            &self.descriptor,
            &format!("transition-{:06}.json", record.sequence),
            &bytes(&record)?,
        )?;
        self.lifecycle = next;
        self.transitions.push(record);
        self.reconcile_journal(log)
    }
    /// Idempotently journals committed transitions in order. Missing earlier
    /// events are filled before further work; conflicting/duplicate evidence
    /// fails closed. A terminal transition is projected exactly once.
    pub fn reconcile_journal(&self, log: &mut ExecutionLog) -> io::Result<()> {
        if log.path() != self.descriptor.channels.execution_log
            || log.campaign_id() != self.descriptor.campaign_id.as_str()
            || log.session_id() != self.descriptor.session_id.as_str()
        {
            return Err(invalid("session journal channel/identity mismatch"));
        }
        let data = log.validated_synced_prefix()?;
        let records = ExecutionLog::validate_prefix(&data, self.descriptor.campaign_id.as_str())?;
        let prepared: Vec<_> = records
            .iter()
            .filter(|record| {
                record.session_id == self.descriptor.session_id.as_str()
                    && record.event == JournalEvent::SessionPrepared
            })
            .collect();
        let expected_prepared = serde_json::json!({"session_descriptor": self.descriptor});
        match prepared.as_slice() {
            [] => {
                log.append(JournalEvent::SessionPrepared, None, expected_prepared)?;
            }
            [record] if record.details == expected_prepared => {}
            _ => return Err(invalid("duplicate or mismatched session-prepared evidence")),
        }
        for recovery in session_publication_recoveries(&self.root, &self.descriptor)? {
            let expected = serde_json::json!({"publication_recovery": recovery});
            let matches: Vec<_> = records
                .iter()
                .filter(|record| {
                    record.session_id == self.descriptor.session_id.as_str()
                        && record.event == JournalEvent::SessionRecovery
                        && record
                            .details
                            .get("publication_recovery")
                            .and_then(|value| value.get("temporary_path"))
                            == expected
                                .get("publication_recovery")
                                .and_then(|value| value.get("temporary_path"))
                })
                .collect();
            match matches.as_slice() {
                [] if records.last().is_some_and(|record| {
                    matches!(
                        record.event,
                        JournalEvent::Complete
                            | JournalEvent::Failed
                            | JournalEvent::Paused
                            | JournalEvent::BudgetExhausted
                            | JournalEvent::Interrupted
                    )
                }) =>
                {
                    // A synced terminal is immutable. Post-terminal descriptor
                    // cleanup remains in durable recovery sidecars and is
                    // covered by the finalizer's artifact checksum bundle.
                }
                [] => {
                    log.append(JournalEvent::SessionRecovery, None, expected)?;
                }
                [record] if record.details == expected => {}
                _ => return Err(invalid("session publication recovery journal conflict")),
            }
        }
        let mut found = BTreeMap::new();
        for record in &records {
            if record.session_id != self.descriptor.session_id.as_str() {
                continue;
            }
            if let Some(value) = record.details.get("session_transition") {
                let saved: TransitionRecord =
                    serde_json::from_value(value.clone()).map_err(wire_error)?;
                if saved.descriptor_sha256 != self.digest
                    || saved.sequence >= self.transitions.len() as u64
                    || self.transitions[saved.sequence as usize] != saved
                    || record.event != saved.transition.event()
                    || found.insert(saved.sequence, record.sequence).is_some()
                {
                    return Err(invalid("conflicting session journal projection"));
                }
            }
        }
        let mut missing = false;
        for transition in &self.transitions {
            if found.contains_key(&transition.sequence) {
                if missing {
                    return Err(invalid("session projection has an interior gap"));
                }
                continue;
            }
            missing = true;
            let details = serde_json::json!({"session_transition":transition});
            match transition.transition {
                SessionTransition::Terminal { outcome } => {
                    log.terminal(outcome.into(), details)?;
                }
                SessionTransition::Interrupted { .. } => {
                    log.interrupted_after_release(
                        serde_json::to_value(transition).map_err(wire_error)?,
                    )?;
                }
                _ => {
                    log.append(transition.transition.event(), None, details)?;
                }
            }
        }
        Ok(())
    }
    /// Removes the active claim only after terminal projection and its checksum
    /// artifact are durable. Existing session evidence remains immutable.
    pub fn retire(self, log: &mut ExecutionLog, checksum: &ArtifactIdentity) -> io::Result<()> {
        if !matches!(
            self.lifecycle.state,
            SessionState::Terminal | SessionState::Interrupted
        ) {
            return Err(invalid("unclosed session cannot retire"));
        }
        self.reconcile_journal(log)?;
        let saved: SessionChecksum = checksum.read()?;
        saved.validate(&self.descriptor, log.path())?;
        let retired = RetirementRecord {
            descriptor_sha256: self.digest.clone(),
            checksum: checksum.clone(),
        };
        let encoded = bytes(&retired)?;
        if self.root.join("retired.json").try_exists()? {
            if fs::read(self.root.join("retired.json"))? != encoded {
                return Err(invalid("retirement checksum changed"));
            }
        } else {
            publish_session_control(
                &self.stage,
                &self.root,
                &self.descriptor,
                "retired.json",
                &encoded,
            )?;
        }
        fs::remove_file(self.stage.join("active-session.json"))?;
        File::open(&self.stage)?.sync_all()
    }
}

/// Child wall-clock outcome recorded by the OS runner. The runner owns actual
/// concurrent pipe drains and process-tree termination; a timeout is never a
/// usable result even if stdout happens to contain a complete line.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "outcome", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ProcessOutcome {
    Exited {
        pid: u32,
        exit_code: i32,
        elapsed_ns: u64,
        all_descendants_reaped: bool,
    },
    Signaled {
        pid: u32,
        signal: i32,
        elapsed_ns: u64,
        all_descendants_reaped: bool,
    },
    TimedOut {
        pid: u32,
        elapsed_ns: u64,
        kill_grace_exhausted: bool,
        all_descendants_reaped: bool,
    },
}
impl ProcessOutcome {
    /// Validates actual observations against the fixed 120+5 second protocol.
    pub fn validate(&self) -> io::Result<()> {
        match self {
            Self::Exited {
                pid,
                exit_code,
                elapsed_ns,
                ..
            } => {
                if *pid == 0 || !(0..=255).contains(exit_code) || *elapsed_ns == 0 {
                    return Err(invalid("invalid process exit"));
                }
            }
            Self::Signaled {
                pid,
                signal,
                elapsed_ns,
                ..
            } => {
                if *pid == 0 || *signal <= 0 || *elapsed_ns == 0 {
                    return Err(invalid("invalid process signal"));
                }
            }
            Self::TimedOut {
                pid,
                elapsed_ns,
                kill_grace_exhausted,
                ..
            } => {
                if *pid == 0
                    || *elapsed_ns < CHILD_TIMEOUT_SECONDS * 1_000_000_000
                    || (*kill_grace_exhausted
                        && *elapsed_ns
                            < (CHILD_TIMEOUT_SECONDS + CHILD_KILL_GRACE_SECONDS) * 1_000_000_000)
                {
                    return Err(invalid("timeout contradicts protocol duration"));
                }
            }
        }
        Ok(())
    }
    /// Only a clean zero exit with a completely reaped tree is eligible.
    pub fn accepts_result(&self) -> io::Result<bool> {
        self.validate()?;
        Ok(matches!(
            self,
            Self::Exited {
                exit_code: 0,
                all_descendants_reaped: true,
                ..
            }
        ))
    }
}
/// Remaining-budget check performed before every launch and after every result.
/// Returns false with fewer than 125 seconds left; no session may widen it.
pub fn may_launch_child(active_elapsed_ns: u64) -> bool {
    active_elapsed_ns
        <= (SESSION_BUDGET_SECONDS - CHILD_TIMEOUT_SECONDS - CHILD_KILL_GRACE_SECONDS)
            * 1_000_000_000
}

/// Exact complete synced journal prefix bound into a checkpoint or checksum.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct JournalPrefix {
    pub byte_len: u64,
    pub sha256: Sha256Digest,
    pub record_count: u64,
}
impl JournalPrefix {
    /// Validates and syncs all currently appended bytes before producing a pin.
    pub fn sync(log: &mut ExecutionLog, campaign_id: &Token) -> io::Result<Self> {
        let data = log.validated_synced_prefix()?;
        let records = ExecutionLog::validate_prefix(&data, campaign_id.as_str())?;
        Ok(Self {
            byte_len: data.len() as u64,
            sha256: Sha256Digest::of(&data),
            record_count: records.len() as u64,
        })
    }
    /// Validates the exact pinned prefix; later appended sessions are allowed.
    /// Missing, shortened, malformed or changed evidence rejects.
    pub fn verify(&self, path: &Path, campaign_id: &Token) -> io::Result<Vec<JournalRecord>> {
        let mut data = Vec::new();
        File::open(path)?
            .take(self.byte_len)
            .read_to_end(&mut data)?;
        if data.len() as u64 != self.byte_len || Sha256Digest::of(&data) != self.sha256 {
            return Err(invalid("journal prefix is missing, shortened or changed"));
        }
        let records = ExecutionLog::validate_prefix(&data, campaign_id.as_str())?;
        if records.len() as u64 != self.record_count {
            return Err(invalid("journal prefix record count mismatch"));
        }
        Ok(records)
    }
}
/// Structured process-start evidence; a fresh attempt has a unique token.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ChildSpawnEvidence {
    pub unit_key: Sha256Digest,
    pub case_sha256: Sha256Digest,
    pub attempt: Token,
    pub pid: u32,
}
/// Process termination and exact diagnostic stream digests.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ChildExitEvidence {
    pub unit_key: Sha256Digest,
    pub case_sha256: Sha256Digest,
    pub attempt: Token,
    pub outcome: ProcessOutcome,
    pub stdout_sha256: Sha256Digest,
    pub stderr_sha256: Sha256Digest,
}
/// Owner validation binding, written only after strict semantic validation.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResultValidationEvidence {
    pub unit_key: Sha256Digest,
    pub case_sha256: Sha256Digest,
    pub attempt: Token,
    pub result_sha256: Sha256Digest,
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct JournalProgress {
    attempt: Token,
    record: ProgressRecord,
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Diagnostic {
    unit_key: Sha256Digest,
    attempt: Token,
    line: String,
}

/// Serial per-child journal adapter. The binary drains stdout/stderr
/// concurrently and feeds stderr lines here immediately on its single journal
/// writer. It must retain raw bytes (including malformed UTF-8) in diagnostics;
/// only UTF-8 complete lines enter this canonical protocol parser.
pub struct ChildAttempt {
    unit: LaunchUnit,
    attempt: Token,
    pid: u32,
    tracker: ProgressTracker,
    result: Option<ChildResult>,
    exited: bool,
    validated: bool,
}
impl ChildAttempt {
    /// Records the observed PID before consuming any child progress.
    pub fn start(
        log: &mut ExecutionLog,
        unit: LaunchUnit,
        attempt: Token,
        pid: u32,
    ) -> io::Result<Self> {
        unit.validate()?;
        if pid == 0 {
            return Err(invalid("child PID is zero"));
        }
        let details = ChildSpawnEvidence {
            unit_key: unit.key.clone(),
            case_sha256: unit.case.digest(),
            attempt: attempt.clone(),
            pid,
        };
        log.append(
            JournalEvent::ChildSpawn,
            Some(serde_json::to_value(&unit.identity).map_err(wire_error)?),
            serde_json::to_value(details).map_err(wire_error)?,
        )?;
        let tracker = ProgressTracker::new(unit.identity.clone(), unit.case.digest())?;
        Ok(Self {
            unit,
            attempt,
            pid,
            tracker,
            result: None,
            exited: false,
            validated: false,
        })
    }
    /// Immediately appends/flushes valid progress or retained diagnostics.
    /// A malformed prefixed line is retained, poisons the attempt and errors.
    pub fn stderr_line(&mut self, log: &mut ExecutionLog, line: &str) -> io::Result<()> {
        if self.exited {
            return Err(invalid("stderr after child exit/drain completion"));
        }
        let parsed = ProgressRecord::parse_line(line);
        match parsed {
            Ok(Some(record)) => {
                let record = match self.tracker.accept(record) {
                    Ok(record) => record,
                    Err(error) => {
                        self.diagnostic(log, line)?;
                        return Err(error);
                    }
                };
                let event = record.journal_event();
                log.append(
                    event,
                    Some(serde_json::to_value(&self.unit.identity).map_err(wire_error)?),
                    serde_json::to_value(JournalProgress {
                        attempt: self.attempt.clone(),
                        record,
                    })
                    .map_err(wire_error)?,
                )?;
                Ok(())
            }
            Ok(None) => self.diagnostic(log, line),
            Err(error) => {
                self.tracker.poisoned = true;
                self.diagnostic(log, line)?;
                Err(error)
            }
        }
    }
    fn diagnostic(&self, log: &mut ExecutionLog, line: &str) -> io::Result<()> {
        log.append(
            JournalEvent::ChildDiagnostic,
            Some(serde_json::to_value(&self.unit.identity).map_err(wire_error)?),
            serde_json::to_value(Diagnostic {
                unit_key: self.unit.key.clone(),
                attempt: self.attempt.clone(),
                line: line.into(),
            })
            .map_err(wire_error)?,
        )?;
        Ok(())
    }
    /// Records termination after both pipes are fully drained. A timeout,
    /// nonzero exit, unreaped descendants or inconsistent result always errors.
    /// Raw stdout bytes are hashed and journaled before UTF-8/result decoding;
    /// malformed bytes remain observable exit evidence, never lossy text.
    pub fn exited(
        &mut self,
        log: &mut ExecutionLog,
        outcome: ProcessOutcome,
        stdout: &[u8],
        stderr_sha256: Sha256Digest,
    ) -> io::Result<&ChildResult> {
        if self.exited {
            return Err(invalid("duplicate child exit"));
        }
        outcome.validate()?;
        let pid = match &outcome {
            ProcessOutcome::Exited { pid, .. }
            | ProcessOutcome::Signaled { pid, .. }
            | ProcessOutcome::TimedOut { pid, .. } => *pid,
        };
        if pid != self.pid {
            return Err(invalid("exit PID differs from spawn"));
        }
        self.exited = true;
        let details = ChildExitEvidence {
            unit_key: self.unit.key.clone(),
            case_sha256: self.unit.case.digest(),
            attempt: self.attempt.clone(),
            outcome: outcome.clone(),
            stdout_sha256: Sha256Digest::of(stdout),
            stderr_sha256,
        };
        if matches!(outcome, ProcessOutcome::TimedOut { .. }) {
            log.append(
                JournalEvent::ChildTimeout,
                Some(serde_json::to_value(&self.unit.identity).map_err(wire_error)?),
                serde_json::to_value(&details).map_err(wire_error)?,
            )?;
        }
        log.append(
            JournalEvent::ChildExit,
            Some(serde_json::to_value(&self.unit.identity).map_err(wire_error)?),
            serde_json::to_value(details).map_err(wire_error)?,
        )?;
        if !outcome.accepts_result()? {
            self.tracker.poisoned = true;
            return Err(invalid("child termination is not eligible for acceptance"));
        }
        let stdout = match std::str::from_utf8(stdout) {
            Ok(stdout) => stdout,
            Err(_) => {
                self.tracker.poisoned = true;
                return Err(invalid(
                    "child stdout is not UTF-8; exact exit digest retained",
                ));
            }
        };
        self.result = Some(self.tracker.finish(stdout)?);
        self.result
            .as_ref()
            .ok_or_else(|| invalid("missing child result"))
    }
    /// Checks the owner's explicit semantic-validation response and syncs its
    /// exact case/result binding. This is required before checkpoint acceptance.
    pub fn owner_validated(
        &mut self,
        log: &mut ExecutionLog,
        response: &OwnerResponse,
    ) -> io::Result<()> {
        if self.validated {
            return Err(invalid("duplicate owner validation"));
        }
        let result = self
            .result
            .as_ref()
            .ok_or_else(|| invalid("owner validation before complete child result"))?;
        let digest = result.digest()?;
        let OwnerResponse::ValidateResult {
            unit_key,
            result_sha256,
        } = response
        else {
            return Err(invalid("owner returned wrong operation"));
        };
        if unit_key != &self.unit.key || result_sha256 != &digest {
            return Err(invalid("owner validation identity mismatch"));
        }
        let details = ResultValidationEvidence {
            unit_key: unit_key.clone(),
            case_sha256: self.unit.case.digest(),
            attempt: self.attempt.clone(),
            result_sha256: digest,
        };
        log.append(
            JournalEvent::ResultValidated,
            Some(serde_json::to_value(&self.unit.identity).map_err(wire_error)?),
            serde_json::to_value(details).map_err(wire_error)?,
        )?;
        self.validated = true;
        Ok(())
    }
}

/// Bound result stored by the generic checkpoint store. Prefix evidence is
/// validated independently on every reopen; a checkpoint never validates itself.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BoundResult {
    pub result: ChildResult,
    pub journal_prefix: JournalPrefix,
    pub attempt: Token,
    pub spawn_sequence: u64,
    pub validation_sequence: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct AcceptanceEvidence {
    unit_key: Sha256Digest,
    checkpoint_sha256: Sha256Digest,
    journal_prefix: JournalPrefix,
}

struct ReplayedAttempt {
    identity: UnitIdentity,
    key: Sha256Digest,
    case_sha256: Sha256Digest,
    attempt: Token,
    pid: u32,
    spawn_sequence: u64,
    tracker: Option<ProgressTracker>,
    timeout: Option<ChildExitEvidence>,
}

fn structured_case(record: &JournalRecord, key: &Sha256Digest) -> io::Result<UnitIdentity> {
    let value = record
        .case
        .as_ref()
        .ok_or_else(|| invalid("child evidence lacks structured case"))?;
    let identity: UnitIdentity = serde_json::from_value(value.clone()).map_err(wire_error)?;
    if identity.key()? != *key
        || identity.campaign_id.as_str() != record.campaign_id
        || serde_json::to_value(&identity).map_err(wire_error)? != *value
    {
        return Err(invalid("child evidence structured case/key mismatch"));
    }
    Ok(identity)
}
fn matches_attempt(
    record: &JournalRecord,
    key: &Sha256Digest,
    attempt: &Token,
    active: &ReplayedAttempt,
) -> io::Result<()> {
    if *key != active.key
        || *attempt != active.attempt
        || structured_case(record, key)? != active.identity
    {
        return Err(invalid("child record crosses case or attempt identity"));
    }
    Ok(())
}

fn validate_child_prefix(
    records: &[JournalRecord],
    unit: &LaunchUnit,
    result: &ChildResult,
) -> io::Result<(Token, u64, u64)> {
    unit.validate()?;
    let expected_result = result.digest()?;
    let mut running: Option<ReplayedAttempt> = None;
    let mut awaiting_validation: Option<ReplayedAttempt> = None;
    let mut attempts = BTreeSet::new();
    let mut validated = BTreeSet::new();
    let mut binding = None;
    for record in records {
        match record.event {
            JournalEvent::ChildSpawn => {
                let event: ChildSpawnEvidence =
                    serde_json::from_value(record.details.clone()).map_err(wire_error)?;
                let identity = structured_case(record, &event.unit_key)?;
                if running.is_some()
                    || awaiting_validation.is_some()
                    || validated.contains(&event.unit_key)
                    || !attempts.insert((event.unit_key.clone(), event.attempt.clone()))
                    || event.pid == 0
                {
                    return Err(invalid("overlapping, repeated, or completed child spawn"));
                }
                let tracker = if event.unit_key == unit.key {
                    if identity != unit.identity || event.case_sha256 != unit.case.digest() {
                        return Err(invalid("prefix spawn case mismatch"));
                    }
                    Some(ProgressTracker::new(
                        unit.identity.clone(),
                        unit.case.digest(),
                    )?)
                } else {
                    None
                };
                running = Some(ReplayedAttempt {
                    identity,
                    key: event.unit_key,
                    case_sha256: event.case_sha256,
                    attempt: event.attempt,
                    pid: event.pid,
                    spawn_sequence: record.sequence,
                    tracker,
                    timeout: None,
                });
            }
            JournalEvent::ExecutionProgress | JournalEvent::WindowProgress => {
                let event: JournalProgress =
                    serde_json::from_value(record.details.clone()).map_err(wire_error)?;
                let key = event.record.identity.key()?;
                let active = running
                    .as_mut()
                    .ok_or_else(|| invalid("progress outside a running child"))?;
                matches_attempt(record, &key, &event.attempt, active)?;
                if active.timeout.is_some()
                    || event.record.identity != active.identity
                    || event.record.case_sha256 != active.case_sha256
                    || record.event != event.record.journal_event()
                {
                    return Err(invalid("progress case/hash/event mismatch"));
                }
                event.record.validate()?;
                if let Some(tracker) = active.tracker.as_mut() {
                    tracker.accept(event.record)?;
                }
            }
            JournalEvent::ChildDiagnostic => {
                let event: Diagnostic =
                    serde_json::from_value(record.details.clone()).map_err(wire_error)?;
                let active = running
                    .as_mut()
                    .ok_or_else(|| invalid("diagnostic outside a running child"))?;
                matches_attempt(record, &event.unit_key, &event.attempt, active)?;
                if active.timeout.is_some() {
                    return Err(invalid("diagnostic after timeout evidence"));
                }
                if event.line.starts_with(PROGRESS_PREFIX) {
                    if let Some(tracker) = active.tracker.as_mut() {
                        tracker.poisoned = true;
                    }
                }
            }
            JournalEvent::ChildTimeout => {
                let event: ChildExitEvidence =
                    serde_json::from_value(record.details.clone()).map_err(wire_error)?;
                let active = running
                    .as_mut()
                    .ok_or_else(|| invalid("timeout outside a running child"))?;
                matches_attempt(record, &event.unit_key, &event.attempt, active)?;
                event.outcome.validate()?;
                if event.case_sha256 != active.case_sha256
                    || active.timeout.is_some()
                    || !matches!(event.outcome, ProcessOutcome::TimedOut { pid, .. } if pid == active.pid)
                {
                    return Err(invalid("timeout case/PID/outcome mismatch"));
                }
                active.timeout = Some(event);
            }
            JournalEvent::ChildExit => {
                let event: ChildExitEvidence =
                    serde_json::from_value(record.details.clone()).map_err(wire_error)?;
                let mut active = running
                    .take()
                    .ok_or_else(|| invalid("exit outside a running child"))?;
                matches_attempt(record, &event.unit_key, &event.attempt, &active)?;
                let observed_pid = match event.outcome {
                    ProcessOutcome::Exited { pid, .. }
                    | ProcessOutcome::Signaled { pid, .. }
                    | ProcessOutcome::TimedOut { pid, .. } => pid,
                };
                if event.case_sha256 != active.case_sha256
                    || observed_pid != active.pid
                    || (matches!(event.outcome, ProcessOutcome::TimedOut { .. })
                        != active.timeout.is_some())
                    || active
                        .timeout
                        .as_ref()
                        .is_some_and(|timeout| timeout != &event)
                {
                    return Err(invalid("exit case/PID/timeout mismatch"));
                }
                if event.outcome.accepts_result()? {
                    if let Some(tracker) = active.tracker.as_mut() {
                        let framed = transport::encode_result_line(result).map_err(wire_error)?;
                        if event.stdout_sha256 != Sha256Digest::of(framed.as_bytes())
                            && event.stdout_sha256
                                != Sha256Digest::of(format!("{framed}\n").as_bytes())
                        {
                            return Err(invalid("prefix stdout differs from bound result"));
                        }
                        tracker.finish(&framed)?;
                    }
                    awaiting_validation = Some(active);
                }
            }
            JournalEvent::ResultValidated => {
                let event: ResultValidationEvidence =
                    serde_json::from_value(record.details.clone()).map_err(wire_error)?;
                let active = awaiting_validation
                    .take()
                    .ok_or_else(|| invalid("validation lacks a clean completed child"))?;
                matches_attempt(record, &event.unit_key, &event.attempt, &active)?;
                if event.case_sha256 != active.case_sha256
                    || !validated.insert(event.unit_key.clone())
                {
                    return Err(invalid("validation case mismatch or duplicate"));
                }
                if event.unit_key == unit.key {
                    if event.result_sha256 != expected_result || active.tracker.is_none() {
                        return Err(invalid("validation result mismatch"));
                    }
                    binding = Some((event.attempt, active.spawn_sequence, record.sequence));
                }
            }
            JournalEvent::Interrupted => {
                // An unfinished child may restart only across the explicitly
                // closed interrupted session. A clean result awaiting owner
                // validation remains reusable evidence, never a resampling slot.
                running = None;
            }
            _ => {}
        }
    }
    binding.ok_or_else(|| invalid("bound prefix lacks complete child validation"))
}

/// Journals the stable pending-recovery identity and entries, syncing before
/// acknowledgement. Retrying after a crash deduplicates by exact batch identity.
pub fn acknowledge_pending_recovery(
    log: &mut ExecutionLog,
    store: &mut CheckpointStore,
    campaign_id: &Token,
) -> io::Result<()> {
    let Some(recovery) = store.pending_recovery().cloned() else {
        return Ok(());
    };
    let data = log.validated_synced_prefix()?;
    let records = ExecutionLog::validate_prefix(&data, campaign_id.as_str())?;
    let mut found = false;
    for record in records
        .into_iter()
        .filter(|r| r.event == JournalEvent::PendingRecovery)
    {
        let saved: PendingRecovery = serde_json::from_value(record.details).map_err(wire_error)?;
        if saved.recovery_id == recovery.recovery_id {
            if found || saved != recovery {
                return Err(invalid("conflicting/duplicate pending recovery evidence"));
            }
            found = true;
        }
    }
    if !found {
        log.append(
            JournalEvent::PendingRecovery,
            None,
            serde_json::to_value(&recovery).map_err(wire_error)?,
        )?;
    }
    log.validated_synced_prefix()?;
    store.acknowledge_pending_recovery(&recovery.recovery_id)
}

/// Enforces durable validated prefix → immutable unit → synced acceptance.
/// The result must already have its explicit owner validation in the journal.
pub fn accept_checkpoint(
    log: &mut ExecutionLog,
    store: &mut CheckpointStore,
    unit: &LaunchUnit,
    result: &ChildResult,
) -> io::Result<CompletedUnit> {
    if store.campaign_id() != unit.identity.campaign_id.as_str()
        || log.campaign_id() != unit.identity.campaign_id.as_str()
    {
        return Err(invalid("checkpoint campaign identity mismatch"));
    }
    if store.pending_recovery().is_some() {
        return Err(invalid(
            "pending recovery must be journaled and acknowledged",
        ));
    }
    let prefix = JournalPrefix::sync(log, &unit.identity.campaign_id)?;
    let records = prefix.verify(log.path(), &unit.identity.campaign_id)?;
    let (attempt, spawn_sequence, validation_sequence) =
        validate_child_prefix(&records, unit, result)?;
    let bound = BoundResult {
        result: result.clone(),
        journal_prefix: prefix.clone(),
        attempt,
        spawn_sequence,
        validation_sequence,
    };
    let completed = store.accept(unit.key.as_str(), unit, &bound)?;
    let evidence = AcceptanceEvidence {
        unit_key: unit.key.clone(),
        checkpoint_sha256: Sha256Digest::new(completed.sha256.clone())?,
        journal_prefix: prefix,
    };
    log.append(
        JournalEvent::CheckpointAccepted,
        Some(serde_json::to_value(&unit.identity).map_err(wire_error)?),
        serde_json::to_value(evidence).map_err(wire_error)?,
    )?;
    Ok(completed)
}

/// Revalidates every completed unit and its exact durable child evidence,
/// then repairs a missing acceptance event once. No missing progress is ever
/// reconstructed from checkpoint data. Returns an ordered owner input bundle.
pub fn reconcile_checkpoints(
    log: &mut ExecutionLog,
    store: &CheckpointStore,
    manifest: &OwnerManifest,
) -> io::Result<AcceptedResultsBundle> {
    reconcile_campaign_checkpoints(log, store, std::slice::from_ref(manifest))?
        .pop()
        .ok_or_else(|| invalid("single-owner reconciliation produced no bundle"))
}

/// Reconciles the exact all-owner checkpoint universe. Every acceptance event
/// must name a declared, immutable, fully validated checkpoint and carry its
/// exact structured case; unknown events are never ignored as another owner.
/// Returned bundles preserve the caller's owner and acquisition order.
pub fn reconcile_campaign_checkpoints(
    log: &mut ExecutionLog,
    store: &CheckpointStore,
    manifests: &[OwnerManifest],
) -> io::Result<Vec<AcceptedResultsBundle>> {
    if manifests.is_empty() {
        return Err(invalid("campaign reconciliation requires owner manifests"));
    }
    let campaign = &manifests[0].campaign_id;
    let mut owners = BTreeSet::new();
    let mut known = BTreeMap::new();
    for manifest in manifests {
        manifest.validate()?;
        if manifest.campaign_id != *campaign || !owners.insert(&manifest.owner) {
            return Err(invalid("campaign owner/identity mismatch"));
        }
        for unit in &manifest.ordered_units {
            if known.insert(unit.key.as_str(), unit).is_some() {
                return Err(invalid("duplicate campaign unit key"));
            }
        }
    }
    if store.campaign_id() != campaign.as_str() || log.campaign_id() != campaign.as_str() {
        return Err(invalid("checkpoint/log/manifest campaign mismatch"));
    }
    if store.pending_recovery().is_some() {
        return Err(invalid("pending recovery must be acknowledged first"));
    }
    for key in store.completed_keys() {
        if !known.contains_key(key) {
            return Err(invalid(
                "checkpoint key absent from immutable campaign manifests",
            ));
        }
    }
    let data = log.validated_synced_prefix()?;
    let records = ExecutionLog::validate_prefix(&data, campaign.as_str())?;
    let mut acceptances = BTreeMap::new();
    for record in records
        .iter()
        .filter(|record| record.event == JournalEvent::CheckpointAccepted)
    {
        let evidence: AcceptanceEvidence =
            serde_json::from_value(record.details.clone()).map_err(wire_error)?;
        let unit = known
            .get(evidence.unit_key.as_str())
            .ok_or_else(|| invalid("unknown checkpoint acceptance key"))?;
        if structured_case(record, &evidence.unit_key)? != unit.identity
            || store.completed_unit(evidence.unit_key.as_str()).is_none()
            || acceptances
                .insert(evidence.unit_key.clone(), (evidence, record.sequence))
                .is_some()
        {
            return Err(invalid(
                "acceptance case mismatch, missing checkpoint, or duplicate",
            ));
        }
    }
    let mut bundles = Vec::new();
    for manifest in manifests {
        let mut accepted = Vec::new();
        for unit in &manifest.ordered_units {
            let Some(completed) = store.completed_unit(unit.key.as_str()) else {
                continue;
            };
            let (saved, bound): (LaunchUnit, BoundResult) = store.load(unit.key.as_str())?;
            if saved != *unit {
                return Err(invalid("checkpoint changed immutable launch unit"));
            }
            let prefix_records = bound.journal_prefix.verify(log.path(), campaign)?;
            let (attempt, spawn_sequence, validation_sequence) =
                validate_child_prefix(&prefix_records, unit, &bound.result)?;
            if attempt != bound.attempt
                || spawn_sequence != bound.spawn_sequence
                || validation_sequence != bound.validation_sequence
            {
                return Err(invalid("checkpoint prefix coordinates mismatch"));
            }
            let expected = AcceptanceEvidence {
                unit_key: unit.key.clone(),
                checkpoint_sha256: Sha256Digest::new(completed.sha256.clone())?,
                journal_prefix: bound.journal_prefix.clone(),
            };
            if let Some((existing, sequence)) = acceptances.get(&unit.key) {
                if *existing != expected || *sequence < bound.journal_prefix.record_count {
                    return Err(invalid("acceptance contradicts checkpoint/prefix ordering"));
                }
            } else {
                log.append(
                    JournalEvent::CheckpointAccepted,
                    Some(serde_json::to_value(&unit.identity).map_err(wire_error)?),
                    serde_json::to_value(&expected).map_err(wire_error)?,
                )?;
            }
            accepted.push(AcceptedResult {
                unit: unit.clone(),
                result: bound.result,
                checkpoint_sha256: expected.checkpoint_sha256,
            });
        }
        bundles.push(AcceptedResultsBundle {
            schema: ACCEPTED_RESULTS_SCHEMA.into(),
            manifest_sha256: manifest.manifest_sha256.clone(),
            accepted,
        });
    }
    Ok(bundles)
}

/// Per-session checksum pins a complete terminal journal prefix. Later sessions
/// append beyond this boundary without invalidating it.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SessionChecksum {
    pub schema: String,
    pub descriptor_sha256: Sha256Digest,
    pub journal_prefix: JournalPrefix,
    pub artifacts: Vec<ArtifactIdentity>,
}
impl SessionChecksum {
    /// Validates terminal identity plus every retained artifact's exact bytes.
    pub fn validate(&self, descriptor: &SessionDescriptor, log_path: &Path) -> io::Result<()> {
        if self.schema != "tuning-campaign-session-checksum-v1"
            || self.descriptor_sha256 != descriptor.digest()?
        {
            return Err(invalid("session checksum identity mismatch"));
        }
        let records = self
            .journal_prefix
            .verify(log_path, &descriptor.campaign_id)?;
        let last = records
            .last()
            .ok_or_else(|| invalid("empty checksum prefix"))?;
        if last.session_id != descriptor.session_id.as_str()
            || !matches!(
                last.event,
                JournalEvent::Complete
                    | JournalEvent::Failed
                    | JournalEvent::Paused
                    | JournalEvent::BudgetExhausted
                    | JournalEvent::Interrupted
            )
        {
            return Err(invalid(
                "checksum prefix does not end at this session terminal",
            ));
        }
        let mut paths = BTreeSet::new();
        for artifact in &self.artifacts {
            absolute_path(&artifact.path)?;
            if !paths.insert(&artifact.path)
                || fs::canonicalize(&artifact.path)? != artifact.path
                || Sha256Digest::of(&fs::read(&artifact.path)?) != artifact.sha256
            {
                return Err(invalid("checksum artifact mismatch or duplicate"));
            }
        }
        Ok(())
    }
}

impl ManifestRequest {
    /// Validates fixed channels and distinct staged process descriptors.
    pub fn validate(&self) -> io::Result<()> {
        self.channels.validate()?;
        if self.processes.is_empty() {
            return Err(invalid("manifest request has no staged processes"));
        }
        let mut ids = BTreeSet::new();
        for process in &self.processes {
            process.validate()?;
            if !ids.insert(&process.id) {
                return Err(invalid("duplicate staged process ID"));
            }
        }
        Ok(())
    }
}
impl AcceptedResultsBundle {
    /// Checks acquisition order, identity and sample coordinates independently
    /// of the owner's mathematical payload validation. `require_complete`
    /// rejects partial inputs at emission; conditional analysis may use a prefix.
    pub fn validate(&self, manifest: &OwnerManifest, require_complete: bool) -> io::Result<()> {
        manifest.validate()?;
        if self.schema != ACCEPTED_RESULTS_SCHEMA
            || self.manifest_sha256 != manifest.manifest_sha256
            || (require_complete && self.accepted.len() != manifest.ordered_units.len())
        {
            return Err(invalid("owner input bundle schema/count/manifest mismatch"));
        }
        let mut previous = None;
        for entry in &self.accepted {
            let ordinal = usize::try_from(entry.unit.ordinal).map_err(wire_error)?;
            if manifest.ordered_units.get(ordinal) != Some(&entry.unit)
                || previous.is_some_and(|p| ordinal <= p)
            {
                return Err(invalid("owner input bundle order or unit mismatch"));
            }
            previous = Some(ordinal);
            let result = &entry.result;
            if result.schema != RESULT_SCHEMA
                || result.identity != entry.unit.identity
                || result.case_sha256 != entry.unit.case.digest()
                || result.outcome != ChildOutcome::Complete
            {
                return Err(invalid("owner input result identity mismatch"));
            }
            match result.identity.task {
                Task::Probe => {
                    if !result.samples.is_empty() {
                        return Err(invalid("probe has timing samples"));
                    }
                }
                Task::Measure { execution } => {
                    if result.samples.len() != WINDOWS as usize {
                        return Err(invalid("timed input lacks five windows"));
                    }
                    let calls = result.samples[0].calls;
                    for (repetition, sample) in result.samples.iter().enumerate() {
                        sample.validate().map_err(wire_error)?;
                        if sample.execution != execution
                            || sample.repetition != repetition as u64
                            || sample.calls != calls
                        {
                            return Err(invalid("owner input timing coordinate mismatch"));
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

impl OwnerManifest {
    /// Resolves predeclared conditional cases while preserving every reserved
    /// slot identity. The original manifest and derivation remain immutable
    /// artifacts in resume provenance; this returned projection has its own
    /// digest for owner input exports. No candidate or acquisition slot changes.
    pub fn apply_derivation(
        &self,
        request: &DeriveManifestRequest,
        derived: &DerivedManifest,
    ) -> io::Result<Self> {
        self.validate()?;
        derived.validate(request)?;
        if request.original_manifest_sha256 != self.manifest_sha256
            || request.campaign_id != self.campaign_id
        {
            return Err(invalid("derivation is not bound to this original manifest"));
        }
        let mut projected = self.clone();
        for (reserved, replacement) in request.reserved_units.iter().zip(&derived.units) {
            let index = usize::try_from(reserved.ordinal).map_err(wire_error)?;
            if self.ordered_units.get(index) != Some(reserved) {
                return Err(invalid(
                    "derivation reserved case differs from original declaration",
                ));
            }
            projected.ordered_units[index] = replacement.clone();
        }
        projected.seal()?;
        Ok(projected)
    }
}

// Each session control publication has a durable intent before its destination
// is touched. The shared atomic-file helper remains the one publication
// mechanism. Only its uncommitted recognized temporaries are cleaned, after
// preserving their exact bytes and a recovery manifest.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SessionPublicationIntent {
    schema: String,
    descriptor_sha256: Sha256Digest,
    target: String,
    payload: CanonicalJson,
    payload_sha256: Sha256Digest,
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SessionPublicationRecovery {
    schema: String,
    descriptor_sha256: Sha256Digest,
    temporary_path: PathBuf,
    temporary_sha256: Sha256Digest,
    diagnostic_path: PathBuf,
    target_present: bool,
}
fn publication_root(root: &Path) -> PathBuf {
    root.join("publication")
}
fn intent_path(root: &Path) -> PathBuf {
    publication_root(root).join("intent.json")
}
fn publication_target(stage: &Path, root: &Path, name: &str) -> io::Result<PathBuf> {
    if name == "active-session.json" {
        return Ok(stage.join(name));
    }
    if matches!(
        name,
        "descriptor.json"
            | "prepare-session.json"
            | "run-session.json"
            | "finalize-session.json"
            | "retired.json"
    ) || (name.starts_with("transition-")
        && name.ends_with(".json")
        && name.len() == "transition-000000.json".len()
        && name[11..17].bytes().all(|b| b.is_ascii_digit()))
    {
        return Ok(root.join(name));
    }
    Err(invalid("unknown session publication target"))
}
fn validate_publication_payload(
    intent: &SessionPublicationIntent,
    expected: &SessionDescriptor,
) -> io::Result<()> {
    if intent.schema != "tuning-campaign-session-publication-v1"
        || intent.descriptor_sha256 != expected.digest()?
        || intent.payload_sha256 != intent.payload.digest()
    {
        return Err(invalid(
            "session publication intent identity/digest mismatch",
        ));
    }
    match intent.target.as_str() {
        "descriptor.json" | "active-session.json" => {
            let descriptor: SessionDescriptor = intent.payload.decode()?;
            if &descriptor != expected {
                return Err(invalid("session publication descriptor changed"));
            }
        }
        "prepare-session.json" | "run-session.json" | "finalize-session.json" => {
            let claim: ModeClaim = intent.payload.decode()?;
            let mode = match intent.target.as_str() {
                "prepare-session.json" => SessionMode::PrepareSession,
                "run-session.json" => SessionMode::RunSession,
                _ => SessionMode::FinalizeSession,
            };
            if claim.schema != LIFECYCLE_SCHEMA
                || claim.descriptor_sha256 != expected.digest()?
                || claim.mode != mode
            {
                return Err(invalid("session publication mode changed"));
            }
        }
        "retired.json" => {
            let retired: RetirementRecord = intent.payload.decode()?;
            if retired.descriptor_sha256 != expected.digest()? {
                return Err(invalid("retirement publication identity mismatch"));
            }
        }
        _ => {
            let record: TransitionRecord = intent.payload.decode()?;
            if record.descriptor_sha256 != expected.digest()?
                || intent.target != format!("transition-{:06}.json", record.sequence)
            {
                return Err(invalid("session publication transition changed"));
            }
        }
    }
    Ok(())
}
fn recognized_temporary(name: &str, target: &str) -> bool {
    let Some(suffix) = name.strip_prefix(&format!(".{target}.tmp-")) else {
        return false;
    };
    let parts: Vec<_> = suffix.split('-').collect();
    if parts.len() != 3
        || parts.iter().any(|part| {
            part.is_empty()
                || !part.bytes().all(|b| b.is_ascii_digit())
                || (*part != "0" && part.starts_with('0'))
        })
    {
        return false;
    }
    parts[0].parse::<u32>().is_ok_and(|pid| pid > 0)
        && parts[1].parse::<u128>().is_ok()
        && parts[2].parse::<u8>().is_ok_and(|attempt| attempt < 128)
}

fn publish_same(path: &Path, content: &[u8]) -> io::Result<()> {
    match atomic_write_new(path, content) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            if fs::read(path)? == content {
                File::open(path)?.sync_all()?;
                File::open(
                    path.parent()
                        .ok_or_else(|| invalid("artifact lacks parent"))?,
                )?
                .sync_all()?;
                Ok(())
            } else {
                Err(invalid(
                    "existing immutable session artifact has different bytes",
                ))
            }
        }
        Err(error) => Err(error),
    }
}
fn preserve_publication_temporary(
    root: &Path,
    expected: &SessionDescriptor,
    temporary: &Path,
    target_present: bool,
) -> io::Result<()> {
    let content = fs::read(temporary)?;
    let hash = Sha256Digest::of(&content);
    let recoveries = publication_root(root).join("recoveries");
    fs::create_dir_all(&recoveries)?;
    File::open(publication_root(root))?.sync_all()?;
    let diagnostic = recoveries.join(format!("{}.bytes", hash.as_str()));
    // A diagnostic publication can itself be interrupted. Its byte identity is
    // known here, so an exact/prefix temporary is safe to discard and republish.
    clean_known_temporary(
        &recoveries,
        diagnostic
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| invalid("diagnostic filename"))?,
        &content,
    )?;
    publish_same(&diagnostic, &content)?;
    let recovery = SessionPublicationRecovery {
        schema: "tuning-campaign-session-publication-recovery-v1".into(),
        descriptor_sha256: expected.digest()?,
        temporary_path: temporary.into(),
        temporary_sha256: hash,
        diagnostic_path: diagnostic,
        target_present,
    };
    let encoded = bytes(&recovery)?;
    let record_name = format!("{}.json", Sha256Digest::of(&encoded).as_str());
    clean_known_temporary(&recoveries, &record_name, &encoded)?;
    publish_same(&recoveries.join(record_name), &encoded)?;
    fs::remove_file(temporary)?;
    File::open(
        temporary
            .parent()
            .ok_or_else(|| invalid("temporary lacks parent"))?,
    )?
    .sync_all()
}
fn clean_known_temporary(parent: &Path, target: &str, expected: &[u8]) -> io::Result<()> {
    for entry in fs::read_dir(parent)? {
        let entry = entry?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| invalid("non-UTF-8 temporary filename"))?;
        if !recognized_temporary(&name, target) {
            continue;
        }
        if !entry.file_type()?.is_file() || !expected.starts_with(&fs::read(entry.path())?) {
            return Err(invalid("ambiguous/tampered diagnostic temporary preserved"));
        }
        fs::remove_file(entry.path())?;
    }
    File::open(parent)?.sync_all()
}
fn recover_session_publication(
    stage: &Path,
    root: &Path,
    expected: &SessionDescriptor,
) -> io::Result<()> {
    let publications = publication_root(root);
    fs::create_dir_all(&publications)?;
    File::open(root)?.sync_all()?;
    let intent_file = intent_path(root);
    if intent_file.try_exists()? {
        let intent: SessionPublicationIntent = decode(&fs::read(&intent_file)?)?;
        validate_publication_payload(&intent, expected)?;
    }
    // No destination can be touched before intent.json is fully published.
    // Thus an uncommitted intent temporary is safely abandoned with exact
    // diagnostic bytes, even if its partial JSON cannot yet be decoded.
    for entry in fs::read_dir(&publications)? {
        let entry = entry?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| invalid("non-UTF-8 publication filename"))?;
        if recognized_temporary(&name, "intent.json") {
            if !entry.file_type()?.is_file() {
                return Err(invalid("publication temporary is not a regular file"));
            }
            if intent_file.try_exists()?
                && !fs::read(&intent_file)?.starts_with(&fs::read(entry.path())?)
            {
                return Err(invalid("ambiguous/tampered intent temporary preserved"));
            }
            preserve_publication_temporary(
                root,
                expected,
                &entry.path(),
                intent_file.try_exists()?,
            )?;
        } else if name != "intent.json" && name != "recoveries" {
            return Err(invalid("unknown session publication entry preserved"));
        }
    }
    if !intent_file.try_exists()? {
        return Ok(());
    }
    let intent: SessionPublicationIntent = decode(&fs::read(&intent_file)?)?;
    validate_publication_payload(&intent, expected)?;
    let target = publication_target(stage, root, &intent.target)?;
    let target_exists = target.try_exists()?;
    if target_exists && fs::read(&target)? != intent.payload.as_str().as_bytes() {
        return Err(invalid(
            "published session destination contradicts durable intent",
        ));
    }
    let parent = target
        .parent()
        .ok_or_else(|| invalid("publication target lacks parent"))?;
    for entry in fs::read_dir(parent)? {
        let entry = entry?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| invalid("non-UTF-8 publication target temporary"))?;
        if !recognized_temporary(&name, &intent.target) {
            continue;
        }
        let content = fs::read(entry.path())?;
        if !entry.file_type()?.is_file()
            || !intent.payload.as_str().as_bytes().starts_with(&content)
        {
            return Err(invalid("ambiguous/tampered target temporary preserved"));
        }
        preserve_publication_temporary(root, expected, &entry.path(), target_exists)?;
    }
    publish_same(&target, intent.payload.as_str().as_bytes())?;
    fs::remove_file(&intent_file)?;
    File::open(&publications)?.sync_all()
}
fn publish_session_control(
    stage: &Path,
    root: &Path,
    expected: &SessionDescriptor,
    name: &str,
    content: &[u8],
) -> io::Result<()> {
    recover_session_publication(stage, root, expected)?;
    let target = publication_target(stage, root, name)?;
    if target.try_exists()? {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "session descriptor already consumed/published",
        ));
    }
    let payload = CanonicalJson::new(String::from_utf8(content.to_vec()).map_err(wire_error)?)?;
    let intent = SessionPublicationIntent {
        schema: "tuning-campaign-session-publication-v1".into(),
        descriptor_sha256: expected.digest()?,
        target: name.into(),
        payload_sha256: payload.digest(),
        payload,
    };
    validate_publication_payload(&intent, expected)?;
    atomic_write_new(&intent_path(root), &bytes(&intent)?)?;
    recover_session_publication(stage, root, expected)
}
fn session_publication_recoveries(
    root: &Path,
    expected: &SessionDescriptor,
) -> io::Result<Vec<SessionPublicationRecovery>> {
    let recoveries = publication_root(root).join("recoveries");
    if !recoveries.try_exists()? {
        return Ok(Vec::new());
    }
    let mut result = Vec::new();
    let mut temporary_paths = BTreeSet::new();
    for entry in fs::read_dir(&recoveries)? {
        let entry = entry?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| invalid("non-UTF-8 recovery filename"))?;
        if !entry.file_type()?.is_file() {
            return Err(invalid("session recovery artifact is not a regular file"));
        }
        if let Some(stem) = name.strip_suffix(".bytes") {
            let digest = Sha256Digest::new(stem)?;
            if Sha256Digest::of(&fs::read(entry.path())?) != digest {
                return Err(invalid("session recovery diagnostic digest mismatch"));
            }
            continue;
        }
        let encoded = fs::read(entry.path())?;
        let recovery: SessionPublicationRecovery = decode(&encoded)?;
        if name != format!("{}.json", Sha256Digest::of(&encoded).as_str())
            || recovery.schema != "tuning-campaign-session-publication-recovery-v1"
            || recovery.descriptor_sha256 != expected.digest()?
            || recovery.diagnostic_path
                != recoveries.join(format!("{}.bytes", recovery.temporary_sha256.as_str()))
            || Sha256Digest::of(&fs::read(&recovery.diagnostic_path)?) != recovery.temporary_sha256
        {
            return Err(invalid(
                "session publication recovery identity/digest mismatch",
            ));
        }
        if !temporary_paths.insert(recovery.temporary_path.clone()) {
            return Err(invalid("duplicate session recovery temporary identity"));
        }
        result.push(recovery);
    }
    result.sort_by(|a, b| a.temporary_path.cmp(&b.temporary_path));
    Ok(result)
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RetirementRecord {
    descriptor_sha256: Sha256Digest,
    checksum: ArtifactIdentity,
}

#[cfg(test)]
mod publication_tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct Scratch(PathBuf);
    impl Scratch {
        fn new() -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "session-publication-{}-{nonce}",
                std::process::id()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn descriptor(stage: &Path) -> SessionDescriptor {
        let hash = Sha256Digest::of(b"fixed").as_str().to_string();
        SessionDescriptor {
            schema: LIFECYCLE_SCHEMA.into(),
            campaign_id: Token::new("campaign").unwrap(),
            session_id: Token::new("session").unwrap(),
            channels: SessionChannels::for_stage(stage).unwrap(),
            identity: ResumeIdentity {
                protocol_digest: hash.clone(),
                source_revision: "a".repeat(40),
                source_sha256: hash.clone(),
                ordered_work_manifest_sha256: hash.clone(),
                process_descriptors_sha256: hash.clone(),
                executable_sha256: BTreeMap::from([("producer".into(), hash.clone())]),
                behavior_sha256: BTreeMap::from([("producer".into(), hash.clone())]),
                lifecycle_schema: LIFECYCLE_SCHEMA.into(),
                lifecycle_behavior_sha256: hash,
                feature_contract: FEATURE_CONTRACT.into(),
                thread_contract: THREAD_CONTRACT.into(),
                host_identity: "host".into(),
            },
            counts: DeclaredCounts::for_cells(1).unwrap(),
            lock_path: "/tmp/campaign.lock".into(),
        }
    }
    fn intent(
        expected: &SessionDescriptor,
        target: &str,
        payload: &impl Serialize,
    ) -> SessionPublicationIntent {
        let payload = CanonicalJson::from_serializable(payload).unwrap();
        SessionPublicationIntent {
            schema: "tuning-campaign-session-publication-v1".into(),
            descriptor_sha256: expected.digest().unwrap(),
            target: target.into(),
            payload_sha256: payload.digest(),
            payload,
        }
    }
    fn root(expected: &SessionDescriptor) -> PathBuf {
        expected
            .channels
            .stage
            .join("sessions")
            .join(expected.session_id.as_str())
    }
    fn mode_intent(expected: &SessionDescriptor) -> SessionPublicationIntent {
        intent(
            expected,
            "run-session.json",
            &ModeClaim {
                schema: LIFECYCLE_SCHEMA.into(),
                descriptor_sha256: expected.digest().unwrap(),
                mode: SessionMode::RunSession,
            },
        )
    }

    #[test]
    fn preparation_recovers_every_uncommitted_initial_intent_boundary() {
        for complete_temporary in [false, true] {
            let tmp = Scratch::new();
            let expected = descriptor(&tmp.0);
            let root = root(&expected);
            fs::create_dir_all(publication_root(&root)).unwrap();
            let pending = publication_root(&root).join(".intent.json.tmp-1-2-3");
            let intent = intent(&expected, "descriptor.json", &expected);
            let content = if complete_temporary {
                bytes(&intent).unwrap()
            } else {
                b"{\"schema\":".to_vec()
            };
            fs::write(&pending, &content).unwrap();
            let store = SessionStore::reopen(expected.clone()).unwrap();
            assert_eq!(store.lifecycle.state(), SessionState::Prepared);
            assert!(!pending.exists());
            assert_eq!(
                fs::read(tmp.0.join("active-session.json")).unwrap(),
                bytes(&expected).unwrap()
            );
            let recovery = session_publication_recoveries(&root, &expected).unwrap();
            assert_eq!(recovery.len(), 1);
            assert_eq!(fs::read(&recovery[0].diagnostic_path).unwrap(), content);
            drop(store);
            let store = SessionStore::reopen(expected.clone()).unwrap();
            assert_eq!(
                session_publication_recoveries(&root, &expected)
                    .unwrap()
                    .len(),
                1
            );
            drop(store);
            assert!(SessionStore::prepare(expected).is_err());
        }
    }

    #[test]
    fn mode_publication_replays_intent_sync_temporary_sync_and_destination_link() {
        for boundary in 0..4 {
            let tmp = Scratch::new();
            let expected = descriptor(&tmp.0);
            drop(SessionStore::prepare(expected.clone()).unwrap());
            let root = root(&expected);
            let intent = mode_intent(&expected);
            fs::write(intent_path(&root), bytes(&intent).unwrap()).unwrap();
            let pending = root.join(".run-session.json.tmp-1-2-3");
            match boundary {
                0 => {}
                1 => {
                    fs::write(&pending, &intent.payload.as_str().as_bytes()[..8]).unwrap();
                }
                2 => {
                    fs::write(&pending, intent.payload.as_str()).unwrap();
                }
                _ => {
                    fs::write(&pending, intent.payload.as_str()).unwrap();
                    fs::hard_link(&pending, root.join("run-session.json")).unwrap();
                }
            }
            let mut store = SessionStore::reopen(expected.clone()).unwrap();
            assert_eq!(
                fs::read(root.join("run-session.json")).unwrap(),
                intent.payload.as_str().as_bytes()
            );
            assert!(!intent_path(&root).exists());
            assert!(!pending.exists());
            assert!(store.consume_mode(SessionMode::RunSession).is_err());
            drop(store);
            assert!(SessionStore::reopen(expected)
                .unwrap()
                .consume_mode(SessionMode::RunSession)
                .is_err());
        }
    }

    #[test]
    fn uncommitted_mode_intent_never_claims_a_launch_occurred() {
        let tmp = Scratch::new();
        let expected = descriptor(&tmp.0);
        drop(SessionStore::prepare(expected.clone()).unwrap());
        let root = root(&expected);
        fs::write(
            publication_root(&root).join(".intent.json.tmp-1-2-3"),
            bytes(&mode_intent(&expected)).unwrap(),
        )
        .unwrap();
        let mut store = SessionStore::reopen(expected).unwrap();
        assert!(!root.join("run-session.json").exists());
        store.consume_mode(SessionMode::RunSession).unwrap();
        assert!(store.consume_mode(SessionMode::RunSession).is_err());
    }

    #[test]
    fn tampered_or_cross_identity_publication_preserves_all_original_evidence() {
        for wrong_identity in [false, true] {
            let tmp = Scratch::new();
            let expected = descriptor(&tmp.0);
            drop(SessionStore::prepare(expected.clone()).unwrap());
            let root = root(&expected);
            let mut publication = mode_intent(&expected);
            if wrong_identity {
                publication.descriptor_sha256 = Sha256Digest::of(b"other");
            }
            let encoded = bytes(&publication).unwrap();
            fs::write(intent_path(&root), &encoded).unwrap();
            let pending = root.join(".run-session.json.tmp-1-2-3");
            fs::write(&pending, b"not-a-prefix").unwrap();
            assert!(SessionStore::reopen(expected).is_err());
            assert_eq!(fs::read(intent_path(&root)).unwrap(), encoded);
            assert_eq!(fs::read(&pending).unwrap(), b"not-a-prefix");
            assert!(!root.join("run-session.json").exists());
        }
    }

    #[test]
    fn synced_transition_intent_recovers_observation_and_journals_it_once() {
        let tmp = Scratch::new();
        let expected = descriptor(&tmp.0);
        let root = root(&expected);
        let mut log = ExecutionLog::create_new(&tmp.0, "campaign", "session").unwrap();
        log.append(JournalEvent::CampaignStart, None, serde_json::json!({}))
            .unwrap();
        let mut store = SessionStore::prepare(expected.clone()).unwrap();
        store.consume_mode(SessionMode::RunSession).unwrap();
        store
            .transition(
                &mut log,
                SessionTransition::LockHeld {
                    evidence: LockEvidence {
                        lock_path: expected.lock_path.clone(),
                        holder_pid: 123,
                        observation: Token::new("inherited").unwrap(),
                    },
                },
            )
            .unwrap();
        drop(store);
        let record = TransitionRecord {
            descriptor_sha256: expected.digest().unwrap(),
            sequence: 1,
            transition: SessionTransition::WorkFinished {
                evidence: WorkEvidence {
                    outcome: SessionOutcome::Paused,
                    all_descendants_reaped: true,
                    active_elapsed_ns: 1,
                },
            },
            recovery: false,
        };
        let publication = intent(&expected, "transition-000001.json", &record);
        fs::write(intent_path(&root), bytes(&publication).unwrap()).unwrap();
        fs::write(
            root.join(".transition-000001.json.tmp-1-2-3"),
            publication.payload.as_str(),
        )
        .unwrap();
        let store = SessionStore::reopen(expected).unwrap();
        assert_eq!(store.lifecycle.state(), SessionState::WorkFinished);
        store.reconcile_journal(&mut log).unwrap();
        let sequence = log.next_sequence();
        store.reconcile_journal(&mut log).unwrap();
        assert_eq!(sequence, log.next_sequence());
        let records =
            ExecutionLog::validate_prefix(&fs::read(log.path()).unwrap(), "campaign").unwrap();
        assert_eq!(
            records
                .iter()
                .filter(|r| r.event == JournalEvent::WorkFinished)
                .count(),
            1
        );
        assert_eq!(
            records
                .iter()
                .filter(|r| r.event == JournalEvent::SessionRecovery)
                .count(),
            1
        );
    }

    #[test]
    fn preparation_replays_descriptor_prepare_and_active_claim_gaps() {
        for boundary in 0..3 {
            let tmp = Scratch::new();
            let expected = descriptor(&tmp.0);
            let root = root(&expected);
            fs::create_dir_all(publication_root(&root)).unwrap();
            fs::write(root.join("descriptor.json"), bytes(&expected).unwrap()).unwrap();
            if boundary >= 1 {
                let claim = ModeClaim {
                    schema: LIFECYCLE_SCHEMA.into(),
                    descriptor_sha256: expected.digest().unwrap(),
                    mode: SessionMode::PrepareSession,
                };
                fs::write(root.join("prepare-session.json"), bytes(&claim).unwrap()).unwrap();
            }
            if boundary == 2 {
                let publication = intent(&expected, "active-session.json", &expected);
                fs::write(intent_path(&root), bytes(&publication).unwrap()).unwrap();
                let pending = tmp.0.join(".active-session.json.tmp-1-2-3");
                fs::write(&pending, bytes(&expected).unwrap()).unwrap();
                fs::hard_link(&pending, tmp.0.join("active-session.json")).unwrap();
            }
            let store = SessionStore::reopen(expected.clone()).unwrap();
            assert_eq!(store.lifecycle.state(), SessionState::Prepared);
            drop(store);
            assert_eq!(
                fs::read(tmp.0.join("active-session.json")).unwrap(),
                bytes(&expected).unwrap()
            );
            assert!(root.join("prepare-session.json").is_file());
        }
    }

    #[test]
    fn resume_identity_precedes_temporary_cleanup_and_namespace_is_exact() {
        let tmp = Scratch::new();
        let expected = descriptor(&tmp.0);
        drop(SessionStore::prepare(expected.clone()).unwrap());
        let root = root(&expected);
        let pending = publication_root(&root).join(".intent.json.tmp-1-2-3");
        fs::write(&pending, b"partial").unwrap();
        let mut changed = expected.clone();
        changed.identity.host_identity = "changed-host".into();
        assert!(SessionStore::reopen(changed).is_err());
        assert_eq!(fs::read(&pending).unwrap(), b"partial");
        fs::remove_file(&pending).unwrap();
        let unknown = publication_root(&root).join(".intent.json.tmp-01-2-3");
        fs::write(&unknown, b"unknown").unwrap();
        assert!(SessionStore::reopen(expected).is_err());
        assert_eq!(fs::read(&unknown).unwrap(), b"unknown");
    }

    #[test]
    fn recovered_interruption_journals_cleanup_after_release_without_inventing_work() {
        let tmp = Scratch::new();
        let expected = descriptor(&tmp.0);
        let root = root(&expected);
        let mut log = ExecutionLog::create_new(&tmp.0, "campaign", "session").unwrap();
        log.append(JournalEvent::CampaignStart, None, serde_json::json!({}))
            .unwrap();
        let mut store = SessionStore::prepare(expected.clone()).unwrap();
        store.consume_mode(SessionMode::RunSession).unwrap();
        store
            .transition(
                &mut log,
                SessionTransition::LockHeld {
                    evidence: LockEvidence {
                        lock_path: expected.lock_path.clone(),
                        holder_pid: 123,
                        observation: Token::new("inherited").unwrap(),
                    },
                },
            )
            .unwrap();
        store
            .recover_transition(
                &mut log,
                SessionTransition::LockRelease {
                    evidence: ReleaseEvidence::Independent {
                        evidence: IndependentReleaseEvidence {
                            holder_dead: true,
                            descendants_dead: true,
                            lock_path: expected.lock_path.clone(),
                            observed_utc: "2026-09-05T00:00:00Z".into(),
                            lock_available: true,
                        },
                    },
                },
            )
            .unwrap();
        drop(store);
        let record = TransitionRecord {
            descriptor_sha256: expected.digest().unwrap(),
            sequence: 2,
            transition: SessionTransition::Interrupted {
                active_elapsed_censored: true,
            },
            recovery: true,
        };
        let publication = intent(&expected, "transition-000002.json", &record);
        fs::write(intent_path(&root), bytes(&publication).unwrap()).unwrap();
        fs::write(
            root.join(".transition-000002.json.tmp-1-2-3"),
            publication.payload.as_str(),
        )
        .unwrap();
        let store = SessionStore::reopen(expected).unwrap();
        store.reconcile_journal(&mut log).unwrap();
        let count = log.next_sequence();
        store.reconcile_journal(&mut log).unwrap();
        assert_eq!(log.next_sequence(), count);
        let records =
            ExecutionLog::validate_prefix(&fs::read(log.path()).unwrap(), "campaign").unwrap();
        assert_eq!(records.last().unwrap().event, JournalEvent::Interrupted);
        assert_eq!(
            records
                .iter()
                .filter(|r| r.event == JournalEvent::SessionRecovery)
                .count(),
            1
        );
        assert!(records
            .iter()
            .all(|r| r.event != JournalEvent::WorkFinished
                && r.event != JournalEvent::WrapperReturned));
    }
}
