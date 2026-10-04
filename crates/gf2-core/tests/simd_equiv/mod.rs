//! Shared scaffolding for SIMD-vs-scalar equivalence tests: a proptest
//! driver, the word-boundary length list and an unaligned-slice helper.
//! Loaded as `mod simd_equiv;` by sibling integration-test binaries.

#![allow(dead_code)]

use proptest::strategy::Strategy;
use proptest::test_runner::{Config, TestCaseError, TestRunner};

/// Bit lengths bracketing the `u64` word boundary (0, 1, 63, 64, 65) and
/// the 128- and 256-bit boundaries.
pub const WORD_BOUNDARY_LENGTHS: &[usize] = &[0, 1, 63, 64, 65, 127, 128, 129, 255, 256, 257];

/// Proptest case count of [`assert_simd_matches_scalar`], below the proptest
/// default of 256 to fit the fast-tier per-test budget.
pub const DEFAULT_CASES: u32 = 64;

/// Runs `gen` through proptest and asserts that `scalar` and `simd` return
/// equal values and leave equal post-call state in `T`. Each function
/// receives its own clone of the generated input.
///
/// # Panics
///
/// Panics if a generated input makes the two disagree or either function
/// panics, reporting proptest's shrunk counterexample.
pub fn assert_simd_matches_scalar<T, R, F, G, S>(scalar: F, simd: G, gen: S)
where
    T: Clone + PartialEq + std::fmt::Debug,
    R: PartialEq + std::fmt::Debug,
    F: Fn(&mut T) -> R,
    G: Fn(&mut T) -> R,
    S: Strategy<Value = T>,
{
    assert_simd_matches_scalar_with_config(scalar, simd, gen, Config::with_cases(DEFAULT_CASES));
}

/// [`assert_simd_matches_scalar`] with an explicit proptest [`Config`].
///
/// # Panics
///
/// As [`assert_simd_matches_scalar`].
pub fn assert_simd_matches_scalar_with_config<T, R, F, G, S>(
    scalar: F,
    simd: G,
    gen: S,
    config: Config,
) where
    T: Clone + PartialEq + std::fmt::Debug,
    R: PartialEq + std::fmt::Debug,
    F: Fn(&mut T) -> R,
    G: Fn(&mut T) -> R,
    S: Strategy<Value = T>,
{
    let mut runner = TestRunner::new(config);
    runner
        .run(&gen, |input| {
            let mut a = input.clone();
            let mut b = input.clone();
            let r_scalar = scalar(&mut a);
            let r_simd = simd(&mut b);
            if r_scalar != r_simd {
                return Err(TestCaseError::fail(format!(
                    "return-value mismatch: scalar={:?} simd={:?} input={:?}",
                    r_scalar, r_simd, input
                )));
            }
            // For in-place kernels `T` carries the post-call state.
            if a != b {
                return Err(TestCaseError::fail(format!(
                    "post-state mismatch: scalar={:?} simd={:?} input={:?}",
                    a, b, input
                )));
            }
            Ok(())
        })
        .expect("scalar and SIMD implementations diverged on a proptest input");
}

/// Returns the `len` words of `buf` starting at word `offset`, for running a
/// kernel at each alignment of an over-allocated buffer.
///
/// # Panics
///
/// Panics if `offset + len > buf.len()`.
pub fn unaligned_slice(buf: &mut [u64], offset: usize, len: usize) -> &mut [u64] {
    assert!(
        offset + len <= buf.len(),
        "unaligned_slice: offset ({}) + len ({}) exceeds buffer size ({})",
        offset,
        len,
        buf.len()
    );
    &mut buf[offset..offset + len]
}
