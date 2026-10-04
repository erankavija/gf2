//! Neutral, resumable owner-campaign process driver. The launcher builds and
//! stages every executable before preparation. Each handoff names one immutable
//! session; only run-session executes measurement or composition, under the
//! inherited full-host flock. Finalization observes wrapper return and release
//! before publishing terminal evidence. After a `complete` terminal,
//! `publish-campaign` copies the validated stage to its repository
//! destinations.
#![deny(unsafe_code)]
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::env;
use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};
use tuning_campaign_support::campaign::*;
use tuning_campaign_support::host::{
    command_text, inherited_lock, lock_available, require_affinity, CpuAffinity, HostObservation,
};
use tuning_campaign_support::journal::{
    CheckpointStore, ExecutionLog, JournalEvent, ResumeIdentity,
};
use tuning_campaign_support::process::{live_group, ProcessResult};
use tuning_campaign_support::provenance::ProducingInputs;
use tuning_campaign_support::transport;

static SESSION_START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
static ALL_REAPED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);
fn check_budget() -> io::Result<()> {
    if SESSION_START
        .get()
        .is_some_and(|start| !may_launch_child(ns(*start)))
    {
        return Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "session-budget-exhausted",
        ));
    }
    Ok(())
}
fn run_process(
    command: Command,
    input: &[u8],
    timeout: Duration,
    grace: Duration,
    started: impl FnMut(u32) -> io::Result<()>,
    stderr_callback: impl FnMut(&[u8]) -> io::Result<()>,
) -> io::Result<ProcessResult> {
    tuning_campaign_support::process::run_process(
        command,
        input,
        timeout,
        grace,
        check_budget,
        &ALL_REAPED,
        started,
        stderr_callback,
    )
}
fn invalid(message: impl ToString) -> io::Error {
    io::Error::other(message.to_string())
}
fn encoded<T: Serialize>(value: &T) -> io::Result<Vec<u8>> {
    serde_json::to_vec(value).map_err(invalid)
}
fn read_json<T: serde::de::DeserializeOwned + Serialize>(path: &Path) -> io::Result<T> {
    transport::decode_case(&fs::read_to_string(path)?).map_err(invalid)
}
fn artifact_stage(path: &Path) -> io::Result<&Path> {
    let parent = path
        .parent()
        .ok_or_else(|| invalid("artifact has no parent"))?;
    Ok(parent
        .ancestors()
        .find(|ancestor| ancestor.join("execution.log").is_file())
        .unwrap_or(parent))
}
fn save<T: Serialize>(path: &Path, value: &T) -> io::Result<ArtifactIdentity> {
    publish_artifact(artifact_stage(path)?, path, &encoded(value)?)
}
fn artifact(path: &Path) -> io::Result<ArtifactIdentity> {
    Ok(ArtifactIdentity {
        path: fs::canonicalize(path)?,
        sha256: Sha256Digest::of(&fs::read(path)?),
    })
}
fn utc() -> io::Result<String> {
    command_text("date", &["-u", "+%Y-%m-%dT%H:%M:%SZ"])
}
fn ns(start: Instant) -> u64 {
    u64::try_from(start.elapsed().as_nanos())
        .unwrap_or(u64::MAX)
        .max(1)
}

fn record_budget(
    log: &mut ExecutionLog,
    boundary: &str,
    process: &str,
    unit_key: Option<&Sha256Digest>,
) -> io::Result<()> {
    let active_elapsed_ns = SESSION_START.get().map_or(0, |start| ns(*start));
    let may_launch = SESSION_START
        .get()
        .is_none_or(|_| may_launch_child(active_elapsed_ns));
    log.append(
        JournalEvent::DriverDiagnostic,
        None,
        json!({
            "kind":"session-budget-observation",
            "boundary":boundary,
            "process":process,
            "unit_key":unit_key,
            "active_elapsed_ns":active_elapsed_ns,
            "may_launch_child":may_launch,
            "session_budget_seconds":SESSION_BUDGET_SECONDS,
            "child_timeout_seconds":CHILD_TIMEOUT_SECONDS,
            "child_kill_grace_seconds":CHILD_KILL_GRACE_SECONDS,
        }),
    )?;
    if boundary == "before-launch" && !may_launch {
        return Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "session-budget-exhausted",
        ));
    }
    Ok(())
}

/// What admits a session to the host.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum HostAdmission {
    /// The single `ccx1-bench-flock.sh --full-host` outer lock.
    FullHostOuterLock,
}
/// The role of the journaled held-lock `HostObservation`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum HostObservationRole {
    /// Retained data that gates neither admission nor resume.
    Descriptive,
}
/// Protocol §7 host admission. Driver and validator both require this exact
/// declaration.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct HostAdmissionPolicy {
    admission: HostAdmission,
    host_observation: HostObservationRole,
    /// The serialized `HostObservation` fields, sorted.
    recorded_observations: Vec<String>,
}
impl HostAdmissionPolicy {
    fn declared() -> Self {
        Self {
            admission: HostAdmission::FullHostOuterLock,
            host_observation: HostObservationRole::Descriptive,
            recorded_observations: [
                "affinity",
                "available_memory_kib",
                "cpu_flags",
                "cpu_model",
                "governors",
                "hostname",
                "load_average",
                "observed_utc",
                "os_kernel",
                "smt_active",
                "topology",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        }
    }
}
/// Schema of a committed campaign declaration.
const DECLARATION_SCHEMA: &str = "tuning-campaign-declaration-v1";
/// The two owners a complete envelope composes, by declaration name.
const OWNER_NAMES: [(&str, &str); 2] = [("core", "gf2-core"), ("algebra", "gf2-algebra")];

/// A repository-relative file pinned by its SHA-256.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RepositoryArtifact {
    path: String,
    sha256: Sha256Digest,
}
impl RepositoryArtifact {
    /// Reads the file below `root` and requires its declared digest.
    fn read(&self, root: &Path) -> io::Result<Vec<u8>> {
        repository_relative(&self.path)?;
        let bytes = fs::read(root.join(&self.path))?;
        if Sha256Digest::of(&bytes) != self.sha256 {
            return Err(invalid(format!(
                "{} differs from its declared SHA-256",
                self.path
            )));
        }
        Ok(bytes)
    }
}
/// Rejects an absolute, traversing or unnormalized repository path.
fn repository_relative(path: &str) -> io::Result<()> {
    let candidate = Path::new(path);
    if path.is_empty()
        || !candidate
            .components()
            .all(|part| matches!(part, std::path::Component::Normal(_)))
        || candidate.components().collect::<PathBuf>().as_os_str() != candidate.as_os_str()
    {
        return Err(invalid("declared path must be repository-relative"));
    }
    Ok(())
}
/// An owner the campaign measures, with its declared cell count.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct MeasuredOwner {
    owner: Token,
    name: Token,
    cells: u64,
}
/// An owner the campaign takes from a committed envelope without measuring
/// it. `complete` is the committed complete envelope whose `section` wrapper
/// the campaign's complete envelope must reproduce byte for byte.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ImportedOwner {
    owner: Token,
    name: Token,
    section: String,
    envelope: RepositoryArtifact,
    complete: RepositoryArtifact,
}
/// The committed declaration of one campaign: its issue, which also forms the
/// run-ID prefix `gf2-<issue>-`, protocol, producing-input manifest, measured
/// owners with their cell counts, imported owners, and whether publication
/// emits an evidence index. The launcher, this driver and the independent
/// validator locate it with [`locate_campaign_declaration`] and identify the
/// recorded declaration by its digest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CampaignDeclaration {
    schema: String,
    issue: String,
    protocol: String,
    producing_manifest: String,
    measured_owners: Vec<MeasuredOwner>,
    imported_owners: Vec<ImportedOwner>,
    evidence_index: bool,
    /// SHA-256 of the located declaration file.
    #[serde(skip)]
    sha256: Option<Sha256Digest>,
}
impl CampaignDeclaration {
    /// The issue named by `gf2-<issue>-...`; eight lowercase hex digits.
    fn issue_of(campaign_id: &str) -> io::Result<&str> {
        campaign_id
            .strip_prefix("gf2-")
            .and_then(|rest| rest.split_once('-'))
            .map(|(issue, _)| issue)
            .filter(|issue| {
                issue.len() == 8
                    && issue
                        .bytes()
                        .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
            })
            .ok_or_else(|| invalid("campaign ID lacks a gf2-<issue>- prefix"))
    }
    /// Locates, reads and validates the declaration that `campaign_id` names
    /// below `root`.
    fn for_campaign(root: &Path, campaign_id: &str) -> io::Result<Self> {
        let issue = Self::issue_of(campaign_id)?;
        let bytes = fs::read(root.join(locate_campaign_declaration(root, issue)?))?;
        let mut declaration: Self = serde_json::from_slice(&bytes).map_err(invalid)?;
        declaration.validate(issue)?;
        declaration.sha256 = Some(Sha256Digest::of(&bytes));
        Ok(declaration)
    }
    /// The one `(path, digest)` entry of `identities` whose digest is this
    /// declaration's, which names the declaration as the campaign recorded it.
    fn recorded_in<'a>(
        &self,
        identities: &'a BTreeMap<String, String>,
    ) -> io::Result<(&'a str, &'a str)> {
        let sha256 = self
            .sha256
            .as_ref()
            .ok_or_else(|| invalid("campaign declaration was not located"))?;
        let mut entries = identities
            .iter()
            .filter(|(_, digest)| digest.as_str() == sha256.as_str());
        match (entries.next(), entries.next()) {
            (Some((path, digest)), None) => Ok((path, digest)),
            _ => Err(invalid(
                "behavior identity does not name the campaign declaration exactly once",
            )),
        }
    }
    fn prefix(&self) -> String {
        format!("gf2-{}-", self.issue)
    }
    /// Requires the schema and issue, normalized paths, the core owner
    /// measured first, and every complete-envelope owner exactly once as
    /// measured or imported.
    fn validate(&self, issue: &str) -> io::Result<()> {
        if self.schema != DECLARATION_SCHEMA || self.issue != issue {
            return Err(invalid("campaign declaration schema/issue mismatch"));
        }
        repository_relative(&self.protocol)?;
        repository_relative(&self.producing_manifest)?;
        let mut names: Vec<(&str, &str)> = self
            .measured_owners
            .iter()
            .map(|owner| (owner.name.as_str(), owner.owner.as_str()))
            .chain(
                self.imported_owners
                    .iter()
                    .map(|owner| (owner.name.as_str(), owner.owner.as_str())),
            )
            .collect();
        names.sort_unstable();
        let mut expected = OWNER_NAMES.to_vec();
        expected.sort_unstable();
        if names != expected
            || self
                .measured_owners
                .first()
                .map(|owner| owner.name.as_str())
                != Some("core")
            || self.measured_owners.iter().any(|owner| owner.cells == 0)
        {
            return Err(invalid(
                "campaign declaration must measure core first and cover each owner once",
            ));
        }
        for imported in &self.imported_owners {
            repository_relative(&imported.envelope.path)?;
            repository_relative(&imported.complete.path)?;
            if imported.section != format!("{}/permanent", imported.owner.as_str()) {
                return Err(invalid("imported owner section mismatch"));
            }
        }
        Ok(())
    }
    fn total_cells(&self) -> u64 {
        self.measured_owners.iter().map(|owner| owner.cells).sum()
    }
    fn imported(&self, name: &str) -> Option<&ImportedOwner> {
        self.imported_owners
            .iter()
            .find(|owner| owner.name.as_str() == name)
    }
    /// Staged process identifiers with their fixed arguments: each measured
    /// owner's producer in declaration order, then the composer and driver.
    fn processes(&self) -> Vec<(String, Vec<String>)> {
        self.measured_owners
            .iter()
            .map(|owner| {
                let arguments = if owner.name.as_str() == "core" {
                    "--fresh-tuning-process-child"
                } else {
                    "--fresh-child"
                };
                (
                    format!("{}-producer", owner.name.as_str()),
                    vec![arguments.to_owned()],
                )
            })
            .chain([
                ("composer".to_owned(), vec![]),
                ("driver".to_owned(), vec![]),
            ])
            .collect()
    }
    /// The staged executable names, sorted.
    fn executables(&self) -> Vec<String> {
        let mut names: Vec<String> = self.processes().into_iter().map(|(id, _)| id).collect();
        names.sort();
        names
    }
}
/// Evidence that an imported owner is the declared committed envelope, staged
/// byte for byte, and strictly reopens through the staged composer's
/// owner-only codec before any timed child.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ImportedOwnerEvidence {
    owner: Token,
    source: RepositoryArtifact,
    staged: ArtifactIdentity,
    reopen: ArtifactIdentity,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CampaignConfig {
    schema: String,
    campaign_id: Token,
    channels: SessionChannels,
    identity: ResumeIdentity,
    manifests: Vec<OwnerManifest>,
    processes: Vec<ProcessDescriptor>,
    runtime: CanonicalJson,
    protocol: ArtifactIdentity,
    validator: ArtifactIdentity,
    receipt: String,
    affinity: CpuAffinity,
    source_tree: String,
    producing_manifest: ArtifactIdentity,
    build_inputs: BTreeMap<String, String>,
    preflight_reports: BTreeMap<String, ArtifactIdentity>,
    host_admission_policy: HostAdmissionPolicy,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    imported_owners: Vec<ImportedOwnerEvidence>,
}
fn process(config: &CampaignConfig, id: &str) -> io::Result<ProcessDescriptor> {
    config
        .processes
        .iter()
        .find(|p| p.id.as_str() == id)
        .cloned()
        .ok_or_else(|| invalid("unknown staged process"))
}
fn operation(
    process: &ProcessDescriptor,
    request: &OwnerOperation,
    log: Option<&mut ExecutionLog>,
) -> io::Result<OwnerResponse> {
    let input = encoded(request)?;
    invoke_owner(process, request, &["--owner-operation".into()], &input, log)
}
fn invoke_owner(
    process: &ProcessDescriptor,
    request: &OwnerOperation,
    arguments: &[String],
    input: &[u8],
    mut log: Option<&mut ExecutionLog>,
) -> io::Result<OwnerResponse> {
    let mut start_sequence = None;
    if let Some(log) = log.as_mut() {
        record_budget(log, "before-launch", process.id.as_str(), None)?;
        // The exit names the start record itself, not the budget diagnostic
        // journaled before it.
        start_sequence = Some(log.next_sequence());
        log.append(JournalEvent::OrchestrationStart,None,json!({"kind":"orchestration-start","process":process.id,"request_sha256":Sha256Digest::of(input),"arguments":arguments}))?;
    }
    let result = run_process(
        process.command(arguments)?,
        input,
        Duration::from_secs(CHILD_TIMEOUT_SECONDS),
        Duration::from_secs(CHILD_KILL_GRACE_SECONDS),
        |_| Ok(()),
        |_| Ok(()),
    )?;
    if let Some(log) = log.as_mut() {
        log.append(JournalEvent::OrchestrationExit,None,json!({"kind":"orchestration-exit","start_sequence":start_sequence,"process":process.id,"outcome":result.outcome,"stdout_sha256":Sha256Digest::of(&result.stdout),"stderr_sha256":Sha256Digest::of(&result.stderr),"stderr":result.stderr}))?;
        record_budget(log, "after-result", process.id.as_str(), None)?;
    }
    if let OwnerOperation::EmitOwner { request } = request {
        let directory = request
            .output
            .parent()
            .ok_or_else(|| invalid("owner output has no parent"))?;
        save(
            &directory.join("exit.json"),
            &json!({"outcome":result.outcome,"stdout":result.stdout,"stderr":result.stderr}),
        )?;
    }
    if !result.outcome.accepts_result()? {
        return Err(invalid("owner orchestration failed"));
    }
    if let Some(error) = result.callback_error {
        return Err(error);
    }
    transport::parse_result(std::str::from_utf8(&result.stdout).map_err(invalid)?).map_err(invalid)
}
fn verify_report_response(label: &str, response: &OwnerResponse) -> io::Result<()> {
    let valid = match response {
        OwnerResponse::SelfCheck { .. } => label.ends_with("self-check"),
        OwnerResponse::ListGrid { .. } => label.ends_with("list-grid"),
        OwnerResponse::CapabilityReport { .. } => label.ends_with("capability-report"),
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(invalid("staged CLI returned the wrong reporting mode"))
    }
}
fn report_cli(
    process: &ProcessDescriptor,
    label: &str,
    log: Option<&mut ExecutionLog>,
) -> io::Result<OwnerResponse> {
    let request = match label {
        "self-check" => OwnerOperation::SelfCheck,
        "list-grid" => OwnerOperation::ListGrid,
        "capability-report" => OwnerOperation::CapabilityReport,
        _ => return Err(invalid("unknown staged CLI reporting mode")),
    };
    let response = invoke_owner(process, &request, &[format!("--{label}")], b"", log)?;
    verify_report_response(label, &response)?;
    Ok(response)
}
#[derive(Serialize, Deserialize)]
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
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AssemblyRuntime {
    source_dirty: bool,
    tool: String,
    tool_sha256: String,
}
fn host_runtime(receipt: &str) -> io::Result<CanonicalJson> {
    let cpu = fs::read_to_string("/proc/cpuinfo")?;
    let value = |key: &str| {
        cpu.lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                (name.trim() == key).then(|| value.trim().to_owned())
            })
            .ok_or_else(|| invalid(format!("missing {key}")))
    };
    let mut governors = BTreeMap::new();
    for cpu in fs::read_dir("/sys/devices/system/cpu")? {
        let path = cpu?.path();
        let file = path.join("cpufreq/scaling_governor");
        if let Ok(governor) = fs::read_to_string(&file) {
            governors.insert(
                path.file_name().unwrap().to_string_lossy().into_owned(),
                governor.trim().to_owned(),
            );
        }
    }
    if governors.is_empty() {
        return Err(invalid("CPU governor observation unavailable"));
    }
    CanonicalJson::from_serializable(&MeasurementRuntime {
        source_dirty: false,
        toolchain: command_text("rustc", &["+1.95.0", "--version"])?,
        cpu_model: value("model name")?,
        cpu_features: value("flags")?
            .split_whitespace()
            .map(str::to_owned)
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect(),
        os_kernel: command_text("uname", &["-sr"])?,
        governor: serde_json::to_string(&governors).map_err(invalid)?,
        receipt: receipt.into(),
    })
}
fn source_state() -> io::Result<(String, String)> {
    let dirty = command_text("git", &["status", "--porcelain", "--untracked-files=all"])?;
    if !dirty.is_empty() {
        return Err(invalid("measurement requires a clean source tree"));
    }
    Ok((
        command_text("git", &["rev-parse", "HEAD"])?,
        command_text("git", &["rev-parse", "HEAD^{tree}"])?,
    ))
}
fn behavior_sources(declaration: &CampaignDeclaration) -> io::Result<BTreeMap<String, String>> {
    let producing = ProducingInputs::read_at(Path::new("."), &declaration.producing_manifest)?;
    ProducingInputs::hashes_at(Path::new("."), &producing.behavior_sources)
}
/// The independent validator. Its recorded identity is informational: the
/// driver runs the validator of the checkout it executes in, and behavior
/// identity comparisons exclude it, since checking is not measurement.
const VALIDATOR: &str = "dev/scripts/validate-tuning-extent-campaign.py";
/// Behavior identity equality over the same inventory, ignoring [`VALIDATOR`].
fn same_behavior(observed: &BTreeMap<String, String>, recorded: &BTreeMap<String, String>) -> bool {
    observed.len() == recorded.len()
        && observed
            .iter()
            .all(|(path, sha)| path == VALIDATOR || recorded.get(path) == Some(sha))
}
fn verify_config(config: &CampaignConfig) -> io::Result<()> {
    let declaration =
        CampaignDeclaration::for_campaign(Path::new("."), config.campaign_id.as_str())?;
    if config.schema != "tuning-extent-campaign-v1"
        || config.manifests.len() != declaration.measured_owners.len()
    {
        return Err(invalid("campaign schema/owner mismatch"));
    }
    config.channels.validate()?;
    require_affinity(&config.affinity, &CpuAffinity::observe()?)?;
    let (revision, tree) = source_state()?;
    let producing_manifest = declaration.producing_manifest.as_str();
    let producing = ProducingInputs::read_at(Path::new("."), producing_manifest)?;
    let staging: StagingManifest = read_json(&config.channels.stage.join("staging-manifest.json"))?;
    let (source_before, source_after) =
        validate_build_source_observations(&config.channels.stage, &staging)?;
    if revision != config.identity.source_revision
        || tree != config.source_tree
        || Sha256Digest::of(tree.as_bytes()).as_str() != config.identity.source_sha256
        || !same_behavior(
            &behavior_sources(&declaration)?,
            &config.identity.behavior_sha256,
        )
        || declaration
            .recorded_in(&config.identity.behavior_sha256)
            .is_err()
        || ProducingInputs::capture(Path::new("."), producing_manifest)?.lifecycle_sha256()?
            != config.identity.lifecycle_behavior_sha256
        || ProducingInputs::hashes_at(Path::new("."), &producing.build_inputs)?
            != config.build_inputs
        || artifact(Path::new(producing_manifest))? != config.producing_manifest
        || source_before.source_revision != revision
        || source_before.source_tree != tree
        || source_after.source_revision != revision
        || source_after.source_tree != tree
    {
        return Err(invalid("producing source identity changed"));
    }
    if config.affinity.host_identity()? != config.identity.host_identity
        || host_runtime(&config.receipt)? != config.runtime
    {
        return Err(invalid("host/toolchain/governor identity changed"));
    }
    for p in &config.processes {
        p.verify_staged()?;
    }
    if artifact(&env::current_exe()?)?.sha256
        != artifact(&process(config, "driver")?.executable)?.sha256
    {
        return Err(invalid(
            "executing driver differs from staged driver identity",
        ));
    }

    for (m, declared) in config.manifests.iter().zip(&declaration.measured_owners) {
        m.validate()?;
        if m.campaign_id != config.campaign_id
            || m.owner != declared.owner
            || m.counts != DeclaredCounts::for_cells(declared.cells)?
            || m.processes.len() != 1
            || !config.processes.contains(&m.processes[0])
        {
            return Err(invalid("owner manifest campaign/process/count mismatch"));
        }
    }
    verify_imported_owners(config, &declaration)?;
    let ordered = Sha256Digest::of(&encoded(
        &config
            .manifests
            .iter()
            .map(|m| &m.ordered_units)
            .collect::<Vec<_>>(),
    )?);
    let descriptors = Sha256Digest::of(&encoded(&config.processes)?);
    let executables: BTreeMap<String, String> = config
        .processes
        .iter()
        .map(|p| (p.id.as_str().into(), p.executable_sha256.as_str().into()))
        .collect();
    if ordered.as_str() != config.identity.ordered_work_manifest_sha256
        || descriptors.as_str() != config.identity.process_descriptors_sha256
        || executables != config.identity.executable_sha256
        || config.protocol.sha256.as_str() != config.identity.protocol_digest
    {
        return Err(invalid(
            "campaign immutable identity disagrees with manifests/processes",
        ));
    }
    if config.protocol.path != fs::canonicalize(&declaration.protocol)? {
        return Err(invalid("campaign protocol differs from the declaration"));
    }
    if artifact(&config.protocol.path)? != config.protocol {
        return Err(invalid("protocol changed"));
    }
    if config.host_admission_policy != HostAdmissionPolicy::declared() {
        return Err(invalid(
            "host admission policy differs from the declaration",
        ));
    }
    validate_preflight_identities(
        &config.channels.stage,
        &config.preflight_reports,
        &declaration,
    )?;
    for (name, saved) in &config.preflight_reports {
        if artifact(&saved.path)? != *saved {
            return Err(invalid("saved staged CLI preflight changed"));
        }
        let response: OwnerResponse = read_json(&saved.path)?;
        verify_report_response(name, &response)?;
    }
    Ok(())
}

/// Requires each declared imported owner's evidence: the committed envelope
/// still has its declared digest, the staged copy is byte-identical at its
/// canonical stage path, and the strict-reopen record is unchanged.
fn verify_imported_owners(
    config: &CampaignConfig,
    declaration: &CampaignDeclaration,
) -> io::Result<()> {
    if config.imported_owners.len() != declaration.imported_owners.len() {
        return Err(invalid(
            "imported owner evidence differs from the declaration",
        ));
    }
    for (evidence, declared) in config
        .imported_owners
        .iter()
        .zip(&declaration.imported_owners)
    {
        let bytes = declared.envelope.read(Path::new("."))?;
        let staged = config
            .channels
            .stage
            .join(format!("{}-owner.json", declared.name.as_str()));
        if evidence.owner != declared.owner
            || evidence.source != declared.envelope
            || evidence.staged.path != staged
            || artifact(&staged)? != evidence.staged
            || evidence.staged.sha256 != Sha256Digest::of(&bytes)
            || artifact(&evidence.reopen.path)? != evidence.reopen
        {
            return Err(invalid("imported owner evidence changed"));
        }
    }
    Ok(())
}

/// The canonical section wrapper `section` of an envelope document.
fn section_wrapper(document: &[u8], section: &str) -> io::Result<Value> {
    let envelope: Value = serde_json::from_slice(document).map_err(invalid)?;
    envelope
        .get("sections")
        .and_then(|sections| sections.get(section))
        .cloned()
        .ok_or_else(|| invalid(format!("envelope lacks section {section}")))
}

/// Verifies and stages one declared imported owner before any timed child.
///
/// The committed envelope must have its declared digest and carry the same
/// section wrapper as its declared committed complete envelope. Its bytes are
/// staged as `<name>-owner.json`, the composition input, and the staged
/// composer then strictly reopens the staged copy with the owner-only codec
/// by composing it with a conservative core probe. Both composer runs are
/// journaled verification actions; their outputs are evidence only.
fn import_owner(
    root: &Path,
    channels: &SessionChannels,
    campaign_id: &Token,
    revision: &str,
    composer: &ProcessDescriptor,
    imported: &ImportedOwner,
    log: &mut ExecutionLog,
) -> io::Result<ImportedOwnerEvidence> {
    let stage = &channels.stage;
    let bytes = imported.envelope.read(root)?;
    let reference = imported.complete.read(root)?;
    let wrapper = section_wrapper(&bytes, &imported.section)?;
    if wrapper != section_wrapper(&reference, &imported.section)? {
        return Err(invalid(
            "imported owner wrapper differs from its committed complete envelope",
        ));
    }
    let staged = publish_artifact(
        stage,
        &stage.join(format!("{}-owner.json", imported.name.as_str())),
        &bytes,
    )?;
    let root = stage.join("imported");
    fs::create_dir_all(&root)?;
    File::open(stage)?.sync_all()?;
    let directory = root.join(format!(
        "{}-{}-{}",
        imported.name.as_str(),
        log.session_id(),
        log.next_sequence()
    ));
    fs::create_dir(&directory)?;
    File::open(&root)?.sync_all()?;
    let probe_core = directory.join("probe-core.json");
    let probe_complete = directory.join("probe-complete.json");
    let path_text = |path: &Path| -> io::Result<String> {
        path.to_str()
            .map(str::to_owned)
            .ok_or_else(|| invalid("non-UTF-8 stage path"))
    };
    let tail = vec![
        campaign_id.as_str().to_owned(),
        utc()?,
        revision.to_owned(),
        "false".to_owned(),
        composer.executable_sha256.as_str().to_owned(),
    ];
    let mut core_args = vec!["core-owner".to_owned(), path_text(&probe_core)?];
    core_args.extend(tail.iter().cloned());
    let mut complete_args = vec![
        "complete".to_owned(),
        path_text(&probe_core)?,
        path_text(&staged.path)?,
        path_text(&probe_complete)?,
    ];
    complete_args.extend(tail);
    let mut runs = Vec::new();
    for (index, args) in [core_args, complete_args].into_iter().enumerate() {
        let request = save(
            &directory.join(format!("request-{index}.json")),
            &json!({"schema":"tuning-campaign-imported-owner-request-v1","process":composer,"args":args}),
        )?;
        record_budget(log, "before-launch", composer.id.as_str(), None)?;
        log.append(
            JournalEvent::OrchestrationStart,
            None,
            json!({"kind":"orchestration-start","process":"composer","request":request}),
        )?;
        let result = run_process(
            composer.command(&args)?,
            b"",
            Duration::from_secs(CHILD_TIMEOUT_SECONDS),
            Duration::from_secs(CHILD_KILL_GRACE_SECONDS),
            |_| Ok(()),
            |_| Ok(()),
        )?;
        let exit = save(
            &directory.join(format!("exit-{index}.json")),
            &json!({"outcome":result.outcome,"stdout":result.stdout,"stderr":result.stderr}),
        )?;
        log.append(
            JournalEvent::OrchestrationExit,
            None,
            json!({"kind":"orchestration-exit","process":"composer","exit":exit,"outcome":result.outcome}),
        )?;
        record_budget(log, "after-result", composer.id.as_str(), None)?;
        if !result.outcome.accepts_result()? || result.callback_error.is_some() {
            return Err(invalid(
                "imported owner failed the composer's strict owner-only reopen",
            ));
        }
        runs.push(json!({"request":request,"exit":exit}));
    }
    File::open(&directory)?.sync_all()?;
    if section_wrapper(&fs::read(&probe_complete)?, &imported.section)? != wrapper {
        return Err(invalid("strict reopen changed the imported owner wrapper"));
    }
    let reopen = save(
        &directory.join("reopen.json"),
        &json!({
            "schema":"tuning-campaign-imported-owner-v1",
            "owner":imported.owner,
            "section":imported.section,
            "source":imported.envelope,
            "reference":imported.complete,
            "staged":staged,
            "wrapper_sha256":Sha256Digest::of(&encoded(&wrapper)?),
            "probe_core":artifact(&probe_core)?,
            "probe_complete":artifact(&probe_complete)?,
            "runs":runs,
        }),
    )?;
    Ok(ImportedOwnerEvidence {
        owner: imported.owner.clone(),
        source: imported.envelope.clone(),
        staged,
        reopen,
    })
}

fn session_descriptor(
    config: &CampaignConfig,
    session_id: &str,
    lock: &Path,
) -> io::Result<SessionDescriptor> {
    let declaration =
        CampaignDeclaration::for_campaign(Path::new("."), config.campaign_id.as_str())?;
    Ok(SessionDescriptor {
        schema: LIFECYCLE_SCHEMA.into(),
        campaign_id: config.campaign_id.clone(),
        session_id: Token::new(session_id)?,
        preparer: ProcessIdentity::current()?,
        channels: config.channels.clone(),
        identity: config.identity.clone(),
        counts: DeclaredCounts::for_cells(declaration.total_cells())?,
        lock_path: fs::canonicalize(lock)?,
    })
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StagingManifest {
    schema: String,
    source_before: ArtifactIdentity,
    source_after: ArtifactIdentity,
    executables: BTreeMap<Token, ArtifactIdentity>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct BuildSourceObservation {
    schema: String,
    phase: String,
    source_revision: String,
    source_tree: String,
    porcelain: String,
}
fn validate_canonical_stage_artifact(
    stage: &Path,
    identity: &ArtifactIdentity,
    relative_path: &str,
    description: &str,
) -> io::Result<()> {
    let stage = fs::canonicalize(stage)?;
    let expected = stage.join(relative_path);
    let metadata = fs::symlink_metadata(&expected)?;
    if identity.path != expected
        || !metadata.file_type().is_file()
        || metadata.file_type().is_symlink()
        || fs::canonicalize(&expected)? != expected
    {
        return Err(invalid(format!(
            "{description} must be the canonical regular stage artifact"
        )));
    }
    Ok(())
}
fn validate_build_source_observations(
    stage: &Path,
    staging: &StagingManifest,
) -> io::Result<(BuildSourceObservation, BuildSourceObservation)> {
    validate_canonical_stage_artifact(
        stage,
        &staging.source_before,
        "build/source-before.json",
        "pre-build source observation",
    )?;
    validate_canonical_stage_artifact(
        stage,
        &staging.source_after,
        "build/source-after.json",
        "post-build source observation",
    )?;
    let before: BuildSourceObservation = read_json(&staging.source_before.path)?;
    let after: BuildSourceObservation = read_json(&staging.source_after.path)?;
    if artifact(&staging.source_before.path)? != staging.source_before
        || artifact(&staging.source_after.path)? != staging.source_after
        || before.schema != "tuning-campaign-build-source-v1"
        || after.schema != before.schema
        || before.phase != "before-build"
        || after.phase != "after-build"
        || !before.porcelain.is_empty()
        || !after.porcelain.is_empty()
        || before.source_revision != after.source_revision
        || before.source_tree != after.source_tree
    {
        return Err(invalid("build source identity changed or was dirty"));
    }
    Ok((before, after))
}
fn validate_preflight_identities(
    stage: &Path,
    reports: &BTreeMap<String, ArtifactIdentity>,
    declaration: &CampaignDeclaration,
) -> io::Result<()> {
    let expected_reports: std::collections::BTreeSet<_> = declaration
        .measured_owners
        .iter()
        .flat_map(|owner| {
            ["self-check", "list-grid", "capability-report"]
                .map(move |mode| format!("{}-{mode}", owner.name.as_str()))
        })
        .collect();
    if reports
        .keys()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>()
        != expected_reports
    {
        return Err(invalid("saved CLI preflight coverage mismatch"));
    }
    for (name, saved) in reports {
        validate_canonical_stage_artifact(
            stage,
            saved,
            &format!("{name}.json"),
            "staged CLI preflight report",
        )?;
    }
    Ok(())
}
fn stage_executables(
    stage: &Path,
    input: &Path,
    declaration: &CampaignDeclaration,
) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let staging: StagingManifest = read_json(input)?;
    if staging.schema != "tuning-campaign-staging-v1"
        || staging
            .executables
            .keys()
            .map(Token::as_str)
            .collect::<Vec<_>>()
            != declaration.executables()
    {
        return Err(invalid("staging executable identity mismatch"));
    }
    validate_build_source_observations(stage, &staging)?;
    let bin = stage.join("bin");
    fs::create_dir_all(&bin)?;
    File::open(stage)?.sync_all()?;
    for (name, source) in &staging.executables {
        if artifact(&source.path)? != *source {
            return Err(invalid("built executable changed during staging"));
        }
        let target = bin.join(name.as_str());
        publish_artifact(stage, &target, &fs::read(&source.path)?)?;
        fs::set_permissions(&target, fs::Permissions::from_mode(0o755))?;
        File::open(&target)?.sync_all()?;
    }
    save(&stage.join("staging-manifest.json"), &staging)?;
    Ok(())
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct BootstrapInputs {
    revision: String,
    tree: String,
    behavior: BTreeMap<String, String>,
    receipt: String,
    runtime: CanonicalJson,
    processes: Vec<ProcessDescriptor>,
    protocol: ArtifactIdentity,
    validator: ArtifactIdentity,
    affinity: CpuAffinity,
    staging: ArtifactIdentity,
    producing_manifest: ArtifactIdentity,
    build_inputs: BTreeMap<String, String>,
    host_admission_policy: HostAdmissionPolicy,
}
fn bootstrap_inputs(
    channels: &SessionChannels,
    campaign_id: &Token,
    declaration: &CampaignDeclaration,
) -> io::Result<BootstrapInputs> {
    let (revision, tree) = source_state()?;
    let staging: StagingManifest = read_json(&channels.stage.join("staging-manifest.json"))?;
    let (source_before, source_after) =
        validate_build_source_observations(&channels.stage, &staging)?;
    if source_before.source_revision != revision
        || source_before.source_tree != tree
        || source_after.source_revision != revision
        || source_after.source_tree != tree
    {
        return Err(invalid(
            "build source identity differs from preparation source",
        ));
    }
    let producing = ProducingInputs::read_at(Path::new("."), &declaration.producing_manifest)?;
    let behavior = ProducingInputs::hashes_at(Path::new("."), &producing.behavior_sources)?;
    let receipt = format!("dev/benchmarks/tuning_profiles/{}.md", campaign_id.as_str());
    let runtime = host_runtime(&receipt)?;
    let affinity = CpuAffinity::observe()?;
    let repository = fs::canonicalize(".")?;
    let mut processes = Vec::new();
    for (id, args) in declaration.processes() {
        let binary = artifact(&channels.stage.join("bin").join(&id))?;
        processes.push(ProcessDescriptor {
            id: Token::new(id)?,
            executable: binary.path,
            executable_sha256: binary.sha256,
            arguments: args,
            environment: measurement_environment(),
            working_directory: repository.clone(),
        });
    }
    Ok(BootstrapInputs {
        revision,
        tree,
        behavior,
        receipt,
        runtime,
        processes,
        protocol: artifact(&repository.join(&declaration.protocol))?,
        validator: artifact(&repository.join(VALIDATOR))?,
        affinity,
        staging: artifact(&channels.stage.join("staging-manifest.json"))?,
        producing_manifest: artifact(Path::new(&declaration.producing_manifest))?,
        build_inputs: ProducingInputs::hashes_at(Path::new("."), &producing.build_inputs)?,
        host_admission_policy: HostAdmissionPolicy::declared(),
    })
}

fn validate_campaign_stage(
    stage: &Path,
    campaign_id: &str,
    declaration: &CampaignDeclaration,
) -> io::Result<()> {
    let suffix = campaign_id
        .strip_prefix(&declaration.prefix())
        .ok_or_else(|| invalid("campaign ID has the wrong issue prefix"))?;
    let (stamp, pid) = suffix
        .rsplit_once('-')
        .ok_or_else(|| invalid("campaign ID lacks launcher PID"))?;
    // Lowercase `t`/`z` keep the ID a valid `ProfileId`; the owners reject any
    // other ID when the campaign manifest is built, before any timed cell.
    if stamp.len() != 16
        || stamp.as_bytes().get(8) != Some(&b't')
        || !stamp[..8].bytes().all(|byte| byte.is_ascii_digit())
        || !stamp[9..15].bytes().all(|byte| byte.is_ascii_digit())
        || !stamp.ends_with('z')
        || pid.starts_with('0')
        || pid.parse::<u32>().is_err()
    {
        return Err(invalid("campaign ID is not the declared UTC/PID form"));
    }
    let expected = fs::canonicalize("/tmp")?.join(campaign_id);
    if stage != expected {
        return Err(invalid("campaign stage must be exactly /tmp/<campaign-id>"));
    }
    if let Ok(metadata) = fs::symlink_metadata(stage) {
        if metadata.file_type().is_symlink() {
            return Err(invalid("campaign stage cannot be a symlink"));
        }
    }
    Ok(())
}
fn prepare(
    stage: &Path,
    campaign_id: &str,
    session_id: &str,
    lock: &Path,
    staging: Option<&Path>,
) -> io::Result<()> {
    let declaration = CampaignDeclaration::for_campaign(Path::new("."), campaign_id)?;
    validate_campaign_stage(stage, campaign_id, &declaration)?;
    if !stage.exists() {
        fs::create_dir(stage)?;
        File::open("/tmp")?.sync_all()?;
    }
    let channels = SessionChannels::for_stage(stage)?;
    if let Some(input) = staging {
        stage_executables(&channels.stage, input, &declaration)?;
    }
    if channels.stage.join("campaign.json").exists() {
        verify_config(&read_json::<CampaignConfig>(
            &channels.stage.join("campaign.json"),
        )?)?;
    }
    if let Some((store, mut checkpoints, mut log)) =
        PreparationStore::resume_pending(&channels.stage)?
    {
        if store.descriptor().campaign_id.as_str() != campaign_id
            || store.descriptor().session_id.as_str() != session_id
        {
            return Err(invalid("pending preparation handoff identity mismatch"));
        }
        let config: CampaignConfig = read_json(&channels.stage.join("campaign.json"))?;
        verify_config(&config)?;
        acknowledge_pending_recovery(&mut log, &mut checkpoints, &config.campaign_id)?;
        reconcile_campaign_checkpoints(&mut log, &checkpoints, &resolved_manifests(&config)?)?;
        store.announce_prepared(&mut log, &mut io::stdout().lock())?;
        println!(
            "GF2_CAMPAIGN_SESSION={}",
            store.descriptor().session_id.as_str()
        );
        return Ok(());
    }
    if channels.stage.join("active-session.json").exists() {
        recover(&channels.stage)?;
    }
    let campaign_id = Token::new(campaign_id)?;
    if campaign_complete(&channels, &campaign_id)? {
        return Err(invalid(
            "campaign is complete; publish-campaign finishes its repository publication",
        ));
    }
    let inputs = bootstrap_inputs(&channels, &campaign_id, &declaration)?;
    let preparation = PreparationStore::begin(
        channels.clone(),
        campaign_id.clone(),
        Token::new(session_id)?,
        fs::canonicalize(lock)?,
        CanonicalJson::from_serializable(&inputs)?,
    )?;
    let mut log = preparation.open_log()?;
    log.announce(io::stdout().lock())?;
    log.append(
        JournalEvent::DriverDiagnostic,
        None,
        json!({"kind":"cpu-affinity","phase":"preparation","observed":inputs.affinity}),
    )?;
    let BootstrapInputs {
        revision,
        tree,
        behavior,
        receipt,
        runtime,
        processes,
        protocol,
        validator,
        affinity,
        ..
    } = inputs;
    let config_path = channels.stage.join("campaign.json");
    let config: CampaignConfig = if config_path.exists() {
        let config: CampaignConfig = read_json(&config_path)?;
        if config.campaign_id != campaign_id {
            return Err(invalid("campaign identity mismatch"));
        }
        verify_config(&config)?;
        config
    } else {
        let mut request = ManifestRequest {
            campaign_id: campaign_id.clone(),
            protocol_sha256: protocol.sha256.clone(),
            channels: channels.clone(),
            processes: processes.clone(),
        };
        let mut manifests = Vec::new();
        let mut preflight_reports = BTreeMap::new();
        for (i, owner) in declaration.measured_owners.iter().enumerate() {
            let name = owner.name.as_str();
            request.processes = vec![processes[i].clone()];
            for label in ["self-check", "list-grid", "capability-report"] {
                let response = report_cli(&processes[i], label, Some(&mut log))?;
                let key = format!("{name}-{label}");
                let saved = save(&channels.stage.join(format!("{key}.json")), &response)?;
                if preflight_reports.insert(key, saved).is_some() {
                    return Err(invalid("duplicate staged CLI preflight report"));
                }
            }
            let OwnerResponse::CampaignManifest { manifest } = operation(
                &processes[i],
                &OwnerOperation::CampaignManifest {
                    request: request.clone(),
                },
                Some(&mut log),
            )?
            else {
                return Err(invalid("owner did not return manifest"));
            };
            save(
                &channels.stage.join(format!("{name}-manifest.json")),
                &manifest,
            )?;
            manifests.push(*manifest);
        }
        let composer = processes
            .iter()
            .find(|process| process.id.as_str() == "composer")
            .ok_or_else(|| invalid("composer is not staged"))?;
        let imported_owners = declaration
            .imported_owners
            .iter()
            .map(|imported| {
                import_owner(
                    Path::new("."),
                    &channels,
                    &campaign_id,
                    &revision,
                    composer,
                    imported,
                    &mut log,
                )
            })
            .collect::<io::Result<Vec<_>>>()?;
        let producing = ProducingInputs::read_at(Path::new("."), &declaration.producing_manifest)?;
        let build_inputs = ProducingInputs::hashes_at(Path::new("."), &producing.build_inputs)?;
        let producing_manifest = artifact(Path::new(&declaration.producing_manifest))?;
        let lifecycle_behavior =
            ProducingInputs::hashes_at(Path::new("."), &producing.lifecycle_sources)?;
        let identity = ResumeIdentity {
            protocol_digest: protocol.sha256.as_str().into(),
            source_revision: revision,
            source_sha256: Sha256Digest::of(tree.as_bytes()).as_str().into(),
            ordered_work_manifest_sha256: Sha256Digest::of(&encoded(
                &manifests
                    .iter()
                    .map(|m| &m.ordered_units)
                    .collect::<Vec<_>>(),
            )?)
            .as_str()
            .into(),
            process_descriptors_sha256: Sha256Digest::of(&encoded(&processes)?).as_str().into(),
            executable_sha256: processes
                .iter()
                .map(|p| (p.id.as_str().into(), p.executable_sha256.as_str().into()))
                .collect(),
            behavior_sha256: behavior.clone(),
            lifecycle_schema: LIFECYCLE_SCHEMA.into(),
            lifecycle_behavior_sha256: Sha256Digest::of(&encoded(&lifecycle_behavior)?)
                .as_str()
                .into(),
            feature_contract: FEATURE_CONTRACT.into(),
            thread_contract: THREAD_CONTRACT.into(),
            host_identity: CpuAffinity::observe()?.host_identity()?,
        };
        let config = CampaignConfig {
            schema: "tuning-extent-campaign-v1".into(),
            campaign_id,
            channels: channels.clone(),
            identity,
            manifests,
            processes,
            runtime,
            protocol,
            validator,
            receipt,
            affinity: affinity.clone(),
            source_tree: tree,
            producing_manifest,
            build_inputs,
            preflight_reports,
            host_admission_policy: HostAdmissionPolicy::declared(),
            imported_owners,
        };

        verify_config(&config)?;
        save(&config_path, &config)?;
        config
    };
    require_affinity(&config.affinity, &affinity)?;
    let mut descriptor = session_descriptor(&config, preparation.session_id().as_str(), lock)?;
    descriptor.preparer = preparation.preparer().clone();
    let (store, mut checkpoints) =
        preparation.finish(descriptor, artifact(&config_path)?, &mut log)?;
    acknowledge_pending_recovery(&mut log, &mut checkpoints, &config.campaign_id)?;
    reconcile_campaign_checkpoints(&mut log, &checkpoints, &resolved_manifests(&config)?)?;
    store.announce_prepared(&mut log, &mut io::stdout().lock())?;
    println!(
        "GF2_CAMPAIGN_SESSION={}",
        store.descriptor().session_id.as_str()
    );
    Ok(())
}
fn resolved_manifests(config: &CampaignConfig) -> io::Result<Vec<OwnerManifest>> {
    let mut manifests = config.manifests.clone();
    let stage = &config.channels.stage;
    if stage.join("derived-manifest.json").exists() {
        let request: DeriveManifestRequest = read_json(&stage.join("derived-request.json"))?;
        let derived: DerivedManifest = read_json(&stage.join("derived-manifest.json"))?;
        if artifact(&request.accepted_inputs.path)? != request.accepted_inputs {
            return Err(invalid("derived input changed"));
        }
        manifests[0] = manifests[0].apply_derivation(&request, &derived)?;
        if stage.join("core-resolved-manifest.json").exists()
            && read_json::<OwnerManifest>(&stage.join("core-resolved-manifest.json"))?
                != manifests[0]
        {
            return Err(invalid("resolved manifest differs from derivation"));
        }
    }
    Ok(manifests)
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum UnitBoundary {
    Completion,
    Exit,
    Validation,
}
fn bounded_unit(
    config: &CampaignConfig,
    unit: &LaunchUnit,
    log: &mut ExecutionLog,
    checkpoints: &mut CheckpointStore,
) -> io::Result<AcceptedResult> {
    bounded_unit_with_hook(config, unit, log, checkpoints, |_| Ok(()))
}
fn bounded_unit_with_hook(
    config: &CampaignConfig,
    unit: &LaunchUnit,
    log: &mut ExecutionLog,
    checkpoints: &mut CheckpointStore,
    mut hook: impl FnMut(UnitBoundary) -> io::Result<()>,
) -> io::Result<AcceptedResult> {
    let process = process(config, unit.process.as_str())?;
    record_budget(log, "before-launch", process.id.as_str(), Some(&unit.key))?;
    let attempt_token = Token::new(format!("attempt-{}", log.next_sequence()))?;
    log.append(
        JournalEvent::CellStart,
        Some(serde_json::to_value(&unit.identity).map_err(invalid)?),
        json!({"key":unit.key,"ordinal":unit.ordinal}),
    )?;
    let state = std::cell::RefCell::new((log, None::<ChildAttempt>, Vec::<u8>::new()));
    let result = run_process(
        process.command(&process.arguments)?,
        unit.case.as_str().as_bytes(),
        Duration::from_secs(CHILD_TIMEOUT_SECONDS),
        Duration::from_secs(CHILD_KILL_GRACE_SECONDS),
        |pid| {
            let mut state = state.borrow_mut();
            state.1 = Some(ChildAttempt::start(
                state.0,
                unit.clone(),
                attempt_token.clone(),
                pid,
            )?);
            Ok(())
        },
        |bytes| {
            let mut state = state.borrow_mut();
            state.2.extend_from_slice(bytes);
            while let Some(end) = state.2.iter().position(|b| *b == b'\n') {
                let line: Vec<_> = state.2.drain(..=end).collect();
                let (log, attempt, _) = &mut *state;
                match std::str::from_utf8(&line[..line.len() - 1]) {
                    Ok(line) => attempt
                        .as_mut()
                        .ok_or_else(|| invalid("stderr before spawn"))?
                        .stderr_line(log, line)?,
                    Err(e) => {
                        log.append(
                            JournalEvent::DriverDiagnostic,
                            Some(serde_json::to_value(&unit.identity).map_err(invalid)?),
                            json!({"attempt":attempt_token,"raw_bytes":line}),
                        )?;
                        return Err(invalid(e));
                    }
                }
            }
            Ok(())
        },
    )?;
    let (log, attempt, pending) = state.into_inner();
    record_budget(log, "after-result", process.id.as_str(), Some(&unit.key))?;
    let raw_root = config.channels.stage.join("raw-attempts");
    fs::create_dir_all(&raw_root)?;
    File::open(&config.channels.stage)?.sync_all()?;
    let stdout_artifact = publish_artifact(
        &config.channels.stage,
        &raw_root.join(format!(
            "{}-{}.stdout",
            log.session_id(),
            attempt_token.as_str()
        )),
        &result.stdout,
    )?;
    let stderr_artifact = publish_artifact(
        &config.channels.stage,
        &raw_root.join(format!(
            "{}-{}.stderr",
            log.session_id(),
            attempt_token.as_str()
        )),
        &result.stderr,
    )?;
    let mut attempt = attempt.ok_or_else(|| invalid("missing attempt"))?;
    let completion = ChildCompletionEvidence::new(
        unit,
        attempt_token.clone(),
        result.outcome.clone(),
        stdout_artifact,
        stderr_artifact,
        result.callback_error.as_ref().map(ToString::to_string),
    )?;
    attempt.record_completion(log, &completion)?;
    hook(UnitBoundary::Completion)?;
    if !pending.is_empty() {
        log.append(
            JournalEvent::DriverDiagnostic,
            Some(serde_json::to_value(&unit.identity).map_err(invalid)?),
            json!({"attempt":attempt_token,"unterminated_stderr_bytes":pending}),
        )?;
    }
    let parsed = attempt
        .exited(
            log,
            result.outcome,
            &result.stdout,
            Sha256Digest::of(&result.stderr),
        )
        .cloned();
    if let Some(error) = result.callback_error {
        return Err(error);
    }
    if !pending.is_empty() {
        return Err(invalid("unterminated child stderr record"));
    }
    let child = parsed?;
    hook(UnitBoundary::Exit)?;
    let response = operation(
        &process,
        &OwnerOperation::ValidateResult {
            unit: Box::new(unit.clone()),
            result: Box::new(child.clone()),
        },
        Some(log),
    )?;
    attempt.owner_validated(log, &response)?;
    hook(UnitBoundary::Validation)?;
    let accepted = accept_checkpoint(log, checkpoints, unit, &child)?;
    log.append(
        JournalEvent::CellComplete,
        Some(serde_json::to_value(&unit.identity).map_err(invalid)?),
        json!({"key":unit.key,"checkpoint_sha256":accepted.sha256}),
    )?;
    Ok(AcceptedResult {
        unit: unit.clone(),
        result: child,
        checkpoint_sha256: Sha256Digest::new(accepted.sha256)?,
    })
}
fn derive(
    config: &CampaignConfig,
    core: &OwnerManifest,
    bundle: &AcceptedResultsBundle,
    log: &mut ExecutionLog,
) -> io::Result<OwnerManifest> {
    let stage = &config.channels.stage;
    let input = save(&stage.join("core-derivation-input.json"), bundle)?;
    let reserved_units = core
        .ordered_units
        .iter()
        .filter(|u| u.identity.phase.as_str() == "m4rm-joint")
        .cloned()
        .collect::<Vec<_>>();
    if reserved_units.len() != 144 {
        return Err(invalid("wrong predeclared conditional slots"));
    }
    let request = DeriveManifestRequest {
        campaign_id: config.campaign_id.clone(),
        original_manifest_sha256: core.manifest_sha256.clone(),
        reserved_units,
        accepted_inputs: input,
    };
    save(&stage.join("derived-request.json"), &request)?;
    let OwnerResponse::DeriveManifest { manifest } = operation(
        &process(config, "core-producer")?,
        &OwnerOperation::DeriveManifest {
            request: request.clone(),
        },
        Some(log),
    )?
    else {
        return Err(invalid("derivation returned wrong response"));
    };
    manifest.validate(&request)?;
    save(&stage.join("derived-manifest.json"), &manifest)?;
    let resolved = core.apply_derivation(&request, &manifest)?;
    save(&stage.join("core-resolved-manifest.json"), &resolved)?;
    Ok(resolved)
}
fn make_receipt(
    config: &CampaignConfig,
    declaration: &CampaignDeclaration,
    bundles: &[AcceptedResultsBundle],
    responses: &[OwnerResponse],
) -> io::Result<()> {
    let stage = &config.channels.stage;
    let records = ExecutionLog::validate_prefix(
        &fs::read(&config.channels.execution_log)?,
        config.campaign_id.as_str(),
    )?;
    let counts = DeclaredCounts::for_cells(config.manifests.iter().map(|m| m.counts.cells).sum())?;
    let attempts = records
        .iter()
        .filter(|r| r.event == JournalEvent::ChildSpawn)
        .count();
    let orchestration = records
        .iter()
        .filter(|r| r.details.get("kind").and_then(Value::as_str) == Some("orchestration-start"))
        .count();
    let sessions = records
        .iter()
        .filter(|r| r.event == JournalEvent::SessionPrepared)
        .count();
    let artifacts:Vec<_>=bundles.iter().flat_map(|b| b.accepted.iter()).map(|entry|json!({"key":entry.unit.key,"path":config.channels.checkpoints.join("units").join(format!("{}.json",Sha256Digest::of(entry.unit.key.as_str().as_bytes()).as_str())),"sha256":entry.checkpoint_sha256})).collect();
    let proposed_projection = json!({"schema":"tuning-campaign-receipt-projection-v1","campaign_id":config.campaign_id,"protocol":config.protocol,"identity":config.identity,"runtime":config.runtime,"affinity":config.affinity,"owners":responses,"counts":counts,"attempts":attempts,"orchestration":orchestration,"sessions":sessions,"raw_artifacts":artifacts,"journal_sequence":records.last().map(|record|record.sequence),"raw_keys":bundles.iter().flat_map(|b|b.accepted.iter().map(|r|r.unit.key.clone())).collect::<Vec<_>>()});
    let projection: Value = if stage.join("receipt-projection.json").exists() {
        read_json(&stage.join("receipt-projection.json"))?
    } else {
        proposed_projection
    };
    let attempts = projection
        .get("attempts")
        .and_then(Value::as_u64)
        .ok_or_else(|| invalid("receipt attempts missing"))?;
    let orchestration = projection
        .get("orchestration")
        .and_then(Value::as_u64)
        .ok_or_else(|| invalid("receipt orchestration missing"))?;
    let sessions = projection
        .get("sessions")
        .and_then(Value::as_u64)
        .ok_or_else(|| invalid("receipt session count missing"))?;
    save(&stage.join("receipt-projection.json"), &projection)?;
    let facts = ReceiptFacts::new(
        config,
        declaration,
        counts,
        [attempts, orchestration, sessions],
    )?;
    publish_receipt_documents(
        stage,
        &facts,
        bundles
            .iter()
            .flat_map(|b| b.accepted.iter().map(|r| &r.unit)),
        responses,
    )
}
/// Publishes the archived raw result index and complete owner decisions, then
/// the receipt that pins both by digest.
fn publish_receipt_documents<'a>(
    stage: &Path,
    facts: &ReceiptFacts,
    units: impl Iterator<Item = &'a LaunchUnit>,
    responses: &[OwnerResponse],
) -> io::Result<()> {
    let index = raw_result_index(units);
    publish_artifact(stage, &stage.join(RAW_RESULT_INDEX), index.as_bytes())?;
    let decisions = encoded(&responses)?;
    publish_artifact(stage, &stage.join(OWNER_DECISIONS), &decisions)?;
    let receipt = render_receipt(facts, stage, responses, index.as_bytes(), &decisions)?;
    publish_artifact(stage, &stage.join("receipt.md"), receipt.as_bytes())?;
    Ok(())
}
/// Stage file listing every accepted raw key; archived beside the checkpoints
/// so the committed receipt stays bounded by the declared grid.
const RAW_RESULT_INDEX: &str = "raw-result-index.md";
/// Stage file holding the complete owner responses, raw timing windows
/// included; archived and pinned by digest from the receipt.
const OWNER_DECISIONS: &str = "owner-decisions.json";
/// Replaces every raw `samples` array with its length as `sample_count`. The
/// surrounding summaries (medians, spreads, selections, ties, fallbacks and
/// curve flags) stay unchanged.
fn summarize_samples(value: &mut Value) {
    match value {
        Value::Object(map) => {
            if let Some(count) = map.get("samples").and_then(Value::as_array).map(Vec::len) {
                map.remove("samples");
                map.insert("sample_count".into(), count.into());
            }
            map.values_mut().for_each(summarize_samples);
        }
        Value::Array(items) => items.iter_mut().for_each(summarize_samples),
        _ => {}
    }
}
/// The owner responses as the receipt shows them: each embedded decisions
/// document with its raw window arrays summarized, and each artifact named by
/// its archived path instead of its stage path.
fn receipt_decisions(
    responses: &[OwnerResponse],
    stage: &Path,
    layout: &RepositoryLayout,
) -> io::Result<Vec<Value>> {
    responses
        .iter()
        .map(|response| {
            let mut value = serde_json::to_value(response).map_err(invalid)?;
            if let Some(decisions) = value.get_mut("decisions") {
                let mut parsed: Value = serde_json::from_str(
                    decisions
                        .as_str()
                        .ok_or_else(|| invalid("owner decisions are not embedded JSON"))?,
                )
                .map_err(invalid)?;
                summarize_samples(&mut parsed);
                *decisions = Value::String(serde_json::to_string(&parsed).map_err(invalid)?);
            }
            if let Some(path) = value.pointer_mut("/artifact/path") {
                let relative = Path::new(
                    path.as_str()
                        .ok_or_else(|| invalid("owner artifact path is not text"))?,
                )
                .strip_prefix(stage)
                .map_err(|_| invalid("owner artifact lies outside the stage"))?
                .to_str()
                .ok_or_else(|| invalid("non-UTF-8 owner artifact path"))?
                .to_owned();
                *path = Value::String(format!("{}/{relative}", layout.archive()));
            }
            Ok(value)
        })
        .collect()
}
/// One line per accepted raw key with its field, stratum, candidate and task.
fn raw_result_index<'a>(units: impl Iterator<Item = &'a LaunchUnit>) -> String {
    let mut index = String::from("# Raw result index\n\n");
    for unit in units {
        index.push_str(&format!(
            "- `{}`: `{}` / `{}` / `{}` / `{:?}`\n",
            unit.key.as_str(),
            unit.identity.field.as_str(),
            unit.identity.stratum.as_str(),
            unit.identity.candidate.as_str(),
            unit.identity.task
        ));
    }
    index
}
/// Stage files the receipt and the evidence index cite, with their labels, in
/// citation order. Every one is immutable once the receipt is rendered.
fn cited_sources(
    declaration: &CampaignDeclaration,
    imported: &[ImportedOwnerEvidence],
    stage: &Path,
) -> io::Result<Vec<(String, String)>> {
    let mut sources = vec![("Campaign record".to_owned(), "campaign.json".to_owned())];
    for owner in &declaration.measured_owners {
        sources.push((
            format!("{} owner manifest", owner.owner.as_str()),
            format!("{}-manifest.json", owner.name.as_str()),
        ));
    }
    sources.push((
        "gf2-core resolved owner manifest".to_owned(),
        "core-resolved-manifest.json".to_owned(),
    ));
    for owner in &declaration.measured_owners {
        sources.push((
            format!("{} owner response", owner.owner.as_str()),
            format!("{}-owner-response.json", owner.name.as_str()),
        ));
    }
    for evidence in imported {
        let relative = evidence
            .reopen
            .path
            .strip_prefix(stage)
            .map_err(|_| invalid("imported-owner record lies outside the stage"))?
            .to_str()
            .ok_or_else(|| invalid("non-UTF-8 imported-owner record path"))?
            .to_owned();
        sources.push((
            format!("{} imported-owner reopen record", evidence.owner.as_str()),
            relative,
        ));
    }
    for (label, source) in [
        ("Composition record", "composition.json"),
        ("Checkpoint manifest", "checkpoints/manifest.json"),
        ("Receipt projection", "receipt-projection.json"),
        ("Raw result index", RAW_RESULT_INDEX),
        ("Owner decisions", OWNER_DECISIONS),
    ] {
        sources.push((label.to_owned(), source.to_owned()));
    }
    Ok(sources)
}
/// The `| label | path | SHA-256 |` rows of cited stage files, each at its
/// publication destination.
fn cited_rows(
    stage: &Path,
    layout: &RepositoryLayout,
    declaration: &CampaignDeclaration,
    sources: &[(String, String)],
) -> io::Result<String> {
    let mut rows = String::new();
    for (label, source) in sources {
        let destination = destination_for(layout, declaration, source)
            .ok_or_else(|| invalid("a cited stage file has no publication destination"))?;
        rows.push_str(&format!(
            "| {label} | `{destination}` | `{}` |\n",
            Sha256Digest::of(&fs::read(stage.join(source))?).as_str()
        ));
    }
    Ok(rows)
}
/// Journal-derived and configuration facts the receipt states.
struct ReceiptFacts {
    campaign_id: String,
    declaration: CampaignDeclaration,
    declaration_path: String,
    declaration_sha256: Sha256Digest,
    protocol_sha256: Sha256Digest,
    source_revision: String,
    affinity: String,
    /// Owner, owner protocol, behavior token and executable digest.
    owners: Vec<[String; 4]>,
    /// Cited stage files: label and stage-relative source.
    cited: Vec<(String, String)>,
    counts: DeclaredCounts,
    attempts: u64,
    orchestration: u64,
    sessions: u64,
}
impl ReceiptFacts {
    fn new(
        config: &CampaignConfig,
        declaration: &CampaignDeclaration,
        counts: DeclaredCounts,
        [attempts, orchestration, sessions]: [u64; 3],
    ) -> io::Result<Self> {
        let (declaration_path, declaration_sha256) =
            declaration.recorded_in(&config.identity.behavior_sha256)?;
        Ok(Self {
            campaign_id: config.campaign_id.as_str().to_owned(),
            declaration: declaration.clone(),
            declaration_path: declaration_path.to_owned(),
            declaration_sha256: Sha256Digest::new(declaration_sha256)?,
            protocol_sha256: config.protocol.sha256.clone(),
            source_revision: config.identity.source_revision.clone(),
            affinity: format!("{:?}", config.affinity.cpus()),
            owners: config
                .manifests
                .iter()
                .map(|manifest| {
                    [
                        manifest.owner.as_str().to_owned(),
                        manifest.owner_protocol.as_str().to_owned(),
                        manifest.behavior_token.as_str().to_owned(),
                        manifest.processes[0].executable_sha256.as_str().to_owned(),
                    ]
                })
                .collect(),
            cited: cited_sources(declaration, &config.imported_owners, &config.channels.stage)?,
            counts,
            attempts,
            orchestration,
            sessions,
        })
    }
}
/// The receipt: fixed sections, the owner decisions with raw windows
/// summarized, and the digests of the archived raw result index and complete
/// owner decisions. Its size is bounded by the declared grid, not by the
/// accepted-result or timing-window count. It names files only by
/// repository-relative committed paths or archived paths with their SHA-256.
fn render_receipt(
    facts: &ReceiptFacts,
    stage: &Path,
    responses: &[OwnerResponse],
    index: &[u8],
    decisions: &[u8],
) -> io::Result<String> {
    let layout = RepositoryLayout {
        id: &facts.campaign_id,
    };
    let mut receipt=format!("# Extent calibration {}\n\n## Campaign identity and protocol\n\nProtocol: `{}`; SHA-256 `{}`. Declaration: `{}`; SHA-256 `{}`. Producing commit: `{}`.\n\n## Section-specific provenance and assembly\n\nThe campaign record, owner responses and composition record listed under cited evidence hold the runtime observations, executable and behavior identities, and strict codec evidence.\n\n## Grids, controls, and seed allocation\n\nThe immutable owner manifests listed under cited evidence contain every acquisition slot and opaque owner case. Each accepted payload contains its full seed, fixture, route and semantic witness.\n\n## Coverage, accounting, and resume history\n\nThe execution journal, pinned by its row in the committed checksum manifest `{}`, and the checkpoint manifest are authoritative for attempts, accepted results, sessions, lock observations, censored intervals, and orchestration.\n\n## Effective routes and semantic witnesses\n\nEach accepted raw payload resolves through the archived raw result index.\n\n## Raw samples and uncertainty\n\nEvery raw key resolves through the receipt projection's raw_artifacts; five timing windows, calls and elapsed nanoseconds remain in each timed record.\n\n## Argmin and threshold decisions\n\nGEMM row/column decisions are joint; dot chunk decisions cite this campaign. Owner projections preserve ties, schedule plateaus, cross-stratum conflicts, conditional M4RM decisions and fallbacks:\n\n```json\n{}\n```\n\n## Owner and complete validation\n\nOwner responses record strict owner-only reopen. Composition preserves each complete section wrapper. Independent validation recomputes the estimators and evidence accounting.\n\n## Limitations\n\nMeasured choices are conditional on this host, declared grid, controls, and protocol. Unmeasured leaves remain omissions. Timing intervals are empirical measurements, not Monte Carlo probability estimates.\n\n",facts.campaign_id,facts.declaration.protocol,facts.protocol_sha256.as_str(),facts.declaration_path,facts.declaration_sha256.as_str(),facts.source_revision,layout.checksum(),serde_json::to_string_pretty(&receipt_decisions(responses, stage, &layout)?).map_err(invalid)?);
    if !facts.declaration.imported_owners.is_empty() {
        receipt.push_str("## Imported owners\n\n");
        for imported in &facts.declaration.imported_owners {
            receipt.push_str(&format!(
                "Owner `{}` is imported unmeasured from `{}` (SHA-256 `{}`). The complete envelope carries its `{}` section wrapper unchanged; the committed complete envelope `{}` (SHA-256 `{}`) holds the same wrapper.\n\n",
                imported.owner.as_str(),
                imported.envelope.path,
                imported.envelope.sha256.as_str(),
                imported.section,
                imported.complete.path,
                imported.complete.sha256.as_str(),
            ));
        }
    }
    receipt.push_str("## Cited evidence\n\nArchived paths lie in the host-local evidence archive that the committed checksum manifest pins.\n\n| Evidence | Path | SHA-256 |\n|---|---|---|\n");
    receipt.push_str(&cited_rows(
        stage,
        &layout,
        &facts.declaration,
        &facts.cited,
    )?);
    receipt.push_str("\n## Raw result index\n\n");
    receipt.push_str(&format!("Preparation CPU affinity: `{}`. Held-lock observations are recorded in each session journal and must equal this set.\n\n",facts.affinity));
    for [owner, protocol, behavior, executable] in &facts.owners {
        receipt.push_str(&format!(
            "Owner `{owner}` uses protocol `{protocol}` and behavior `{behavior}`; executable `{executable}`.\n\n"
        ));
    }
    let counts = &facts.counts;
    receipt.push_str(&format!("Accepted accounting: {} cells, {} probes, {} timed children, {} accepted results, {} raw windows, {} timing progress records. Observed {} attempts, {} orchestration actions, {} sessions at the receipt projection journal_sequence. Later finalization and resume events remain in the authoritative journal.\n\n",counts.cells,counts.probes,counts.timed_children,counts.accepted_results,counts.windows,counts.progress_records,facts.attempts,facts.orchestration,facts.sessions));
    receipt.push_str(&format!(
        "The archived `{}/{RAW_RESULT_INDEX}` lists every accepted raw key with its field, stratum, candidate and task; SHA-256 `{}`.\n",
        layout.archive(),
        Sha256Digest::of(index).as_str()
    ));
    receipt.push_str(&format!(
        "The archived `{}/{OWNER_DECISIONS}` holds the complete owner decisions, every raw timing window included; SHA-256 `{}`. The decisions above replace each raw `samples` array with its `sample_count`.\n",
        layout.archive(),
        Sha256Digest::of(decisions).as_str()
    ));
    Ok(receipt)
}
/// The campaign identities the evidence index states.
struct EvidenceIdentity<'a> {
    campaign_id: &'a str,
    protocol_sha256: &'a Sha256Digest,
    source_revision: &'a str,
    declaration_path: &'a str,
    declaration_sha256: &'a str,
    imported: &'a [ImportedOwnerEvidence],
}
impl<'a> EvidenceIdentity<'a> {
    fn of(config: &'a CampaignConfig, declaration: &CampaignDeclaration) -> io::Result<Self> {
        let (declaration_path, declaration_sha256) =
            declaration.recorded_in(&config.identity.behavior_sha256)?;
        Ok(Self {
            campaign_id: config.campaign_id.as_str(),
            protocol_sha256: &config.protocol.sha256,
            source_revision: &config.identity.source_revision,
            declaration_path,
            declaration_sha256,
            imported: &config.imported_owners,
        })
    }
}
/// The evidence index committed beside the receipt: every cited stage file,
/// the execution journal and every published or imported envelope, each at
/// its repository path with its SHA-256.
fn render_evidence_index(
    stage: &Path,
    identity: &EvidenceIdentity,
    declaration: &CampaignDeclaration,
) -> io::Result<String> {
    let id = identity.campaign_id;
    let layout = RepositoryLayout { id };
    let mut text = format!(
        "# Evidence index for {id}\n\nThe receipt [`{id}.md`]({id}.md) is checksum-pinned by [`{id}.sha256`]({id}.sha256). This index resolves the evidence the receipt cites, the execution journal and every envelope to its repository path and SHA-256.\n\nArchived paths live under `{}/`, which is host-local and git-ignored. The committed checksum manifest pins the archive through its `SHA256SUMS` and `execution.log` rows; `SHA256SUMS` lists every archived file.\n\n| Evidence | Path | SHA-256 |\n|---|---|---|\n| Protocol | `{}` at commit `{}` | `{}` |\n| Declaration | `{}` | `{}` |\n",
        layout.archive(),
        declaration.protocol,
        identity.source_revision,
        identity.protocol_sha256.as_str(),
        identity.declaration_path,
        identity.declaration_sha256,
    );
    text.push_str(&cited_rows(
        stage,
        &layout,
        declaration,
        &cited_sources(declaration, identity.imported, stage)?,
    )?);
    text.push_str(&format!(
        "| Execution journal | `{}` | `{}` |\n",
        layout.execution_log(),
        Sha256Digest::of(&fs::read(stage.join("execution.log"))?).as_str()
    ));
    for owner in &declaration.measured_owners {
        let source = format!("{}-owner.json", owner.name.as_str());
        text.push_str(&format!(
            "| {} owner envelope | `{}` | `{}` |\n",
            owner.owner.as_str(),
            destination_for(&layout, declaration, &source)
                .ok_or_else(|| invalid("owner envelope has no destination"))?,
            Sha256Digest::of(&fs::read(stage.join(&source))?).as_str()
        ));
    }
    for imported in &declaration.imported_owners {
        text.push_str(&format!(
            "| {} owner envelope (imported) | `{}` | `{}` |\n",
            imported.owner.as_str(),
            imported.envelope.path,
            imported.envelope.sha256.as_str()
        ));
    }
    text.push_str(&format!(
        "| Complete envelope | `{}` | `{}` |\n| Receipt | `{}` | `{}` |\n\nRe-validation on the producing host: `python3 dev/scripts/validate-tuning-extent-campaign.py --stage <stage> --publication <checkout>`.\n",
        layout.complete(),
        Sha256Digest::of(&fs::read(stage.join("complete.json"))?).as_str(),
        layout.receipt(),
        Sha256Digest::of(&fs::read(stage.join("receipt.md"))?).as_str()
    ));
    Ok(text)
}
fn candidate_directory(
    stage: &Path,
    process: &str,
    log: &ExecutionLog,
) -> io::Result<std::path::PathBuf> {
    let root = stage.join("candidates");
    fs::create_dir_all(&root)?;
    File::open(stage)?.sync_all()?;
    let path = root.join(format!(
        "{process}-{}-{}",
        log.session_id(),
        log.next_sequence()
    ));
    fs::create_dir(&path)?;
    File::open(&root)?.sync_all()?;
    Ok(path)
}
fn promote(source: &ArtifactIdentity, target: &Path) -> io::Result<ArtifactIdentity> {
    if artifact(&source.path)? != *source {
        return Err(invalid("candidate changed before promotion"));
    }
    let bytes = fs::read(&source.path)?;
    publish_artifact(artifact_stage(target)?, target, &bytes)?;
    let result = artifact(target)?;
    if result.sha256 != source.sha256 {
        return Err(invalid("promotion changed candidate bytes"));
    }
    Ok(result)
}
fn emit(
    config: &CampaignConfig,
    manifests: &[OwnerManifest],
    bundles: &[AcceptedResultsBundle],
    log: &mut ExecutionLog,
    start: Instant,
) -> io::Result<SessionOutcome> {
    let stage = &config.channels.stage;
    let declaration =
        CampaignDeclaration::for_campaign(Path::new("."), config.campaign_id.as_str())?;
    let mut responses = Vec::new();
    for (i, owner) in declaration.measured_owners.iter().enumerate() {
        let name = owner.name.as_str();
        if !may_launch_child(ns(start)) {
            return Ok(SessionOutcome::BudgetExhausted);
        }
        bundles[i].validate(&manifests[i], true)?;
        let input = save(&stage.join(format!("{name}-accepted.json")), &bundles[i])?;
        let producer = process(config, &format!("{name}-producer"))?;
        let response_path = stage.join(format!("{name}-owner-response.json"));
        let response = if response_path.exists() {
            read_json(&response_path)?
        } else {
            let directory = candidate_directory(stage, producer.id.as_str(), log)?;
            let timestamp = utc()?;
            let measurement = ObservedProvenance {
                identity: config.identity.clone(),
                process: producer.id.clone(),
                observed_utc: timestamp.clone(),
                runtime: config.runtime.clone(),
            };
            let assembly = ObservedProvenance {
                identity: config.identity.clone(),
                process: producer.id.clone(),
                observed_utc: timestamp,
                runtime: CanonicalJson::from_serializable(&AssemblyRuntime {
                    source_dirty: false,
                    tool: format!("crates/gf2-{name}/benches/tuning_calibration.rs"),
                    tool_sha256: producer.executable_sha256.as_str().into(),
                })?,
            };
            let request = OwnerOperation::EmitOwner {
                request: Box::new(EmitOwnerRequest {
                    campaign_id: config.campaign_id.clone(),
                    manifest_sha256: manifests[i].manifest_sha256.clone(),
                    accepted_results: input,
                    measurement,
                    assembly,
                    output: directory.join("output.json"),
                }),
            };
            let request_artifact = save(&directory.join("request.json"), &request)?;
            log.append(
                JournalEvent::OwnerWrite,
                None,
                json!({"owner":manifests[i].owner,"request":request_artifact}),
            )?;
            let response = operation(&producer, &request, Some(log))?;
            let OwnerResponse::EmitOwner {
                artifact: output, ..
            } = &response
            else {
                return Err(invalid("owner emitted wrong response"));
            };
            if artifact(&directory.join("output.json"))? != *output {
                return Err(invalid("owner candidate path/digest differs from request"));
            }
            File::open(&output.path)?.sync_all()?;
            File::open(&directory)?.sync_all()?;
            save(&directory.join("response.json"), &response)?;
            save(&response_path, &response)?;
            response
        };
        let OwnerResponse::EmitOwner {
            artifact: owner,
            decisions,
        } = &response
        else {
            return Err(invalid("owner emission returned wrong operation"));
        };
        let canonical = promote(owner, &stage.join(format!("{name}-owner.json")))?;
        log.append(JournalEvent::OwnerReopen,None,json!({"owner":manifests[i].owner,"candidate":owner,"artifact":canonical,"decisions":decisions}))?;
        responses.push(response);
    }
    if !may_launch_child(ns(start)) {
        return Ok(SessionOutcome::BudgetExhausted);
    }
    let composer = process(config, "composer")?;
    let core = artifact(&stage.join("core-owner.json"))?;
    let algebra = artifact(&stage.join("algebra-owner.json"))?;
    let output = stage.join("complete.json");
    let candidate_record = stage.join("composition-candidate.json");
    let composition: Value = if candidate_record.exists() {
        read_json(&candidate_record)?
    } else {
        let directory = candidate_directory(stage, "composer", log)?;
        let candidate = directory.join("output.json");
        let args = vec![
            "complete".into(),
            core.path.to_string_lossy().into_owned(),
            algebra.path.to_string_lossy().into_owned(),
            candidate.to_string_lossy().into_owned(),
            config.campaign_id.as_str().into(),
            utc()?,
            config.identity.source_revision.clone(),
            "false".into(),
            composer.executable_sha256.as_str().into(),
        ];
        let request = save(
            &directory.join("request.json"),
            &json!({"schema":"tuning-campaign-composition-request-v1","process":composer,"args":args,"core":core,"algebra":algebra}),
        )?;
        log.append(
            JournalEvent::Composition,
            None,
            json!({"request":request,"args":args,"process":composer}),
        )?;
        record_budget(log, "before-launch", composer.id.as_str(), None)?;
        log.append(
            JournalEvent::OrchestrationStart,
            None,
            json!({"kind":"orchestration-start","process":"composer","request":request}),
        )?;
        let result = run_process(
            composer.command(&args)?,
            b"",
            Duration::from_secs(CHILD_TIMEOUT_SECONDS),
            Duration::from_secs(CHILD_KILL_GRACE_SECONDS),
            |_| Ok(()),
            |_| Ok(()),
        )?;
        let exit = save(
            &directory.join("exit.json"),
            &json!({"outcome":result.outcome,"stdout":result.stdout,"stderr":result.stderr}),
        )?;
        log.append(JournalEvent::OrchestrationExit,None,json!({"kind":"orchestration-exit","process":"composer","exit":exit,"outcome":result.outcome}))?;
        record_budget(log, "after-result", composer.id.as_str(), None)?;
        if !result.outcome.accepts_result()? {
            return Err(invalid("composition failed"));
        }
        File::open(&candidate)?.sync_all()?;
        File::open(&directory)?.sync_all()?;
        let composition = json!({"schema":"tuning-campaign-composition-v1","args":args,"source_revision":config.identity.source_revision,"source_dirty":false,"tool_sha256":composer.executable_sha256,"core":core,"algebra":algebra,"candidate":artifact(&candidate)?,"request":request,"exit":exit});
        save(&directory.join("response.json"), &composition)?;
        save(&candidate_record, &composition)?;
        composition
    };
    let candidate: ArtifactIdentity = serde_json::from_value(
        composition
            .get("candidate")
            .cloned()
            .ok_or_else(|| invalid("missing composition candidate"))?,
    )
    .map_err(invalid)?;
    let canonical = promote(&candidate, &output)?;
    let mut composition = composition;
    composition
        .as_object_mut()
        .ok_or_else(|| invalid("invalid composition record"))?
        .insert(
            "output".into(),
            serde_json::to_value(&canonical).map_err(invalid)?,
        );
    save(&stage.join("composition.json"), &composition)?;
    log.append(
        JournalEvent::CompositionReopen,
        None,
        json!({"candidate":candidate,"artifact":canonical}),
    )?;
    make_receipt(config, &declaration, bundles, &responses)?;
    if ns(start) > SESSION_BUDGET_SECONDS * 1_000_000_000 {
        return Ok(SessionOutcome::BudgetExhausted);
    }
    Ok(SessionOutcome::Complete)
}
fn recover_unit(
    config: &CampaignConfig,
    unit: &LaunchUnit,
    log: &mut ExecutionLog,
    checkpoints: &mut CheckpointStore,
    records: &[tuning_campaign_support::journal::JournalRecord],
) -> io::Result<AcceptedResult> {
    let exit = records
        .iter()
        .rev()
        .find(|record| {
            record.event == JournalEvent::ChildExit
                && record.details.get("unit_key").and_then(Value::as_str) == Some(unit.key.as_str())
        })
        .ok_or_else(|| invalid("missing recoverable exit"))?;
    let attempt: Token = serde_json::from_value(
        exit.details
            .get("attempt")
            .cloned()
            .ok_or_else(|| invalid("exit attempt missing"))?,
    )
    .map_err(invalid)?;
    let streams = records
        .iter()
        .rev()
        .find(|record| {
            record.event == JournalEvent::RawStreams
                && record
                    .details
                    .pointer("/exit/attempt")
                    .and_then(Value::as_str)
                    == Some(attempt.as_str())
                && record.case == serde_json::to_value(&unit.identity).ok()
        })
        .ok_or_else(|| invalid("clean exit has no durable raw stream artifacts"))?;
    let stdout: ArtifactIdentity = serde_json::from_value(
        streams
            .details
            .get("stdout")
            .cloned()
            .ok_or_else(|| invalid("raw stdout missing"))?,
    )
    .map_err(invalid)?;
    let stderr: ArtifactIdentity = serde_json::from_value(
        streams
            .details
            .get("stderr")
            .cloned()
            .ok_or_else(|| invalid("raw stderr missing"))?,
    )
    .map_err(invalid)?;
    for stream in [&stdout, &stderr] {
        if !stream
            .path
            .starts_with(config.channels.stage.join("raw-attempts"))
            || artifact(&stream.path)? != *stream
        {
            return Err(invalid("raw recovered stream path/digest changed"));
        }
    }
    let mut recovered = ChildAttempt::recover_exited(
        log,
        unit.clone(),
        attempt,
        &fs::read(&stdout.path)?,
        stderr.sha256,
    )?;
    let result = recovered
        .result()
        .cloned()
        .ok_or_else(|| invalid("recovered attempt has no result"))?;
    if !recovered.is_validated() {
        let response = operation(
            &process(config, unit.process.as_str())?,
            &OwnerOperation::ValidateResult {
                unit: Box::new(unit.clone()),
                result: Box::new(result.clone()),
            },
            Some(log),
        )?;
        recovered.owner_validated(log, &response)?;
    }
    let accepted = accept_checkpoint(log, checkpoints, unit, &result)?;
    log.append(
        JournalEvent::CellComplete,
        Some(serde_json::to_value(&unit.identity).map_err(invalid)?),
        json!({"key":unit.key,"checkpoint_sha256":accepted.sha256,"recovered":true}),
    )?;
    Ok(AcceptedResult {
        unit: unit.clone(),
        result,
        checkpoint_sha256: Sha256Digest::new(accepted.sha256)?,
    })
}
fn restore_derived_projection(
    stage: &Path,
    original: &OwnerManifest,
    request: &DeriveManifestRequest,
    observed: &DerivedManifest,
) -> io::Result<OwnerManifest> {
    let saved: DerivedManifest = read_json(&stage.join("derived-manifest.json"))?;
    if *observed != saved {
        return Err(invalid("owner derivation changed on resume"));
    }
    let resolved = original.apply_derivation(request, observed)?;
    save(&stage.join("core-resolved-manifest.json"), &resolved)?;
    Ok(resolved)
}
fn work(
    config: &CampaignConfig,
    log: &mut ExecutionLog,
    start: Instant,
) -> io::Result<SessionOutcome> {
    let mut manifests = resolved_manifests(config)?;
    let mut checkpoints = CheckpointStore::resume(
        &config.channels.checkpoints,
        config.campaign_id.as_str(),
        config.identity.clone(),
    )?;
    acknowledge_pending_recovery(log, &mut checkpoints, &config.campaign_id)?;
    let mut bundles = reconcile_campaign_checkpoints(log, &checkpoints, &manifests)?;
    let recovery_records = ExecutionLog::validate_prefix(
        &log.validated_synced_prefix()?,
        config.campaign_id.as_str(),
    )?;
    let recovery_keys: std::collections::BTreeSet<_> = recovery_records
        .iter()
        .filter(|record| record.event == JournalEvent::ChildExit)
        .filter_map(|record| {
            record
                .details
                .get("unit_key")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .collect();

    // Re-run derivation against the immutable inputs on resume before consuming
    // any derived opaque case; it must reproduce exactly the saved output.
    if config.channels.stage.join("derived-manifest.json").exists() {
        let request: DeriveManifestRequest =
            read_json(&config.channels.stage.join("derived-request.json"))?;
        if !may_launch_child(ns(start)) {
            return Ok(SessionOutcome::BudgetExhausted);
        }
        let OwnerResponse::DeriveManifest { manifest } = operation(
            &process(config, "core-producer")?,
            &OwnerOperation::DeriveManifest {
                request: request.clone(),
            },
            Some(log),
        )?
        else {
            return Err(invalid("invalid derived response"));
        };
        manifests[0] = restore_derived_projection(
            &config.channels.stage,
            &config.manifests[0],
            &request,
            &manifest,
        )?;
        bundles[0].manifest_sha256 = manifests[0].manifest_sha256.clone();
    }
    for owner in 0..manifests.len() {
        let mut phase = None::<Token>;
        for index in 0..manifests[owner].ordered_units.len() {
            let unit = &manifests[owner].ordered_units[index];
            if unit.identity.phase.as_str() == "m4rm-joint"
                && !config.channels.stage.join("derived-manifest.json").exists()
            {
                if !may_launch_child(ns(start)) {
                    return Ok(SessionOutcome::BudgetExhausted);
                }
                manifests[owner] = derive(config, &config.manifests[owner], &bundles[owner], log)?;
                bundles[owner].manifest_sha256 = manifests[owner].manifest_sha256.clone();
            }
            let unit = &manifests[owner].ordered_units[index];
            if checkpoints.completed_unit(unit.key.as_str()).is_some() {
                continue;
            }
            if !may_launch_child(ns(start)) {
                return Ok(SessionOutcome::BudgetExhausted);
            }
            if phase.as_ref() != Some(&unit.identity.phase) {
                if let Some(old) = phase.take() {
                    log.append(JournalEvent::PhaseComplete, None, json!({"phase":old}))?;
                }
                phase = Some(unit.identity.phase.clone());
                log.append(
                    JournalEvent::PhaseStart,
                    None,
                    json!({"phase":phase,"owner":manifests[owner].owner}),
                )?;
            }
            let accepted = if recovery_keys.contains(unit.key.as_str()) {
                recover_unit(config, unit, log, &mut checkpoints, &recovery_records)?
            } else {
                bounded_unit(config, unit, log, &mut checkpoints)?
            };
            bundles[owner].accepted.push(accepted);
            if !may_launch_child(ns(start)) {
                return Ok(SessionOutcome::BudgetExhausted);
            }
        }
        if let Some(old) = phase.take() {
            log.append(JournalEvent::PhaseComplete, None, json!({"phase":old}))?;
        }
    }
    emit(config, &manifests, &bundles, log, start)
}
fn run_session(stage: &Path, session_id: &str) -> io::Result<SessionOutcome> {
    let start = Instant::now();
    let config: CampaignConfig = read_json(&stage.join("campaign.json"))?;
    let descriptor: SessionDescriptor = read_json(&stage.join("active-session.json"))?;
    if descriptor.session_id.as_str() != session_id {
        return Err(invalid("stale or mismatched session handoff"));
    }
    let mut store = SessionStore::reopen(descriptor.clone())?;
    store.consume_mode(SessionMode::RunSession)?;
    let mut log = store.repair_log()?;
    store.reconcile_journal(&mut log)?;
    let holder = inherited_lock(&descriptor.lock_path)?;
    store.transition(
        &mut log,
        SessionTransition::LockHeld {
            evidence: LockEvidence {
                lock_path: descriptor.lock_path,
                holder_pid: holder,
                observation: Token::new("inherited-fd-and-independent-flock-conflict")?,
            },
        },
    )?;
    SESSION_START
        .set(start)
        .map_err(|_| invalid("duplicate run mode in one process"))?;
    let result = match (|| {
        let observed = CpuAffinity::observe()?;
        log.append(JournalEvent::DriverDiagnostic,None,json!({"kind":"cpu-affinity","phase":"held-lock","observed":observed,"expected":config.affinity}))?;
        log.append(JournalEvent::DriverDiagnostic,None,json!({"kind":"host-observation","phase":"held-lock","observed":HostObservation::observe()?}))?;
        require_affinity(&config.affinity, &observed)?;
        verify_config(&config)?;
        work(&config, &mut log, start)
    })() {
        Err(error) if error.kind() == io::ErrorKind::TimedOut => {
            Ok(SessionOutcome::BudgetExhausted)
        }
        other => other,
    };
    let outcome = match &result {
        Ok(outcome) => *outcome,
        Err(error) => {
            log.append(
                JournalEvent::DriverDiagnostic,
                None,
                json!({"driver_error":error.to_string()}),
            )?;
            SessionOutcome::Failed
        }
    };
    if !ALL_REAPED.load(std::sync::atomic::Ordering::SeqCst) {
        return Err(invalid(
            "process cleanup not proven; finalizer must independently prove release",
        ));
    }
    store.transition(
        &mut log,
        SessionTransition::WorkFinished {
            evidence: WorkEvidence {
                outcome,
                all_descendants_reaped: true,
                active_elapsed_ns: ns(start),
            },
        },
    )?;
    result
}
fn session_records(
    log: &mut ExecutionLog,
    descriptor: &SessionDescriptor,
) -> io::Result<Vec<tuning_campaign_support::journal::JournalRecord>> {
    Ok(ExecutionLog::validate_prefix(
        &log.validated_synced_prefix()?,
        descriptor.campaign_id.as_str(),
    )?
    .into_iter()
    .filter(|r| r.session_id == descriptor.session_id.as_str())
    .collect())
}
fn proof(
    descriptor: &SessionDescriptor,
    log: &mut ExecutionLog,
) -> io::Result<IndependentReleaseEvidence> {
    let records = session_records(log, descriptor)?;
    let mut holder = None;
    let mut groups = Vec::new();
    for record in records {
        if record.event == JournalEvent::LockHold {
            if let Some(pid) = record
                .details
                .pointer("/session_transition/transition/evidence/holder_pid")
                .and_then(Value::as_u64)
            {
                holder = Some(pid as u32);
            }
        }
        if record.event == JournalEvent::ChildSpawn {
            if let Some(pid) = record.details.get("pid").and_then(Value::as_u64) {
                groups.push(pid as u32);
            }
        }
    }
    let holder = holder.ok_or_else(|| invalid("release proof lacks observed holder"))?;
    Ok(IndependentReleaseEvidence {
        holder_dead: !Path::new(&format!("/proc/{holder}")).exists(),
        descendants_dead: groups
            .into_iter()
            .map(live_group)
            .collect::<io::Result<Vec<_>>>()?
            .iter()
            .all(|live| !*live),
        lock_path: descriptor.lock_path.clone(),
        observed_utc: utc()?,
        lock_available: lock_available(&descriptor.lock_path)?,
    })
}
/// Regular files below `directory`, skipping entries named in `skip` at any
/// depth; a symlink is rejected.
fn stage_files(directory: &Path, skip: &[&str], files: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        if skip.iter().any(|name| entry.file_name() == *name) {
            continue;
        }
        if entry.file_type()?.is_symlink() {
            return Err(invalid("symlink in staged evidence"));
        }
        if path.is_dir() {
            stage_files(&path, skip, files)?;
        } else {
            files.push(path);
        }
    }
    Ok(())
}
/// Files the final session checksum pins: everything except the journal,
/// the active claims, session control state and publication journals.
fn checksum_boundary(stage: &Path) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    stage_files(
        stage,
        &[
            "execution.log",
            "active-session.json",
            "active-preparation.json",
            "session-writer.lock",
            "sessions",
            "artifact-publications",
            REPOSITORY_PUBLICATION_DIR,
        ],
        &mut files,
    )?;
    Ok(files)
}
fn finish_checksum(store: SessionStore, log: &mut ExecutionLog) -> io::Result<()> {
    let stage = store.descriptor().channels.stage.clone();
    let artifacts = checksum_boundary(&stage)?
        .iter()
        .map(|path| artifact(path))
        .collect::<io::Result<Vec<_>>>()?;
    let identity = store.write_checksum(log, artifacts)?;
    store.retire(log, &identity)
}
/// Runs the independent validator with `arguments` after `--stage STAGE` and
/// returns its stdout, which is also forwarded with its stderr.
fn run_validator(validator: &Path, stage: &Path, arguments: &[&OsStr]) -> io::Result<Vec<u8>> {
    let output = Command::new("python3")
        .arg(validator)
        .arg("--stage")
        .arg(stage)
        .args(arguments)
        .output()?;
    io::stdout().write_all(&output.stdout)?;
    io::stderr().write_all(&output.stderr)?;
    if !output.status.success() {
        return Err(invalid("independent validation failed"));
    }
    Ok(output.stdout)
}
fn validator(config: &CampaignConfig, preterminal: bool) -> io::Result<()> {
    let arguments: &[&OsStr] = if preterminal {
        &[OsStr::new("--preterminal")]
    } else {
        &[]
    };
    run_validator(
        &fs::canonicalize(VALIDATOR)?,
        &config.channels.stage,
        arguments,
    )
    .map(drop)
}
fn observed_failure(records: &[tuning_campaign_support::journal::JournalRecord]) -> bool {
    records.iter().any(|record| {
        record.details.get("driver_error").is_some()
            || (record.event == JournalEvent::RawStreams
                && serde_json::from_value::<ChildCompletionEvidence>(record.details.clone())
                    .map_or(true, |evidence| {
                        !evidence.stream_validation.accepts_result()
                    }))
            || (record.event == JournalEvent::WorkFinished
                && record
                    .details
                    .pointer("/session_transition/transition/evidence/outcome")
                    .and_then(Value::as_str)
                    == Some("failed"))
            || (record.event == JournalEvent::ChildExit
                && record
                    .details
                    .get("outcome")
                    .cloned()
                    .and_then(|v| serde_json::from_value::<ProcessOutcome>(v).ok())
                    .is_some_and(|outcome| !outcome.accepts_result().unwrap_or(false)))
    })
}
fn finalize(stage: &Path, session_id: &str, exit_code: i32) -> io::Result<SessionOutcome> {
    let config: CampaignConfig = read_json(&stage.join("campaign.json"))?;
    let descriptor: SessionDescriptor = read_json(&stage.join("active-session.json"))?;
    if descriptor.session_id.as_str() != session_id {
        return Err(invalid("stale or mismatched session handoff"));
    }
    let mut store = SessionStore::reopen(descriptor.clone())?;
    store.consume_mode(SessionMode::FinalizeSession)?;
    let mut log = store.repair_log()?;
    store.reconcile_journal(&mut log)?;
    let before = store.lifecycle().state();
    let records = session_records(&mut log, &descriptor)?;
    let proposed = records
        .iter()
        .rev()
        .find_map(|record| {
            if record.event == JournalEvent::WorkFinished {
                record
                    .details
                    .pointer("/session_transition/transition/evidence/outcome")
                    .cloned()
            } else {
                None
            }
        })
        .map(serde_json::from_value::<SessionOutcome>)
        .transpose()
        .map_err(invalid)?
        .unwrap_or(SessionOutcome::Failed);
    store.transition(
        &mut log,
        SessionTransition::WrapperReturned {
            evidence: WrapperEvidence {
                exit_code: Some(exit_code),
                signal: None,
            },
        },
    )?;
    if before != SessionState::Prepared {
        let evidence = proof(&descriptor, &mut log)?;
        if !evidence.holder_dead || !evidence.descendants_dead || !evidence.lock_available {
            store.transition(&mut log, SessionTransition::ReleaseUnobserved)?;
            return Err(invalid(
                "lock release remains unobserved; active session retained",
            ));
        }
        store.transition(
            &mut log,
            SessionTransition::LockRelease {
                evidence: ReleaseEvidence::Independent { evidence },
            },
        )?;
    }
    if before == SessionState::LockHeld && !observed_failure(&records) {
        store.recover_transition(
            &mut log,
            SessionTransition::Interrupted {
                active_elapsed_censored: true,
            },
        )?;
        finish_checksum(store, &mut log)?;
        return Ok(SessionOutcome::Paused);
    }
    let mut outcome = if exit_code == 0 {
        proposed
    } else {
        SessionOutcome::Failed
    };
    let manifests = resolved_manifests(&config)?;
    let mut checkpoints = CheckpointStore::resume(
        &config.channels.checkpoints,
        config.campaign_id.as_str(),
        config.identity.clone(),
    )?;
    acknowledge_pending_recovery(&mut log, &mut checkpoints, &config.campaign_id)?;
    let bundles = reconcile_campaign_checkpoints(&mut log, &checkpoints, &manifests)?;
    if outcome == SessionOutcome::Complete {
        for (bundle, manifest) in bundles.iter().zip(&manifests) {
            bundle.validate(manifest, true)?;
        }
        if let Err(error) = validator(&config, true) {
            log.append(
                JournalEvent::DriverDiagnostic,
                None,
                json!({"validation_error":error.to_string()}),
            )?;
            outcome = SessionOutcome::Failed;
        }
    }
    store.transition(&mut log, SessionTransition::Terminal { outcome })?;
    finish_checksum(store, &mut log)?;
    if outcome == SessionOutcome::Complete {
        validator(&config, false)?;
    }
    Ok(outcome)
}
fn writer_dead(writer: &ProcessIdentity) -> io::Result<bool> {
    writer.validate()?;
    let boot = fs::read_to_string("/proc/sys/kernel/random/boot_id")?;
    if boot.trim() != writer.boot_id.as_str() {
        return Ok(true);
    }
    let stat = match fs::read_to_string(format!("/proc/{}/stat", writer.pid)) {
        Ok(stat) => stat,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(true),
        Err(error) => return Err(error),
    };
    let (_, tail) = stat
        .rsplit_once(')')
        .ok_or_else(|| invalid("invalid process stat"))?;
    let ticks = tail
        .split_whitespace()
        .nth(19)
        .ok_or_else(|| invalid("missing start ticks"))?
        .parse::<u64>()
        .map_err(invalid)?;
    Ok(ticks != writer.start_time_ticks)
}
fn recover(stage: &Path) -> io::Result<()> {
    let descriptor: SessionDescriptor = read_json(&stage.join("active-session.json"))?;
    let mut store = SessionStore::reopen(descriptor.clone())?;
    let mut log = store.repair_log()?;
    store.reconcile_journal(&mut log)?;
    let state = store.lifecycle().state();
    if matches!(state, SessionState::Terminal | SessionState::Interrupted) {
        return finish_checksum(store, &mut log);
    }
    if state == SessionState::Prepared {
        let writers = store
            .writer_identities()?
            .into_iter()
            .map(|writer| {
                Ok(WriterDeathEvidence {
                    writer_dead: writer_dead(&writer)?,
                    writer,
                })
            })
            .collect::<io::Result<Vec<_>>>()?;
        let evidence = PrelockInterruptionEvidence {
            writers,
            lock_path: descriptor.lock_path.clone(),
            lock_available: lock_available(&descriptor.lock_path)?,
            observed_utc: utc()?,
            active_elapsed_censored: true,
        };
        store.recover_transition(&mut log, SessionTransition::PrelockInterrupted { evidence })?;
        return finish_checksum(store, &mut log);
    }
    let evidence = proof(&descriptor, &mut log)?;
    if !evidence.holder_dead || !evidence.descendants_dead || !evidence.lock_available {
        if state != SessionState::ReleaseUnobserved {
            store.recover_transition(&mut log, SessionTransition::ReleaseUnobserved)?;
        }
        return Err(invalid(
            "prior process/lock remains live or release unobserved",
        ));
    }
    if state != SessionState::LockReleased {
        store.recover_transition(
            &mut log,
            SessionTransition::LockRelease {
                evidence: ReleaseEvidence::Independent { evidence },
            },
        )?;
    }
    if observed_failure(&session_records(&mut log, &descriptor)?) {
        store.recover_transition(
            &mut log,
            SessionTransition::Terminal {
                outcome: SessionOutcome::Failed,
            },
        )?;
    } else {
        store.recover_transition(
            &mut log,
            SessionTransition::Interrupted {
                active_elapsed_censored: true,
            },
        )?;
    }
    finish_checksum(store, &mut log)
}
fn discover_preparation(stage: &Path) -> io::Result<()> {
    let identity = PreparationStore::discover_pending(stage)?;
    let mut output = io::stdout().lock();
    serde_json::to_writer(&mut output, &identity).map_err(invalid)?;
    writeln!(output)?;
    output.flush()
}
const TUNING_EVIDENCE: &str = "dev/benchmarks/tuning_profiles";
/// Stage records small enough to commit in the session record. Every other
/// published stage file, bulk evidence and the execution log included, goes
/// to the git-ignored evidence archive.
const COMMITTED_SESSION_RECORDS: [&str; 10] = [
    "algebra-capability-report.json",
    "algebra-list-grid.json",
    "algebra-self-check.json",
    "build/source-after.json",
    "build/source-before.json",
    "composition.json",
    "core-capability-report.json",
    "core-list-grid.json",
    "core-self-check.json",
    "staging-manifest.json",
];
/// Protocol §9 repository destinations, all derived from the run ID.
struct RepositoryLayout<'a> {
    id: &'a str,
}
impl RepositoryLayout<'_> {
    fn core_owner(&self) -> String {
        format!("crates/gf2-core/data/tuning-profiles/{}.json", self.id)
    }
    fn algebra_owner(&self) -> String {
        format!("crates/gf2-algebra/data/tuning-profiles/{}.json", self.id)
    }
    fn complete(&self) -> String {
        format!("dev/reference_data/tuning-profiles/{}.json", self.id)
    }
    fn receipt(&self) -> String {
        format!("{TUNING_EVIDENCE}/{}.md", self.id)
    }
    fn evidence_index(&self) -> String {
        format!("{TUNING_EVIDENCE}/{}-evidence.md", self.id)
    }
    fn checksum(&self) -> String {
        format!("{TUNING_EVIDENCE}/{}.sha256", self.id)
    }
    fn session(&self) -> String {
        format!("{TUNING_EVIDENCE}/{}-session", self.id)
    }
    fn archive(&self) -> String {
        format!(".agents/campaign-evidence/{}", self.id)
    }
    fn execution_log(&self) -> String {
        format!("{}/execution.log", self.archive())
    }
}
/// Stage path of the evidence index rendered at publication.
const EVIDENCE_INDEX: &str = "repository-publication/evidence-index.md";
/// The protocol §9 destination of one published stage file: the measured
/// envelopes and receipt to their committed rows, the declared small records,
/// validation record and evidence index to committed paths, and every other
/// stage file, an imported owner's staged copy included, to the same relative
/// path in the archive. Staged executables under `bin/` are pinned by digest
/// in the staging manifest and have no destination.
fn destination_for(
    layout: &RepositoryLayout,
    declaration: &CampaignDeclaration,
    source: &str,
) -> Option<String> {
    Some(match source {
        "core-owner.json" => layout.core_owner(),
        "algebra-owner.json" if declaration.imported("algebra").is_none() => layout.algebra_owner(),
        "complete.json" => layout.complete(),
        "receipt.md" => layout.receipt(),
        "repository-publication/validation.json" => {
            format!("{}/validation.json", layout.session())
        }
        EVIDENCE_INDEX => layout.evidence_index(),
        other if other.starts_with("bin/") => return None,
        other if COMMITTED_SESSION_RECORDS.contains(&other) => {
            format!("{}/{other}", layout.session())
        }
        other => format!("{}/{other}", layout.archive()),
    })
}
/// Maps every published stage file to its [`destination_for`].
fn repository_mapping(
    stage: &Path,
    layout: &RepositoryLayout,
    declaration: &CampaignDeclaration,
) -> io::Result<BTreeMap<String, String>> {
    let mut files = checksum_boundary(stage)?;
    stage_files(&stage.join("sessions"), &[], &mut files)?;
    files.push(stage.join("execution.log"));
    files.push(
        stage
            .join(REPOSITORY_PUBLICATION_DIR)
            .join("validation.json"),
    );
    if declaration.evidence_index {
        files.push(stage.join(EVIDENCE_INDEX));
    }
    let mut mapping = BTreeMap::new();
    for path in files {
        let source = path
            .strip_prefix(stage)
            .map_err(invalid)?
            .to_str()
            .ok_or_else(|| invalid("non-UTF-8 stage path"))?
            .to_owned();
        let Some(destination) = destination_for(layout, declaration, &source) else {
            continue;
        };
        if mapping.insert(destination, source).is_some() {
            return Err(invalid("two stage files map to one repository destination"));
        }
    }
    let mut required = vec![
        layout.core_owner(),
        layout.complete(),
        layout.receipt(),
        layout.execution_log(),
    ];
    if declaration.imported("algebra").is_none() {
        required.push(layout.algebra_owner());
    }
    if declaration.evidence_index {
        required.push(layout.evidence_index());
    }
    for required in required {
        if !mapping.contains_key(&required) {
            return Err(invalid(format!("stage lacks the source of {required}")));
        }
    }
    Ok(mapping)
}
/// The composer's profile/provenance arguments from the accepted composition.
fn reopen_arguments(stage: &Path, campaign_id: &Token) -> io::Result<Vec<String>> {
    let composition: Value = read_json(&stage.join("composition.json"))?;
    let args: Vec<String> = serde_json::from_value(
        composition
            .get("args")
            .cloned()
            .ok_or_else(|| invalid("composition lacks its arguments"))?,
    )
    .map_err(invalid)?;
    if args.len() != 9 || args[0] != "complete" || args[4] != campaign_id.as_str() {
        return Err(invalid("composition arguments are not the declared form"));
    }
    Ok(args[4..].to_vec())
}
/// Strictly reopens the three published envelopes through the staged
/// composer's complete loader: both owners with their owner-only codecs and
/// the recomposed complete envelope with both. Its output must equal the
/// published complete envelope byte for byte; it is verification evidence
/// only and never a publishable artifact.
fn reopen_published(
    stage: &Path,
    repository: &Path,
    layout: &RepositoryLayout,
    declaration: &CampaignDeclaration,
    composer: &ProcessDescriptor,
    profile_arguments: &[String],
) -> io::Result<Vec<u8>> {
    let algebra = declaration.imported("algebra").map_or_else(
        || layout.algebra_owner(),
        |imported| imported.envelope.path.clone(),
    );
    let published = [layout.core_owner(), algebra, layout.complete()];
    let identities = published
        .iter()
        .map(|path| {
            Ok(json!({"path":path,"sha256":Sha256Digest::of(&fs::read(repository.join(path))?)}))
        })
        .collect::<io::Result<Vec<_>>>()?;
    let attempts = stage.join(REPOSITORY_PUBLICATION_DIR).join("reopen");
    fs::create_dir_all(&attempts)?;
    File::open(stage.join(REPOSITORY_PUBLICATION_DIR))?.sync_all()?;
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_nanos();
    let directory = attempts.join(format!("{}-{nonce}", std::process::id()));
    fs::create_dir(&directory)?;
    File::open(&attempts)?.sync_all()?;
    let output = directory.join("complete.json");
    let mut args = vec!["complete".to_owned()];
    for path in [
        repository.join(&published[0]),
        repository.join(&published[1]),
        output.clone(),
    ] {
        args.push(
            path.to_str()
                .ok_or_else(|| invalid("non-UTF-8 reopen path"))?
                .to_owned(),
        );
    }
    args.extend(profile_arguments.iter().cloned());
    let result = run_process(
        composer.command(&args)?,
        b"",
        Duration::from_secs(CHILD_TIMEOUT_SECONDS),
        Duration::from_secs(CHILD_KILL_GRACE_SECONDS),
        |_| Ok(()),
        |_| Ok(()),
    )?;
    save(
        &directory.join("exit.json"),
        &json!({"args":args,"outcome":result.outcome,"stdout":result.stdout,"stderr":result.stderr}),
    )?;
    if !result.outcome.accepts_result()? || result.callback_error.is_some() {
        return Err(invalid(
            "published envelopes failed the composer's strict reopen",
        ));
    }
    if fs::read(&output)? != fs::read(repository.join(&published[2]))? {
        return Err(invalid(
            "strictly reopened composition differs from the published complete envelope",
        ));
    }
    encoded(&json!({
        "schema":"tuning-campaign-repository-reopen-v1",
        "composer_sha256":composer.executable_sha256,
        "profile_arguments":profile_arguments,
        "core":identities[0],
        "algebra":identities[1],
        "complete":identities[2],
    }))
}
/// Publishes a complete, validated stage into `repository`: plans the
/// protocol §9 destinations, copies them through the durable publication
/// journal, strictly reopens the published envelopes, has the independent
/// validator check the publication, then writes the checksum manifest.
#[allow(clippy::too_many_arguments)]
fn publish_to_repository(
    stage: &Path,
    repository: &Path,
    campaign_id: &Token,
    declaration: &CampaignDeclaration,
    validation: &[u8],
    evidence_index: Option<&[u8]>,
    composer: &ProcessDescriptor,
    validator: &Path,
) -> io::Result<ArtifactIdentity> {
    let journal = stage.join(REPOSITORY_PUBLICATION_DIR);
    fs::create_dir_all(&journal)?;
    File::open(stage)?.sync_all()?;
    publish_artifact(stage, &journal.join("validation.json"), validation)?;
    match (declaration.evidence_index, evidence_index) {
        (true, Some(index)) => {
            publish_artifact(stage, &stage.join(EVIDENCE_INDEX), index)?;
        }
        (false, None) => {}
        _ => {
            return Err(invalid(
                "evidence index presence differs from the declaration",
            ))
        }
    }
    let layout = RepositoryLayout {
        id: campaign_id.as_str(),
    };
    let profile_arguments = reopen_arguments(stage, campaign_id)?;
    let plan = RepositoryPlan::new(
        stage,
        campaign_id.clone(),
        repository_mapping(stage, &layout, declaration)?,
        vec![layout.session()],
        layout.archive(),
        vec![layout.execution_log()],
        format!("{}/repository-reopen.json", layout.session()),
        layout.checksum(),
    )?;
    publish_repository(stage, repository, &plan, |repository| {
        let evidence = reopen_published(
            stage,
            repository,
            &layout,
            declaration,
            composer,
            &profile_arguments,
        )?;
        run_validator(
            validator,
            stage,
            &[OsStr::new("--publication"), repository.as_os_str()],
        )?;
        Ok(evidence)
    })
}
/// Post-finalization publication of a complete campaign into the repository
/// checkout the driver runs in, checked by that checkout's validator. Returns
/// `None` while the campaign is not complete. Idempotent: a retry after any
/// interruption finishes the rest.
fn publish_campaign(stage: &Path) -> io::Result<Option<ArtifactIdentity>> {
    let config: CampaignConfig = read_json(&stage.join("campaign.json"))?;
    let composer = process(&config, "composer")?;
    let repository = fs::canonicalize(".")?;
    let validator_path = repository.join(VALIDATOR);
    let declaration = CampaignDeclaration::for_campaign(&repository, config.campaign_id.as_str())?;
    validate_campaign_stage(stage, config.campaign_id.as_str(), &declaration)?;
    if config.channels.stage != stage {
        return Err(invalid("campaign channels name another stage"));
    }
    if !campaign_complete(&config.channels, &config.campaign_id)? {
        return Ok(None);
    }
    let stdout = run_validator(&validator_path, stage, &[])?;
    let validation = stdout
        .strip_prefix(b"GF2_TUNING_VALIDATION=")
        .and_then(|line| line.strip_suffix(b"\n"))
        .ok_or_else(|| invalid("validator output is not one validation record"))?;
    let record: Value = serde_json::from_slice(validation).map_err(invalid)?;
    if record.get("status").and_then(Value::as_str) != Some("complete-valid")
        || record.get("campaign_id").and_then(Value::as_str) != Some(config.campaign_id.as_str())
    {
        return Err(invalid("validation record is not this complete campaign"));
    }
    let evidence_index = declaration
        .evidence_index
        .then(|| {
            render_evidence_index(
                stage,
                &EvidenceIdentity::of(&config, &declaration)?,
                &declaration,
            )
        })
        .transpose()?;
    publish_to_repository(
        stage,
        &repository,
        &config.campaign_id,
        &declaration,
        validation,
        evidence_index.as_deref().map(str::as_bytes),
        &composer,
        &validator_path,
    )
    .map(Some)
}
fn main() {
    let args: Vec<_> = env::args().skip(1).collect();
    if let [mode, stage] = args.as_slice() {
        if mode == "publish-campaign" {
            match publish_campaign(Path::new(stage)) {
                Ok(Some(manifest)) => {
                    println!("GF2_CAMPAIGN_PUBLICATION={}", manifest.path.display());
                    return;
                }
                Ok(None) => {
                    eprintln!("tuning-extent-campaign-driver: campaign is not complete; nothing to publish");
                    std::process::exit(3);
                }
                Err(error) => {
                    eprintln!("tuning-extent-campaign-driver: {error}");
                    std::process::exit(1);
                }
            }
        }
    }
    let result=match args.as_slice(){
        [mode,stage] if mode=="discover-preparation"=>discover_preparation(Path::new(stage)).map(|_|SessionOutcome::Paused),
        [mode,stage,campaign,session,lock,staging] if mode=="prepare-session"=>prepare(Path::new(stage),campaign,session,Path::new(lock),Some(Path::new(staging))).map(|_|SessionOutcome::Paused),
        [mode,stage,campaign,session,lock] if mode=="prepare-session"=>prepare(Path::new(stage),campaign,session,Path::new(lock),None).map(|_|SessionOutcome::Paused),
        [mode,stage,session] if mode=="run-session"=>run_session(Path::new(stage),session),
        [mode,stage,session,status] if mode=="finalize-session"=>status.parse::<i32>().map_err(invalid).and_then(|status|finalize(Path::new(stage),session,status)),
        _=>Err(invalid("usage: driver discover-preparation STAGE | prepare-session STAGE CAMPAIGN SESSION LOCK [STAGING_INPUT] | run-session STAGE SESSION | finalize-session STAGE SESSION WRAPPER_EXIT | publish-campaign STAGE"))
    };
    match result {
        Ok(SessionOutcome::Failed) => std::process::exit(1),
        Ok(_) => {}
        Err(error) => {
            eprintln!("tuning-extent-campaign-driver: {error}");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tuning_campaign_support::repository::repository_root;
    use tuning_campaign_support::scratch::{scratch, Scratch};

    /// A recorded declaration path distinct from the located one: receipts
    /// and evidence indexes name the declaration as the campaign recorded it.
    const RECORDED_DECLARATION: &str = "recorded/campaign-declaration.json";

    /// The committed declaration of the published extent campaign.
    fn extent_declaration() -> CampaignDeclaration {
        CampaignDeclaration::for_campaign(
            &repository_root().unwrap(),
            "gf2-a83583e0-19700101t000000z-1",
        )
        .unwrap()
    }
    /// The committed declaration of the seam calibration campaign.
    fn seam_declaration() -> CampaignDeclaration {
        CampaignDeclaration::for_campaign(
            &repository_root().unwrap(),
            "gf2-dbd8787d-19700101t000000z-1",
        )
        .unwrap()
    }

    #[test]
    fn staged_process_receives_only_the_declared_measurement_environment() {
        let executable = artifact(Path::new("/usr/bin/env")).unwrap();
        let process = ProcessDescriptor {
            id: Token::new("environment-probe").unwrap(),
            executable: executable.path,
            executable_sha256: executable.sha256,
            arguments: vec![],
            environment: measurement_environment(),
            working_directory: fs::canonicalize(".").unwrap(),
        };
        let output = process.command(&[]).unwrap().output().unwrap();
        assert!(output.status.success());
        let actual: BTreeMap<String, String> = String::from_utf8(output.stdout)
            .unwrap()
            .lines()
            .map(|line| {
                let (key, value) = line.split_once('=').unwrap();
                (key.to_owned(), value.to_owned())
            })
            .collect();
        assert!(
            actual == process.environment,
            "child inherited undeclared environment"
        );
    }

    #[test]
    fn staged_reporting_preflights_use_their_actual_cli_flags_and_empty_stdin() {
        use std::os::unix::fs::PermissionsExt;

        let stage = scratch("gf2-driver-report-cli");
        let executable = stage.join("owner");
        fs::write(
            &executable,
            b"#!/bin/sh\ninput=$(cat)\ntest -z \"$input\" || exit 8\ntest \"$#\" = 1 || exit 9\ncase \"$1\" in\n  --self-check) operation=self-check ;;\n  --list-grid) operation=list-grid ;;\n  --capability-report) operation=capability-report ;;\n  *) exit 10 ;;\nesac\nprintf 'GF2_TUNING_RESULT={\"operation\":\"%s\",\"evidence\":\"{}\"}' \"$operation\"\n",
        )
        .unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
        let executable = artifact(&executable).unwrap();
        let process = ProcessDescriptor {
            id: Token::new("report-owner").unwrap(),
            executable: executable.path,
            executable_sha256: executable.sha256,
            arguments: vec!["--owner-operation".into()],
            environment: measurement_environment(),
            working_directory: fs::canonicalize(".").unwrap(),
        };
        for label in ["self-check", "list-grid", "capability-report"] {
            let response = report_cli(&process, label, None).unwrap();
            verify_report_response(label, &response).unwrap();
        }
        assert!(report_cli(&process, "unknown", None).is_err());

        let mut log = ExecutionLog::create_new(&stage, "campaign", "session").unwrap();
        log.append(JournalEvent::CampaignStart, None, json!({}))
            .unwrap();
        report_cli(&process, "self-check", Some(&mut log)).unwrap();
        let records =
            ExecutionLog::validate_prefix(&fs::read(log.path()).unwrap(), "campaign").unwrap();
        let start = records
            .iter()
            .find(|record| record.event == JournalEvent::OrchestrationStart)
            .unwrap();
        let exit = records
            .iter()
            .find(|record| record.event == JournalEvent::OrchestrationExit)
            .unwrap();
        assert_eq!(
            exit.details["start_sequence"].as_u64(),
            Some(start.sequence)
        );
    }

    #[test]
    fn incomplete_executable_staging_replays_before_campaign_config_exists() {
        let stage = scratch("gf2-driver-staging-replay");
        fs::create_dir(stage.join("build")).unwrap();
        let source_revision = command_text("git", &["rev-parse", "HEAD"]).unwrap();
        let source_tree = command_text("git", &["rev-parse", "HEAD^{tree}"]).unwrap();
        let before_path = stage.join("build/source-before.json");
        let after_path = stage.join("build/source-after.json");
        fs::write(
            &before_path,
            encoded(&BuildSourceObservation {
                schema: "tuning-campaign-build-source-v1".into(),
                phase: "before-build".into(),
                source_revision: source_revision.clone(),
                source_tree: source_tree.clone(),
                porcelain: String::new(),
            })
            .unwrap(),
        )
        .unwrap();
        fs::write(
            &after_path,
            encoded(&BuildSourceObservation {
                schema: "tuning-campaign-build-source-v1".into(),
                phase: "after-build".into(),
                source_revision,
                source_tree,
                porcelain: String::new(),
            })
            .unwrap(),
        )
        .unwrap();
        let source = stage.join("built-driver");
        fs::write(&source, b"unchanged staged executable").unwrap();
        let source = artifact(&source).unwrap();
        let staging = StagingManifest {
            schema: "tuning-campaign-staging-v1".into(),
            source_before: artifact(&before_path).unwrap(),
            source_after: artifact(&after_path).unwrap(),
            executables: ["algebra-producer", "composer", "core-producer", "driver"]
                .into_iter()
                .map(|name| (Token::new(name).unwrap(), source.clone()))
                .collect(),
        };
        let input = stage.join("build-input.json");
        fs::write(&input, encoded(&staging).unwrap()).unwrap();
        fs::create_dir(stage.join("bin")).unwrap();
        publish_artifact(
            &stage,
            &stage.join("bin/algebra-producer"),
            &fs::read(&source.path).unwrap(),
        )
        .unwrap();
        stage_executables(&stage, &input, &extent_declaration()).unwrap();
        assert!(!stage.join("campaign.json").exists());
        for name in staging.executables.keys() {
            assert_eq!(
                artifact(&stage.join("bin").join(name.as_str()))
                    .unwrap()
                    .sha256,
                source.sha256
            );
        }
        stage_executables(&stage, &input, &extent_declaration()).unwrap();
        assert!(
            stage_executables(&stage, &input, &seam_declaration()).is_err(),
            "the seam campaign stages no algebra producer"
        );
        fs::write(&source.path, b"changed producer").unwrap();
        assert!(stage_executables(&stage, &input, &extent_declaration()).is_err());
    }

    #[test]
    fn relocated_build_source_observation_is_rejected_before_campaign_work() {
        let stage = scratch("gf2-driver-relocated-build-source");
        fs::create_dir(stage.join("build")).unwrap();
        let relocated = stage.join("source-before.json");
        fs::write(&relocated, b"{}").unwrap();
        let expected_after = stage.join("build/source-after.json");
        fs::write(&expected_after, b"{}").unwrap();
        let staging = StagingManifest {
            schema: "tuning-campaign-staging-v1".into(),
            source_before: artifact(&relocated).unwrap(),
            source_after: artifact(&expected_after).unwrap(),
            executables: BTreeMap::new(),
        };

        assert!(validate_build_source_observations(&stage, &staging).is_err());
        assert!(!stage.join("execution.log").exists());
        assert!(!stage.join("active-session.json").exists());
    }

    #[test]
    fn relocated_preflight_report_is_rejected_before_campaign_work() {
        let stage = scratch("gf2-driver-relocated-preflight");
        let mut reports = BTreeMap::new();
        for owner in ["core", "algebra"] {
            for mode in ["self-check", "list-grid", "capability-report"] {
                let name = format!("{owner}-{mode}");
                let path = stage.join(format!("{name}.json"));
                fs::write(&path, b"{}").unwrap();
                reports.insert(name, artifact(&path).unwrap());
            }
        }
        let relocated = stage.join("relocated-core-list-grid.json");
        fs::write(&relocated, b"{}").unwrap();
        reports.insert("core-list-grid".into(), artifact(&relocated).unwrap());

        assert!(validate_preflight_identities(&stage, &reports, &extent_declaration()).is_err());
        assert!(!stage.join("execution.log").exists());
        assert!(!stage.join("active-session.json").exists());
    }
    #[test]
    fn affinity_is_observed_from_the_current_os_mask_and_binds_resume() {
        let affinity = CpuAffinity::observe().unwrap();
        assert!(!affinity.cpus().is_empty());
        let cpu = affinity.cpus()[0];
        let output=Command::new("taskset").args(["-c",&cpu.to_string(),"python3","-c","print(next(x.split(':',1)[1].strip() for x in open('/proc/self/status') if x.startswith('Cpus_allowed_list:')))"]).output().unwrap();
        assert!(output.status.success());
        let restricted =
            CpuAffinity::parse(std::str::from_utf8(&output.stdout).unwrap().trim()).unwrap();
        assert_eq!(restricted.cpus(), &[cpu]);
        assert!(CpuAffinity::parse("2-1").is_err());
        assert!(CpuAffinity::parse("").is_err());
        let changed = CpuAffinity::try_from(vec![cpu + 1]).unwrap();
        assert!(require_affinity(&restricted, &changed).is_err());
    }
    fn probe_fixture(
        stderr: &[u8],
    ) -> (
        Scratch,
        CampaignConfig,
        LaunchUnit,
        ChildResult,
        ExecutionLog,
        CheckpointStore,
    ) {
        use std::os::unix::fs::PermissionsExt;
        let stage = scratch("gf2-driver-probe-checkpoint");
        let channels = SessionChannels::for_stage(&stage).unwrap();
        let campaign = Token::new("driver-probe-test").unwrap();
        let unit = LaunchUnit::new(
            0,
            UnitIdentity {
                protocol: Token::new("mock-protocol").unwrap(),
                owner: Token::new("mock-owner").unwrap(),
                campaign_id: campaign.clone(),
                phase: Token::new("mock-phase").unwrap(),
                field: Token::new("mock-field").unwrap(),
                stratum: Token::new("mock-stratum").unwrap(),
                candidate: Token::new("mock-candidate").unwrap(),
                task: Task::Probe,
            },
            Token::new("core-producer").unwrap(),
            CanonicalJson::new("{}".to_owned()).unwrap(),
        )
        .unwrap();
        let expected = ChildResult {
            schema: RESULT_SCHEMA.into(),
            identity: unit.identity.clone(),
            case_sha256: unit.case.digest(),
            outcome: ChildOutcome::Complete,
            samples: vec![],
            payload: CanonicalJson::new("{}").unwrap(),
        };
        let response = OwnerResponse::ValidateResult {
            unit_key: unit.key.clone(),
            result_sha256: expected.digest().unwrap(),
        };
        let child_line = transport::encode_result_line(&expected).unwrap();
        let owner_line = transport::encode_result_line(&response).unwrap();
        let script = stage.join("mock-owner");
        fs::write(&script,format!("#!/usr/bin/env python3\nimport sys\nsys.stdin.buffer.read()\nprint({} if sys.argv[1]=='--owner-operation' else {})\nif sys.argv[1]!='--owner-operation': sys.stderr.buffer.write(bytes({}))\n",serde_json::to_string(&owner_line).unwrap(),serde_json::to_string(&child_line).unwrap(),serde_json::to_string(stderr).unwrap())).unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
        let executable = artifact(&script).unwrap();
        let descriptor = ProcessDescriptor {
            id: Token::new("core-producer").unwrap(),
            executable: executable.path.clone(),
            executable_sha256: executable.sha256.clone(),
            arguments: vec!["--fresh-child".into()],
            environment: measurement_environment(),
            working_directory: stage.to_path_buf(),
        };
        let digest = Sha256Digest::of(b"test identity").as_str().to_owned();
        let identity = ResumeIdentity {
            protocol_digest: digest.clone(),
            source_revision: "a".repeat(40),
            source_sha256: digest.clone(),
            ordered_work_manifest_sha256: digest.clone(),
            process_descriptors_sha256: digest.clone(),
            executable_sha256: BTreeMap::from([(
                "core-producer".into(),
                executable.sha256.as_str().into(),
            )]),
            behavior_sha256: BTreeMap::from([("mock-owner".into(), digest.clone())]),
            lifecycle_schema: LIFECYCLE_SCHEMA.into(),
            lifecycle_behavior_sha256: digest,
            feature_contract: FEATURE_CONTRACT.into(),
            thread_contract: THREAD_CONTRACT.into(),
            host_identity: "test-host".into(),
        };
        let config = CampaignConfig {
            schema: "tuning-extent-campaign-v1".into(),
            campaign_id: campaign.clone(),
            channels: channels.clone(),
            identity: identity.clone(),
            manifests: vec![],
            processes: vec![descriptor],
            runtime: CanonicalJson::new("{}").unwrap(),
            protocol: executable.clone(),
            validator: executable.clone(),
            receipt: "test.md".into(),
            affinity: CpuAffinity::observe().unwrap(),
            source_tree: String::new(),
            producing_manifest: executable.clone(),
            build_inputs: BTreeMap::new(),
            preflight_reports: BTreeMap::new(),
            host_admission_policy: HostAdmissionPolicy::declared(),
            imported_owners: vec![],
        };
        let mut log = ExecutionLog::create_new(&stage, campaign.as_str(), "first-session").unwrap();
        log.append(JournalEvent::CampaignStart, None, json!({}))
            .unwrap();
        let checkpoints =
            CheckpointStore::create_new(&channels.checkpoints, campaign.as_str(), identity)
                .unwrap();
        (stage, config, unit, expected, log, checkpoints)
    }
    #[test]
    fn interrupted_derived_projection_publication_is_replayed_and_bound() {
        let (_stage, config, unit, _result, _log, _checkpoints) = probe_fixture(b"");
        let stage = &config.channels.stage;
        let mut units = Vec::new();
        for (ordinal, task) in std::iter::once(Task::Probe)
            .chain((0..5).map(|execution| Task::Measure { execution }))
            .enumerate()
        {
            let mut identity = unit.identity.clone();
            identity.task = task;
            units.push(
                LaunchUnit::new(
                    ordinal as u64,
                    identity,
                    unit.process.clone(),
                    CanonicalJson::new("{}").unwrap(),
                )
                .unwrap(),
            );
        }
        let mut original = OwnerManifest {
            schema: MANIFEST_SCHEMA.into(),
            owner: unit.identity.owner.clone(),
            owner_protocol: unit.identity.protocol.clone(),
            behavior_token: Token::new("mock-behavior").unwrap(),
            campaign_id: config.campaign_id.clone(),
            phases: vec![unit.identity.phase.clone()],
            candidate_blocks: vec![CandidateBlock {
                phase: unit.identity.phase.clone(),
                field: unit.identity.field.clone(),
                stratum: unit.identity.stratum.clone(),
                base_candidates: vec![unit.identity.candidate.clone()],
            }],
            counts: DeclaredCounts::for_cells(1).unwrap(),
            processes: config.processes.clone(),
            ordered_units: units.clone(),
            manifest_sha256: Sha256Digest::of(b""),
        };
        original.seal().unwrap();
        let input = save(&stage.join("mock-input.json"), &json!({})).unwrap();
        let request = DeriveManifestRequest {
            campaign_id: config.campaign_id.clone(),
            original_manifest_sha256: original.manifest_sha256.clone(),
            reserved_units: units.clone(),
            accepted_inputs: input.clone(),
        };
        for unit in &mut units {
            unit.case = CanonicalJson::new("{\"derived\":true}").unwrap();
        }
        let observed = DerivedManifest {
            original_manifest_sha256: original.manifest_sha256.clone(),
            accepted_inputs_sha256: input.sha256,
            units,
            derivation: CanonicalJson::new("{}").unwrap(),
        };
        save(&stage.join("derived-manifest.json"), &observed).unwrap();
        let resolved = restore_derived_projection(stage, &original, &request, &observed).unwrap();
        assert_eq!(
            read_json::<OwnerManifest>(&stage.join("core-resolved-manifest.json")).unwrap(),
            resolved
        );
        assert_eq!(
            restore_derived_projection(stage, &original, &request, &observed).unwrap(),
            resolved
        );
        let mut changed = observed;
        changed.derivation = CanonicalJson::new("{\"changed\":true}").unwrap();
        assert!(restore_derived_projection(stage, &original, &request, &changed).is_err());
    }
    #[test]
    fn rejected_raw_streams_cannot_recover_after_completion_crash() {
        for stderr in [
            b"unterminated".as_slice(),
            b"\xff".as_slice(),
            b"GF2_TUNING_PROGRESS={}\n".as_slice(),
        ] {
            let (_stage, config, unit, _expected, mut log, mut checkpoints) = probe_fixture(stderr);
            assert!(bounded_unit_with_hook(
                &config,
                &unit,
                &mut log,
                &mut checkpoints,
                |boundary| if boundary == UnitBoundary::Completion {
                    Err(invalid("injected completion crash"))
                } else {
                    Ok(())
                }
            )
            .is_err());
            repair_child_exits(&mut log).unwrap();
            let records = ExecutionLog::validate_prefix(
                &log.validated_synced_prefix().unwrap(),
                config.campaign_id.as_str(),
            )
            .unwrap();
            assert!(recover_unit(&config, &unit, &mut log, &mut checkpoints, &records).is_err());
            assert!(checkpoints.completed_keys().is_empty());
        }
    }
    #[test]
    fn clean_completion_crashes_recover_without_fresh_child_replay() {
        for stop in [
            UnitBoundary::Completion,
            UnitBoundary::Exit,
            UnitBoundary::Validation,
        ] {
            let (_stage, config, unit, expected, mut log, mut checkpoints) = probe_fixture(b"");
            assert!(bounded_unit_with_hook(
                &config,
                &unit,
                &mut log,
                &mut checkpoints,
                |boundary| if boundary == stop {
                    Err(invalid("injected completion crash"))
                } else {
                    Ok(())
                }
            )
            .is_err());
            repair_child_exits(&mut log).unwrap();
            let records = ExecutionLog::validate_prefix(
                &log.validated_synced_prefix().unwrap(),
                config.campaign_id.as_str(),
            )
            .unwrap();
            let recovered =
                recover_unit(&config, &unit, &mut log, &mut checkpoints, &records).unwrap();
            assert_eq!(recovered.result, expected);
            let records = ExecutionLog::validate_prefix(
                &log.validated_synced_prefix().unwrap(),
                config.campaign_id.as_str(),
            )
            .unwrap();
            assert_eq!(
                records
                    .iter()
                    .filter(|r| r.event == JournalEvent::ChildSpawn)
                    .count(),
                1
            );
        }
    }
    #[test]
    fn a_probe_streams_validates_and_commits_one_bound_checkpoint() {
        let (_stage, config, unit, expected, mut log, mut checkpoints) = probe_fixture(b"");
        let campaign = config.campaign_id.clone();
        let accepted = bounded_unit(&config, &unit, &mut log, &mut checkpoints).unwrap();
        assert_eq!(accepted.result, expected);
        let (saved, bound): (LaunchUnit, BoundResult) =
            checkpoints.load(unit.key.as_str()).unwrap();
        assert_eq!(saved, unit);
        assert_eq!(bound.result, expected);
        let records = ExecutionLog::validate_prefix(
            &log.validated_synced_prefix().unwrap(),
            campaign.as_str(),
        )
        .unwrap();
        assert_eq!(
            records
                .iter()
                .filter(|r| r.event == JournalEvent::ChildSpawn)
                .count(),
            1
        );
        assert_eq!(
            records
                .iter()
                .filter(|r| r.event == JournalEvent::RawStreams)
                .count(),
            1
        );
        assert_eq!(
            records
                .iter()
                .filter(|r| r.event == JournalEvent::OrchestrationStart)
                .count(),
            1
        );
        assert!(records.iter().all(|r| !matches!(
            r.event,
            JournalEvent::ExecutionProgress | JournalEvent::WindowProgress
        )));
    }
    #[test]
    fn progress_is_delivered_before_exit() {
        let mut command = Command::new("python3");
        command.args([
            "-c",
            "import os,time; os.write(2,b'progress\\n'); time.sleep(.03); os.write(1,b'done')",
        ]);
        let mut order = Vec::new();
        let result = run_process(
            command,
            b"",
            Duration::from_secs(2),
            Duration::from_millis(100),
            |_| Ok(()),
            |bytes| {
                order.push(bytes.to_vec());
                Ok(())
            },
        )
        .unwrap();
        order.push(result.stdout);
        assert_eq!(order, vec![b"progress\n".to_vec(), b"done".to_vec()]);
    }
    #[test]
    fn nonzero_exit_is_never_eligible() {
        let mut command = Command::new("sh");
        command.args(["-c", "printf diagnostic >&2; exit 7"]);
        let result = run_process(
            command,
            b"",
            Duration::from_secs(2),
            Duration::from_millis(100),
            |_| Ok(()),
            |_| Ok(()),
        )
        .unwrap();
        assert!(!result.outcome.accepts_result().unwrap());
        assert_eq!(result.stderr, b"diagnostic");
    }
    #[test]
    fn kill_grace_handles_a_child_ignoring_term() {
        let mut command = Command::new("python3");
        command.args([
            "-c",
            "import signal,time; signal.signal(signal.SIGTERM,signal.SIG_IGN); time.sleep(20)",
        ]);
        let result = run_process(
            command,
            b"",
            Duration::from_millis(100),
            Duration::from_millis(30),
            |_| Ok(()),
            |_| Ok(()),
        )
        .unwrap();
        assert!(matches!(
            result.outcome,
            ProcessOutcome::TimedOut {
                kill_grace_exhausted: true,
                all_descendants_reaped: true,
                ..
            }
        ));
    }
    #[test]
    fn timeout_reaps_an_escaped_descendant() {
        let mut command = Command::new("python3");
        command.args(["-c","import os,time; pid=os.fork();\nif pid==0: os.setsid(); time.sleep(20)\nelse: time.sleep(20)"]);
        let result = run_process(
            command,
            b"",
            Duration::from_millis(100),
            Duration::from_millis(30),
            |_| Ok(()),
            |_| Ok(()),
        )
        .unwrap();
        assert!(matches!(
            result.outcome,
            ProcessOutcome::TimedOut {
                all_descendants_reaped: true,
                ..
            }
        ));
    }
    #[test]
    fn promotion_is_idempotent_and_rejects_different_occupied_bytes() {
        let stage = scratch("gf2-driver-promotion");
        let source = stage.join("candidate.json");
        let target = stage.join("owner.json");
        fs::write(&source, b"canonical candidate").unwrap();
        let source = artifact(&source).unwrap();
        let first = promote(&source, &target).unwrap();
        assert_eq!(promote(&source, &target).unwrap(), first);
        fs::write(&target, b"different").unwrap();
        assert!(promote(&source, &target).is_err());
    }
    #[test]
    fn live_writer_and_reused_identity_are_distinguished() {
        let mut writer = ProcessIdentity::current().unwrap();
        assert!(!writer_dead(&writer).unwrap());
        writer.start_time_ticks += 1;
        assert!(writer_dead(&writer).unwrap());
    }
    #[test]
    fn held_lock_requires_an_inherited_descriptor() {
        let stage = scratch("gf2-driver-lock");
        let path = stage.join("host.lock");
        let lock = File::create(&path).unwrap();
        assert!(lock_available(&path).unwrap());
        lock.lock().unwrap();
        assert!(!lock_available(&path).unwrap());
        assert_eq!(inherited_lock(&path).unwrap(), std::process::id());
        lock.unlock().unwrap();
        assert!(inherited_lock(&path).is_err());
    }
    #[test]
    fn host_admission_policy_names_exactly_the_recorded_host_observation() {
        let recorded = serde_json::to_value(HostObservation::observe().unwrap()).unwrap();
        let mut fields: Vec<String> = recorded.as_object().unwrap().keys().cloned().collect();
        fields.sort_unstable();
        assert_eq!(
            fields,
            HostAdmissionPolicy::declared().recorded_observations
        );
    }
    #[test]
    fn simultaneous_binary_streams_are_drained_without_loss() {
        let mut command = Command::new("python3");
        command.args(["-c", "import os,threading; t=threading.Thread(target=lambda: os.write(2,b'\\xff'*131072)); t.start(); os.write(1,b'x'*131072); t.join()"]);
        let mut stderr = Vec::new();
        let result = run_process(
            command,
            b"",
            Duration::from_secs(2),
            Duration::from_millis(100),
            |_| Ok(()),
            |chunk| {
                stderr.extend_from_slice(chunk);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(result.stdout, vec![b'x'; 131072]);
        assert_eq!(stderr, vec![255; 131072]);
        assert!(result.outcome.accepts_result().unwrap());
    }

    #[test]
    fn campaign_stage_policy_rejects_non_tmp_and_mismatched_paths() {
        let declaration = seam_declaration();
        let campaign = format!("gf2-dbd8787d-20261001t000000z-{}", std::process::id());
        let expected = Path::new("/tmp").join(&campaign);
        assert!(validate_campaign_stage(&expected, &campaign, &declaration).is_ok());
        assert!(validate_campaign_stage(Path::new("/tmp/other"), &campaign, &declaration).is_err());
        assert!(
            validate_campaign_stage(&expected, "gf2-dbd8787d-20261001-000000-1", &declaration)
                .is_err()
        );
        assert!(validate_campaign_stage(
            Path::new("/home/example/gf2-dbd8787d-20261001t000000z-1"),
            "gf2-dbd8787d-20261001t000000z-1",
            &declaration
        )
        .is_err());
        let uppercase = format!("gf2-dbd8787d-20261001T000000Z-{}", std::process::id());
        assert!(validate_campaign_stage(
            &Path::new("/tmp").join(&uppercase),
            &uppercase,
            &declaration
        )
        .is_err());
        let other = format!("gf2-a83583e0-20261001t000000z-{}", std::process::id());
        assert!(
            validate_campaign_stage(&Path::new("/tmp").join(&other), &other, &declaration).is_err(),
            "a run ID of another declaration's issue was accepted"
        );
    }

    #[test]
    fn campaign_stage_policy_accepts_the_launcher_minted_id() {
        let launcher = fs::read_to_string(
            repository_root()
                .unwrap()
                .join("dev/scripts/tuning-extent-campaign.sh"),
        )
        .unwrap();
        let format = launcher
            .lines()
            .find_map(|line| {
                line.trim()
                    .strip_prefix("campaign=gf2-$issue-$(date -u ")?
                    .strip_suffix(")-$$")
            })
            .expect("launcher mints its campaign ID from one date format");
        let stamp = Command::new("date").args(["-u", format]).output().unwrap();
        assert!(stamp.status.success());
        let declaration = seam_declaration();
        let campaign = format!(
            "{}{}-{}",
            declaration.prefix(),
            String::from_utf8(stamp.stdout).unwrap().trim_end(),
            std::process::id()
        );
        let stage = fs::canonicalize("/tmp").unwrap().join(&campaign);
        validate_campaign_stage(&stage, &campaign, &declaration).unwrap();
    }

    #[test]
    fn declarations_select_owners_counts_imports_and_publication() {
        let root = repository_root().unwrap();
        let extent = extent_declaration();
        assert_eq!(extent.prefix(), "gf2-a83583e0-");
        assert_eq!(extent.total_cells(), 717);
        assert_eq!(
            extent.executables(),
            ["algebra-producer", "composer", "core-producer", "driver"]
        );
        assert!(extent.imported_owners.is_empty() && !extent.evidence_index);
        let seam = seam_declaration();
        assert_eq!(seam.prefix(), "gf2-dbd8787d-");
        assert_eq!(seam.total_cells(), 756);
        assert_eq!(seam.executables(), ["composer", "core-producer", "driver"]);
        assert!(seam.evidence_index);
        let imported = seam.imported("algebra").unwrap();
        let envelope = imported.envelope.read(&root).unwrap();
        let complete = imported.complete.read(&root).unwrap();
        assert_eq!(
            section_wrapper(&envelope, &imported.section).unwrap(),
            section_wrapper(&complete, &imported.section).unwrap()
        );
        for (mutate, what) in [
            (
                (|d: &mut CampaignDeclaration| d.imported_owners.clear())
                    as fn(&mut CampaignDeclaration),
                "an uncovered owner",
            ),
            (|d| d.issue = "a83583e0".into(), "another issue"),
            (|d| d.measured_owners[0].cells = 0, "an empty owner"),
            (
                |d| d.protocol = "/abs/protocol.md".into(),
                "an absolute protocol",
            ),
            (
                |d| d.imported_owners[0].section = "gf2-core/selectors".into(),
                "a foreign imported section",
            ),
        ] {
            let mut changed = seam.clone();
            mutate(&mut changed);
            assert!(changed.validate("dbd8787d").is_err(), "accepted {what}");
        }
        let mut changed = imported.envelope.clone();
        changed.sha256 = Sha256Digest::of(b"other");
        assert!(changed.read(&root).is_err());
        for invalid_id in ["gf2-DBD8787D-1", "gf2-dbd8787-1", "dbd8787d-1"] {
            assert!(
                CampaignDeclaration::issue_of(invalid_id).is_err(),
                "{invalid_id}"
            );
        }
    }

    #[test]
    fn producing_input_manifest_rejects_authority_and_path_mutations() {
        let root = repository_root().unwrap();
        for declaration in [extent_declaration(), seam_declaration()] {
            let manifest: ProducingInputs = serde_json::from_slice(
                &fs::read(root.join(&declaration.producing_manifest)).unwrap(),
            )
            .unwrap();
            manifest.validate_at(&root).unwrap();
            let mut changed = manifest.clone();
            changed.schema = "unknown".into();
            assert!(changed.validate_at(&root).is_err());

            let mut changed = manifest.clone();
            changed.behavior_sources.swap(0, 1);
            assert!(changed.validate_at(&root).is_err());

            let mut changed = manifest.clone();
            changed.lifecycle_sources.push("not/in/behavior.rs".into());
            changed.lifecycle_sources.sort();
            assert!(changed.validate_at(&root).is_err());

            let mut changed = manifest;
            changed.build_inputs[0] = "crates/*/Cargo.toml".into();
            changed.build_inputs.sort();
            assert!(changed.validate_at(&root).is_err());
        }
        let seam = seam_declaration();
        let manifest = ProducingInputs::read_at(&root, &seam.producing_manifest).unwrap();
        let lifecycle = ProducingInputs::hashes_at(&root, &manifest.lifecycle_sources).unwrap();
        assert!(seam.recorded_in(&lifecycle).is_ok());
    }

    #[test]
    fn timeout_kills_descendant_holding_pipes() {
        let mut command = Command::new("sh");
        command.args(["-c", "sleep 20 & wait"]);
        let result = run_process(
            command,
            b"",
            Duration::from_millis(80),
            Duration::from_millis(100),
            |_| Ok(()),
            |_| Ok(()),
        )
        .unwrap();
        assert!(matches!(result.outcome, ProcessOutcome::TimedOut { .. }));
        assert!(match result.outcome {
            ProcessOutcome::Exited {
                all_descendants_reaped,
                ..
            }
            | ProcessOutcome::Signaled {
                all_descendants_reaped,
                ..
            }
            | ProcessOutcome::TimedOut {
                all_descendants_reaped,
                ..
            } => all_descendants_reaped,
        });
    }

    #[test]
    fn stderr_callback_failure_still_drains_and_reaps() {
        let mut command = Command::new("sh");
        command.args(["-c", "printf bad >&2; sleep 20"]);
        let result = run_process(
            command,
            b"",
            Duration::from_secs(2),
            Duration::from_millis(100),
            |_| Ok(()),
            |_| Err(io::Error::other("bad progress")),
        )
        .unwrap();
        assert!(result.callback_error.is_some());
        assert!(match result.outcome {
            ProcessOutcome::Exited {
                all_descendants_reaped,
                ..
            }
            | ProcessOutcome::Signaled {
                all_descendants_reaped,
                ..
            }
            | ProcessOutcome::TimedOut {
                all_descendants_reaped,
                ..
            } => all_descendants_reaped,
        });
    }

    /// A synthetic complete stage, a stand-in composer that accepts only the
    /// exact published owners and a real validator publication check.
    struct PublicationFixture {
        _root: Scratch,
        root: PathBuf,
        stage: PathBuf,
        campaign: Token,
        declaration: CampaignDeclaration,
        composer: ProcessDescriptor,
        validator: PathBuf,
        evidence_index: Option<Vec<u8>>,
    }
    impl PublicationFixture {
        fn new() -> Self {
            Self::with(extent_declaration())
        }
        fn with(declaration: CampaignDeclaration) -> Self {
            use std::os::unix::fs::PermissionsExt;
            let scratch_root = scratch("gf2-driver-publication");
            let root = fs::canonicalize(scratch_root.path()).unwrap();
            let campaign =
                Token::new(format!("{}19700101t000000z-1", declaration.prefix())).unwrap();
            let stage = root.join("stage");
            let imported = declaration.imported("algebra").cloned();
            let algebra = imported.as_ref().map_or_else(
                || b"{\"owner\":\"algebra\"}".to_vec(),
                |imported| imported.envelope.read(&repository_root().unwrap()).unwrap(),
            );
            let mut files: Vec<(String, Vec<u8>)> = [
                ("core-owner.json", &b"{\"owner\":\"core\"}"[..]),
                ("complete.json", b"{\"owners\":2}"),
                ("receipt.md", b"# receipt\n"),
                ("execution.log", b"{\"sequence\":0}\n"),
                ("checkpoints/manifest.json", b"{}"),
                ("checkpoints/units/unit.json", b"{\"unit\":1}"),
                ("raw-attempts/attempt.stdout", b"GF2_TUNING_RESULT={}"),
                ("core-accepted.json", b"{\"accepted\":1}"),
                ("bin/driver", b"staged executable"),
                ("sessions/s1/checksum.json", b"{\"checksum\":1}"),
                ("build/source-before.json", b"{\"phase\":\"before-build\"}"),
                ("core-manifest.json", b"{\"manifest\":\"core\"}"),
                (
                    "core-resolved-manifest.json",
                    b"{\"manifest\":\"resolved\"}",
                ),
                ("core-owner-response.json", b"{\"response\":\"core\"}"),
                ("receipt-projection.json", b"{\"projection\":1}"),
                (RAW_RESULT_INDEX, b"# Raw result index\n"),
                (OWNER_DECISIONS, b"[]"),
            ]
            .into_iter()
            .map(|(name, content)| (name.to_owned(), content.to_vec()))
            .collect();
            files.push(("algebra-owner.json".into(), algebra));
            let mut imported_evidence = Vec::new();
            if let Some(imported) = &imported {
                let reopen = "imported/algebra-session-1/reopen.json";
                files.push((reopen.into(), b"{\"reopen\":\"algebra\"}".to_vec()));
                imported_evidence.push((imported.clone(), reopen));
            } else {
                files.push(("algebra-accepted.json".into(), b"{\"accepted\":2}".to_vec()));
            }
            for (relative, content) in &files {
                fs::create_dir_all(stage.join(relative).parent().unwrap()).unwrap();
                fs::write(stage.join(relative), content).unwrap();
            }
            let imported_owners: Vec<ImportedOwnerEvidence> = imported_evidence
                .into_iter()
                .map(|(imported, reopen)| ImportedOwnerEvidence {
                    owner: imported.owner.clone(),
                    source: imported.envelope.clone(),
                    staged: artifact(&stage.join("algebra-owner.json")).unwrap(),
                    reopen: artifact(&stage.join(reopen)).unwrap(),
                })
                .collect();
            let protocol = Sha256Digest::of(b"protocol");
            let declaration_sha = declaration.sha256.clone().unwrap();
            let revision = "0".repeat(40);
            fs::write(
                stage.join("campaign.json"),
                encoded(&json!({
                    "campaign_id": campaign,
                    "protocol": {"path": "/repository/protocol.md", "sha256": protocol},
                    "identity": {"source_revision": revision,
                                 "behavior_sha256": {RECORDED_DECLARATION: declaration_sha}},
                    "imported_owners": imported_owners,
                }))
                .unwrap(),
            )
            .unwrap();
            let digest =
                |relative: &str| Sha256Digest::of(&fs::read(stage.join(relative)).unwrap());
            let tail = [
                campaign.as_str(),
                "1970-01-01T00:00:00Z",
                "0000000000000000000000000000000000000000",
                "false",
            ];
            let executable = root.join("composer");
            fs::write(
                &executable,
                format!(
                    "#!/bin/sh\nPATH=/usr/bin:/bin\nset -C\nsum() {{ sha256sum < \"$1\" | cut -d' ' -f1; }}\ntest \"$1\" = complete || exit 2\ntest -e {root}/reject && exit 3\ntest \"$(sum \"$2\")\" = {core} || exit 4\ntest \"$(sum \"$3\")\" = {algebra} || exit 5\ntest \"$5 $6 $7 $8\" = \"{tail}\" || exit 6\nif test -e {root}/skew; then printf skew > \"$4\"; else cat {complete} > \"$4\"; fi\n",
                    root = root.display(),
                    core = digest("core-owner.json").as_str(),
                    algebra = digest("algebra-owner.json").as_str(),
                    tail = tail.join(" "),
                    complete = stage.join("complete.json").display(),
                ),
            )
            .unwrap();
            fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
            let executable = artifact(&executable).unwrap();
            let mut arguments: Vec<String> = ["complete", "core", "algebra", "output"]
                .into_iter()
                .chain(tail)
                .map(str::to_owned)
                .collect();
            arguments.push(executable.sha256.as_str().into());
            fs::write(
                stage.join("composition.json"),
                encoded(&json!({"args":arguments,"tool_sha256":executable.sha256})).unwrap(),
            )
            .unwrap();
            let composer = ProcessDescriptor {
                id: Token::new("composer").unwrap(),
                executable: executable.path,
                executable_sha256: executable.sha256,
                arguments: vec![],
                environment: measurement_environment(),
                working_directory: root.clone(),
            };
            let validator = fs::canonicalize(
                repository_root()
                    .unwrap()
                    .join("dev/scripts/validate-tuning-extent-campaign.py"),
            )
            .unwrap();
            let evidence_index = declaration.evidence_index.then(|| {
                render_evidence_index(
                    &stage,
                    &EvidenceIdentity {
                        campaign_id: campaign.as_str(),
                        protocol_sha256: &protocol,
                        source_revision: &revision,
                        declaration_path: RECORDED_DECLARATION,
                        declaration_sha256: declaration_sha.as_str(),
                        imported: &imported_owners,
                    },
                    &declaration,
                )
                .unwrap()
                .into_bytes()
            });
            Self {
                _root: scratch_root,
                root,
                stage,
                campaign,
                declaration,
                composer,
                validator,
                evidence_index,
            }
        }
        /// An empty checkout holding only the committed envelopes the
        /// campaign imports.
        fn repository(&self, name: &str) -> PathBuf {
            let repository = self.root.join(name);
            fs::create_dir(&repository).unwrap();
            for imported in &self.declaration.imported_owners {
                let target = repository.join(&imported.envelope.path);
                fs::create_dir_all(target.parent().unwrap()).unwrap();
                fs::write(
                    target,
                    imported.envelope.read(&repository_root().unwrap()).unwrap(),
                )
                .unwrap();
            }
            repository
        }
        fn publish(&self, repository: &Path) -> io::Result<ArtifactIdentity> {
            publish_to_repository(
                &self.stage,
                repository,
                &self.campaign,
                &self.declaration,
                b"{\"status\":\"complete-valid\"}",
                self.evidence_index.as_deref(),
                &self.composer,
                &self.validator,
            )
        }
    }

    #[test]
    fn publication_populates_every_protocol_destination_and_refuses_overwrite() {
        let fixture = PublicationFixture::new();
        let id = fixture.campaign.as_str();
        let layout = RepositoryLayout { id };

        let occupied = fixture.repository("occupied");
        let foreign = occupied.join(layout.core_owner());
        fs::create_dir_all(foreign.parent().unwrap()).unwrap();
        fs::write(&foreign, b"existing evidence").unwrap();
        let refused = fixture.publish(&occupied).unwrap_err().to_string();
        assert!(refused.contains("contradicts durable intent"), "{refused}");
        assert_eq!(fs::read(&foreign).unwrap(), b"existing evidence");
        assert!(!occupied.join(layout.checksum()).exists());

        let repository = fixture.repository("repository");
        let manifest = fixture.publish(&repository).unwrap();
        assert_eq!(manifest.path, repository.join(layout.checksum()));
        let archive = |relative: &str| format!("{}/{relative}", layout.archive());
        let session = |relative: &str| format!("{}/{relative}", layout.session());
        for (source, destination) in [
            ("core-owner.json", layout.core_owner()),
            ("algebra-owner.json", layout.algebra_owner()),
            ("complete.json", layout.complete()),
            ("receipt.md", layout.receipt()),
            (
                "build/source-before.json",
                session("build/source-before.json"),
            ),
            (
                "repository-publication/validation.json",
                session("validation.json"),
            ),
            ("execution.log", layout.execution_log()),
            ("campaign.json", archive("campaign.json")),
            (
                "checkpoints/units/unit.json",
                archive("checkpoints/units/unit.json"),
            ),
            (
                "raw-attempts/attempt.stdout",
                archive("raw-attempts/attempt.stdout"),
            ),
            ("core-accepted.json", archive("core-accepted.json")),
            (
                "sessions/s1/checksum.json",
                archive("sessions/s1/checksum.json"),
            ),
        ] {
            assert_eq!(
                fs::read(repository.join(&destination)).unwrap(),
                fs::read(fixture.stage.join(source)).unwrap(),
                "{destination}"
            );
        }
        assert!(!repository.join(archive("bin/driver")).exists());
        let reopen: Value = serde_json::from_slice(
            &fs::read(repository.join(session("repository-reopen.json"))).unwrap(),
        )
        .unwrap();
        assert_eq!(
            reopen["complete"]["sha256"].as_str(),
            Some(Sha256Digest::of(&fs::read(repository.join(layout.complete())).unwrap()).as_str())
        );
        // Every committed file, the archived journal and the archive manifest
        // are pinned by the committed manifest; the archive manifest pins every
        // other archived file.
        let pinned = |manifest: &Path| -> BTreeMap<String, String> {
            fs::read_to_string(manifest)
                .unwrap()
                .lines()
                .map(|line| {
                    let (digest, path) = line.split_once("  ").unwrap();
                    assert_eq!(
                        Sha256Digest::of(&fs::read(repository.join(path)).unwrap()).as_str(),
                        digest,
                        "{path}"
                    );
                    (path.to_owned(), digest.to_owned())
                })
                .collect()
        };
        let committed = pinned(&manifest.path);
        let archived = pinned(&repository.join(archive("SHA256SUMS")));
        assert!(committed.contains_key(&layout.execution_log()));
        assert!(committed.contains_key(&archive("SHA256SUMS")));
        assert!(committed.contains_key(&layout.core_owner()));
        assert!(archived.contains_key(&archive("raw-attempts/attempt.stdout")));
        assert!(archived.contains_key(&archive("repository-publication/plan.json")));
        let plan: RepositoryPlan = read_json(
            &fixture
                .stage
                .join(REPOSITORY_PUBLICATION_DIR)
                .join("plan.json"),
        )
        .unwrap();
        for entry in &plan.entries {
            let listed = if entry.destination.starts_with(&layout.archive()) {
                &archived
            } else {
                &committed
            };
            assert_eq!(
                listed.get(&entry.destination),
                Some(&entry.sha256.as_str().to_owned())
            );
        }
        assert_eq!(fixture.publish(&repository).unwrap(), manifest);
    }

    #[test]
    fn seam_publication_keeps_the_imported_owner_and_commits_its_evidence_index() {
        let fixture = PublicationFixture::with(seam_declaration());
        let layout = RepositoryLayout {
            id: fixture.campaign.as_str(),
        };
        let imported = fixture.declaration.imported("algebra").unwrap().clone();

        let drifted = fixture.repository("drifted");
        fs::write(drifted.join(&imported.envelope.path), b"{}").unwrap();
        let refused = fixture.publish(&drifted).unwrap_err().to_string();
        assert!(refused.contains("strict reopen"), "{refused}");
        assert!(!drifted.join(layout.checksum()).exists());

        let repository = fixture.repository("repository");
        let manifest = fixture.publish(&repository).unwrap();
        assert!(
            !repository.join(layout.algebra_owner()).exists(),
            "an imported owner must not publish a run-named algebra envelope"
        );
        let archive = |relative: &str| format!("{}/{relative}", layout.archive());
        assert_eq!(
            fs::read(repository.join(archive("algebra-owner.json"))).unwrap(),
            imported.envelope.read(&repository_root().unwrap()).unwrap()
        );
        let index = fs::read(repository.join(layout.evidence_index())).unwrap();
        assert_eq!(Some(&index), fixture.evidence_index.as_ref());
        let index = String::from_utf8(index).unwrap();
        assert!(index.contains(&format!("`{}`", imported.envelope.path)));
        assert!(index.contains(&format!("`{}`", archive("receipt-projection.json"))));
        assert!(!index.contains("/tmp/") && !index.contains(fixture.stage.to_str().unwrap()));
        let committed = fs::read_to_string(&manifest.path).unwrap();
        assert!(committed.contains(&layout.evidence_index()));
        let reopen: Value = serde_json::from_slice(
            &fs::read(repository.join(format!("{}/repository-reopen.json", layout.session())))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            reopen["algebra"],
            json!({"path": imported.envelope.path, "sha256": imported.envelope.sha256})
        );
        assert_eq!(fixture.publish(&repository).unwrap(), manifest);
    }

    #[test]
    fn evidence_index_presence_and_content_follow_the_declaration() {
        let mut fixture = PublicationFixture::with(seam_declaration());
        let mut forged = fixture.evidence_index.clone().unwrap();
        forged.extend_from_slice(b"| Forged | `x` | `y` |\n");
        let correct = fixture.evidence_index.replace(forged);
        let repository = fixture.repository("forged");
        let refused = fixture.publish(&repository).unwrap_err().to_string();
        assert!(
            refused.contains("independent validation failed"),
            "{refused}"
        );
        fixture.evidence_index = None;
        assert!(fixture.publish(&fixture.repository("absent")).is_err());
        fixture.evidence_index = correct;
        let extent = PublicationFixture::new();
        let mut unexpected = extent;
        unexpected.evidence_index = Some(b"# index\n".to_vec());
        assert!(unexpected
            .publish(&unexpected.repository("unexpected"))
            .is_err());
    }

    #[test]
    fn oversized_committed_destination_fails_before_the_plan_is_journaled() {
        let fixture = PublicationFixture::new();
        let layout = RepositoryLayout {
            id: fixture.campaign.as_str(),
        };
        fs::write(
            fixture.stage.join("receipt.md"),
            vec![b'#'; COMMITTED_LIMIT_BYTES as usize + 1],
        )
        .unwrap();
        let repository = fixture.repository("repository");
        let error = fixture.publish(&repository).unwrap_err().to_string();
        assert!(
            error.contains(&format!(
                "committed destination {} is {} bytes",
                layout.receipt(),
                COMMITTED_LIMIT_BYTES + 1
            )),
            "{error}"
        );
        assert!(!fixture
            .stage
            .join(REPOSITORY_PUBLICATION_DIR)
            .join("plan.json")
            .exists());
        assert_eq!(fs::read_dir(&repository).unwrap().count(), 0);
    }

    #[test]
    fn interrupted_publication_resumes_under_the_same_plan() {
        let fixture = PublicationFixture::new();
        let layout = RepositoryLayout {
            id: fixture.campaign.as_str(),
        };
        let repository = fixture.repository("repository");
        let checksum = repository.join(layout.checksum());
        for (marker, reason) in [
            ("skew", "differs from the published complete envelope"),
            ("reject", "failed the composer's strict reopen"),
        ] {
            fs::write(fixture.root.join(marker), b"").unwrap();
            let error = fixture.publish(&repository).unwrap_err().to_string();
            assert!(error.contains(reason), "{error}");
            fs::remove_file(fixture.root.join(marker)).unwrap();
            assert!(repository.join(layout.complete()).is_file());
            assert!(!checksum.exists());
        }
        // A crash left one archived destination missing and a committed one
        // only as a partial temporary beside its absent final name.
        let removed = repository.join(layout.archive()).join("core-accepted.json");
        fs::remove_file(&removed).unwrap();
        let receipt = repository.join(layout.receipt());
        let bytes = fs::read(&receipt).unwrap();
        fs::remove_file(&receipt).unwrap();
        let temporary = receipt.with_file_name(format!(
            ".{}.tmp-1-2-0",
            receipt.file_name().unwrap().to_str().unwrap()
        ));
        fs::write(&temporary, &bytes[..3]).unwrap();
        fixture.publish(&repository).unwrap();
        assert!(checksum.is_file());
        assert!(!temporary.exists());
        assert_eq!(fs::read(&receipt).unwrap(), bytes);
        assert_eq!(
            fs::read(&removed).unwrap(),
            fs::read(fixture.stage.join("core-accepted.json")).unwrap()
        );
        let unplanned = repository.join(layout.archive()).join("unplanned.json");
        fs::write(&unplanned, b"{}").unwrap();
        assert!(fixture.publish(&repository).is_err());
    }

    /// Worst-case full-scale receipt: 4,536 accepted units and owner decisions
    /// carrying every one of the 9,000 retained-threshold timing windows at
    /// the widest values the protocol admits (2^32 calls, the 120 s child
    /// limit), plus the one-factor, GEMM and joint decisions at twice 30,947 bytes.
    #[test]
    fn full_scale_receipt_stays_within_a_quarter_of_the_committed_limit() {
        let campaign = Token::new("gf2-dbd8787d-19700101t000000z-1").unwrap();
        let units: Vec<LaunchUnit> = (0..4536u64)
            .map(|ordinal| {
                let task = match ordinal % 6 {
                    0 => Task::Probe,
                    execution => Task::Measure {
                        execution: execution - 1,
                    },
                };
                LaunchUnit::new(
                    ordinal,
                    UnitIdentity {
                        protocol: Token::new("core-tuning-campaign-v4").unwrap(),
                        owner: Token::new("gf2-core").unwrap(),
                        campaign_id: campaign.clone(),
                        phase: Token::new("extent").unwrap(),
                        field: Token::new("triangular.trsm_blocked_min_dim").unwrap(),
                        stratum: Token::new(format!("size-{ordinal}-standard")).unwrap(),
                        candidate: Token::new("conservative").unwrap(),
                        task,
                    },
                    Token::new("core-producer").unwrap(),
                    CanonicalJson::new("{}".to_owned()).unwrap(),
                )
                .unwrap()
            })
            .collect();
        let index = raw_result_index(units.iter());
        assert_eq!(
            index.lines().filter(|line| line.starts_with("- `")).count(),
            4536
        );
        let window = json!({"execution":4,"repetition":4,"calls":1u64 << 32,"elapsed_ns":120_000_000_000u64});
        let samples = vec![window; 25];
        let points: Vec<_> = (0..180)
            .map(|size| json!({"size":size,"conservative":{"median":1.0e9,"spread":0.5,"samples":samples},"asymptotic":{"median":1.0e9,"spread":0.5,"samples":samples}}))
            .collect();
        let decisions = json!({
            "schema":"core-tuning-campaign-v4",
            "retained_thresholds":points,
            "extents":"x".repeat(2 * 30_947),
        });
        let artifact = ArtifactIdentity {
            path: PathBuf::from("/tmp/owner.json"),
            sha256: Sha256Digest::of(b""),
        };
        let responses = [
            OwnerResponse::EmitOwner {
                artifact: artifact.clone(),
                decisions: CanonicalJson::from_serializable(&decisions).unwrap(),
            },
            OwnerResponse::EmitOwner {
                artifact,
                decisions: CanonicalJson::from_serializable(&json!({"selected":65536})).unwrap(),
            },
        ];
        let facts = ReceiptFacts {
            campaign_id: campaign.as_str().to_owned(),
            declaration: seam_declaration(),
            declaration_path: RECORDED_DECLARATION.to_owned(),
            declaration_sha256: Sha256Digest::of(b"declaration"),
            protocol_sha256: Sha256Digest::of(b"protocol"),
            source_revision: "0000000000000000000000000000000000000000".to_owned(),
            affinity: format!("{:?}", (0..64).collect::<Vec<u32>>()),
            owners: vec![["gf2-core".into(), "p".into(), "b".into(), "0".repeat(64)]; 2],
            cited: vec![],
            counts: DeclaredCounts::for_cells(756).unwrap(),
            attempts: u64::MAX,
            orchestration: u64::MAX,
            sessions: u64::MAX,
        };
        let decisions = encoded(&responses).unwrap();
        let receipt = render_receipt(
            &facts,
            Path::new("/tmp"),
            &responses,
            index.as_bytes(),
            &decisions,
        )
        .unwrap();
        assert!(receipt.contains(Sha256Digest::of(&decisions).as_str()));
        assert!(!receipt.contains("elapsed_ns"));
        assert!(receipt.contains("sample_count"));
        assert!(receipt.contains(Sha256Digest::of(index.as_bytes()).as_str()));
        assert!(!receipt.contains("size-4301-standard"));
        let limit = usize::try_from(COMMITTED_LIMIT_BYTES).unwrap();
        assert!(
            receipt.len() <= limit / 4,
            "receipt is {} bytes against its {}-byte target",
            receipt.len(),
            limit / 4
        );
        eprintln!(
            "full-scale receipt bytes: {}; raw index bytes: {}",
            receipt.len(),
            index.len()
        );
    }

    #[test]
    fn rendered_receipt_documents_pass_the_validator_receipt_checks() {
        let scratch_root = scratch("gf2-driver-receipt-check");
        let stage = fs::canonicalize(scratch_root.path()).unwrap();
        let declaration = seam_declaration();
        let campaign = Token::new(format!("{}19700101t000000z-1", declaration.prefix())).unwrap();
        let fields = [
            "gemm.tiles",
            "field_vec.dot_chunk_len",
            "gemm.winograd_min_dim",
        ];
        let units: Vec<LaunchUnit> = (0..18u64)
            .map(|ordinal| {
                let task = match ordinal % 6 {
                    0 => Task::Probe,
                    execution => Task::Measure {
                        execution: execution - 1,
                    },
                };
                LaunchUnit::new(
                    ordinal,
                    UnitIdentity {
                        protocol: Token::new("core-tuning-campaign-v4").unwrap(),
                        owner: Token::new("gf2-core").unwrap(),
                        campaign_id: campaign.clone(),
                        phase: Token::new("extent").unwrap(),
                        field: Token::new(fields[ordinal as usize / 6]).unwrap(),
                        stratum: Token::new("shape-0").unwrap(),
                        candidate: Token::new("128").unwrap(),
                        task,
                    },
                    Token::new("core-producer").unwrap(),
                    CanonicalJson::new("{}".to_owned()).unwrap(),
                )
                .unwrap()
            })
            .collect();
        let bundle =
            json!({"accepted": units.iter().map(|unit| json!({"unit": unit})).collect::<Vec<_>>()});
        let arm = json!({"median":2.5,"spread":0.125,"samples":[{"execution":0,"repetition":0,"calls":7,"elapsed_ns":250_000_001u64}]});
        fs::create_dir_all(stage.join("candidates/core-producer-s-9")).unwrap();
        fs::write(
            stage.join("candidates/core-producer-s-9/output.json"),
            b"core",
        )
        .unwrap();
        let responses = [OwnerResponse::EmitOwner {
            artifact: ArtifactIdentity {
                path: stage.join("candidates/core-producer-s-9/output.json"),
                sha256: Sha256Digest::of(b"core"),
            },
            decisions: CanonicalJson::from_serializable(&json!({"schema":"core-tuning-campaign-v4","retained_thresholds":[{"points":[{"size":8,"conservative":arm}]}],"gemm":{"selected":[32,64],"reason":"structural-schedule-tie"}})).unwrap(),
        }];
        let protocol = ArtifactIdentity {
            path: PathBuf::from("/repository/dev/active/dbd8787d/premeasurement-protocol.md"),
            sha256: Sha256Digest::of(b"protocol"),
        };
        let affinity: Vec<u32> = vec![0, 2, 4, 6];
        let executable = "e".repeat(64);
        let counts = DeclaredCounts::for_cells(756).unwrap();
        let declaration_sha = declaration.sha256.clone().unwrap();
        for (relative, content) in [
            ("core-manifest.json", "{\"manifest\":1}"),
            ("core-resolved-manifest.json", "{\"manifest\":2}"),
            ("core-owner-response.json", "{\"response\":1}"),
            ("composition.json", "{\"composition\":1}"),
            ("checkpoints/manifest.json", "{\"checkpoints\":1}"),
            ("imported/algebra-s-1/reopen.json", "{\"reopen\":1}"),
        ] {
            fs::create_dir_all(stage.join(relative).parent().unwrap()).unwrap();
            fs::write(stage.join(relative), content).unwrap();
        }
        let imported = vec![ImportedOwnerEvidence {
            owner: Token::new("gf2-algebra").unwrap(),
            source: declaration.imported_owners[0].envelope.clone(),
            staged: ArtifactIdentity {
                path: stage.join("algebra-owner.json"),
                sha256: declaration.imported_owners[0].envelope.sha256.clone(),
            },
            reopen: artifact(&stage.join("imported/algebra-s-1/reopen.json")).unwrap(),
        }];
        let files = [
            (
                "campaign.json",
                json!({
                    "campaign_id": campaign,
                    "protocol": protocol,
                    "identity": {"source_revision": "0".repeat(40),
                                 "behavior_sha256": {RECORDED_DECLARATION: declaration_sha}},
                    "affinity": affinity,
                    "manifests": [json!({
                        "owner": "gf2-core",
                        "owner_protocol": "core-tuning-campaign-v4",
                        "behavior_token": "tuning-calibration-v4",
                        "processes": [{"executable_sha256": executable}],
                    })],
                    "imported_owners": imported,
                }),
            ),
            ("core-accepted.json", bundle),
            (
                "receipt-projection.json",
                json!({"owners": responses, "counts": counts, "attempts": 4401, "orchestration": 9, "sessions": 2}),
            ),
        ];
        for (name, value) in &files {
            fs::write(stage.join(name), encoded(value).unwrap()).unwrap();
        }
        let facts = ReceiptFacts {
            campaign_id: campaign.as_str().to_owned(),
            declaration: declaration.clone(),
            declaration_path: RECORDED_DECLARATION.to_owned(),
            declaration_sha256: declaration_sha,
            protocol_sha256: protocol.sha256.clone(),
            source_revision: "0".repeat(40),
            affinity: format!("{affinity:?}"),
            owners: vec![[
                "gf2-core".to_owned(),
                "core-tuning-campaign-v4".to_owned(),
                "tuning-calibration-v4".to_owned(),
                executable.clone(),
            ]],
            cited: cited_sources(&declaration, &imported, &stage).unwrap(),
            counts,
            attempts: 4401,
            orchestration: 9,
            sessions: 2,
        };
        publish_receipt_documents(&stage, &facts, units.iter(), &responses).unwrap();
        let receipt = stage.join("receipt.md");
        let text = fs::read_to_string(&receipt).unwrap();
        assert!(!text.contains(stage.to_str().unwrap()) && !text.contains("/repository/"));
        assert!(text.contains(".agents/campaign-evidence/gf2-dbd8787d-19700101t000000z-1/candidates/core-producer-s-9/output.json"));
        assert!(text.contains("`dev/benchmarks/tuning_profiles/gf2-dbd8787d-19700101t000000z-1-session/composition.json`"));
        assert!(text.contains(&format!(
            "`{}`",
            declaration.imported_owners[0].envelope.path
        )));
        let validator = fs::canonicalize(
            repository_root()
                .unwrap()
                .join("dev/scripts/validate-tuning-extent-campaign.py"),
        )
        .unwrap();
        let check = || {
            Command::new("python3")
                .arg(&validator)
                .arg("--stage")
                .arg(&stage)
                .arg("--receipt-check")
                .output()
                .unwrap()
        };
        let output = check();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        fs::write(&receipt, text.replace("sample_count", "samples_count")).unwrap();
        let output = check();
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("receipt"));
        fs::write(&receipt, &text).unwrap();
        fs::write(stage.join("composition.json"), "{\"composition\":2}").unwrap();
        let output = check();
        assert!(
            !output.status.success(),
            "a cited file changed after rendering was accepted"
        );
    }

    #[test]
    fn imported_owner_is_staged_reopened_journaled_and_independently_validated() {
        use std::os::unix::fs::PermissionsExt;
        let scratch_root = scratch("gf2-driver-import");
        let root = fs::canonicalize(scratch_root.path()).unwrap();
        let stage = root.join("stage");
        fs::create_dir(&stage).unwrap();
        let declaration = seam_declaration();
        let imported = declaration.imported("algebra").unwrap().clone();
        let campaign = Token::new(format!("{}19700101t000000z-1", declaration.prefix())).unwrap();
        let reference = repository_root().unwrap().join(&imported.complete.path);
        let composer_path = root.join("composer");
        fs::write(
            &composer_path,
            format!(
                "#!/bin/sh\nPATH=/usr/bin:/bin\nset -C\ncase \"$1\" in\n  core-owner) printf '{{}}' > \"$2\" ;;\n  complete) test \"$(sha256sum < \"$3\" | cut -d' ' -f1)\" = {algebra} || exit 5; test -e {root}/reject && exit 3; cat {reference} > \"$4\" ;;\n  *) exit 2 ;;\nesac\n",
                algebra = imported.envelope.sha256.as_str(),
                root = root.display(),
                reference = reference.display(),
            ),
        )
        .unwrap();
        fs::set_permissions(&composer_path, fs::Permissions::from_mode(0o755)).unwrap();
        let executable = artifact(&composer_path).unwrap();
        let composer = ProcessDescriptor {
            id: Token::new("composer").unwrap(),
            executable: executable.path,
            executable_sha256: executable.sha256,
            arguments: vec![],
            environment: measurement_environment(),
            working_directory: root.clone(),
        };
        let channels = SessionChannels::for_stage(&stage).unwrap();
        let mut log = ExecutionLog::create_new(&stage, campaign.as_str(), "session-1").unwrap();
        log.append(JournalEvent::CampaignStart, None, json!({}))
            .unwrap();
        let revision = "0".repeat(40);
        fs::write(root.join("reject"), b"").unwrap();
        let refused = import_owner(
            &repository_root().unwrap(),
            &channels,
            &campaign,
            &revision,
            &composer,
            &imported,
            &mut log,
        )
        .unwrap_err();
        assert!(
            refused.to_string().contains("strict owner-only reopen"),
            "{refused}"
        );
        fs::remove_file(root.join("reject")).unwrap();
        let evidence = import_owner(
            &repository_root().unwrap(),
            &channels,
            &campaign,
            &revision,
            &composer,
            &imported,
            &mut log,
        )
        .unwrap();
        assert_eq!(evidence.staged.path, stage.join("algebra-owner.json"));
        assert_eq!(evidence.staged.sha256, imported.envelope.sha256);
        assert_eq!(evidence.source, imported.envelope);
        let mut drifted = imported.clone();
        drifted.envelope.sha256 = Sha256Digest::of(b"other");
        assert!(import_owner(
            &repository_root().unwrap(),
            &channels,
            &campaign,
            &revision,
            &composer,
            &drifted,
            &mut log,
        )
        .is_err());
        fs::write(
            stage.join("campaign.json"),
            encoded(&json!({
                "campaign_id": campaign,
                "identity": {"source_revision": revision},
                "processes": [composer],
                "imported_owners": [evidence],
            }))
            .unwrap(),
        )
        .unwrap();
        let validator = fs::canonicalize(
            repository_root()
                .unwrap()
                .join("dev/scripts/validate-tuning-extent-campaign.py"),
        )
        .unwrap();
        let check = || {
            Command::new("python3")
                .arg(&validator)
                .arg("--stage")
                .arg(&stage)
                .arg("--imported-owner-check")
                .output()
                .unwrap()
        };
        let output = check();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        fs::write(stage.join("algebra-owner.json"), b"{}").unwrap();
        assert!(
            !check().status.success(),
            "a changed staged import was accepted"
        );
    }

    #[test]
    fn validator_reproduces_the_rust_encodings_it_binds() {
        use tuning_campaign_support::statistics::DecisionReason;
        use tuning_campaign_support::timing::TimingSample;
        let scratch_root = scratch("gf2-driver-encoding-check");
        let identity = |task| UnitIdentity {
            protocol: Token::new("core-tuning-campaign-v4").unwrap(),
            owner: Token::new("gf2-core").unwrap(),
            campaign_id: Token::new("gf2-a83583e0-19700101t000000z-1").unwrap(),
            phase: Token::new("extent").unwrap(),
            field: Token::new("field_vec.dot_chunk_len").unwrap(),
            stratum: Token::new("shape-0").unwrap(),
            candidate: Token::new("128").unwrap(),
            task,
        };
        let pair = |value: &dyn erased::Encode| json!({"value": value.value(), "bytes": String::from_utf8(value.bytes()).unwrap()});
        let mut units = Vec::new();
        let mut results = Vec::new();
        let mut progress = Vec::new();
        let mut requests = Vec::new();
        for task in [Task::Probe, Task::Measure { execution: 3 }] {
            let unit = LaunchUnit::new(
                7,
                identity(task),
                Token::new("core-producer").unwrap(),
                CanonicalJson::new("{\"spread\":0.00009182966022193755}".to_owned()).unwrap(),
            )
            .unwrap();
            let samples = match task {
                Task::Probe => vec![],
                Task::Measure { execution } => (0..5)
                    .map(|repetition| {
                        TimingSample::new(execution, repetition, 1 << 32, 250_000_001).unwrap()
                    })
                    .collect(),
            };
            let result = ChildResult {
                schema: RESULT_SCHEMA.into(),
                identity: identity(task),
                case_sha256: Sha256Digest::of(unit.case.as_str().as_bytes()),
                outcome: ChildOutcome::Complete,
                samples,
                payload: CanonicalJson::new("{\"median\":1.5e-7}".to_owned()).unwrap(),
            };
            units.push(pair(&unit));
            results.push(pair(&result));
            requests.push(pair(&OwnerOperation::ValidateResult {
                unit: Box::new(unit.clone()),
                result: Box::new(result.clone()),
            }));
            if let Task::Measure { .. } = task {
                for kind in [
                    ProgressKind::CalibrationComplete { calls: 9 },
                    ProgressKind::WindowComplete {
                        repetition: 4,
                        calls: 9,
                        elapsed_ns: 250_000_001,
                    },
                ] {
                    progress.push(pair(
                        &ProgressRecord::new(identity(task), result.case_sha256.clone(), kind)
                            .unwrap(),
                    ));
                }
            }
        }
        // Typed f64 fields (child results, owner decisions) and the sorted
        // `Value` copies in checkpoints and journals.
        let values = [
            1e-5,
            9.182966022193755e-05,
            1.5e-6,
            1e-4,
            123.0,
            1e15,
            1e16,
            1e300,
            2.5e-7,
            -3.25e-8,
            5e-324,
            219_961_611.0,
        ];
        let mut floats: Vec<String> = values
            .iter()
            .map(|value| serde_json::to_string(&json!({ "x": [value] })).unwrap())
            .collect();
        floats.extend(
            values
                .iter()
                .map(|value| format!("{{\"x\":[{}]}}", serde_json::to_string(value).unwrap())),
        );
        let fixture = json!({
            "typed": {"unit": units, "result": results, "progress": progress, "validate_request": requests},
            "floats": floats,
            "selected_non_default": DecisionReason::SelectedNonDefault,
        });
        let path = scratch_root.path().join("fixture.json");
        fs::write(&path, serde_json::to_vec(&fixture).unwrap()).unwrap();
        let validator = fs::canonicalize(
            repository_root()
                .unwrap()
                .join("dev/scripts/validate-tuning-extent-campaign.py"),
        )
        .unwrap();
        let output = Command::new("python3")
            .arg(&validator)
            .arg("--encoding-check")
            .arg(&path)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    /// Serialized forms of one Rust value: the sorted-key JSON value that
    /// checkpoints and journals store, and the typed bytes that are hashed.
    mod erased {
        use serde::Serialize;
        pub trait Encode {
            fn value(&self) -> serde_json::Value;
            fn bytes(&self) -> Vec<u8>;
        }
        impl<T: Serialize> Encode for T {
            fn value(&self) -> serde_json::Value {
                serde_json::to_value(self).unwrap()
            }
            fn bytes(&self) -> Vec<u8> {
                serde_json::to_vec(self).unwrap()
            }
        }
    }
}
