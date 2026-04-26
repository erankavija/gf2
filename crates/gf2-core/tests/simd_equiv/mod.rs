//! Shared scaffolding for SIMD-vs-scalar equivalence tests.
//!
//! Every new SIMD kernel landed under the PPC-spiral epic uses these
//! helpers to prove bit-exact equivalence with its scalar reference.
//! Centralising the helper guarantees consistent coverage — random
//! proptest generation, the canonical word-boundary length list, and
//! an unaligned-slice harness — across all kernels.
//!
//! This module is loaded as a `mod` from sibling integration-test
//! binaries (e.g. `tests/simd_equiv_demo.rs`). It is not a test binary
//! on its own; the standard Rust integration-test convention gives every
//! `tests/*.rs` its own crate, and a `tests/foo/mod.rs` file is only
//! compiled when another `tests/*.rs` declares `mod foo;`. We use a
//! demo-suffix on the driver to dodge the `tests/foo.rs` vs
//! `tests/foo/mod.rs` ambiguity that rustc would otherwise flag.
//!
//! # Helpers
//!
//! * [`assert_simd_matches_scalar`] — runs proptest over a generator
//!   and asserts that a scalar reference and a SIMD candidate produce
//!   identical output for every case.
//! * [`WORD_BOUNDARY_LENGTHS`] — canonical bit-length boundary list
//!   that every kernel iterates in a `#[test]`.
//! * [`unaligned_slice`] — carves an unaligned view out of an
//!   over-allocated buffer to exercise unaligned SIMD paths.
//!
//! # Demo
//!
//! The sibling `tests/simd_equiv_demo.rs` driver exercises the helper
//! against `LogicalFns::xor_fn` (the AVX2 in-place XOR kernel) versus
//! the trivial scalar reference, and contains the mutation-test that
//! confirms the helper rejects a deliberately-broken scalar fn.

#![allow(dead_code)]

use proptest::strategy::Strategy;
use proptest::test_runner::{Config, TestCaseError, TestRunner};

/// Canonical word-boundary lengths that every SIMD kernel must cover
/// in a dedicated `#[test]`.
///
/// The values bracket the natural boundaries of a `u64`-packed bit
/// representation: zero, one, the last sub-word bit (63), the first
/// full-word boundary (64), one past it (65), then the same triplet
/// at 128-bit and 256-bit boundaries. Kernels that operate on AVX2
/// 256-bit lanes or AVX-512 512-bit lanes hit lane-tail bugs at these
/// exact lengths most often.
///
/// Tier A–D kernels iterate this list directly:
///
/// ```ignore
/// for &len in WORD_BOUNDARY_LENGTHS {
///     // build inputs of `len` bits, run scalar + simd, compare.
/// }
/// ```
pub const WORD_BOUNDARY_LENGTHS: &[usize] = &[0, 1, 63, 64, 65, 127, 128, 129, 255, 256, 257];

/// Default proptest case count for kernel equivalence runs.
///
/// Set well below the proptest default of 256 so the helper can be
/// invoked from many tests without blowing the 5 s per-test fast-tier
/// cap. Individual call sites can build their own [`Config`] and use
/// [`assert_simd_matches_scalar_with_config`] when they want more.
pub const DEFAULT_CASES: u32 = 64;

/// Runs `gen` through proptest and asserts that `scalar` and `simd`
/// produce bit-exact-equal outputs for every generated input.
///
/// The helper drives a [`TestRunner`] directly so it can be called
/// from any `#[test]` without the `proptest!` macro's syntactic
/// overhead. Both functions take `T` by mutable reference so they can
/// observe / verify any in-place buffer mutation; the helper clones
/// the generated value before each invocation so `scalar` and `simd`
/// see equivalent starting state.
///
/// # Type parameters
///
/// * `T` — the input fixture, must be `Clone + PartialEq + Debug` so
///   the helper can fork the input, compare the post-mutation state of
///   the two clones directly, and report mismatches via proptest.
/// * `R` — the output value compared for equality. For in-place
///   kernels (e.g. `xor_inplace`) this is typically `()` and the
///   verification compares the buffer state captured back through
///   `T`.
/// * `F`, `G` — the scalar reference and SIMD candidate.
/// * `S` — the proptest [`Strategy`] producing fresh inputs.
///
/// # Arguments
///
/// * `scalar` — the reference implementation. Treated as the source
///   of truth.
/// * `simd` — the candidate implementation under test.
/// * `gen` — proptest strategy producing input fixtures.
///
/// # Examples
///
/// ```ignore
/// assert_simd_matches_scalar(
///     |(dst, src): &mut (Vec<u64>, Vec<u64>)| { /* scalar xor */ },
///     |(dst, src): &mut (Vec<u64>, Vec<u64>)| { /* simd xor   */ },
///     (proptest::collection::vec(any::<u64>(), 0..32),
///      proptest::collection::vec(any::<u64>(), 0..32))
///         .prop_map(|(a, b)| (a, b)),
/// );
/// ```
///
/// # Panics
///
/// Panics if proptest finds an input for which `scalar(input) !=
/// simd(input)`, with proptest's standard counterexample-shrinking
/// report. Panics also propagate from `scalar` or `simd` themselves.
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

/// As [`assert_simd_matches_scalar`] but with an explicit proptest
/// [`Config`] so callers can tune case count, shrink iters, and
/// failure-persistence.
///
/// # Panics
///
/// Same as [`assert_simd_matches_scalar`].
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
            // For in-place kernels `T` carries the post-mutation state.
            // Compare the two post-mutation clones directly via
            // `PartialEq`; this is canonical, avoids per-iteration
            // `format!` overhead, and dodges any risk of false
            // negatives from non-stable `Debug` implementations.
            // `Debug` is still required so a counterexample renders.
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

/// Returns a mutable view of exactly `len` `u64` words starting at
/// `offset` words inside `buf`.
///
/// SIMD load/store paths often have alignment-dependent codegen;
/// passing a buffer carved out of a larger over-allocation at offsets
/// 0..7 forces the kernel through every alignment class on a 64-byte
/// AVX-512 boundary.
///
/// # Arguments
///
/// * `buf` — over-allocated backing buffer. Caller is responsible for
///   sizing it at least `offset + len` words.
/// * `offset` — number of `u64` words to skip from the start of `buf`.
/// * `len` — number of `u64` words the returned slice must span.
///
/// # Examples
///
/// ```ignore
/// let mut backing = vec![0u64; 32];
/// for offset in 0..8 {
///     let view = unaligned_slice(&mut backing, offset, 16);
///     assert_eq!(view.len(), 16);
///     // run kernel against `view`...
/// }
/// ```
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
