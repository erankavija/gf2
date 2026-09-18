//! Canonical child-v2 wire types shared by both arms.
//!
//! `transport::decode_case` accepts only the exact bytes [`Request`]
//! re-encodes, so every field spelling, order and omission rule matches the
//! runner's own request type: the runner omits `cold_calls` and `decoder` for a
//! cell that declares neither, and a mirror that spells them `null` makes the
//! runner's request noncanonical and the child rejects it.
//!
//! Both arms share these types rather than each carrying a private mirror, so
//! the wire contract has one form.

use crate::cells::{Cache, M4riShape, MatvecShape, Workload, M4RI_SHAPES};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io;
use tuning_campaign_support::timing::TimingSample;
use tuning_campaign_support::transport;

/// Schema identity of one arm result line.
pub const RESULT_SCHEMA: &str = "zen3-benchmark-arm-result-v1";

/// Mirror of the runner's arm request.
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    /// Request schema identity.
    pub schema: String,
    /// Frozen cell identifier.
    pub cell_id: String,
    /// Arm name the plan gave this child.
    pub arm: String,
    /// `baseline` or `candidate`.
    pub role: String,
    /// Zero-based pair index.
    pub pair: u32,
    /// Opaque cell case; see [`Case`].
    pub case: Value,
    /// Declared cache state.
    pub cache_state: String,
    /// Frozen fixed call count of a cold cell.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cold_calls: Option<u64>,
    /// Decoder declaration; absent for every dense-parity cell.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decoder: Option<Value>,
    /// Timing windows in this execution.
    pub windows: u32,
    /// Target length of one window, in milliseconds.
    pub window_target_ms: u32,
    /// CPUs the runner resolved for the cell's core arm.
    pub cpus: Vec<u32>,
    /// Workers the cell declares.
    pub workers_declared: u32,
}

impl Request {
    /// Refuses a request whose window protocol is not the frozen one.
    ///
    /// Every timed execution of the addendum runs five windows targeted at
    /// 100 ms (§ Cache, warmup, and sampling), which are the protocol's shared
    /// settings; the non-timed smoke's zero-window request is the only other
    /// form an arm answers. An arm that followed a timing override would
    /// measure a window protocol the addendum does not declare.
    pub fn verify_window_protocol(&self) -> Result<(), String> {
        let frozen = (
            tuning_campaign_support::protocol::SHARED_SETTINGS.windows_per_execution,
            tuning_campaign_support::protocol::SHARED_SETTINGS.window_target_ms,
        );
        let observed = (self.windows, self.window_target_ms);
        if observed == frozen || observed == (0, 0) {
            return Ok(());
        }
        Err(format!(
            "cell {} requests {} windows of {} ms rather than the frozen {} of {} ms or a \
             zero-window arrangement pass",
            self.cell_id, self.windows, self.window_target_ms, frozen.0, frozen.1
        ))
    }
}

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

/// One timing window of an arm result.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Window {
    /// Calls in the window.
    pub calls: u64,
    /// Wall-clock nanoseconds the window took.
    pub elapsed_ns: u64,
}

/// Conversion costs an arm observed outside its measured operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConversionCosts {
    /// Fixture and route setup, once per execution.
    pub setup_ns: u64,
    /// Packing into external coordinates; zero for every gf2 arm.
    pub pack_ns: u64,
    /// Unpacking from external coordinates; zero for every gf2 arm.
    pub unpack_ns: u64,
    /// Batch-fill cost; zero for every dense-parity cell.
    pub batch_fill_ns: u64,
    /// Separately observed per-call arrangement cost.
    pub dispatch_ns: u64,
}

/// One arm result line.
///
/// The type both arms emit is the type the non-timed smoke parses, so the
/// result contract has one form and a canonical re-encode of a parsed line
/// reproduces the bytes the arm wrote.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArmResult {
    /// Result schema identity.
    pub schema: String,
    /// Timing windows in acquisition order.
    pub windows: Vec<Window>,
    /// Cache state the arm applied.
    pub cache_state_applied: String,
    /// Workers the arm observed.
    pub workers_observed: u32,
    /// CPUs the arm observed.
    pub cpus_observed: Vec<u32>,
    /// Route provenance observed at run time.
    pub selected_path: Option<String>,
    /// Conversion costs.
    pub conversion: Option<ConversionCosts>,
    /// Decoder quality; absent for every dense-parity cell.
    pub quality: Option<Value>,
    /// False exactly when the execution used the frozen fixed call count.
    pub calibrated: Option<bool>,
}

impl ArmResult {
    /// Assembles a result from observed samples.
    pub fn new(
        samples: &[TimingSample],
        cache: Cache,
        selected_path: String,
        conversion: ConversionCosts,
    ) -> Self {
        let cpus_observed = tuning_campaign_support::host::CpuAffinity::observe()
            .map(|affinity| affinity.cpus().to_vec())
            .unwrap_or_default();
        Self {
            schema: RESULT_SCHEMA.to_owned(),
            windows: samples
                .iter()
                .map(|sample| Window { calls: sample.calls, elapsed_ns: sample.elapsed_ns })
                .collect(),
            cache_state_applied: cache.id().to_owned(),
            workers_observed: 1,
            cpus_observed,
            selected_path: Some(selected_path),
            conversion: Some(conversion),
            quality: None,
            calibrated: Some(cache.cold_calls().is_none()),
        }
    }

    /// Writes the one canonical result line.
    pub fn emit(&self) -> io::Result<()> {
        transport::write_result_line(io::stdout().lock(), self)
    }
}

/// Reads and validates the guarded child-v2 request.
pub fn read_request() -> Result<(Request, Case, Cache), String> {
    let sentinel = std::env::var(transport::FRESH_CASE_VAR).ok();
    let request: Request = transport::read_guarded_case(sentinel.as_deref(), io::stdin().lock())?;
    if request.workers_declared != 1 {
        return Err("every dense-parity cell is a one-worker serial cell".into());
    }
    if request.decoder.is_some() {
        return Err("no dense-parity cell is a decoder cell".into());
    }
    request.verify_window_protocol()?;
    let cache = Cache::from_request(&request.cache_state)?;
    let case: Case = serde_json::from_value(request.case.clone())
        .map_err(|error| format!("case does not decode: {error}"))?;
    Ok((request, case, cache))
}

/// Refuses a hand invocation outside the benchmark window.
///
/// `benchmark-ab-runner` clears the child environment and installs only the
/// plan's per-arm variables plus the child-v2 sentinel, so a campaign child
/// never observes the window variables its launcher exported. A child carrying
/// the sentinel therefore defers to the layers that do enforce the window, the
/// launcher's `window` subcommand and `dev/scripts/ccx1-bench-flock.sh`, and to
/// the non-timed smoke, whose zero-window requests collect no timing sample.
/// Every other invocation requires the window variables and exits before
/// reading a request.
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
