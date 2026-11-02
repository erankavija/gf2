//! Bounded runs with a write-ahead execution log and a generic checkpoint.
//! One caller writes a directory at a time; a resumed trajectory replays its
//! active prefix, while completed trajectories remain in exact histograms.

use std::collections::BTreeMap;
use std::fmt;
use std::fs::File;
use std::path::Path;

use crate::execution_log::{self, ExecutionLog};
use experiments_common::{publish_atomic, RunIdentity, RunRecord, Sha256Digest};
use gf2_algebra::permanent::SupportedPrimeField;
use gf2_core::gfp::Fp;
use gf2_sim::checkpoint::{
    CheckpointLoadError, CheckpointPayload, CheckpointReader, CheckpointWriter,
};
use gf2_stats::sampler::{
    FieldOrder, MatrixAddress, StreamIndex, StreamPurpose, STREAM_INDEX_LIMIT,
};
use gf2_stats::weighted::{ExponentHistogram, WeightedError, WeightedRuns};
use serde::{Deserialize, Serialize};

use super::{RareEventError, TrajectoryCursor};

/// Frozen sampling and interval parameters, read from a JSON configuration.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Configuration {
    /// Root of the domain-separated sampler streams.
    pub root_seed: u64,
    /// Prime field, serialized as `3`, `5`, or `7`.
    #[serde(with = "field_order")]
    pub field_order: FieldOrder,
    /// Rows in each three-column matrix.
    pub rows: usize,
    /// Number of independent runs; at least two for an interval.
    pub target_runs: u32,
    /// Number of importance trajectories in each run.
    pub trajectories_per_run: u32,
    /// Rows advanced between durable prefix boundaries; positive.
    pub prefix_rows: usize,
    /// Numerator of the Student critical value chosen for these runs.
    pub student_critical_numerator: u64,
    /// Positive denominator of the Student critical value.
    pub student_critical_denominator: u64,
}

impl Configuration {
    /// Checks the stream range, exponent range, and nonempty run allocation.
    ///
    /// # Errors
    ///
    /// Returns [`RunError::Configuration`] for parameters outside those ranges
    /// or a zero Student critical-value numerator or denominator.
    pub fn validate(&self) -> Result<(), RunError> {
        if self.target_runs < 2
            || self.trajectories_per_run == 0
            || self.prefix_rows == 0
            || self.rows > (u32::MAX / 3) as usize
            || self.total_trajectories() > STREAM_INDEX_LIMIT
            || self.student_critical_numerator == 0
            || self.student_critical_denominator == 0
        {
            return Err(RunError::Configuration(
                "need at least two nonempty runs, positive prefix size and critical value, \
                 and representable stream indices and likelihood exponents"
                    .into(),
            ));
        }
        Ok(())
    }

    /// SHA-256 of the serialized parameters; JSON whitespace is immaterial.
    ///
    /// # Errors
    ///
    /// Returns a JSON serialization error.
    pub fn digest(&self) -> Result<Sha256Digest, RunError> {
        Ok(Sha256Digest::of(&serde_json::to_vec(self)?))
    }

    fn total_trajectories(&self) -> u64 {
        u64::from(self.target_runs) * u64::from(self.trajectories_per_run)
    }

    fn address(&self, trajectory: u64) -> Result<MatrixAddress, RunError> {
        Ok(MatrixAddress::new(
            self.root_seed,
            self.field_order,
            self.rows,
            StreamPurpose::RareEvent,
            StreamIndex::new(trajectory)
                .map_err(|error| RunError::Configuration(error.to_string()))?,
        ))
    }
}

use execution_log::field_order;

/// A sampling, configuration, persistence, or reduction failure.
#[derive(Debug)]
pub enum RunError {
    /// Parameters or the frozen run identity do not match.
    Configuration(String),
    /// Trajectory sampling failed.
    Sampling(RareEventError),
    /// Filesystem access failed.
    Io(std::io::Error),
    /// Configuration or log encoding failed.
    Json(serde_json::Error),
    /// The generic checkpoint could not be loaded.
    Checkpoint(CheckpointLoadError),
    /// Exact statistical reduction failed.
    Weighted(WeightedError),
}

impl fmt::Display for RunError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Configuration(message) => formatter.write_str(message),
            Self::Sampling(error) => error.fmt(formatter),
            Self::Io(error) => error.fmt(formatter),
            Self::Json(error) => error.fmt(formatter),
            Self::Checkpoint(error) => error.fmt(formatter),
            Self::Weighted(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for RunError {}

macro_rules! from_error {
    ($error:ty, $variant:ident) => {
        impl From<$error> for RunError {
            fn from(error: $error) -> Self {
                Self::$variant(error)
            }
        }
    };
}
from_error!(RareEventError, Sampling);
from_error!(std::io::Error, Io);
from_error!(serde_json::Error, Json);
from_error!(CheckpointLoadError, Checkpoint);
from_error!(WeightedError, Weighted);

/// Completion of a bounded invocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    /// All configured trajectories are retained and the result is published.
    Complete,
    /// A durable prefix remains for another invocation.
    Incomplete,
}

/// Exact decimal ratios and outward-rounded Student interval endpoints.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Summary {
    /// Number of independent runs.
    pub run_count: usize,
    /// Number of importance trajectories.
    pub sample_count: u64,
    /// Estimated event probability, as numerator and denominator.
    pub likelihood: (String, String),
    /// Pooled sum of likelihood weights.
    pub weight_sum: (String, String),
    /// Pooled sum of squared likelihood weights.
    pub squared_weight_sum: (String, String),
    /// Independent-run means, in run order.
    pub run_means: Vec<(String, String)>,
    /// Variance of the grand mean, estimated from independent runs.
    pub mean_variance: (String, String),
    /// Effective sample size of the pooled final weights.
    pub final_weight_ess: (String, String),
    /// Effective sample sizes of individual runs.
    pub per_run_ess: Vec<(String, String)>,
    /// Lower interval endpoint, rounded downward to 16 significant digits.
    pub interval_lower: String,
    /// Upper interval endpoint, rounded upward to 16 significant digits.
    pub interval_upper: String,
}

/// Progress and, on completion, the statistical result of an invocation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunOutcome {
    /// Whether all trajectories are complete.
    pub status: RunStatus,
    /// Trajectories already accumulated in histograms.
    pub completed_trajectories: u64,
    /// Retained row boundary of the next unfinished trajectory.
    pub completed_prefix: usize,
    /// Reduction of all runs, available only on completion.
    pub summary: Option<Summary>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct Progress {
    completed_trajectories: u64,
    prefix: usize,
    histograms: Vec<BTreeMap<u32, u64>>,
}

impl CheckpointPayload for Progress {
    const IDENTITY: &'static str = "permanent-statistics/rare-event";
    const SCHEMA_VERSION: u32 = 1;
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ExecutionEvent {
    Start { record: Box<RunRecord> },
    Progress { progress: Progress },
    Terminal { status: RunStatus },
    Failed { error: String },
}

fn logged_progress(path: &Path, identity: &RunIdentity) -> Result<Option<Progress>, RunError> {
    let mut latest = None;
    for event in ExecutionLog::read::<ExecutionEvent>(path)? {
        match event {
            ExecutionEvent::Start { record } if &record.identity != identity => {
                return Err(RunError::Configuration(
                    "execution log run identity differs".into(),
                ));
            }
            ExecutionEvent::Progress { progress } => latest = Some(progress),
            _ => {}
        }
    }
    Ok(latest)
}

/// Advances one frozen configuration by at most `prefix_budget` boundaries.
///
/// `record.identity` is the complete resume identity. Its source revision is
/// only a rebuild pointer. Worker scheduling and output paths are immaterial.
/// The caller coordinates a single writer per directory and supplies observed
/// identity facts for the executable doing the sampling.
///
/// `execution.jsonl` opens before sampling and records starts, durable progress,
/// and terminal status. Progress is synced there before checkpoint publication;
/// resume recovers a lagging checkpoint from the log without resampling it.
/// A completed invocation atomically publishes `result.json`.
///
/// # Errors
///
/// Returns invalid parameters, a changed run identity, checkpoint read errors,
/// and filesystem, sampling, or exact-reduction failures.
pub fn execute_frozen_run(
    configuration: &Configuration,
    record: &RunRecord,
    directory: &Path,
    prefix_budget: Option<usize>,
) -> Result<RunOutcome, RunError> {
    configuration.validate()?;
    if record.identity.configuration_sha256 != configuration.digest()?
        || record.identity.seed != configuration.root_seed
    {
        return Err(RunError::Configuration(
            "run identity does not match the configuration".into(),
        ));
    }
    let hash = format!(
        "sha256:{}",
        Sha256Digest::of(&serde_json::to_vec(&record.identity)?)
    );
    let path = directory.join("checkpoint.json");
    let checkpoint = CheckpointReader::<Progress, _>::for_payload(&path, &hash).load_payload()?;
    let logged = logged_progress(&directory.join("execution.jsonl"), &record.identity)?;
    let mut progress = match (checkpoint, logged) {
        (Some(checkpoint), Some(logged)) if checkpoint == logged => checkpoint,
        (_, Some(logged)) => logged,
        (None, None) => Progress {
            completed_trajectories: 0,
            prefix: 0,
            histograms: vec![BTreeMap::new(); configuration.target_runs as usize],
        },
        (Some(_), None) => {
            return Err(RunError::Configuration(
                "checkpoint has no execution log progress".into(),
            ))
        }
    };
    let writer = CheckpointWriter::for_payload(path, hash)?;
    let mut log = ExecutionLog::open(&directory.join("execution.jsonl"))?;
    File::open(directory)?.sync_all()?;
    log.append(&ExecutionEvent::Start {
        record: Box::new(record.clone()),
    })?;
    let result = (|| {
        save_progress(&progress, &mut log, &writer)?;
        let budget = prefix_budget.unwrap_or(usize::MAX);
        match configuration.field_order {
            FieldOrder::F3 => {
                advance::<3>(configuration, &mut progress, budget, &mut log, &writer)?
            }
            FieldOrder::F5 => {
                advance::<5>(configuration, &mut progress, budget, &mut log, &writer)?
            }
            FieldOrder::F7 => {
                advance::<7>(configuration, &mut progress, budget, &mut log, &writer)?
            }
        }
        let complete = progress.completed_trajectories == configuration.total_trajectories();
        let summary = complete
            .then(|| summarize(configuration, &progress))
            .transpose()?;
        if let Some(summary) = &summary {
            #[derive(Serialize)]
            struct ResultRecord<'a> {
                run: &'a RunRecord,
                configuration: &'a Configuration,
                summary: &'a Summary,
            }
            publish_atomic(
                &directory.join("result.json"),
                &serde_json::to_vec_pretty(&ResultRecord {
                    run: record,
                    configuration,
                    summary,
                })?,
            )?;
        }
        Ok(RunOutcome {
            status: if complete {
                RunStatus::Complete
            } else {
                RunStatus::Incomplete
            },
            completed_trajectories: progress.completed_trajectories,
            completed_prefix: progress.prefix,
            summary,
        })
    })();
    match &result {
        Ok(outcome) => log.append(&ExecutionEvent::Terminal {
            status: outcome.status,
        })?,
        Err(error) => {
            let _ = log.append(&ExecutionEvent::Failed {
                error: format!("{error}"),
            });
        }
    }
    result
}

fn save_progress(
    progress: &Progress,
    log: &mut ExecutionLog,
    writer: &CheckpointWriter<Progress, String>,
) -> Result<(), RunError> {
    log.append(&ExecutionEvent::Progress {
        progress: progress.clone(),
    })?;
    writer.write_payload(progress)?;
    Ok(())
}

fn advance<const Q: u64>(
    configuration: &Configuration,
    progress: &mut Progress,
    mut budget: usize,
    log: &mut ExecutionLog,
    writer: &CheckpointWriter<Progress, String>,
) -> Result<(), RunError>
where
    Fp<Q>: SupportedPrimeField,
{
    while progress.completed_trajectories < configuration.total_trajectories() && budget > 0 {
        let mut cursor = TrajectoryCursor::<Q>::new(
            configuration.address(progress.completed_trajectories)?,
            progress.prefix,
        )?;
        loop {
            let boundary = progress
                .prefix
                .saturating_add(configuration.prefix_rows)
                .min(configuration.rows);
            cursor.advance(boundary - progress.prefix);
            progress.prefix = boundary;
            let complete = boundary == configuration.rows;
            if complete {
                let run = (progress.completed_trajectories
                    / u64::from(configuration.trajectories_per_run))
                    as usize;
                *progress.histograms[run].entry(cursor.exponent).or_default() += 1;
                progress.completed_trajectories += 1;
                progress.prefix = 0;
            }
            save_progress(progress, log, writer)?;
            budget -= 1;
            if complete || budget == 0 {
                break;
            }
        }
    }
    Ok(())
}

fn summarize(configuration: &Configuration, progress: &Progress) -> Result<Summary, RunError> {
    let mut histograms = Vec::with_capacity(progress.histograms.len());
    for counts in &progress.histograms {
        let mut histogram = ExponentHistogram::new(
            configuration.field_order.as_u64() as u32,
            configuration.rows as u32 * 3,
        )?;
        for (&exponent, &count) in counts {
            histogram.record_many(exponent, count)?;
        }
        histograms.push(histogram);
    }
    let runs = WeightedRuns::from_histograms(histograms)?;
    let interval = runs
        .student_interval(
            configuration.student_critical_numerator,
            configuration.student_critical_denominator,
        )?
        .render_outward(16)?;
    Ok(Summary {
        run_count: runs.run_count(),
        sample_count: runs.sample_count(),
        likelihood: runs.mean_decimal(),
        weight_sum: runs.weight_sum_decimal(),
        squared_weight_sum: runs.squared_weight_sum_decimal(),
        run_means: runs.run_means_decimal(),
        mean_variance: runs
            .mean_variance_decimal()
            .ok_or(WeightedError::TooFewRuns)?,
        final_weight_ess: runs.final_weight_ess_decimal(),
        per_run_ess: runs.per_run_ess_decimal(),
        interval_lower: interval.lower,
        interval_upper: interval.upper,
    })
}
