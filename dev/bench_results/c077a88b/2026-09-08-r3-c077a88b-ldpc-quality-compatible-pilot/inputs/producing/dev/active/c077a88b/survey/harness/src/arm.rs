//! Shared child-v2 arm framing for every decoder arm of the survey.
//!
//! Each arm — the gf2 arm and every external adapter — reads one canonical
//! child-v2 request, decodes the recorded frames of an input bundle, and
//! writes exactly one canonical result line. This module owns the request and
//! result contracts, the fixed-window timing protocol, the Wilson interval and
//! the runtime affinity observation, so no arm carries a private copy of them.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io;
use std::time::Duration;
pub use tuning_campaign_support::protocol::{
    Normalization, NormalizationKind, Precision, Schedule, Stopping, StoppingKind,
};
pub use tuning_campaign_support::receipt::{
    ArmQuality as Quality, ConversionCosts, DecoderArmSettings as ArmSettings,
    IterationDistribution,
};
pub use tuning_campaign_support::transport::{
    FRESH_CASE_VALUE, FRESH_CASE_VAR, FRESH_RESULT_PREFIX,
};
use tuning_campaign_support::{timing, transport};

/// Schema of the one result line an arm writes.
pub const ARM_RESULT_SCHEMA: &str = "zen3-benchmark-arm-result-v1";

/// The request the runner forwards to an arm child.
#[derive(Clone, Debug, Deserialize, Serialize)]
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

/// The decoder case every arm of a cell receives.
///
/// The runner forwards one `case` object verbatim to both arms of a cell, so
/// every arm decodes the identical bundle under the identical algorithmic
/// declaration. An arm's own build and backend selection travels in its plan
/// `environment`, never in the shared case.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DecoderCase {
    /// Directory of the input bundle.
    pub bundle: String,
    /// `ComparisonCode` name; must equal the bundle's.
    pub code: String,
    /// Maximum belief-propagation iterations.
    pub iteration_cap: u32,
    /// Normalized min-sum factor.
    pub normalization_factor: f32,
    /// Whether the decoder stops on a passing syndrome.
    pub syndrome_stopping: bool,
    /// Frames the cell declares per timed call.
    pub batch_size: u32,
    /// Frames decoded in the untimed quality pass.
    pub quality_frames: u32,
    /// Optional file the quality pass writes its per-frame information-window
    /// hard decisions into, one byte per bit. The validator sets it to compare
    /// decisions across arms; the benchmark runner leaves it absent.
    #[serde(default)]
    pub decisions_out: Option<String>,
}

/// One raw timing window.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct Window {
    pub calls: u64,
    pub elapsed_ns: u64,
}

/// The one result line an arm writes.
#[derive(Clone, Debug, Serialize)]
pub struct ArmResult {
    pub schema: String,
    pub windows: Vec<Window>,
    pub cache_state_applied: String,
    pub workers_observed: u32,
    pub cpus_observed: Vec<u32>,
    pub selected_path: Option<String>,
    pub conversion: Option<ConversionCosts>,
    pub quality: Option<Quality>,
}

/// Reads the guarded child-v2 request from standard input.
///
/// # Errors
///
/// Returns an error when the sentinel is absent or wrong, when standard input
/// cannot be read, or when it does not decode as exactly one request.
pub fn read_request() -> Result<Request, String> {
    let sentinel = std::env::var(FRESH_CASE_VAR).ok();
    transport::read_guarded_case(sentinel.as_deref(), io::stdin().lock())
}

/// Writes the one canonical result line and flushes it.
///
/// # Errors
///
/// Returns an error when the result cannot be encoded or written.
pub fn write_result(result: &ArmResult) -> Result<(), String> {
    transport::write_result_line(io::stdout().lock(), result).map_err(|e| e.to_string())
}

/// Wilson score interval at 95% for `successes` out of `trials`.
///
/// Returns the whole unit interval for an empty sample.
#[must_use]
pub fn wilson_interval_95(successes: u64, trials: u64) -> [f64; 2] {
    let (lower, upper) = tuning_campaign_support::abtest::wilson_interval_95(successes, trials)
        .unwrap_or((0.0, 1.0));
    [lower, upper]
}

/// Reads the CPU affinity mask this process actually observes.
#[must_use]
pub fn observed_cpus() -> Vec<u32> {
    tuning_campaign_support::host::CpuAffinity::observe()
        .map(|a| a.cpus().to_vec())
        .unwrap_or_default()
}

/// Peak resident set size of this process in bytes, as the kernel reports it.
///
/// Returns zero when the field is unavailable.
#[must_use]
pub fn peak_rss_bytes() -> u64 {
    let Ok(status) = std::fs::read_to_string("/proc/self/status") else {
        return 0;
    };
    status
        .lines()
        .find_map(|line| line.strip_prefix("VmHWM:"))
        .and_then(|line| line.split_whitespace().next()?.parse::<u64>().ok())
        .map(|kibibytes| kibibytes * 1024)
        .unwrap_or(0)
}

/// The `fraction` quantile of an ascending sample by nearest rank.
#[must_use]
pub fn quantile(sorted: &[u32], fraction: f64) -> u32 {
    if sorted.is_empty() {
        return 0;
    }
    let index = ((sorted.len() as f64 - 1.0) * fraction).round() as usize;
    sorted[index.min(sorted.len() - 1)]
}

/// Runs the canonical protocol timing mechanism. Each call processes the
/// same declared batch in either arm, including representation conversions.
pub fn timing_windows(request: &Request, body: &mut impl FnMut(usize)) -> Vec<Window> {
    if request.role == "validation" {
        return Vec::new();
    }
    // Warm and cold cells use one fixed fixture bank. Only streaming cells
    // rotate through the canonical bank set, as in ab-smoke-workload.
    let banks = if request.cache_state == "streaming" {
        timing::FIXTURE_BANKS
    } else {
        1
    };
    if request.cache_state == "warm" {
        body(0);
    }
    let mut selected = |bank: usize| body(bank % banks);
    timing::execution_windows_configured(
        0,
        u64::from(request.windows),
        Duration::from_millis(u64::from(request.window_target_ms)),
        &mut selected,
        |_| Ok(()),
    )
    .expect("validated timing request")
    .into_iter()
    .map(|w| Window {
        calls: w.calls,
        elapsed_ns: w.elapsed_ns,
    })
    .collect()
}

/// Accumulated quality of a decoding pass.
#[derive(Clone, Debug, Default)]
pub struct QualityAccumulator {
    pub frames: u64,
    pub frame_errors: u64,
    pub bits: u64,
    pub bit_errors: u64,
    pub iterations: Vec<u32>,
    pub latencies: Vec<u64>,
}

impl QualityAccumulator {
    /// Scores one decoded frame against its transmitted information bits.
    ///
    /// `decoded` and `expected` hold the `k` information-window bits of the
    /// frame as 0/1 bytes.
    pub fn observe(&mut self, decoded: &[u8], expected: &[u8], iterations: u32, latency_ns: u64) {
        let errors = decoded
            .iter()
            .zip(expected)
            .filter(|(got, want)| got != want)
            .count() as u64;
        self.frames += 1;
        self.bits += decoded.len() as u64;
        self.bit_errors += errors;
        self.frame_errors += u64::from(errors > 0);
        self.iterations.push(iterations);
        self.latencies.push(latency_ns);
    }

    /// Renders the accumulated pass as reportable quality.
    #[must_use]
    pub fn finish(self, memory_bytes: u64, settings: ArmSettings) -> Quality {
        let mut sorted = self.iterations.clone();
        sorted.sort_unstable();
        let mut latencies = self.latencies.clone();
        latencies.sort_unstable();
        let frames = self.frames.max(1) as f64;
        Quality {
            frames: self.frames,
            frame_errors: self.frame_errors,
            bits: self.bits,
            bit_errors: self.bit_errors,
            fer: self.frame_errors as f64 / frames,
            fer_interval: wilson_interval_95(self.frame_errors, self.frames),
            ber: self.bit_errors as f64 / self.bits.max(1) as f64,
            ber_interval: wilson_interval_95(self.bit_errors, self.bits),
            interval_method: "wilson-95".to_owned(),
            iterations: IterationDistribution {
                mean: self.iterations.iter().map(|i| f64::from(*i)).sum::<f64>()
                    / self.iterations.len().max(1) as f64,
                p50: quantile(&sorted, 0.5),
                p90: quantile(&sorted, 0.9),
                max: sorted.last().copied().unwrap_or(0),
            },
            memory_bytes,
            latency_ns_p50: latencies
                .get(latencies.len() / 2)
                .copied()
                .unwrap_or_default(),
            settings,
        }
    }
}

/// Frozen untimed quality evidence selected for this arm and code.
/// Validation runs compute it; timing runs reuse its exact bytes.
pub fn prepared_quality() -> Result<Option<Quality>, String> {
    match std::env::var("GF2_LDPC_QUALITY") {
        Ok(path) => {
            let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
            serde_json::from_slice(&bytes)
                .map(Some)
                .map_err(|e| e.to_string())
        }
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

/// First frame of the declared batch in a timing fixture bank.
/// The caller supplies a positive batch dividing the recorded frame count.
pub fn timed_batch_start(bank: usize, batch: usize, frames: usize) -> usize {
    (bank * batch) % frames
}

#[cfg(test)]
mod tests {
    use super::timed_batch_start;
    use tuning_campaign_support::timing::FIXTURE_BANKS;

    #[test]
    fn timing_uses_the_declared_cache_working_set() {
        for cache in ["warm", "streaming"] {
            let request = super::Request {
                schema: "zen3-benchmark-arm-request-v1".into(),
                cell_id: "cache-fixture".into(),
                arm: "fixture".into(),
                role: "exploratory".into(),
                pair: 0,
                case: serde_json::json!({}),
                cache_state: cache.into(),
                windows: 1,
                window_target_ms: 1,
                cpus: vec![],
                workers_declared: 1,
            };
            let mut visited = [false; FIXTURE_BANKS];
            let windows = super::timing_windows(&request, &mut |bank| {
                visited[std::hint::black_box(bank)] = true;
            });
            assert!(!windows.is_empty());
            assert!(visited[0]);
            if cache == "warm" {
                assert!(visited[1..].iter().all(|seen| !seen));
            } else {
                assert!(visited.iter().all(|seen| *seen));
            }
        }
    }

    #[test]
    fn timing_banks_cover_both_alternating_codeword_classes() {
        for batch in [1, 16] {
            let indices: Vec<_> = (0..FIXTURE_BANKS)
                .flat_map(|bank| {
                    let start = timed_batch_start(bank, batch, 128);
                    start..start + batch
                })
                .collect();
            assert!(indices.iter().any(|i| i % 2 == 0));
            assert!(indices.iter().any(|i| i % 2 == 1));
            assert!(indices.iter().all(|i| *i < 128));
        }
    }
}
