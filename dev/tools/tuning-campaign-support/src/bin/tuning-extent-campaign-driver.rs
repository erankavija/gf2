//! Neutral, resumable owner-campaign process driver.
//!
//! The launcher builds and stages every executable before preparation. Each
//! handoff names one immutable session; only run-session executes measurement
//! or composition, under the inherited full-host flock. Finalization observes
//! wrapper return and release before publishing terminal evidence.
#![deny(unsafe_code)]
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::env;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};
use tuning_campaign_support::campaign::*;
use tuning_campaign_support::host::{
    command_text, inherited_lock, lock_available, require_affinity, CpuAffinity,
};
use tuning_campaign_support::journal::{
    CheckpointStore, ExecutionLog, JournalEvent, ResumeIdentity,
};
use tuning_campaign_support::process::{live_group, ProcessResult};
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct HostAdmissionPolicy {
    required_observations: Vec<String>,
    no_competing_substantial_work_required: bool,
    isolated_process_listing_is_sufficient: bool,
    held_mutex_is_sufficient: bool,
}
impl HostAdmissionPolicy {
    fn declared() -> Self {
        Self {
            required_observations: [
                "affinity",
                "available-memory",
                "competing-cpu-gpu-work",
                "cpu-features",
                "cpu-model",
                "governor",
                "load",
                "os-kernel",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
            no_competing_substantial_work_required: true,
            isolated_process_listing_is_sufficient: false,
            held_mutex_is_sufficient: false,
        }
    }
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
}
fn process(config: &CampaignConfig, id: &str) -> io::Result<ProcessDescriptor> {
    config
        .processes
        .iter()
        .find(|p| p.id.as_str() == id)
        .cloned()
        .ok_or_else(|| invalid("unknown staged process"))
}
fn process_command(process: &ProcessDescriptor, args: &[String]) -> io::Result<Command> {
    process.verify_staged()?;
    let mut command = Command::new(&process.executable);
    command
        .args(args)
        .current_dir(&process.working_directory)
        .env_clear()
        .envs(&process.environment);
    Ok(command)
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
    let start_sequence = log.as_ref().map(|l| l.next_sequence());
    if let Some(log) = log.as_mut() {
        record_budget(log, "before-launch", process.id.as_str(), None)?;
        log.append(JournalEvent::OrchestrationStart,None,json!({"kind":"orchestration-start","process":process.id,"request_sha256":Sha256Digest::of(input),"arguments":arguments}))?;
    }
    let result = run_process(
        process_command(process, arguments)?,
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
const PRODUCING_MANIFEST: &str = "dev/active/a83583e0/producing-build-inputs.json";
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProducingInputs {
    schema: String,
    behavior_sources: Vec<String>,
    lifecycle_sources: Vec<String>,
    build_inputs: Vec<String>,
}
impl ProducingInputs {
    fn read() -> io::Result<Self> {
        let value: Self =
            serde_json::from_slice(&fs::read(PRODUCING_MANIFEST)?).map_err(invalid)?;
        value.validate_at(Path::new("."))?;
        Ok(value)
    }
    fn validate_at(&self, root: &Path) -> io::Result<()> {
        if self.schema != "tuning-campaign-producing-inputs-v1" {
            return Err(invalid("producing input manifest schema mismatch"));
        }
        for paths in [
            &self.behavior_sources,
            &self.lifecycle_sources,
            &self.build_inputs,
        ] {
            if paths.is_empty() || paths.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(invalid(
                    "source manifest paths must be nonempty, sorted and unique",
                ));
            }
            for path in paths {
                if Path::new(path).is_absolute()
                    || Path::new(path)
                        .components()
                        .any(|part| !matches!(part, std::path::Component::Normal(_)))
                    || path.contains(['*', '?', '[', ']'])
                {
                    return Err(invalid(
                        "source manifest contains a nonliteral repository path",
                    ));
                }
                let metadata = fs::symlink_metadata(root.join(path))?;
                if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
                    return Err(invalid("source manifest path is not a regular file"));
                }
            }
        }
        if self
            .behavior_sources
            .iter()
            .any(|path| !self.build_inputs.contains(path))
            || self
                .lifecycle_sources
                .iter()
                .any(|path| !self.behavior_sources.contains(path))
        {
            return Err(invalid(
                "lifecycle/behavior manifest is not a subset of producing inputs",
            ));
        }
        Ok(())
    }
    fn hashes(paths: &[String]) -> io::Result<BTreeMap<String, String>> {
        paths
            .iter()
            .map(|path| {
                Ok((
                    path.clone(),
                    Sha256Digest::of(&fs::read(path)?).as_str().into(),
                ))
            })
            .collect()
    }
    fn lifecycle_digest(&self) -> io::Result<String> {
        Ok(
            Sha256Digest::of(&encoded(&Self::hashes(&self.lifecycle_sources)?)?)
                .as_str()
                .into(),
        )
    }
}
fn behavior_sources() -> io::Result<BTreeMap<String, String>> {
    ProducingInputs::hashes(&ProducingInputs::read()?.behavior_sources)
}
fn verify_config(config: &CampaignConfig) -> io::Result<()> {
    if config.schema != "tuning-extent-campaign-v1" || config.manifests.len() != 2 {
        return Err(invalid("campaign schema/owner mismatch"));
    }
    config.channels.validate()?;
    require_affinity(&config.affinity, &CpuAffinity::observe()?)?;
    let (revision, tree) = source_state()?;
    let producing = ProducingInputs::read()?;
    let staging: StagingManifest = read_json(&config.channels.stage.join("staging-manifest.json"))?;
    let (source_before, source_after) =
        validate_build_source_observations(&config.channels.stage, &staging)?;
    if revision != config.identity.source_revision
        || tree != config.source_tree
        || Sha256Digest::of(tree.as_bytes()).as_str() != config.identity.source_sha256
        || behavior_sources()? != config.identity.behavior_sha256
        || producing.lifecycle_digest()? != config.identity.lifecycle_behavior_sha256
        || ProducingInputs::hashes(&producing.build_inputs)? != config.build_inputs
        || artifact(Path::new(PRODUCING_MANIFEST))? != config.producing_manifest
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

    for m in &config.manifests {
        m.validate()?;
        if m.campaign_id != config.campaign_id
            || m.processes.len() != 1
            || !config.processes.contains(&m.processes[0])
        {
            return Err(invalid("owner manifest campaign/process mismatch"));
        }
    }
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
    if config.manifests[0].counts != DeclaredCounts::for_cells(702)?
        || config.manifests[1].counts != DeclaredCounts::for_cells(15)?
    {
        return Err(invalid("campaign cell accounting changed"));
    }
    if artifact(&config.protocol.path)? != config.protocol
        || artifact(&config.validator.path)? != config.validator
    {
        return Err(invalid("protocol or validator changed"));
    }
    if config.host_admission_policy != HostAdmissionPolicy::declared() {
        return Err(invalid(
            "host admission policy differs from the declaration",
        ));
    }
    validate_preflight_identities(&config.channels.stage, &config.preflight_reports)?;
    for (name, saved) in &config.preflight_reports {
        if artifact(&saved.path)? != *saved {
            return Err(invalid("saved staged CLI preflight changed"));
        }
        let response: OwnerResponse = read_json(&saved.path)?;
        verify_report_response(name, &response)?;
    }
    Ok(())
}

fn session_descriptor(
    config: &CampaignConfig,
    session_id: &str,
    lock: &Path,
) -> io::Result<SessionDescriptor> {
    Ok(SessionDescriptor {
        schema: LIFECYCLE_SCHEMA.into(),
        campaign_id: config.campaign_id.clone(),
        session_id: Token::new(session_id)?,
        preparer: ProcessIdentity::current()?,
        channels: config.channels.clone(),
        identity: config.identity.clone(),
        counts: DeclaredCounts::for_cells(717)?,
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
) -> io::Result<()> {
    let expected_reports: std::collections::BTreeSet<_> = ["core", "algebra"]
        .into_iter()
        .flat_map(|owner| {
            ["self-check", "list-grid", "capability-report"]
                .map(move |mode| format!("{owner}-{mode}"))
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
fn stage_executables(stage: &Path, input: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let staging: StagingManifest = read_json(input)?;
    if staging.schema != "tuning-campaign-staging-v1"
        || staging
            .executables
            .keys()
            .map(Token::as_str)
            .collect::<Vec<_>>()
            != ["algebra-producer", "composer", "core-producer", "driver"]
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
    let producing = ProducingInputs::read()?;
    let behavior = ProducingInputs::hashes(&producing.behavior_sources)?;
    let receipt = format!("dev/benchmarks/tuning_profiles/{}.md", campaign_id.as_str());
    let runtime = host_runtime(&receipt)?;
    let affinity = CpuAffinity::observe()?;
    let repository = fs::canonicalize(".")?;
    let mut processes = Vec::new();
    for (id, args) in [
        ("core-producer", vec!["--fresh-tuning-process-child".into()]),
        ("algebra-producer", vec!["--fresh-child".into()]),
        ("composer", vec![]),
        ("driver", vec![]),
    ] {
        let binary = artifact(&channels.stage.join("bin").join(id))?;
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
        protocol: artifact(&repository.join("dev/active/a83583e0/premeasurement-protocol.md"))?,
        validator: artifact(&repository.join("dev/scripts/validate-tuning-extent-campaign.py"))?,
        affinity,
        staging: artifact(&channels.stage.join("staging-manifest.json"))?,
        producing_manifest: artifact(Path::new(PRODUCING_MANIFEST))?,
        build_inputs: ProducingInputs::hashes(&producing.build_inputs)?,
        host_admission_policy: HostAdmissionPolicy::declared(),
    })
}

fn validate_campaign_stage(stage: &Path, campaign_id: &str) -> io::Result<()> {
    let suffix = campaign_id
        .strip_prefix("gf2-a83583e0-")
        .ok_or_else(|| invalid("campaign ID has the wrong issue prefix"))?;
    let (stamp, pid) = suffix
        .rsplit_once('-')
        .ok_or_else(|| invalid("campaign ID lacks launcher PID"))?;
    if stamp.len() != 16
        || stamp.as_bytes().get(8) != Some(&b'T')
        || !stamp[..8].bytes().all(|byte| byte.is_ascii_digit())
        || !stamp[9..15].bytes().all(|byte| byte.is_ascii_digit())
        || !stamp.ends_with('Z')
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
    validate_campaign_stage(stage, campaign_id)?;
    if !stage.exists() {
        fs::create_dir(stage)?;
        File::open("/tmp")?.sync_all()?;
    }
    let channels = SessionChannels::for_stage(stage)?;
    if let Some(input) = staging {
        stage_executables(&channels.stage, input)?;
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
    let inputs = bootstrap_inputs(&channels, &campaign_id)?;
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
        for (i, name) in [(0, "core"), (1, "algebra")] {
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
        let producing = ProducingInputs::read()?;
        let build_inputs = ProducingInputs::hashes(&producing.build_inputs)?;
        let producing_manifest = artifact(Path::new(PRODUCING_MANIFEST))?;
        let lifecycle_behavior = ProducingInputs::hashes(&producing.lifecycle_sources)?;
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
        process_command(&process, &process.arguments)?,
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
    let mut receipt=format!("# Extent calibration {}\n\n## Campaign identity and protocol\n\nProtocol: `{}`; SHA-256 `{}`. Producing commit: `{}`.\n\n## Section-specific provenance and assembly\n\nSee `campaign.json`, owner responses and `composition.json` for runtime observations, executable and behavior identities, and strict codec evidence.\n\n## Grids, controls, and seed allocation\n\nThe immutable owner manifests contain every acquisition slot and opaque owner case. Each accepted payload contains its full seed, fixture, route and semantic witness.\n\n## Coverage, accounting, and resume history\n\nThe execution journal and checkpoint manifest are authoritative for attempts, accepted results, sessions, lock observations, censored intervals, and orchestration.\n\n## Effective routes and semantic witnesses\n\nSee each raw result payload below.\n\n## Raw samples and uncertainty\n\nEvery raw key resolves through `receipt-projection.json` raw_artifacts; five timing windows, calls and elapsed nanoseconds remain in each timed record.\n\n## Argmin and threshold decisions\n\nGEMM row/column decisions are joint; dot chunk decisions cite this campaign. Owner projections preserve ties, schedule plateaus, cross-stratum conflicts, conditional M4RM decisions and fallbacks:\n\n```json\n{}\n```\n\n## Owner and complete validation\n\nOwner responses record strict owner-only reopen. Composition preserves each complete section wrapper. Independent validation recomputes the estimators and evidence accounting.\n\n## Limitations\n\nMeasured choices are conditional on this host, declared grid, controls, and protocol. Unmeasured leaves remain omissions. Timing intervals are empirical measurements, not Monte Carlo probability estimates.\n\n## Raw result index\n\n",config.campaign_id.as_str(),config.protocol.path.display(),config.protocol.sha256.as_str(),config.identity.source_revision,serde_json::to_string_pretty(responses).map_err(invalid)?);
    receipt.push_str(&format!("Preparation CPU affinity: `{:?}`. Held-lock observations are recorded in each session journal and must equal this set.\n\n",config.affinity.cpus()));
    for manifest in &config.manifests {
        receipt.push_str(&format!(
            "Owner `{}` uses protocol `{}` and behavior `{}`; executable `{}`.\n\n",
            manifest.owner.as_str(),
            manifest.owner_protocol.as_str(),
            manifest.behavior_token.as_str(),
            manifest.processes[0].executable_sha256.as_str()
        ));
    }
    receipt.push_str(&format!("Accepted accounting: {} cells, {} probes, {} timed children, {} accepted results, {} raw windows, {} timing progress records. Observed {} attempts, {} orchestration actions, {} sessions at the receipt projection journal_sequence. Later finalization and resume events remain in the authoritative journal.\n\n",counts.cells,counts.probes,counts.timed_children,counts.accepted_results,counts.windows,counts.progress_records,attempts,orchestration,sessions));
    for bundle in bundles {
        for accepted in &bundle.accepted {
            receipt.push_str(&format!(
                "- `{}`: `{}` / `{}` / `{}` / `{:?}`\n",
                accepted.unit.key.as_str(),
                accepted.unit.identity.field.as_str(),
                accepted.unit.identity.stratum.as_str(),
                accepted.unit.identity.candidate.as_str(),
                accepted.unit.identity.task
            ));
        }
    }
    let path = stage.join("receipt.md");
    publish_artifact(stage, &path, receipt.as_bytes())?;
    Ok(())
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
    let mut responses = Vec::new();
    for (i, name) in [(0, "core"), (1, "algebra")] {
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
            process_command(&composer, &args)?,
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
    make_receipt(config, bundles, &responses)?;
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
fn finish_checksum(store: SessionStore, log: &mut ExecutionLog) -> io::Result<()> {
    let descriptor = store.descriptor().clone();
    let stage = &descriptor.channels.stage;
    fn collect(path: &Path, artifacts: &mut Vec<ArtifactIdentity>) -> io::Result<()> {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let path = entry.path();
            let name = entry.file_name();
            if [
                "execution.log",
                "active-session.json",
                "active-preparation.json",
                "session-writer.lock",
                "sessions",
                "artifact-publications",
            ]
            .iter()
            .any(|skip| name == *skip)
            {
                continue;
            }
            if entry.file_type()?.is_symlink() {
                return Err(invalid("symlink in staged evidence"));
            }
            if path.is_dir() {
                collect(&path, artifacts)?;
            } else {
                artifacts.push(artifact(&path)?);
            }
        }
        Ok(())
    }
    let mut artifacts = Vec::new();
    collect(stage, &mut artifacts)?;
    let identity = store.write_checksum(log, artifacts)?;
    store.retire(log, &identity)
}
fn validator(config: &CampaignConfig, preterminal: bool) -> io::Result<()> {
    if artifact(&config.validator.path)? != config.validator {
        return Err(invalid("validator identity changed"));
    }
    let mut command = Command::new("python3");
    command
        .arg(&config.validator.path)
        .arg("--stage")
        .arg(&config.channels.stage);
    if preterminal {
        command.arg("--preterminal");
    }
    let output = command.output()?;
    io::stdout().write_all(&output.stdout)?;
    io::stderr().write_all(&output.stderr)?;
    if !output.status.success() {
        return Err(invalid("independent validation failed"));
    }
    Ok(())
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
fn main() {
    let args: Vec<_> = env::args().skip(1).collect();
    let result=match args.as_slice(){
        [mode,stage] if mode=="discover-preparation"=>discover_preparation(Path::new(stage)).map(|_|SessionOutcome::Paused),
        [mode,stage,campaign,session,lock,staging] if mode=="prepare-session"=>prepare(Path::new(stage),campaign,session,Path::new(lock),Some(Path::new(staging))).map(|_|SessionOutcome::Paused),
        [mode,stage,campaign,session,lock] if mode=="prepare-session"=>prepare(Path::new(stage),campaign,session,Path::new(lock),None).map(|_|SessionOutcome::Paused),
        [mode,stage,session] if mode=="run-session"=>run_session(Path::new(stage),session),
        [mode,stage,session,status] if mode=="finalize-session"=>status.parse::<i32>().map_err(invalid).and_then(|status|finalize(Path::new(stage),session,status)),
        _=>Err(invalid("usage: driver discover-preparation STAGE | prepare-session STAGE CAMPAIGN SESSION LOCK [STAGING_INPUT] | run-session STAGE SESSION | finalize-session STAGE SESSION WRAPPER_EXIT"))
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
        let output = process_command(&process, &[]).unwrap().output().unwrap();
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

        let stage = scratch("report-cli");
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
        fs::remove_dir_all(stage).unwrap();
    }

    fn scratch(name: &str) -> std::path::PathBuf {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = env::temp_dir().join(format!(
            "gf2-driver-{}-{}-{name}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        path
    }
    #[test]
    fn incomplete_executable_staging_replays_before_campaign_config_exists() {
        let stage = scratch("staging-replay");
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
        stage_executables(&stage, &input).unwrap();
        assert!(!stage.join("campaign.json").exists());
        for name in staging.executables.keys() {
            assert_eq!(
                artifact(&stage.join("bin").join(name.as_str()))
                    .unwrap()
                    .sha256,
                source.sha256
            );
        }
        stage_executables(&stage, &input).unwrap();
        fs::write(&source.path, b"changed producer").unwrap();
        assert!(stage_executables(&stage, &input).is_err());
        fs::remove_dir_all(stage).unwrap();
    }

    #[test]
    fn relocated_build_source_observation_is_rejected_before_campaign_work() {
        let stage = scratch("relocated-build-source");
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
        fs::remove_dir_all(stage).unwrap();
    }

    #[test]
    fn relocated_preflight_report_is_rejected_before_campaign_work() {
        let stage = scratch("relocated-preflight");
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

        assert!(validate_preflight_identities(&stage, &reports).is_err());
        assert!(!stage.join("execution.log").exists());
        assert!(!stage.join("active-session.json").exists());
        fs::remove_dir_all(stage).unwrap();
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
        CampaignConfig,
        LaunchUnit,
        ChildResult,
        ExecutionLog,
        CheckpointStore,
    ) {
        use std::os::unix::fs::PermissionsExt;
        let stage = scratch("probe-checkpoint");
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
            working_directory: stage.clone(),
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
        };
        let mut log = ExecutionLog::create_new(&stage, campaign.as_str(), "first-session").unwrap();
        log.append(JournalEvent::CampaignStart, None, json!({}))
            .unwrap();
        let checkpoints =
            CheckpointStore::create_new(&channels.checkpoints, campaign.as_str(), identity)
                .unwrap();
        (config, unit, expected, log, checkpoints)
    }
    #[test]
    fn interrupted_derived_projection_publication_is_replayed_and_bound() {
        let (config, unit, _result, _log, _checkpoints) = probe_fixture(b"");
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
        fs::remove_dir_all(stage).unwrap();
    }
    #[test]
    fn rejected_raw_streams_cannot_recover_after_completion_crash() {
        for stderr in [
            b"unterminated".as_slice(),
            b"\xff".as_slice(),
            b"GF2_TUNING_PROGRESS={}\n".as_slice(),
        ] {
            let (config, unit, _expected, mut log, mut checkpoints) = probe_fixture(stderr);
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
            fs::remove_dir_all(&config.channels.stage).unwrap();
        }
    }
    #[test]
    fn clean_completion_crashes_recover_without_fresh_child_replay() {
        for stop in [
            UnitBoundary::Completion,
            UnitBoundary::Exit,
            UnitBoundary::Validation,
        ] {
            let (config, unit, expected, mut log, mut checkpoints) = probe_fixture(b"");
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
            fs::remove_dir_all(&config.channels.stage).unwrap();
        }
    }
    #[test]
    fn a_probe_streams_validates_and_commits_one_bound_checkpoint() {
        let (config, unit, expected, mut log, mut checkpoints) = probe_fixture(b"");
        let stage = config.channels.stage.clone();
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
        fs::remove_dir_all(stage).unwrap();
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
        let stage = scratch("promotion");
        let source = stage.join("candidate.json");
        let target = stage.join("owner.json");
        fs::write(&source, b"canonical candidate").unwrap();
        let source = artifact(&source).unwrap();
        let first = promote(&source, &target).unwrap();
        assert_eq!(promote(&source, &target).unwrap(), first);
        fs::write(&target, b"different").unwrap();
        assert!(promote(&source, &target).is_err());
        fs::remove_dir_all(stage).unwrap();
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
        let stage = scratch("lock");
        let path = stage.join("host.lock");
        let lock = File::create(&path).unwrap();
        assert!(lock_available(&path).unwrap());
        lock.lock().unwrap();
        assert!(!lock_available(&path).unwrap());
        assert_eq!(inherited_lock(&path).unwrap(), std::process::id());
        lock.unlock().unwrap();
        assert!(inherited_lock(&path).is_err());
        fs::remove_dir_all(stage).unwrap();
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
        let campaign = format!("gf2-a83583e0-20260905T000000Z-{}", std::process::id());
        let expected = Path::new("/tmp").join(&campaign);
        assert!(validate_campaign_stage(&expected, &campaign).is_ok());
        assert!(validate_campaign_stage(Path::new("/tmp/other"), &campaign).is_err());
        assert!(validate_campaign_stage(&expected, "gf2-a83583e0-20260905-000000-1").is_err());
        assert!(validate_campaign_stage(
            Path::new("/home/example/gf2-a83583e0-20260905T000000Z-1"),
            "gf2-a83583e0-20260905T000000Z-1"
        )
        .is_err());
    }

    #[test]
    fn producing_input_manifest_rejects_authority_and_path_mutations() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .canonicalize()
            .unwrap();
        let manifest: ProducingInputs =
            serde_json::from_slice(&fs::read(root.join(PRODUCING_MANIFEST)).unwrap()).unwrap();
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
}
