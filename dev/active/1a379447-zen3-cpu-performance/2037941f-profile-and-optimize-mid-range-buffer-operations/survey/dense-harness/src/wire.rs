//! The dense-parity arm's side of the campaign wire.
//!
//! The request an arm reads and the result it writes are
//! `tuning_campaign_support::arm`'s own [`ArmRequest`] and [`ArmResult`], so the
//! harness declares no mirror of either. This module adds only what the frozen
//! addendum constrains: the cell case, the window protocol an arm answers, and
//! the hand-invocation refusal.

use crate::cells::{Cache, M4riShape, MatvecShape, Workload, M4RI_SHAPES};
use serde::{Deserialize, Serialize};
use std::io;
use tuning_campaign_support::arm::{ArmRequest, ArmResult, ARM_RESULT_SCHEMA};
use tuning_campaign_support::protocol::SHARED_SETTINGS;
use tuning_campaign_support::receipt::{ConversionCosts, WindowRecord};
use tuning_campaign_support::timing::TimingSample;
use tuning_campaign_support::transport;

/// One cell's case, tagged by the canonical question it belongs to.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "question", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Case {
    /// Isolated fused AND-population-count over one row and the vector.
    IsolatedFusedParity {
        /// Words one logical operation consumes.
        words: usize,
        /// Workload seed.
        seed: u64,
    },
    /// Public `BitMatrix::matvec` including its output allocation.
    AllocatedMatvec {
        /// Row stride in words.
        words: usize,
        /// Column shape name.
        shape: String,
        /// Workload seed.
        seed: u64,
    },
    /// Whole-consumer gap against the qualified external M4RI product.
    MatvecVsM4ri {
        /// Rows of the qualified shape.
        rows: usize,
        /// Columns of the qualified shape.
        cols: usize,
        /// True when both arms reuse their converted inputs across calls.
        retained: bool,
        /// Workload seed.
        seed: u64,
    },
}

impl Case {
    /// Builds the case of one frozen cell.
    pub fn of(cell: &crate::cells::Cell) -> Self {
        match cell.workload {
            Workload::AndPopcnt { words } => {
                Self::IsolatedFusedParity { words, seed: cell.seed }
            }
            Workload::Matvec { words, shape } => Self::AllocatedMatvec {
                words,
                shape: shape.id().to_owned(),
                seed: cell.seed,
            },
            Workload::M4riGap { shape, retained } => Self::MatvecVsM4ri {
                rows: shape.rows,
                cols: shape.cols,
                retained,
                seed: cell.seed,
            },
        }
    }

    /// The workload seed.
    pub fn seed(&self) -> u64 {
        match self {
            Self::IsolatedFusedParity { seed, .. }
            | Self::AllocatedMatvec { seed, .. }
            | Self::MatvecVsM4ri { seed, .. } => *seed,
        }
    }

    /// The workload this case declares.
    pub fn workload(&self) -> Result<Workload, String> {
        match self {
            Self::IsolatedFusedParity { words, .. } => Ok(Workload::AndPopcnt { words: *words }),
            Self::AllocatedMatvec { words, shape, .. } => Ok(Workload::Matvec {
                words: *words,
                shape: parse_shape(shape)?,
            }),
            Self::MatvecVsM4ri { rows, cols, retained, .. } => M4RI_SHAPES
                .into_iter()
                .find(|shape: &M4riShape| shape.rows == *rows && shape.cols == *cols)
                .map(|shape| Workload::M4riGap { shape, retained: *retained })
                .ok_or_else(|| format!("{rows}x{cols} is not a qualified M4RI shape")),
        }
    }
}

fn parse_shape(name: &str) -> Result<MatvecShape, String> {
    match name {
        "full" => Ok(MatvecShape::Full),
        "tail1" => Ok(MatvecShape::Tail1),
        other => Err(format!("shape {other:?} is not a frozen shape")),
    }
}

/// Refuses a request whose window protocol is not the frozen one.
///
/// Every timed execution of the addendum runs the shared settings' windows at
/// their target (§ Cache, warmup, and sampling); the validation position's
/// zero-window request is the only other form an arm answers. An arm that
/// followed a timing override would measure a window protocol the addendum does
/// not declare.
pub fn verify_window_protocol(request: &ArmRequest) -> Result<(), String> {
    let frozen = (
        SHARED_SETTINGS.windows_per_execution,
        SHARED_SETTINGS.window_target_ms,
    );
    let observed = (request.windows, request.window_target_ms);
    if observed == frozen || observed == (0, 0) {
        return Ok(());
    }
    Err(format!(
        "cell {} requests {} windows of {} ms rather than the frozen {} of {} ms or a \
         zero-window arrangement pass",
        request.cell_id, request.windows, request.window_target_ms, frozen.0, frozen.1
    ))
}

/// Refuses a request the frozen addendum declares no cell for, and resolves the
/// cell's case and cache policy.
pub fn accept(request: &ArmRequest) -> Result<(Case, Cache), String> {
    if request.workers_declared != 1 {
        return Err("every dense-parity cell is a one-worker serial cell".into());
    }
    if request.decoder.is_some() {
        return Err("no dense-parity cell is a decoder cell".into());
    }
    verify_window_protocol(request)?;
    let case: Case = serde_json::from_value(request.case.clone())
        .map_err(|error| format!("case does not decode: {error}"))?;
    Ok((case, Cache::from_request(request.cache_state)))
}

/// Reads the guarded request on stdin and accepts it.
pub fn read_request() -> Result<(ArmRequest, Case, Cache), String> {
    let sentinel = std::env::var(transport::FRESH_CASE_VAR).ok();
    let request: ArmRequest = transport::read_guarded_case(sentinel.as_deref(), io::stdin().lock())?;
    let (case, cache) = accept(&request)?;
    Ok((request, case, cache))
}

/// Writes the one canonical result line from observed samples.
///
/// `calibrated` is false exactly when the execution used the frozen fixed call
/// count, which is the cache policy's own declaration.
pub fn emit_result(
    samples: &[TimingSample],
    cache: Cache,
    selected_path: String,
    conversion: ConversionCosts,
) -> io::Result<()> {
    let cpus_observed = tuning_campaign_support::host::CpuAffinity::observe()
        .map(|affinity| affinity.cpus().to_vec())
        .unwrap_or_default();
    let result = ArmResult {
        schema: ARM_RESULT_SCHEMA.to_owned(),
        windows: samples
            .iter()
            .map(|sample| WindowRecord { calls: sample.calls, elapsed_ns: sample.elapsed_ns })
            .collect(),
        cache_state_applied: cache.state(),
        workers_observed: 1,
        cpus_observed,
        selected_path: Some(selected_path),
        conversion: Some(conversion),
        quality: None,
        calibrated: Some(cache.cold_calls().is_none()),
    };
    transport::write_result_line(io::stdout().lock(), &result)
}

/// Refuses a hand invocation outside the benchmark window.
///
/// `benchmark-ab-runner` clears the child environment and installs only the
/// plan's per-arm variables plus the child-v2 sentinel, so a campaign child
/// never observes the window variables its launcher exported. A child carrying
/// the sentinel therefore defers to the layers that do enforce the window, the
/// launcher's `window` subcommand and `dev/scripts/ccx1-bench-flock.sh`, and to
/// the non-timed smoke, whose requests carry no timing window. Every other
/// invocation requires the window variables and exits before reading a request.
pub fn require_window_unless_child() -> Result<(), String> {
    if std::env::var(transport::FRESH_CASE_VAR).as_deref() == Ok(transport::FRESH_CASE_VALUE) {
        return Ok(());
    }
    if std::env::var("GF2_BENCH_WINDOW").as_deref() != Ok("1")
        || std::env::var("GF2_BENCH").as_deref() != Ok("1")
    {
        return Err("timed dense-parity arms require GF2_BENCH_WINDOW=1 and GF2_BENCH=1".into());
    }
    Ok(())
}
