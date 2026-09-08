//! Fixed-window timing used by calibration children.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::{Duration, Instant};

/// Fixture banks traversed by every execution.
pub const FIXTURE_BANKS: usize = 8;
/// Timed executions in the extent protocol.
pub const EXECUTIONS: u64 = 5;
/// Windows in each timed execution.
pub const WINDOWS: u64 = 5;
/// Target length of one timing window.
pub const TARGET: Duration = Duration::from_millis(250);
/// Maximum calls in a timing window.
pub const MAX_CALLS: u64 = 1 << 32;
const CALIBRATION_PROBE_TARGET: Duration = Duration::from_millis(20);

/// One untrimmed raw timing window.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TimingSample {
    pub execution: u64,
    pub repetition: u64,
    pub calls: u64,
    pub elapsed_ns: u64,
}

impl TimingSample {
    /// Constructs a valid positive timing sample.
    pub fn new(
        execution: u64,
        repetition: u64,
        calls: u64,
        elapsed_ns: u64,
    ) -> Result<Self, TimingError> {
        let sample = Self {
            execution,
            repetition,
            calls,
            elapsed_ns,
        };
        sample.validate()?;
        Ok(sample)
    }

    /// Rejects zero calls, zero duration, and out-of-protocol coordinates.
    pub fn validate(&self) -> Result<(), TimingError> {
        if self.execution >= EXECUTIONS {
            return Err(TimingError(format!(
                "execution {} is outside 0..{EXECUTIONS}",
                self.execution
            )));
        }
        if self.repetition >= WINDOWS {
            return Err(TimingError(format!(
                "repetition {} is outside 0..{WINDOWS}",
                self.repetition
            )));
        }
        if self.calls == 0 || self.calls > MAX_CALLS {
            return Err(TimingError(format!(
                "calls {} is outside 1..={MAX_CALLS}",
                self.calls
            )));
        }
        if self.elapsed_ns == 0 {
            return Err(TimingError("elapsed_ns must be positive".to_owned()));
        }
        Ok(())
    }

    /// Returns the observed nanoseconds per logical call.
    pub fn ns_per_call(self) -> f64 {
        self.elapsed_ns as f64 / self.calls as f64
    }
}

/// Invalid timing evidence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimingError(pub String);

impl fmt::Display for TimingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for TimingError {}

/// Fixture-bank start for a timed execution/window pair.
pub fn execution_window_start(execution: u64, repetition: u64) -> usize {
    ((execution * WINDOWS + repetition) as usize) & (FIXTURE_BANKS - 1)
}

/// Calibrates the exact call count with the reviewed doubling protocol.
pub fn calibrated_calls(target: Duration, body: &mut impl FnMut(usize)) -> u64 {
    calibrated_calls_with(target, body, |calls, body| time_calls(calls, 0, body))
}

/// Injectable form of [`calibrated_calls`] used to test the exact arithmetic.
pub fn calibrated_calls_with<F, M>(target: Duration, body: &mut F, mut measure: M) -> u64
where
    F: FnMut(usize),
    M: FnMut(u64, &mut F) -> Duration,
{
    let probe_target = target.min(CALIBRATION_PROBE_TARGET);
    let mut calls = 1_u64;
    loop {
        let elapsed = measure(calls, body);
        if elapsed >= probe_target || calls >= MAX_CALLS {
            let elapsed_ns = elapsed.as_nanos().max(1);
            let wanted = target.as_nanos().saturating_mul(calls as u128) / elapsed_ns;
            return wanted.clamp(1, MAX_CALLS as u128) as u64;
        }
        calls = calls.saturating_mul(2).min(MAX_CALLS);
    }
}

/// Completed timing interval exposed to post-interval progress callbacks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimingProgress {
    /// Calibration selected this positive call count for every window.
    CalibrationComplete { calls: u64 },
    /// One completed untrimmed timing window.
    WindowComplete(TimingSample),
}

/// Measures five windows for one execution, retaining acquisition order.
pub fn execution_windows(
    execution: u64,
    body: &mut impl FnMut(usize),
) -> Result<Vec<TimingSample>, TimingError> {
    execution_windows_with_progress(execution, body, |_| Ok(()))
        .map_err(|error| TimingError(error.to_string()))
}

/// Measures the fixed protocol with callbacks strictly outside timed intervals.
///
/// The first callback reports calibration, followed by repetitions 0 through 4.
/// A callback failure stops the execution and returns its I/O error; incomplete
/// executions cannot be accepted. The callback may flush durable progress.
pub fn execution_windows_with_progress(
    execution: u64,
    body: &mut impl FnMut(usize),
    progress: impl FnMut(TimingProgress) -> std::io::Result<()>,
) -> std::io::Result<Vec<TimingSample>> {
    execution_windows_configured(execution, WINDOWS, TARGET, body, progress)
}

/// Shared mechanics for an explicitly declared/test timing protocol.
///
/// `repetitions` must be in 1..=WINDOWS and `target` must be positive. This
/// helper satisfies a835 only with exactly WINDOWS repetitions and TARGET;
/// authoritative campaign callers use `execution_windows_with_progress`.
/// Callbacks execute strictly after their interval and errors stop sampling.
pub fn execution_windows_configured(
    execution: u64,
    repetitions: u64,
    target: Duration,
    body: &mut impl FnMut(usize),
    progress: impl FnMut(TimingProgress) -> std::io::Result<()>,
) -> std::io::Result<Vec<TimingSample>> {
    if repetitions == 0 || repetitions > WINDOWS || target.is_zero() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "invalid declared timing protocol",
        ));
    }
    if execution >= EXECUTIONS {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("execution {execution} is outside 0..{EXECUTIONS}"),
        ));
    }
    execution_windows_fixed_or_calibrated(execution, repetitions, target, None, body, progress)
}

/// Measures windows with a predeclared call count, or calibrates when `calls`
/// is absent. Fixed calls execute no workload before the first timed window.
/// Cold callers must supply a positive count; counts above MAX_CALLS fail.
pub fn execution_windows_fixed_or_calibrated(
    execution: u64,
    repetitions: u64,
    target: Duration,
    calls: Option<u64>,
    body: &mut impl FnMut(usize),
    mut progress: impl FnMut(TimingProgress) -> std::io::Result<()>,
) -> std::io::Result<Vec<TimingSample>> {
    if repetitions == 0
        || repetitions > WINDOWS
        || target.is_zero()
        || execution >= EXECUTIONS
        || calls.is_some_and(|n| n == 0 || n > MAX_CALLS)
    {
        return Err(std::io::Error::other("invalid fixed timing protocol"));
    }
    let calls = match calls {
        Some(calls) => calls,
        None => {
            let calls = calibrated_calls(target, body);
            progress(TimingProgress::CalibrationComplete { calls })?;
            calls
        }
    };
    let mut samples = Vec::with_capacity(repetitions as usize);
    for repetition in 0..repetitions {
        let start = execution_window_start(execution, repetition);
        let elapsed = time_calls(calls, start, body);
        let elapsed_ns = u64::try_from(elapsed.as_nanos())
            .map_err(|_| std::io::Error::other("one timing window exceeded u64 nanoseconds"))?;
        let sample = TimingSample::new(execution, repetition, calls, elapsed_ns)
            .map_err(std::io::Error::other)?;
        progress(TimingProgress::WindowComplete(sample))?;
        samples.push(sample);
    }
    Ok(samples)
}

fn time_calls(calls: u64, start_index: usize, body: &mut impl FnMut(usize)) -> Duration {
    let start = Instant::now();
    for call in 0..calls {
        body(start_index.wrapping_add(call as usize) & (FIXTURE_BANKS - 1));
    }
    start.elapsed()
}

#[cfg(test)]
mod tests {
    use super::time_calls;

    #[test]
    fn timed_calls_supply_the_exact_rotating_fixture_bank() {
        let mut banks = Vec::new();
        time_calls(10, 7, &mut |bank| banks.push(bank));
        assert_eq!(banks, [7, 0, 1, 2, 3, 4, 5, 6, 7, 0]);
    }
}
