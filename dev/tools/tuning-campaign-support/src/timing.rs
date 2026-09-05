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

/// Measures five windows for one execution, retaining acquisition order.
pub fn execution_windows(
    execution: u64,
    body: &mut impl FnMut(usize),
) -> Result<Vec<TimingSample>, TimingError> {
    if execution >= EXECUTIONS {
        return Err(TimingError(format!(
            "execution {execution} is outside 0..{EXECUTIONS}"
        )));
    }
    let calls = calibrated_calls(TARGET, body);
    let mut samples = Vec::with_capacity(WINDOWS as usize);
    for repetition in 0..WINDOWS {
        let start = execution_window_start(execution, repetition);
        let elapsed = time_calls(calls, start, body);
        let elapsed_ns = u64::try_from(elapsed.as_nanos())
            .map_err(|_| TimingError("one timing window exceeded u64 nanoseconds".to_owned()))?;
        samples.push(TimingSample::new(execution, repetition, calls, elapsed_ns)?);
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
