//! Shared arm driver for the byte-field survey (jit:6c6b09b1).
//!
//! The benchmark runner launches one fresh process per arm execution, writes
//! a request on its stdin and expects exactly one canonical result line on
//! its stdout. This crate owns that framing, the case schema both arms
//! decode, and the conversion-cost accounting the whole-consumer cells
//! report, so the gf2 arm and the external arm cannot drift apart on any of
//! them.
//!
//! An arm supplies a [`Workload`]: it prepares its operands once, reports
//! what the conversions around its kernel cost, and exposes a closure the
//! timing protocol calls. Everything outside that closure is untimed.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io;
use std::time::{Duration, Instant};
pub use tuning_campaign_support::abtest::SplitMix64;
use tuning_campaign_support::host::CpuAffinity;
use tuning_campaign_support::protocol::{CacheState, CellRole};
use tuning_campaign_support::timing::{execution_windows_fixed_or_calibrated, FIXTURE_BANKS};
use tuning_campaign_support::transport;

/// Request the runner writes on the arm's stdin.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub schema: String,
    pub cell_id: String,
    pub arm: String,
    pub role: CellRole,
    pub pair: u32,
    pub case: Value,
    pub cache_state: CacheState,
    /// Frozen call count of a `cold` cell; absent cells calibrate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cold_calls: Option<u64>,
    /// Decoder contract; always absent in this survey, whose cells carry
    /// no decoder, and rejected if present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decoder: Option<Value>,
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
    /// The window starts and ends at the shared byte region, so it includes
    /// representation conversion, table preparation and output conversion:
    /// what a byte-region consumer pays.
    WholeConsumer,
}

/// The cell case, forwarded verbatim by the runner to both arms.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    pub operation: Operation,
    /// Region length in bytes for `axpy` and `pairwise`; data-region length
    /// for `encode`.
    #[serde(default)]
    pub bytes: usize,
    /// Square dimension for `matmul`.
    #[serde(default)]
    pub n: usize,
    /// Source-region count for `encode`.
    #[serde(default)]
    pub k: usize,
    /// Output-region count for `encode`.
    #[serde(default)]
    pub rows: usize,
    /// Full reduction polynomial of the field, for example 285 for 0x11D.
    pub poly: u32,
    pub metric: Metric,
    /// Seed of the [`OperandStream`] over the shared [`SplitMix64`].
    pub seed: u64,
    pub workers: u32,
}

/// Conversion and setup costs the arm reports alongside its windows.
///
/// Each field is measured outside the timing windows by [`probe_ns`], which
/// repeats the named operation and reports the median repetition. For a
/// whole-consumer cell the same work also happens inside every timed call,
/// so these values say how the timed cost is composed rather than adding to
/// it. A field whose operation the arm does not perform is zero.
#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct Conversion {
    /// Building the field or library context.
    pub setup_ns: u64,
    /// Converting the byte-region operands into the arm's native layout.
    pub pack_ns: u64,
    /// Converting the arm's native result back into a byte region.
    pub unpack_ns: u64,
    /// Preparing the coefficient or generator tables the kernel consumes,
    /// when the library exposes that step separately from the kernel.
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
    cache_state_applied: CacheState,
    workers_observed: u32,
    cpus_observed: Vec<u32>,
    selected_path: Option<String>,
    conversion: Option<Conversion>,
    quality: Option<Value>,
    calibrated: bool,
}

/// One arm's prepared workload.
pub struct Workload<'a> {
    /// Runtime-observed identity of the code path the kernel selected.
    pub selected_path: String,
    /// Costs of the conversions around the kernel.
    pub conversion: Conversion,
    /// The measured body. The argument is the fixture bank index, which
    /// rotates across the eight banks when the cell declares `streaming`.
    pub body: Box<dyn FnMut(usize) + 'a>,
}

/// Deterministic operands over the shared [`SplitMix64`] [Steele2014]
/// (`tuning_campaign_support::abtest`), so both arms see the same bytes for
/// the same seed.
pub trait OperandStream {
    /// Fills a byte region with the low eight bits of successive outputs.
    fn fill(&mut self, bytes: &mut [u8]);
    /// The fixed coefficient every region cell reuses: the low byte of the
    /// next output with its low bit set, so it is never zero.
    fn coefficient(&mut self) -> u8;
}

impl OperandStream for SplitMix64 {
    fn fill(&mut self, bytes: &mut [u8]) {
        for byte in bytes.iter_mut() {
            *byte = (self.next_u64() & 0xFF) as u8;
        }
    }

    fn coefficient(&mut self) -> u8 {
        (self.next_u64() | 1) as u8
    }
}

/// Number of fixture banks a cell rotates through under `streaming`.
pub fn banks(cache_state: CacheState) -> usize {
    if cache_state == CacheState::Streaming {
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

/// Repetitions [`probe_ns`] runs at most.
pub const PROBE_MAX_REPETITIONS: usize = 31;
/// Time budget after which [`probe_ns`] stops repeating.
pub const PROBE_BUDGET: Duration = Duration::from_millis(50);

/// Measures one named conversion by repeating it and returning the median
/// repetition in nanoseconds.
///
/// The operation runs at least three times and at most
/// [`PROBE_MAX_REPETITIONS`] times, stopping once [`PROBE_BUDGET`] has
/// elapsed, so a sub-microsecond table preparation is not a single timer
/// read while an eight-mebibyte conversion stays bounded.
pub fn probe_ns(mut operation: impl FnMut()) -> u64 {
    let started = Instant::now();
    let mut samples = Vec::with_capacity(PROBE_MAX_REPETITIONS);
    while samples.len() < 3
        || (samples.len() < PROBE_MAX_REPETITIONS && started.elapsed() < PROBE_BUDGET)
    {
        let ((), ns) = timed(&mut operation);
        samples.push(ns);
    }
    samples.sort_unstable();
    samples[samples.len() / 2]
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
    if request.decoder.is_some() {
        fatal("this survey's arms carry no decoder");
    }
    let case: Case = match serde_json::from_value(request.case.clone()) {
        Ok(case) => case,
        Err(error) => fatal(&format!("case does not decode: {error}")),
    };
    if request.cache_state == CacheState::Cold && request.cold_calls.is_none() {
        fatal("a cold cell requires frozen calls; calibration would pre-run the workload");
    }
    let workload = match build(&request, &case) {
        Ok(workload) => workload,
        Err(reason) => fatal(&reason),
    };
    let Workload {
        selected_path,
        conversion,
        mut body,
    } = workload;

    // `warm` gives the working set one untimed pass before calibration;
    // `cold` runs the frozen call count on first use; `streaming` rotates
    // the fixture banks through the bank index the timing protocol passes.
    if request.cache_state == CacheState::Warm {
        body(0);
    }
    let samples = match execution_windows_fixed_or_calibrated(
        0,
        u64::from(request.windows),
        Duration::from_millis(u64::from(request.window_target_ms)),
        request.cold_calls,
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
    let workers_observed = match observed_threads() {
        Ok(threads) => threads,
        Err(error) => fatal(&format!("cannot observe the thread count: {error}")),
    };
    let result = ArmResult {
        schema: "zen3-benchmark-arm-result-v1".into(),
        windows: samples
            .iter()
            .map(|sample| Window {
                calls: sample.calls,
                elapsed_ns: sample.elapsed_ns,
            })
            .collect(),
        cache_state_applied: request.cache_state,
        workers_observed,
        cpus_observed,
        selected_path: Some(selected_path),
        conversion: Some(conversion),
        quality: None,
        calibrated: request.cold_calls.is_none(),
    };
    if let Err(error) = transport::write_result_line(io::stdout().lock(), &result) {
        fatal(&format!("cannot write the result line: {error}"));
    }
    std::process::exit(0)
}

/// Threads alive in this process after the timed windows, read from
/// `/proc/self/status`. A library that spawned a worker pool during the
/// measured calls leaves it here, so the count is observed rather than
/// restated from the declaration.
fn observed_threads() -> io::Result<u32> {
    let status = std::fs::read_to_string("/proc/self/status")?;
    status
        .lines()
        .find_map(|line| line.strip_prefix("Threads:"))
        .and_then(|value| value.trim().parse().ok())
        .ok_or_else(|| io::Error::other("no Threads line in /proc/self/status"))
}

fn fatal(message: &str) -> ! {
    eprintln!("byte-field arm: {message}");
    std::process::exit(2)
}
