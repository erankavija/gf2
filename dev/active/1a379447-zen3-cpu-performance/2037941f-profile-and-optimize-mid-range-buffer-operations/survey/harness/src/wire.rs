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

use crate::cells::{Cache, Layout, NrTarget, RowShape, Workload, NR_TARGETS};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io;
use tuning_campaign_support::timing::TimingSample;
use tuning_campaign_support::transport;

/// Schema identity of one arm result line.
pub const RESULT_SCHEMA: &str = "zen3-benchmark-arm-result-v1";

/// Mirror of the runner's arm request.
#[derive(Deserialize, Serialize)]
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
    /// Decoder declaration; absent for every logical-buffer cell.
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

/// One cell's case, tagged by the canonical question it belongs to.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "question", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Case {
    /// Isolated public in-place XOR.
    IsolatedXor {
        /// Words one logical operation consumes.
        words: usize,
        /// Address layout name.
        layout: String,
        /// Workload seed.
        seed: u64,
    },
    /// Public `BitMatrix::row_xor`.
    PublicRowXor {
        /// Row stride in words.
        words: usize,
        /// Column shape name.
        shape: String,
        /// Workload seed.
        seed: u64,
    },
    /// Whole public NR constructor.
    NrBg2Construction {
        /// Public constructor base graph.
        base_graph: u8,
        /// Public constructor target codeword length.
        target_n: usize,
        /// Public constructor target message length.
        target_k: usize,
        /// Workload seed.
        seed: u64,
    },
    /// Operation-equivalent ISA-L scalar gap.
    IsalBaseGap {
        /// Words one logical operation consumes.
        words: usize,
        /// Address layout name.
        layout: String,
        /// Workload seed.
        seed: u64,
    },
}

impl Case {
    /// Builds the case of one frozen cell.
    pub fn of(cell: &crate::cells::Cell) -> Self {
        match cell.workload {
            Workload::Xor { words, layout } => Self::IsolatedXor {
                words,
                layout: layout.id().to_owned(),
                seed: cell.seed,
            },
            Workload::RowXor { words, shape } => Self::PublicRowXor {
                words,
                shape: shape.id().to_owned(),
                seed: cell.seed,
            },
            Workload::Nr(target) => Self::NrBg2Construction {
                base_graph: target.base_graph,
                target_n: target.target_n,
                target_k: target.target_k,
                seed: cell.seed,
            },
            Workload::IsalGap { words } => Self::IsalBaseGap {
                words,
                layout: Layout::A64.id().to_owned(),
                seed: cell.seed,
            },
        }
    }

    /// The workload seed.
    pub fn seed(&self) -> u64 {
        match self {
            Self::IsolatedXor { seed, .. }
            | Self::PublicRowXor { seed, .. }
            | Self::NrBg2Construction { seed, .. }
            | Self::IsalBaseGap { seed, .. } => *seed,
        }
    }

    /// The workload this case declares.
    pub fn workload(&self) -> Result<Workload, String> {
        match self {
            Self::IsolatedXor { words, layout, .. } => Ok(Workload::Xor {
                words: *words,
                layout: parse_layout(layout)?,
            }),
            Self::PublicRowXor { words, shape, .. } => Ok(Workload::RowXor {
                words: *words,
                shape: parse_shape(shape)?,
            }),
            Self::NrBg2Construction {
                base_graph,
                target_n,
                target_k,
                ..
            } => NR_TARGETS
                .into_iter()
                .find(|target: &NrTarget| {
                    target.base_graph == *base_graph
                        && target.target_n == *target_n
                        && target.target_k == *target_k
                })
                .map(Workload::Nr)
                .ok_or_else(|| {
                    format!("({base_graph}, {target_n}, {target_k}) is not a frozen NR route")
                }),
            Self::IsalBaseGap { words, layout, .. } => {
                if parse_layout(layout)? != Layout::A64 {
                    return Err("ISA-L cells use only the aligned layout".into());
                }
                Ok(Workload::IsalGap { words: *words })
            }
        }
    }
}

fn parse_layout(name: &str) -> Result<Layout, String> {
    match name {
        "a64" => Ok(Layout::A64),
        "o8" => Ok(Layout::O8),
        other => Err(format!("layout {other:?} is not a frozen layout")),
    }
}

fn parse_shape(name: &str) -> Result<RowShape, String> {
    match name {
        "full" => Ok(RowShape::Full),
        "tail63" => Ok(RowShape::Tail63),
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
    /// Packing cost; zero for every logical-buffer cell.
    pub pack_ns: u64,
    /// Unpacking cost; zero for every logical-buffer cell.
    pub unpack_ns: u64,
    /// Batch-fill cost; zero for every logical-buffer cell.
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
    /// Decoder quality; absent for every logical-buffer cell.
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
                .map(|sample| Window {
                    calls: sample.calls,
                    elapsed_ns: sample.elapsed_ns,
                })
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
        return Err("every logical-buffer cell is a one-worker serial cell".into());
    }
    if request.decoder.is_some() {
        return Err("no logical-buffer cell is a decoder cell".into());
    }
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
        return Err("timed logical-buffer arms require GF2_BENCH_WINDOW=1 and GF2_BENCH=1".into());
    }
    Ok(())
}
