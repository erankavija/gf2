//! Reusable execution of one frozen rare-event configuration.
//!
//! This module owns the whole run: runtime observation, attempt lifecycle,
//! block production, checkpoint publication, reduction, and final receipt
//! publication. A binary consuming it decodes its configuration argument,
//! delegates here, and maps the returned status onto an exit code.
//!
//! A run is bounded and resumable. `GF2_RARE_EVENT_BLOCK_BUDGET` caps how many
//! checkpoint blocks one invocation produces; the next invocation reconstructs
//! the published prefix from the dataset directories and continues.
//!
//! All environment access that can affect execution passes through one
//! instrumented layer: `read_declared_environment` refuses any name outside
//! `ENVIRONMENT_INPUT_NAMES`, each declared name is read exactly once, and the
//! start receipt's environment record is derived from those same reads.

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::Path;

use gf2_stats::sampler::MatrixAddress;

use super::artifact::observe::observation_utc_now;
use super::artifact::result::{
    coverage_result_payload, decode_exact_target_result, target_result_payload,
};
use super::artifact::{
    attempt_id, begin_attempt, canonical_bytes, coverage_final_artifact, create_dataset_directory,
    dataset_id, expected_checkpoint_block_addresses, finish_attempt,
    normalized_accelerator_not_used_evidence, observe_host, observe_self_identity,
    publish_artifact, publish_checkpoint, read_repository_file, reconstruct_attempt_chain,
    reconstruct_execution_lineage, reconstruct_published_checkpoints, recover_interrupted_attempt,
    run_id, sha256_hex, target_final_artifact, validate_configuration, validate_relative_path,
    verify_atomic_publication_support, AcceleratorObservationV1, ArtifactError, AttemptOutcomeV1,
    AttemptPhaseV1, AttemptPredecessorV1, AttemptStartArtifactV1, AttemptTerminalArtifactV1,
    CampaignAuthorityV1, CheckpointArtifactV1, CheckpointRefV1, CoverageTrajectoryRecordV1,
    EndTimeMeaningV1, EnvironmentInputV1, EnvironmentValueV1, ExecutionAttemptReceiptV1,
    InputOriginV1, InputResolutionV1, InvocationV1, OutcomeObserverV1, PreregistrationIdentityV1,
    ProducerBackendV1, RareEventConfigurationV1, RareEventDatasetIdentityV1, RunAddressV1,
    ScientificIdentityV1, StatisticalConstantsV1, TargetTrajectoryRecordV1, TrajectoryCheckpointV1,
    TrajectoryRecordsV1, ValidatedAttemptArtifact,
};
use super::{
    coverage_address, sample_trajectories_in_order, target_address, RareEventError,
    COVERAGE_TRAJECTORIES_PER_RUN,
};

/// Environment name declaring the per-invocation checkpoint-block budget.
pub const BLOCK_BUDGET_ENVIRONMENT: &str = "GF2_RARE_EVENT_BLOCK_BUDGET";
/// Environment name declaring the requested and effective worker count.
pub const WORKER_ENVIRONMENT: &str = "RAYON_NUM_THREADS";
/// Artifact-root-relative exact target result a completing target run reads.
///
/// The issue that executes a target campaign commits these bytes before its
/// first draw; the final receipt then cites them by path and digest. No such
/// file is committed for this issue, which exercises the estimator rather than
/// executing a campaign.
pub const EXACT_TARGET_RESULT_FILE: &str = "exact-target-result.json";

/// Every environment name this runner may consult.
///
/// This one inventory is what the module's environment reader permits, what a
/// start receipt records, and what a frozen configuration's behavior closure
/// declares, so reading, recording, and declaring an environment input cannot
/// drift apart. A receipt whose declared names differ from its closure is
/// refused at publication.
pub const ENVIRONMENT_INPUT_NAMES: [&str; 2] = [BLOCK_BUDGET_ENVIRONMENT, WORKER_ENVIRONMENT];

/// Reads one declared environment input, refusing an undeclared name.
///
/// Every environment read in this module passes through here, so consulting a
/// name outside [`ENVIRONMENT_INPUT_NAMES`] fails closed instead of silently
/// letting an unrecorded input affect execution.
///
/// An absent name reads as `None`, which a receipt records as unset. A name
/// holding bytes that are not UTF-8 is refused rather than reported absent,
/// because a receipt may record only an exact UTF-8 value or an exact absence.
///
/// # Errors
///
/// Refuses a name this runner has not declared, and a declared name whose
/// value is not UTF-8.
fn read_declared_environment(name: &str) -> Result<Option<String>, RunError> {
    if !ENVIRONMENT_INPUT_NAMES.contains(&name) {
        return Err(RunError::Configuration(format!(
            "{name} is consulted but is not a declared runner environment input"
        )));
    }
    match std::env::var(name) {
        Ok(value) => Ok(Some(value)),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(std::env::VarError::NotUnicode(_)) => Err(RunError::Configuration(format!(
            "{name} is set to a non-UTF-8 value, which the behavior closure rejects"
        ))),
    }
}

/// A configuration, observation, sampling, or artifact refusal.
#[derive(Debug)]
pub enum RunError {
    /// An artifact schema, identity, lineage, or publication refusal.
    Artifact(ArtifactError),
    /// A trajectory address or sampling refusal.
    Sampling(RareEventError),
    /// A frozen configuration or declared runtime input is unusable.
    Configuration(String),
    /// Filesystem access failed.
    Io(std::io::Error),
}

impl fmt::Display for RunError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Artifact(error) => write!(formatter, "{error}"),
            Self::Sampling(error) => write!(formatter, "rare-event sampling refusal: {error}"),
            Self::Configuration(message) => {
                write!(formatter, "rare-event configuration refusal: {message}")
            }
            Self::Io(error) => write!(formatter, "rare-event filesystem error: {error}"),
        }
    }
}

impl std::error::Error for RunError {}

impl From<ArtifactError> for RunError {
    fn from(error: ArtifactError) -> Self {
        Self::Artifact(error)
    }
}

impl From<RareEventError> for RunError {
    fn from(error: RareEventError) -> Self {
        Self::Sampling(error)
    }
}

impl From<std::io::Error> for RunError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

/// Closed completion status of one bounded run.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunStatus {
    /// Every block is published and the final receipt exists.
    Complete,
    /// Durable progress is published and blocks remain for the next run.
    Incomplete,
}

/// What one bounded run durably established.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunOutcome {
    status: RunStatus,
    dataset_id: String,
    attempt_id: String,
    published_blocks: usize,
    remaining_blocks: usize,
    final_receipt_sha256: Option<String>,
}

impl RunOutcome {
    /// Returns whether the dataset is complete or awaits another run.
    #[must_use]
    pub fn status(&self) -> RunStatus {
        self.status
    }

    /// Returns the recomputed dataset ID this run advanced.
    #[must_use]
    pub fn dataset_id(&self) -> &str {
        &self.dataset_id
    }

    /// Returns the attempt this run opened and closed.
    #[must_use]
    pub fn attempt_id(&self) -> &str {
        &self.attempt_id
    }

    /// Returns how many checkpoint blocks this run published.
    #[must_use]
    pub fn published_blocks(&self) -> usize {
        self.published_blocks
    }

    /// Returns how many checkpoint blocks remain unpublished.
    #[must_use]
    pub fn remaining_blocks(&self) -> usize {
        self.remaining_blocks
    }

    /// Returns the published final-receipt digest once the dataset completes.
    #[must_use]
    pub fn final_receipt_sha256(&self) -> Option<&str> {
        self.final_receipt_sha256.as_deref()
    }
}

/// Reconstructs the complete dataset identity from one frozen configuration.
///
/// Both the configuration and the path it was read from are revalidated here
/// through the library's canonical checks, so a caller holding a hand-built
/// configuration is held to exactly the grammar `decode_configuration`
/// enforces. This runs before any filesystem effect in a run.
///
/// # Errors
///
/// Refuses a configuration outside its closed grammar, and a configuration
/// path that is not normalized repository-relative.
pub fn dataset_identity(
    configuration: &RareEventConfigurationV1,
    configuration_path: &str,
) -> Result<RareEventDatasetIdentityV1, ArtifactError> {
    validate_configuration(configuration)?;
    validate_relative_path(configuration_path)?;
    let configuration_sha256 = sha256_hex(&canonical_bytes(configuration)?);
    Ok(RareEventDatasetIdentityV1 {
        campaign: CampaignAuthorityV1::default(),
        preregistration: PreregistrationIdentityV1 {
            design: configuration.design_identity.clone(),
            configuration_path: configuration_path.to_owned(),
            configuration_schema: configuration.configuration_schema.clone(),
            configuration_sha256,
        },
        scientific: configuration.scientific_identity.clone(),
        statistical: StatisticalConstantsV1::default(),
        behavior: configuration.behavior.clone(),
    })
}

/// Executes one bounded, resumable run of a frozen configuration.
///
/// The run recovers any attempt an earlier process left open, opens its own
/// attempt against the reconstructed published prefix, produces up to its
/// declared block budget, closes the attempt, and publishes the final receipt
/// once the closed block set is complete.
///
/// Nothing is created on disk until the configuration, its path, and its
/// artifact root have all been accepted, so an invalid configuration leaves no
/// trace. The dataset directory is then created through held descriptors like
/// every other directory beneath the artifact root.
///
/// # Errors
///
/// Refuses an unusable configuration or artifact root, a running executable
/// that is not the one the configuration pins, an unset worker environment,
/// and every artifact schema, lineage, or publication refusal beneath.
pub fn execute_frozen_run(
    configuration_path: &str,
    configuration: &RareEventConfigurationV1,
) -> Result<RunOutcome, RunError> {
    // Every acceptance check runs before the first filesystem effect.
    let identity = dataset_identity(configuration, configuration_path)?;
    let dataset_id = dataset_id(&identity)?;
    let dataset_dir = create_dataset_directory(&configuration.artifact_root, &dataset_id)?;
    verify_atomic_publication_support(&dataset_dir)?;

    let runtime = RuntimeInputs::observe(configuration, configuration_path)?;
    if runtime.invocation.executable_sha256 != configuration.behavior.executable_sha256 {
        return Err(RunError::Configuration(
            "the running executable is not the one this configuration pins".into(),
        ));
    }

    // Close whatever an interrupted process left open before extending the chain.
    let attempts = reconstruct_attempt_chain(&dataset_dir, &identity)?;
    if let Some(open_start) = attempts.open_start() {
        recover_interrupted_attempt(
            &dataset_dir,
            open_start,
            &runtime.invocation.executable_sha256,
        )?;
    }

    let attempts = reconstruct_attempt_chain(&dataset_dir, &identity)?;
    let completed = attempts.completed_phases();
    let accepted: Vec<CheckpointRefV1> =
        reconstruct_published_checkpoints(&dataset_dir, &identity)?
            .iter()
            .map(|checkpoint| checkpoint.checkpoint_ref().clone())
            .collect();
    let expected_addresses = expected_checkpoint_block_addresses(&identity)?;
    let published: Vec<_> = accepted
        .iter()
        .map(|reference| reference.block_address.clone())
        .collect();
    let missing: Vec<_> = expected_addresses
        .iter()
        .filter(|address| published.binary_search(address).is_err())
        .cloned()
        .collect();

    let ordinal = (completed.len() / 2) as u64;
    let predecessor = match completed.last() {
        None => AttemptPredecessorV1::None {},
        Some(terminal) => AttemptPredecessorV1::Terminal {
            terminal_sha256: terminal.digest().to_owned(),
        },
    };
    let start = AttemptStartArtifactV1::new(ExecutionAttemptReceiptV1 {
        dataset_id: dataset_id.clone(),
        attempt_id: attempt_id(&identity, ordinal, &predecessor)?,
        attempt_ordinal: ordinal,
        predecessor,
        dataset_identity: identity.clone(),
        phase: AttemptPhaseV1::Start {
            resume_checkpoint_refs: accepted,
            start_utc: runtime.start_utc.clone(),
            start_receipt_utc: observation_utc_now()?,
            invocation: Box::new(runtime.invocation.clone()),
            environment_inputs: runtime.environment_inputs.clone(),
            worker_configuration: runtime.worker.clone(),
            host_observation: Box::new(runtime.host.clone()),
            accelerator_observation: runtime.accelerator.clone(),
        },
    })?;
    let validated_start = begin_attempt(&dataset_dir, &start)?;

    let budget = runtime.block_budget.unwrap_or(missing.len());
    let mut produced = Vec::new();
    for address in missing.iter().take(budget) {
        let payload = produce_block(&identity, &dataset_id, &validated_start, &runtime, address)?;
        let published = publish_checkpoint(&dataset_dir, &CheckpointArtifactV1::new(payload)?)?;
        produced.push(published.checkpoint_ref().clone());
    }

    let terminal = AttemptTerminalArtifactV1::new(ExecutionAttemptReceiptV1 {
        dataset_id: dataset_id.clone(),
        attempt_id: validated_start.attempt_id().to_owned(),
        attempt_ordinal: ordinal,
        predecessor: validated_start.payload().predecessor.clone(),
        dataset_identity: identity.clone(),
        phase: AttemptPhaseV1::Terminal {
            attempt_start_sha256: validated_start.digest().to_owned(),
            start_utc: runtime.start_utc.clone(),
            end_utc: observation_utc_now()?,
            end_time_meaning: EndTimeMeaningV1::ProcessObserved,
            monotonic_elapsed_ns: None,
            host_observation_sha256: sha256_hex(&canonical_bytes(&runtime.host)?),
            accelerator_observation_sha256: sha256_hex(&canonical_bytes(&runtime.accelerator)?),
            outcome_observer: OutcomeObserverV1::SupervisingLauncher {
                launcher_sha256: runtime.invocation.executable_sha256.clone(),
            },
            outcome: AttemptOutcomeV1::Completed {},
            checkpoint_refs: produced.clone(),
        },
    })?;
    finish_attempt(&dataset_dir, &validated_start, &terminal)?;

    let remaining = missing.len() - produced.len();
    let final_receipt_sha256 = if remaining == 0 {
        Some(publish_final_receipt(
            &dataset_dir,
            &identity,
            &configuration.artifact_root,
        )?)
    } else {
        None
    };
    Ok(RunOutcome {
        status: if remaining == 0 {
            RunStatus::Complete
        } else {
            RunStatus::Incomplete
        },
        dataset_id,
        attempt_id: validated_start.attempt_id().to_owned(),
        published_blocks: produced.len(),
        remaining_blocks: remaining,
        final_receipt_sha256,
    })
}

/// Reduces the complete dataset and publishes its final receipt.
fn publish_final_receipt(
    dataset_dir: &Path,
    identity: &RareEventDatasetIdentityV1,
    artifact_root: &str,
) -> Result<String, RunError> {
    let (checkpoints, lineage) = reconstruct_execution_lineage(dataset_dir, identity)?;
    let published = match &identity.scientific {
        ScientificIdentityV1::Target { .. } => {
            let path = format!("{artifact_root}/{EXACT_TARGET_RESULT_FILE}");
            let bytes = read_repository_file(&path)?;
            let exact = decode_exact_target_result(identity, &path, &bytes)?;
            let payload = target_result_payload(&checkpoints, &exact)?;
            let receipt = target_final_artifact(identity.clone(), payload, &checkpoints, &lineage)?;
            publish_artifact(dataset_dir, &receipt)?
        }
        ScientificIdentityV1::Coverage { .. } => {
            let payload = coverage_result_payload(identity, &checkpoints)?;
            let receipt =
                coverage_final_artifact(identity.clone(), payload, &checkpoints, &lineage)?;
            publish_artifact(dataset_dir, &receipt)?
        }
    };
    Ok(published.digest().to_owned())
}

/// Samples and records one complete immutable checkpoint block.
fn produce_block(
    identity: &RareEventDatasetIdentityV1,
    dataset_id: &str,
    start: &ValidatedAttemptArtifact,
    runtime: &RuntimeInputs,
    block_address: &str,
) -> Result<TrajectoryCheckpointV1, RunError> {
    let components: Vec<_> = block_address.split('/').collect();
    let (run_address, block_index, trajectory_start, trajectory_end, records) =
        match &identity.scientific {
            ScientificIdentityV1::Target { n, block_size, .. } => {
                let run = parse_component(components.get(1), "target run")?;
                let block: u16 = parse_component(components.get(2), "target block")?;
                let start_index = u32::from(block) * block_size;
                let end_index = start_index + block_size;
                let addresses: Vec<MatrixAddress> = (start_index..end_index)
                    .map(|trajectory| target_address(run, trajectory))
                    .collect::<Result<_, _>>()?;
                let outcomes = sample_trajectories_in_order::<3>(
                    usize::from(*n),
                    &addresses,
                    runtime.worker.effective_workers,
                )?;
                let records = outcomes
                    .iter()
                    .zip(start_index..end_index)
                    .map(|(outcome, trajectory)| TargetTrajectoryRecordV1 {
                        run,
                        trajectory,
                        stream_index: outcome.stream_index,
                        exponent: outcome.exponent,
                    })
                    .collect();
                (
                    RunAddressV1::Target { run },
                    block,
                    start_index,
                    end_index,
                    TrajectoryRecordsV1::Target(records),
                )
            }
            ScientificIdentityV1::Coverage { cases, .. } => {
                let q: u8 =
                    parse_component(components.get(1).map(|field| &field[1..]), "coverage field")?;
                let replicate: u16 = parse_component(
                    components.get(2).map(|field| &field[1..]),
                    "coverage replicate",
                )?;
                let run: u16 =
                    parse_component(components.get(3).map(|field| &field[1..]), "coverage run")?;
                let case = cases.iter().find(|case| case.q == q).ok_or_else(|| {
                    RunError::Configuration("coverage block names an unregistered field".into())
                })?;
                let addresses: Vec<MatrixAddress> = (0..COVERAGE_TRAJECTORIES_PER_RUN)
                    .map(|trajectory| coverage_address(q, replicate, run, trajectory))
                    .collect::<Result<_, _>>()?;
                let rows = usize::from(case.n);
                let workers = runtime.worker.effective_workers;
                let outcomes = match q {
                    3 => sample_trajectories_in_order::<3>(rows, &addresses, workers)?,
                    5 => sample_trajectories_in_order::<5>(rows, &addresses, workers)?,
                    7 => sample_trajectories_in_order::<7>(rows, &addresses, workers)?,
                    other => return Err(RareEventError::UnsupportedField(other).into()),
                };
                let records = outcomes
                    .iter()
                    .enumerate()
                    .map(|(index, outcome)| CoverageTrajectoryRecordV1 {
                        q,
                        replicate,
                        run,
                        trajectory: index as u32,
                        stream_index: outcome.stream_index,
                        exponent: outcome.exponent,
                    })
                    .collect();
                (
                    RunAddressV1::Coverage { q, replicate, run },
                    0,
                    0,
                    COVERAGE_TRAJECTORIES_PER_RUN,
                    TrajectoryRecordsV1::Coverage(records),
                )
            }
        };
    Ok(TrajectoryCheckpointV1 {
        dataset_id: dataset_id.to_owned(),
        run_id: run_id(identity, &run_address)?,
        dataset_identity: identity.clone(),
        run_address,
        block_index,
        trajectory_start,
        trajectory_end,
        attempt_id: start.attempt_id().to_owned(),
        attempt_start_sha256: start.digest().to_owned(),
        producer: ProducerBackendV1::Cpu {},
        accelerator_observation_sha256: sha256_hex(&canonical_bytes(&runtime.accelerator)?),
        records,
    })
}

/// Parses one decimal component of a canonical block address.
fn parse_component<T: std::str::FromStr>(
    component: Option<impl AsRef<str>>,
    name: &str,
) -> Result<T, RunError> {
    component
        .ok_or_else(|| RunError::Configuration(format!("block address has no {name}")))?
        .as_ref()
        .parse()
        .map_err(|_| RunError::Configuration(format!("block address {name} is not decimal")))
}

/// Everything this process observed about itself before opening its attempt.
struct RuntimeInputs {
    start_utc: String,
    invocation: InvocationV1,
    environment_inputs: Vec<EnvironmentInputV1>,
    worker: super::artifact::WorkerConfigurationV1,
    host: super::artifact::HostObservationV1,
    accelerator: AcceleratorObservationV1,
    block_budget: Option<usize>,
}

impl RuntimeInputs {
    /// Observes this process, its environment, and its host.
    fn observe(
        configuration: &RareEventConfigurationV1,
        configuration_path: &str,
    ) -> Result<Self, RunError> {
        let start_utc = observation_utc_now()?;
        let self_identity = observe_self_identity()?;
        let host = observe_host()?;
        let reason = "the frozen configuration selects no accelerator".to_owned();
        let accelerator = AcceleratorObservationV1::NotUsed {
            evidence: vec![normalized_accelerator_not_used_evidence(&reason)?],
            reason,
        };

        // Every environment read this run performs happens here, once per
        // declared name, so what is read, declared, and recorded cannot differ.
        let mut observed = BTreeMap::new();
        for name in ENVIRONMENT_INPUT_NAMES {
            observed.insert(name, read_declared_environment(name)?);
        }
        let workers_value = observed
            .get(WORKER_ENVIRONMENT)
            .and_then(Option::as_deref)
            .ok_or_else(|| {
                RunError::Configuration(format!(
                    "{WORKER_ENVIRONMENT} must declare the worker count this run uses"
                ))
            })?
            .to_owned();
        let workers: usize = workers_value.parse().map_err(|_| {
            RunError::Configuration(format!("{WORKER_ENVIRONMENT} is not a decimal count"))
        })?;
        if workers == 0 {
            return Err(RunError::Configuration(format!(
                "{WORKER_ENVIRONMENT} must be positive"
            )));
        }
        let block_budget = observed
            .get(BLOCK_BUDGET_ENVIRONMENT)
            .and_then(Option::as_deref)
            .map(|value| {
                value.parse::<usize>().map_err(|_| {
                    RunError::Configuration(format!(
                        "{BLOCK_BUDGET_ENVIRONMENT} is not a decimal count"
                    ))
                })
            })
            .transpose()?;

        let worker = super::artifact::WorkerConfigurationV1 {
            requested_workers: workers,
            effective_workers: workers,
            executor_mode: "cpu".into(),
            cpu_affinity: "unpinned".into(),
            work_queue_policy: "canonical-block-queue/v1".into(),
            block_assignment_policy: "round-robin/v1".into(),
            accelerator_selection: "none".into(),
            fallback_policy: "safe-cpu/v1".into(),
            effective_devices: Vec::new(),
        };
        // Derived from the same reads above, in the sorted order a receipt needs.
        let environment_inputs: Vec<_> = observed
            .iter()
            .map(|(name, value)| EnvironmentInputV1 {
                name: (*name).to_owned(),
                value: match value {
                    None => EnvironmentValueV1::Unset {},
                    Some(value) => EnvironmentValueV1::Set(value.clone()),
                },
            })
            .collect();

        let executable_path = std::env::current_exe()?;
        let executable_sha256 = sha256_hex(&fs::read(&executable_path)?);
        let effective_configuration = canonical_bytes(configuration)?;
        let scientific_mode = match configuration.scientific_identity {
            ScientificIdentityV1::Target { .. } => "target",
            ScientificIdentityV1::Coverage { .. } => "coverage",
        };
        let invocation = InvocationV1 {
            argv: std::env::args().collect(),
            executable_path: executable_path.to_string_lossy().into_owned(),
            executable_sha256,
            process_id: self_identity.process_id(),
            process_start_token: self_identity.process_start_token().to_owned(),
            boot_identity: self_identity.boot_identity().to_owned(),
            configuration_path: configuration_path.to_owned(),
            configuration_sha256: sha256_hex(&effective_configuration),
            effective_configuration_hex: effective_configuration
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect(),
            input_resolution: vec![
                schema_default_input("accelerator_selection", &worker.accelerator_selection),
                configuration_input("artifact_root", &configuration.artifact_root),
                schema_default_input("block_assignment_policy", &worker.block_assignment_policy),
                schema_default_input("cpu_affinity", &worker.cpu_affinity),
                schema_default_input("effective_devices", ""),
                environment_input("effective_workers", &workers_value),
                schema_default_input("executor_mode", &worker.executor_mode),
                schema_default_input("fallback_policy", &worker.fallback_policy),
                environment_input("requested_workers", &workers_value),
                configuration_input("scientific_identity", scientific_mode),
                schema_default_input("work_queue_policy", &worker.work_queue_policy),
            ],
        };
        Ok(Self {
            start_utc,
            invocation,
            environment_inputs,
            worker,
            host,
            accelerator,
            block_budget,
        })
    }
}

/// Records one input owned by the immutable scientific configuration.
fn configuration_input(field: &str, value: &str) -> InputResolutionV1 {
    InputResolutionV1 {
        field: field.to_owned(),
        origin: InputOriginV1::Configuration,
        value: value.to_owned(),
    }
}

/// Records one input taking its closed schema default.
fn schema_default_input(field: &str, value: &str) -> InputResolutionV1 {
    InputResolutionV1 {
        field: field.to_owned(),
        origin: InputOriginV1::SchemaDefault,
        value: value.to_owned(),
    }
}

/// Records one input resolved from a declared environment name.
fn environment_input(field: &str, value: &str) -> InputResolutionV1 {
    InputResolutionV1 {
        field: field.to_owned(),
        origin: InputOriginV1::Environment,
        value: value.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    use std::sync::{Mutex, MutexGuard};

    use super::{
        read_declared_environment, RunError, BLOCK_BUDGET_ENVIRONMENT, ENVIRONMENT_INPUT_NAMES,
    };

    /// Serializes the tests that mutate this process's own environment.
    static ENVIRONMENT: Mutex<()> = Mutex::new(());

    fn environment_guard() -> MutexGuard<'static, ()> {
        ENVIRONMENT
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// An environment name outside the declared inventory cannot be consulted.
    #[test]
    fn undeclared_environment_names_fail_closed() {
        let _guard = environment_guard();
        let refusal = read_declared_environment("GF2_RARE_EVENT_UNDECLARED")
            .expect_err("an undeclared environment name must fail closed");
        assert!(
            matches!(&refusal, RunError::Configuration(_)),
            "unexpected refusal: {refusal}"
        );
        for name in ENVIRONMENT_INPUT_NAMES {
            read_declared_environment(name).expect("a declared name is readable");
        }
    }

    /// A declared name holding bytes that are not UTF-8 is refused outright.
    ///
    /// Reporting it absent would let a receipt record an unset input while the
    /// value was in fact set, falsifying the run's observed provenance.
    #[test]
    fn non_utf8_environment_values_fail_closed() {
        let _guard = environment_guard();
        let restore = std::env::var_os(BLOCK_BUDGET_ENVIRONMENT);
        std::env::set_var(
            BLOCK_BUDGET_ENVIRONMENT,
            OsStr::from_bytes(&[0x36, 0x34, 0xff]),
        );
        let observed = read_declared_environment(BLOCK_BUDGET_ENVIRONMENT);
        match restore {
            Some(value) => std::env::set_var(BLOCK_BUDGET_ENVIRONMENT, value),
            None => std::env::remove_var(BLOCK_BUDGET_ENVIRONMENT),
        }
        let refusal = observed.expect_err("a non-UTF-8 declared value must fail closed");
        assert!(
            matches!(&refusal, RunError::Configuration(_)),
            "unexpected refusal: {refusal}"
        );
        read_declared_environment(BLOCK_BUDGET_ENVIRONMENT)
            .expect("the restored environment reads cleanly");
    }

    /// The declared inventory is the sorted, duplicate-free list a receipt records.
    #[test]
    fn declared_environment_inventory_is_sorted_and_unique() {
        assert!(ENVIRONMENT_INPUT_NAMES
            .windows(2)
            .all(|pair| pair[0] < pair[1]));
    }
}
