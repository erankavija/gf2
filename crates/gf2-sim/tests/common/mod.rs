//! Shared determinism-assertion helpers for the `gf2-sim` byte-identity
//! integration tests (design doc §11).
//!
//! Both [`parallel_determinism.rs`](../parallel_determinism.rs) (the direct
//! `frame_sim` path, issue `3fcb7025`) and [`determinism.rs`](../determinism.rs)
//! (the typestate preset production path, issue `48a0db6c`) compare
//! [`WorkerCounters`] for byte-identity across worker counts. The comparison is
//! the **single source of truth** for the four byte-identity columns and the
//! BER exclusion: it lives here so neither test binary re-implements (and
//! risks diverging) the column set the determinism contract pins.
//!
//! # The four byte-identity columns (design doc §11)
//!
//! The CPU-only / CPU-parallel contract pins exactly four columns as
//! byte-identical across worker counts `{1, 2, 4, 8, 24}` at a fixed seed:
//! `fer`, `frames`, `errors`, `mean_iters`. [`assert_four_columns_byte_identical`]
//! asserts all four (the two `f64` ratios via their exact bit patterns, plus
//! the underlying `total_iterations` whose ratio `mean_iters` is). **BER is
//! deliberately excluded** — see the function docs and the cited issue
//! `152388f4` / design-doc §11 "Always-excluded".

#![allow(dead_code)] // each test binary uses a subset of these helpers.

use gf2_sim::parallel::WorkerCounters;

/// Asserts the four byte-identity columns of `actual` match `baseline`
/// (design doc §11 CPU-only / CPU-parallel contract).
///
/// The four columns the determinism contract pins as byte-identical across
/// worker counts are `frames`, `errors`, `fer`, and `mean_iters`. This helper
/// asserts all four:
///
/// * `frames` and `errors` — the integer-exact `u64` counters, asserted
///   directly.
/// * `fer` (`errors / frames`) and `mean_iters` (`total_iterations / frames`) —
///   derived `f64` ratios, asserted via their exact **bit patterns**
///   ([`f64::to_bits`]) so the check is strictly byte-identical, not merely
///   approximately equal. `total_iterations` (the numerator of `mean_iters`) is
///   asserted directly too, so a regression in either the ratio or its inputs is
///   caught.
///
/// # BER is excluded (issue `152388f4`, design-doc §11)
///
/// The bit-error-rate column (`total_bit_errors / total_bits`) is **NOT**
/// asserted here. Per design-doc §11 "Always-excluded", BER is a
/// non-associative `f32` horizontal reduction whose value depends on summation
/// order, so it is not byte-identical across worker counts (status-quo
/// amendment from issue `152388f4`). Callers may *record* BER for diagnostics
/// but must never assert it; this helper intentionally provides no BER
/// comparison so that exclusion cannot be circumvented by accident.
///
/// # Arguments
///
/// * `actual` — the counters from a non-baseline worker count.
/// * `baseline` — the 1-worker reference counters.
/// * `label` — a human-readable config/worker label for assertion messages.
///
/// # Panics
///
/// Panics (via `assert_eq!`) if any of the four byte-identity columns differ
/// between `actual` and `baseline`: `frames`, `errors`, the `fer` bit pattern,
/// or the `mean_iters` bit pattern (including its `total_iterations`
/// numerator). The panic message names the offending column and both values.
#[track_caller]
pub fn assert_four_columns_byte_identical(
    actual: &WorkerCounters,
    baseline: &WorkerCounters,
    label: &str,
) {
    // Column 1: frames (u64, integer-exact).
    assert_eq!(
        actual.frames, baseline.frames,
        "{label}: `frames` differs ({} vs baseline {})",
        actual.frames, baseline.frames
    );
    // Column 2: errors (u64, integer-exact).
    assert_eq!(
        actual.errors, baseline.errors,
        "{label}: `errors` differs ({} vs baseline {})",
        actual.errors, baseline.errors
    );
    // Column 3: fer = errors/frames, asserted by exact bit pattern.
    assert_eq!(
        actual.fer().to_bits(),
        baseline.fer().to_bits(),
        "{label}: `fer` bit pattern differs ({} vs baseline {})",
        actual.fer(),
        baseline.fer()
    );
    // Column 4: mean_iters = total_iterations/frames, asserted by exact bit
    // pattern, plus the integer numerator it is derived from.
    assert_eq!(
        actual.total_iterations, baseline.total_iterations,
        "{label}: `total_iterations` differs ({} vs baseline {})",
        actual.total_iterations, baseline.total_iterations
    );
    assert_eq!(
        actual.mean_iters().to_bits(),
        baseline.mean_iters().to_bits(),
        "{label}: `mean_iters` bit pattern differs ({} vs baseline {})",
        actual.mean_iters(),
        baseline.mean_iters()
    );

    // BER (total_bit_errors / total_bits) is intentionally NOT asserted — it is
    // always excluded from byte-identity (issue `152388f4`; design-doc §11
    // "Always-excluded"). No comparison is offered for it on purpose.
}
