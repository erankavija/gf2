//! Write-ahead shard acceptance and bounded cell execution.
use super::dataset::{
    self, AcceptanceVerdict, HaltReason, Manifest, RunCells, ShardRecord, SummaryRow,
};
use super::{
    backend_available, backend_supports_cell, evaluate_shard, invalid, AcceptancePlan, Backend,
    CampaignConfig, CampaignError, Cell,
};
use crate::execution_log::ExecutionLog;
use experiments_common::{RunIdentity, RunRecord};
use gf2_sim::checkpoint::{CheckpointPayload, CheckpointReader, CheckpointWriter};
use gf2_stats::sampler::StreamPurpose;
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

/// Explicit policy for an unavailable configured backend.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OnUnavailable {
    /// Operator error; leaves the output untouched.
    Refuse,
    /// Preserve a campaign-wide mechanical halt.
    Halt,
    /// Use a specified supporting backend for this invocation's remaining shards.
    Fallback(Backend),
}
impl std::str::FromStr for OnUnavailable {
    type Err = CampaignError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "refuse" => Ok(Self::Refuse),
            "halt" => Ok(Self::Halt),
            _ => s
                .strip_prefix("fallback=")
                .ok_or_else(|| {
                    CampaignError::Invalid("expected refuse, halt, or fallback=BACKEND".into())
                })
                .and_then(|s| s.parse().map(Self::Fallback)),
        }
    }
}
/// Runtime observations and invocation budget, outside the run identity.
#[derive(Clone, Debug)]
pub struct Invocation {
    /// Local Rayon workers.
    pub workers: usize,
    /// Accepted-shard budget; failures do not consume it.
    pub max_shards: Option<usize>,
    /// Explicit availability policy.
    pub on_unavailable: OnUnavailable,
    /// Unquoted invocation tokens.
    pub argv: Vec<String>,
}
impl Default for Invocation {
    fn default() -> Self {
        Self {
            workers: 1,
            max_shards: None,
            on_unavailable: OnUnavailable::Refuse,
            argv: Vec::new(),
        }
    }
}
/// Result of one bounded campaign invocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    /// The selected cell has a terminal row.
    Terminal,
    /// Accepted progress remains to be drawn.
    Incomplete,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Event {
    Start {
        q: u8,
        n: u16,
        record: Box<RunRecord>,
        workers: usize,
        argv: Vec<String>,
        on_unavailable: OnUnavailable,
    },
    ShardAccepted {
        q: u8,
        n: u16,
        shard: ShardRecord,
    },
    ShardFailed {
        q: u8,
        n: u16,
        shard_id: u64,
        attempt: u32,
        error: String,
    },
    CellTerminal {
        q: u8,
        n: u16,
        row: SummaryRow,
    },
    CampaignHalted {
        q: u8,
        n: u16,
        reason: HaltReason,
    },
    InvocationEnd {
        status: String,
    },
}
#[derive(Clone, Debug, Default)]
pub(crate) struct CellProgress {
    identity: Option<RunIdentity>,
    pub(crate) shards: Vec<ShardRecord>,
    failures: u32,
    pub(crate) terminal: Option<SummaryRow>,
}
#[derive(Clone, Debug)]
pub(crate) struct State {
    pub(crate) cells: Vec<CellProgress>,
    pub(crate) runs: Vec<RunCells>,
    halted: Option<HaltReason>,
    halt_logged: bool,
    progressed: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Tally {
    next_shard: u64,
    matrix_count: u64,
    permanent_zero_count: u64,
    determinant_zero_count: Option<u64>,
    failure_count: u32,
    terminal: Option<SummaryRow>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Checkpoint {
    cells: Vec<Tally>,
}
impl CheckpointPayload for Checkpoint {
    const IDENTITY: &'static str = "permanent-statistics/campaign";
    const SCHEMA_VERSION: u32 = 1;
}
impl State {
    fn checkpoint(&self, cells: &[Cell]) -> Checkpoint {
        Checkpoint {
            cells: cells
                .iter()
                .zip(&self.cells)
                .map(|(c, p)| {
                    let (matrix_count, permanent_zero_count, determinant_zero_count) =
                        dataset::pool(c, &p.shards);
                    Tally {
                        next_shard: p.shards.len() as u64,
                        matrix_count,
                        permanent_zero_count,
                        determinant_zero_count,
                        failure_count: p.failures,
                        terminal: p.terminal.clone(),
                    }
                })
                .collect(),
        }
    }
}
pub(crate) fn replay(directory: &Path, manifest: &Manifest) -> Result<State, CampaignError> {
    let cells = manifest.expanded_cells();
    let mut state = State {
        cells: vec![CellProgress::default(); cells.len()],
        runs: Vec::new(),
        halted: None,
        halt_logged: false,
        progressed: false,
    };
    for event in ExecutionLog::read::<Event>(&directory.join("execution.jsonl"))? {
        let (q, n) = match &event {
            Event::Start { q, n, .. }
            | Event::ShardAccepted { q, n, .. }
            | Event::ShardFailed { q, n, .. }
            | Event::CellTerminal { q, n, .. }
            | Event::CampaignHalted { q, n, .. } => (*q, *n),
            Event::InvocationEnd { .. } => continue,
        };
        let i = cells
            .iter()
            .position(|c| (c.q, c.n) == (q, n))
            .ok_or_else(|| CampaignError::Invalid("log names unregistered cell".into()))?;
        let p = &mut state.cells[i];
        match event {
            Event::Start { record, .. } => {
                if record.identity.configuration_sha256 != manifest.configuration_sha256
                    || record.identity.seed != manifest.root_seed
                    || p.identity.as_ref().is_some_and(|id| id != &record.identity)
                {
                    return invalid("cell run identity differs");
                }
                p.identity = Some(record.identity.clone());
                if let Some(run) = state.runs.iter_mut().find(|r| r.record == *record) {
                    if !run.cells.contains(&(q, n)) {
                        run.cells.push((q, n));
                    }
                } else {
                    state.runs.push(RunCells {
                        record: *record,
                        cells: vec![(q, n)],
                    });
                }
            }
            Event::ShardAccepted { shard, .. } => {
                if p.identity.is_none()
                    || p.terminal.is_some()
                    || shard.shard_id != p.shards.len() as u64
                {
                    return invalid("accepted shard out of order");
                }
                dataset::validate_shard(&cells[i], &shard)?;
                p.shards.push(shard);
                p.failures = 0;
                state.progressed = true;
            }
            Event::ShardFailed {
                shard_id, attempt, ..
            } => {
                if p.identity.is_none()
                    || p.terminal.is_some()
                    || shard_id != p.shards.len() as u64
                    || attempt != p.failures + 1
                {
                    return invalid("failure out of order");
                }
                p.failures = attempt;
                state.progressed = true;
            }
            Event::CellTerminal { row, .. } => {
                dataset::validate_summary_aggregate(manifest, &cells[i], &p.shards, &row)?;
                if p.terminal.is_some() {
                    return invalid("duplicate terminal cell");
                }
                if row.permanent_verdict == Some(AcceptanceVerdict::Rejected)
                    || row.determinant_verdict == Some(AcceptanceVerdict::Rejected)
                {
                    state.halted = Some(HaltReason::AcceptanceFailure);
                }
                p.terminal = Some(row);
                state.progressed = true;
            }
            Event::CampaignHalted { reason, .. } => {
                state.halted = Some(reason);
                state.halt_logged = true;
                state.progressed = true;
            }
            Event::InvocationEnd { .. } => unreachable!(),
        }
    }
    Ok(state)
}
fn append_terminal(
    directory: &Path,
    log: &mut ExecutionLog,
    cell: &Cell,
    progress: &mut CellProgress,
    row: SummaryRow,
) -> Result<(), CampaignError> {
    dataset::publish_shards(directory, cell, &progress.shards)?;
    log.append(&Event::CellTerminal {
        q: cell.q,
        n: cell.n,
        row: row.clone(),
    })?;
    progress.terminal = Some(row);
    Ok(())
}
fn halt(
    directory: &Path,
    log: &mut ExecutionLog,
    cells: &[Cell],
    state: &mut State,
    origin: usize,
    reason: HaltReason,
) -> Result<(), CampaignError> {
    log.append(&Event::CampaignHalted {
        q: cells[origin].q,
        n: cells[origin].n,
        reason,
    })?;
    state.halted = Some(reason);
    halt_remaining(directory, log, cells, state, reason)
}
fn halt_remaining(
    directory: &Path,
    log: &mut ExecutionLog,
    cells: &[Cell],
    state: &mut State,
    reason: HaltReason,
) -> Result<(), CampaignError> {
    for (cell, p) in cells.iter().zip(&mut state.cells) {
        if p.terminal.is_none() {
            let row = SummaryRow::halted(cell, &p.shards, reason);
            append_terminal(directory, log, cell, p, row)?;
        }
    }
    Ok(())
}
/// Runs bounded streams with write-ahead acceptance and per-cell identity.
/// An interrupted draw spends no retry; a reported error or panic does.
/// A differing identity, invalid selector, admission violation, or lock contention refuses.
/// The default unavailable-backend policy writes nothing.
pub fn run_cell(
    config: &CampaignConfig,
    record: &RunRecord,
    directory: &Path,
    q: u8,
    n: u16,
    invocation: &Invocation,
) -> Result<RunStatus, CampaignError> {
    config.validate()?;
    if invocation.workers == 0 || invocation.max_shards == Some(0) {
        return invalid("positive workers and shard budget required");
    }
    let cells = config.cells();
    let i = cells
        .iter()
        .position(|c| (c.q, c.n) == (q, n))
        .ok_or_else(|| CampaignError::Invalid("unknown cell selector".into()))?;
    let cell = &cells[i];
    if record.identity.configuration_sha256 != config.digest()?
        || record.identity.seed != config.root_seed
    {
        return invalid("identity does not match configuration");
    }
    if let OnUnavailable::Fallback(backend) = invocation.on_unavailable {
        if !backend_supports_cell(backend, q, n) || !backend_available(backend) {
            return Err(CampaignError::BackendUnavailable);
        }
    }
    let manifest = Manifest::from_config(config)?;
    let already_terminal = if directory.join("execution.jsonl").exists() {
        replay(directory, &manifest)?.cells[i].terminal.is_some()
    } else {
        false
    };
    let backend = if backend_available(cell.backend) {
        Some(cell.backend)
    } else {
        match invocation.on_unavailable {
            OnUnavailable::Refuse if !already_terminal => {
                return Err(CampaignError::BackendUnavailable)
            }
            OnUnavailable::Fallback(backend) => Some(backend),
            _ => None,
        }
    };
    fs::create_dir_all(directory)?;
    let mut log = ExecutionLog::open(&directory.join("execution.jsonl"))?;
    let result = (|| {
        manifest.publish(directory)?;
        let mut state = replay(directory, &manifest)?;
        let hash = format!("sha256:{}", config.digest()?);
        let path = directory.join("checkpoint.json");
        let checkpoint =
            CheckpointReader::<Checkpoint, _>::for_payload(&path, &hash).load_payload()?;
        if checkpoint.is_some() && !state.progressed {
            return invalid("checkpoint has no execution log progress");
        }
        let writer = CheckpointWriter::for_payload(path, hash)?;
        if state.cells[i]
            .identity
            .as_ref()
            .is_some_and(|id| id != &record.identity)
        {
            return invalid("cell run identity differs");
        }
        // A halt may stop between terminal rows. Complete that projection on resume.
        if let Some(reason) = state.halted {
            if !state.halt_logged {
                log.append(&Event::CampaignHalted { q, n, reason })?;
            }
            halt_remaining(directory, &mut log, &cells, &mut state, reason)?;
            writer.write_payload(&state.checkpoint(&cells))?;
            return invalid("campaign halted");
        }
        if state.cells[i].terminal.is_some() {
            return Ok(RunStatus::Terminal);
        }
        let first = cells
            .iter()
            .position(|c| (c.q, c.n) == config.execution.first_cell)
            .unwrap();
        if i != first && state.cells[first].terminal.is_none() {
            return invalid("first_cell must terminate first");
        }
        if state
            .cells
            .iter()
            .enumerate()
            .any(|(j, p)| j != i && p.identity.is_some() && p.terminal.is_none())
        {
            return invalid("another cell is active");
        }
        log.append(&Event::Start {
            q,
            n,
            record: Box::new(record.clone()),
            workers: invocation.workers,
            argv: invocation.argv.clone(),
            on_unavailable: invocation.on_unavailable,
        })?;
        if backend.is_none() {
            halt(
                directory,
                &mut log,
                &cells,
                &mut state,
                i,
                HaltReason::BackendUnavailable,
            )?;
            writer.write_payload(&state.checkpoint(&cells))?;
            return Ok(RunStatus::Terminal);
        }
        let backend = backend.unwrap();
        let mut budget = invocation.max_shards.unwrap_or(usize::MAX);
        while state.cells[i].shards.len() < cell.shard_count() as usize && budget > 0 {
            if state.cells[i].failures >= config.execution.max_shard_attempts {
                halt(
                    directory,
                    &mut log,
                    &cells,
                    &mut state,
                    i,
                    HaltReason::ExecutionFailure,
                )?;
                break;
            }
            let shard = cell.shard(state.cells[i].shards.len() as u64)?;
            let evaluated = super::guard_dispatch(|| {
                macro_rules! evaluate {
                    ($q:literal) => {
                        evaluate_shard::<$q, _>(
                            config.root_seed,
                            StreamPurpose::CampaignCell,
                            cell,
                            shard,
                            backend,
                            invocation.workers,
                            config.execution.accelerator_launch_cap_ms,
                            &mut |_, _, _| {},
                        )
                    };
                }
                match q {
                    3 => evaluate!(3),
                    5 => evaluate!(5),
                    7 => evaluate!(7),
                    _ => unreachable!(),
                }
            });
            match evaluated.and_then(|r| {
                dataset::validate_shard(cell, &r)?;
                Ok(r)
            }) {
                Ok(shard) => {
                    log.append(&Event::ShardAccepted {
                        q,
                        n,
                        shard: shard.clone(),
                    })?;
                    state.cells[i].shards.push(shard);
                    state.cells[i].failures = 0;
                    budget -= 1;
                }
                Err(error) => {
                    state.cells[i].failures += 1;
                    log.append(&Event::ShardFailed {
                        q,
                        n,
                        shard_id: shard.shard_id,
                        attempt: state.cells[i].failures,
                        error: error.to_string(),
                    })?;
                    if matches!(error, CampaignError::BackendUnavailable)
                        || state.cells[i].failures >= config.execution.max_shard_attempts
                    {
                        let reason = if matches!(error, CampaignError::BackendUnavailable) {
                            HaltReason::BackendUnavailable
                        } else {
                            HaltReason::ExecutionFailure
                        };
                        halt(directory, &mut log, &cells, &mut state, i, reason)?;
                    }
                }
            }
            writer.write_payload(&state.checkpoint(&cells))?;
            if state.halted.is_some() {
                break;
            }
        }
        if state.cells[i].terminal.is_none()
            && state.cells[i].shards.len() == cell.shard_count() as usize
        {
            let (count, zeros, det) = dataset::pool(cell, &state.cells[i].shards);
            let row = super::assess_completed_cell(
                &AcceptancePlan::for_manifest(&manifest)?,
                cell,
                count,
                zeros,
                det,
            )?;
            let rejected = row.permanent_verdict == Some(AcceptanceVerdict::Rejected)
                || row.determinant_verdict == Some(AcceptanceVerdict::Rejected);
            append_terminal(directory, &mut log, cell, &mut state.cells[i], row)?;
            if rejected {
                halt(
                    directory,
                    &mut log,
                    &cells,
                    &mut state,
                    i,
                    HaltReason::AcceptanceFailure,
                )?;
            }
            writer.write_payload(&state.checkpoint(&cells))?;
        }
        Ok(if state.cells[i].terminal.is_some() {
            RunStatus::Terminal
        } else {
            RunStatus::Incomplete
        })
    })();
    log.append(&Event::InvocationEnd {
        status: match &result {
            Ok(status) => format!("{status:?}"),
            Err(e) => format!("failed: {e}"),
        },
    })?;
    result
}
