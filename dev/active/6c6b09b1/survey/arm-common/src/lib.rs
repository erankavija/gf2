//! Shared child-v2 arm driver for the byte-field survey (jit:6c6b09b1).
//!
//! The benchmark runner launches one fresh process per arm execution, writes
//! a request on its stdin and expects exactly one canonical result line on
//! its stdout. This crate owns that framing, the case schema both arms
//! decode, and the conversion-cost accounting the whole-consumer cells
//! report, so the gf2 arm and the external arm cannot drift apart on any of
//! them.
//!
//! An arm supplies a [`Workload`]: it prepares its operands once, reports
//! what that preparation cost, and exposes a closure the timing protocol
//! calls. Everything outside that closure is untimed.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io;
use std::time::{Duration, Instant};
use tuning_campaign_support::host::CpuAffinity;
use tuning_campaign_support::timing::{execution_windows_configured, FIXTURE_BANKS};
use tuning_campaign_support::transport;

/// Request the runner writes on the arm's stdin.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub schema: String,
    pub cell_id: String,
    pub arm: String,
    pub role: String,
    pub pair: u32,
    pub case: Value,
    pub cache_state: String,
    pub windows: u32,
    pub window_target_ms: u32,
    pub cpus: Vec<u32>,
    pub workers_declared: u32,
}

/// The operation a cell measures.
///
/// Every variant names one mathematical operation. An arm that cannot
/// perform the named operation reports that fact rather than substituting a
/// neighbouring one.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Operation {
    /// Fixed-coefficient region multiply-accumulate `y[i] += a * x[i]`.
    Axpy,
    /// Arbitrary pairwise product `z[i] = x[i] * y[i]`, no coefficient reuse.
    Pairwise,
    /// Dense square matrix product `C = A * B`.
    Matmul,
    /// Generator-matrix region encode `C[r][i] = sum_j G[r][j] * D[j][i]`.
    Encode,
}

/// Whether the timed window covers the kernel alone or the whole consumer.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Metric {
    /// Operands are already in the arm's native representation.
    KernelIsolated,
    /// The window includes representation conversion, table preparation and
    /// output conversion, so it measures what a byte-region consumer pays.
    WholeConsumer,
}

/// The cell case, forwarded verbatim by the runner to both arms.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    pub operation: Operation,
    /// Region length in bytes for `axpy` and `pairwise`.
    #[serde(default)]
    pub bytes: usize,
    /// Square dimension for `matmul`.
    #[serde(default)]
    pub n: usize,
    /// Source count for `encode`.
    #[serde(default)]
    pub k: usize,
    /// Output row count for `encode`.
    #[serde(default)]
    pub rows: usize,
    /// Full reduction polynomial of the field, for example 285 for 0x11D.
    pub poly: u32,
    pub metric: Metric,
    pub seed: u64,
    pub workers: u32,
}

/// Conversion and setup costs the arm reports alongside its windows.
///
/// Each field is a one-off cost measured outside the timing windows. For a
/// whole-consumer cell the same work also happens inside every timed call,
/// so these values say how the timed cost is composed rather than adding to
/// it.
#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct Conversion {
    /// Building the field or library context.
    pub setup_ns: u64,
    /// Converting a byte region into the arm's native operand layout.
    pub pack_ns: u64,
    /// Converting the arm's native result back into a byte region.
    pub unpack_ns: u64,
    /// Preparing the coefficient or generator tables the kernel consumes.
    pub batch_fill_ns: u64,
    /// Per-call implementation selection outside the kernel body.
    pub dispatch_ns: u64,
}

#[derive(Serialize)]
struct Window {
    calls: u64,
    elapsed_ns: u64,
}

#[derive(Serialize)]
struct ArmResult {
    schema: String,
    windows: Vec<Window>,
    cache_state_applied: String,
    workers_observed: u32,
    cpus_observed: Vec<u32>,
    selected_path: Option<String>,
    conversion: Option<Conversion>,
    quality: Option<Value>,
}

/// One arm's prepared workload.
pub struct Workload<'a> {
    /// Runtime-observed identity of the code path the kernel selected.
    pub selected_path: String,
    /// Costs measured while preparing the operands.
    pub conversion: Conversion,
    /// The measured body. The argument is the fixture bank index, which
    /// rotates across the eight banks when the cell declares `streaming`.
    pub body: Box<dyn FnMut(usize) + 'a>,
}

/// Deterministic operand fill, shared by both arms so the two see the same
/// bytes for the same seed.
pub struct SplitMix64(u64);

impl SplitMix64 {
    pub fn new(seed: u64) -> Self {
        SplitMix64(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Fills a byte region, so both arms start from identical operands.
    pub fn fill(&mut self, bytes: &mut [u8]) {
        for byte in bytes.iter_mut() {
            *byte = (self.next_u64() & 0xFF) as u8;
        }
    }
}

/// Number of fixture banks a cell rotates through under `streaming`.
pub fn banks(cache_state: &str) -> usize {
    if cache_state == "streaming" {
        FIXTURE_BANKS
    } else {
        1
    }
}

/// Measures the elapsed nanoseconds of one closure.
pub fn timed<T>(work: impl FnOnce() -> T) -> (T, u64) {
    let start = Instant::now();
    let value = work();
    let elapsed = start.elapsed().as_nanos();
    (value, u64::try_from(elapsed).unwrap_or(u64::MAX))
}

/// Reads the request, builds the workload, runs the timing protocol and
/// writes the one canonical result line.
///
/// `build` receives the decoded request and case and returns the prepared
/// workload. An error string from `build` names an operation the arm cannot
/// perform; the process then exits with status 2 and the reason on stderr,
/// so a cell that declares an operation an arm lacks fails loudly instead of
/// silently measuring something else.
pub fn run<F>(build: F) -> !
where
    F: for<'a> FnOnce(&'a Request, &'a Case) -> Result<Workload<'a>, String>,
{
    let sentinel = std::env::var(transport::FRESH_CASE_VAR).ok();
    let request: Request =
        match transport::read_guarded_case(sentinel.as_deref(), io::stdin().lock()) {
            Ok(request) => request,
            Err(error) => fatal(&format!("request does not decode: {error}")),
        };
    let case: Case = match serde_json::from_value(request.case.clone()) {
        Ok(case) => case,
        Err(error) => fatal(&format!("case does not decode: {error}")),
    };
    let workload = match build(&request, &case) {
        Ok(workload) => workload,
        Err(reason) => fatal(&reason),
    };
    let Workload {
        selected_path,
        conversion,
        mut body,
    } = workload;

    // `warm` gives the working set one untimed pass; `cold` would be
    // defeated by the calibration pass the timing protocol runs first, so no
    // cell in this family declares it.
    if request.cache_state == "warm" {
        body(0);
    }

    let samples = match execution_windows_configured(
        0,
        u64::from(request.windows),
        Duration::from_millis(u64::from(request.window_target_ms)),
        &mut body,
        |_| Ok(()),
    ) {
        Ok(samples) => samples,
        Err(error) => fatal(&format!("timing failed: {error}")),
    };
    drop(body);

    let cpus_observed = CpuAffinity::observe()
        .map(|affinity| affinity.cpus().to_vec())
        .unwrap_or_default();
    let result = ArmResult {
        schema: "zen3-benchmark-arm-result-v1".into(),
        windows: samples
            .iter()
            .map(|sample| Window {
                calls: sample.calls,
                elapsed_ns: sample.elapsed_ns,
            })
            .collect(),
        cache_state_applied: request.cache_state.clone(),
        workers_observed: case.workers,
        cpus_observed,
        selected_path: Some(selected_path),
        conversion: match case.metric {
            Metric::WholeConsumer => Some(conversion),
            Metric::KernelIsolated => None,
        },
        quality: None,
    };
    if let Err(error) = transport::write_result_line(io::stdout().lock(), &result) {
        fatal(&format!("cannot write the result line: {error}"));
    }
    std::process::exit(0)
}

fn fatal(message: &str) -> ! {
    eprintln!("byte-field arm: {message}");
    std::process::exit(2)
}
