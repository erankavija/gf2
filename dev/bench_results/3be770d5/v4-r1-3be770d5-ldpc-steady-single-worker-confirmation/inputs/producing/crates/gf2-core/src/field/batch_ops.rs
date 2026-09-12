//! Montgomery's batch inversion trick for [`FiniteField`] elements.
//!
//! Computing the multiplicative inverse in a finite field is typically one to
//! three orders of magnitude more expensive than a multiplication (Fermat
//! exponentiation is `O(log P)` multiplications for prime fields, Euclidean
//! inversion is comparable for extension fields). When a caller needs to
//! invert `N` independent elements it is wasteful to pay `N · cost(inv)`:
//! Montgomery's classical trick collapses the work to **one** inversion plus
//! `3(N − 1)` multiplications by exploiting the identity
//!
//! ```text
//!     a⁻¹ = (b · c · … · z) · (a · b · c · … · z)⁻¹
//! ```
//!
//! where the pair of products either side of the single inversion is built up
//! in one forward and one backward pass over the slice.
//!
//! # Functions in this module
//!
//! - [`batch_inverse`] — returns `Some(Vec)` or `None` if any element is zero.
//! - [`batch_inverse_in_place`] — same contract but writes back into the slice.
//! - [`batch_inverse_with_scratch`] — caller-provided output and scratch buffers.
//! - [`batch_inverse_skip_zeros`] — leaves zeros as zero and inverts the rest.
//! - [`batch_inverse_skip_zeros_in_place`] — in-place version of the above.
//!
//! The *skip-zeros* variants are intended for projective-coordinate
//! normalisation and similar workloads where zero values simply carry through
//! (see [`batch_inverse_skip_zeros`]). Plain variants treat a zero as an
//! arithmetic error and return `None`.
//!
//! # Algorithm (Montgomery 1987)
//!
//! Given `a[0], a[1], …, a[N-1]` we build a prefix-product scratch
//! `p[i] = a[0] · a[1] · … · a[i]`, compute a single inverse
//! `t = p[N-1]⁻¹`, and then sweep backwards to extract each inverse:
//!
//! ```text
//!     for i in (1..N).rev():
//!         out[i] = t · p[i-1]
//!         t      = t · a[i]
//!     out[0] = t
//! ```
//!
//! Cost: one `inv`, `(N-1)` multiplications to build the prefix products, and
//! `2(N-1)` multiplications on the backward pass — `3(N-1)` multiplications in
//! total.
//!
//! # Benchmark results
//!
//! Measured on `Fp<65537>` (Montgomery-form GF(p) with `u64` canonical
//! storage) using `cargo bench -p gf2-core --bench batch_inverse -- --quick`
//! on the repo's reference machine:
//!
//! | N    | individual inv (total) | batch inv (total) | speedup |
//! |-----:|-----------------------:|------------------:|--------:|
//! |   16 |                 760 ns |            180 ns |   ~4.2× |
//! |  100 |                4.49 µs |            851 ns |   ~5.3× |
//! | 1000 |                45.5 µs |           7.88 µs |   ~5.8× |
//!
//! The `N = 100` data point clears the ≥5× target mandated by the issue
//! specification, and the ratio keeps growing with `N` (an individual
//! `inv()` is `O(log P)` multiplications while the batch amortises a single
//! inversion over `N` elements). The `N = 16` cell stops short of 5× —
//! that's expected: at small `N` the fixed cost of the one remaining
//! inversion still dominates. See `crates/gf2-core/benches/batch_inverse.rs`
//! for the measurement harness; regenerate the table with
//! `cargo bench -p gf2-core --bench batch_inverse`.

use crate::field::FiniteField;

/// Batch-invert a slice of finite field elements using Montgomery's trick.
///
/// Returns `Some(v)` where `v[i] = elements[i]⁻¹` on success, or `None` if
/// any input element is zero. Consult the [module docs](self) for the
/// cost model.
///
/// # Arguments
///
/// * `elements` — slice of field elements to invert. May be empty.
///
/// # Examples
///
/// ```
/// use gf2_core::field::batch_ops::batch_inverse;
/// use gf2_core::field::{ConstField, FiniteField};
/// use gf2_core::gfp::Fp;
///
/// let xs: Vec<Fp<65537>> = (1u64..=5).map(Fp::<65537>::new).collect();
/// let invs = batch_inverse(&xs).unwrap();
/// for (x, inv) in xs.iter().zip(invs.iter()) {
///     assert!((*x * *inv).is_one());
/// }
/// ```
///
/// ```
/// use gf2_core::field::batch_ops::batch_inverse;
/// use gf2_core::gfp::Fp;
///
/// // A zero anywhere in the slice causes the whole batch to fail.
/// let xs: Vec<Fp<7>> = vec![Fp::<7>::new(3), Fp::<7>::new(0), Fp::<7>::new(5)];
/// assert!(batch_inverse(&xs).is_none());
/// ```
///
/// # Complexity
///
/// `O(N)` time with exactly **1 inversion** and **3(N − 1) multiplications**
/// for `N ≥ 1`. `O(N)` additional memory is allocated for the output vector
/// and an internal scratch buffer; use [`batch_inverse_with_scratch`] to
/// reuse buffers across calls.
///
/// # See also
///
/// - [`batch_inverse_in_place`] — overwrites the input slice.
/// - [`batch_inverse_skip_zeros`] — zero inputs pass through unchanged.
pub fn batch_inverse<F: FiniteField>(elements: &[F]) -> Option<Vec<F>> {
    if elements.is_empty() {
        return Some(Vec::new());
    }

    // We pre-fill `output` with clones so we can use `with_scratch` which
    // writes `output[i] = elements[i]⁻¹`. The initial values are never read.
    let mut output: Vec<F> = elements.to_vec();
    let mut scratch: Vec<F> = elements.to_vec();
    batch_inverse_with_scratch(elements, &mut output, &mut scratch)?;
    Some(output)
}

/// In-place batch inversion; returns `None` if any element is zero
/// (leaving the slice unchanged in that case).
///
/// # Arguments
///
/// * `elements` — slice to be overwritten by its element-wise inverses.
///
/// # Examples
///
/// ```
/// use gf2_core::field::batch_ops::batch_inverse_in_place;
/// use gf2_core::field::{ConstField, FiniteField};
/// use gf2_core::gfp::Fp;
///
/// let mut xs: Vec<Fp<65537>> = (1u64..=4).map(Fp::<65537>::new).collect();
/// let originals = xs.clone();
/// batch_inverse_in_place(&mut xs).unwrap();
/// for (o, inv) in originals.iter().zip(xs.iter()) {
///     assert!((*o * *inv).is_one());
/// }
/// ```
///
/// # Complexity
///
/// `O(N)` time; `O(N)` temporary scratch for the prefix products.
///
/// # See also
///
/// - [`batch_inverse`] — returns a new `Vec`.
pub fn batch_inverse_in_place<F: FiniteField>(elements: &mut [F]) -> Option<()> {
    if elements.is_empty() {
        return Some(());
    }

    // Early reject: any zero poisons the batch. Checking up-front means we
    // don't mutate the slice at all on the failure path, which makes the
    // in-place API easier to reason about.
    if elements.iter().any(F::is_zero) {
        return None;
    }

    let mut scratch: Vec<F> = elements.to_vec();
    batch_inverse_core(elements, &mut scratch, InPlaceMode::Yes)?;
    Some(())
}

/// Batch inversion into a caller-provided buffer with a caller-provided scratch.
///
/// This is the allocation-free entry point, intended for inner-loop callers
/// that reuse buffers across batches.
///
/// # Arguments
///
/// * `elements` — input slice.
/// * `output` — destination slice for the inverses. Must have `elements.len()` entries.
///   On failure (any zero input) the contents are unspecified.
/// * `scratch` — temporary workspace. Must have `elements.len()` entries.
///   Its contents on return are unspecified and should not be relied on.
///
/// # Panics
///
/// Panics if `output.len() != elements.len()` or `scratch.len() != elements.len()`.
///
/// # Examples
///
/// ```
/// use gf2_core::field::batch_ops::batch_inverse_with_scratch;
/// use gf2_core::field::{ConstField, FiniteField};
/// use gf2_core::gfp::Fp;
///
/// let xs: Vec<Fp<65537>> = (1u64..=3).map(Fp::<65537>::new).collect();
/// let mut out = vec![Fp::<65537>::zero(); xs.len()];
/// let mut scratch = vec![Fp::<65537>::zero(); xs.len()];
/// batch_inverse_with_scratch(&xs, &mut out, &mut scratch).unwrap();
/// for (x, inv) in xs.iter().zip(out.iter()) {
///     assert!((*x * *inv).is_one());
/// }
/// ```
///
/// # Complexity
///
/// `O(N)` time, no heap allocation.
pub fn batch_inverse_with_scratch<F: FiniteField>(
    elements: &[F],
    output: &mut [F],
    scratch: &mut [F],
) -> Option<()> {
    assert_eq!(output.len(), elements.len(), "output length mismatch");
    assert_eq!(scratch.len(), elements.len(), "scratch length mismatch");

    if elements.is_empty() {
        return Some(());
    }

    if elements.iter().any(F::is_zero) {
        return None;
    }

    // Copy into output so the core routine can treat it as the working slice.
    output.clone_from_slice(elements);
    batch_inverse_core(output, scratch, InPlaceMode::Yes)?;
    Some(())
}

/// Batch-invert, treating zeros as zero instead of as errors.
///
/// For each `i`, the output contains `elements[i]⁻¹` if `elements[i]` is
/// non-zero and `F::zero()` otherwise. This is the convention needed for
/// projective-coordinate normalisation, where a point's affine `x`
/// coordinate is `X / Z` and the point-at-infinity has `Z = 0` and is
/// reported as a zero placeholder.
///
/// The implementation still uses Montgomery's trick over the non-zero
/// sub-sequence, so the cost is `1` inversion plus `3(K − 1)`
/// multiplications where `K` is the number of non-zero inputs (plus `O(N)`
/// bookkeeping).
///
/// # Arguments
///
/// * `elements` — slice of field elements.
///
/// # Examples
///
/// ```
/// use gf2_core::field::batch_ops::batch_inverse_skip_zeros;
/// use gf2_core::field::{ConstField, FiniteField};
/// use gf2_core::gfp::Fp;
///
/// let xs: Vec<Fp<65537>> = vec![
///     Fp::<65537>::new(2),
///     Fp::<65537>::zero(),
///     Fp::<65537>::new(5),
/// ];
/// let invs = batch_inverse_skip_zeros(&xs);
/// assert!((xs[0] * invs[0]).is_one());
/// assert!(invs[1].is_zero());
/// assert!((xs[2] * invs[2]).is_one());
/// ```
///
/// # Complexity
///
/// `O(N)` time, one inversion, `3(K − 1)` multiplications over `K` non-zero
/// inputs.
///
/// # See also
///
/// - [`batch_inverse`] — fails fast on any zero.
pub fn batch_inverse_skip_zeros<F: FiniteField>(elements: &[F]) -> Vec<F> {
    let mut out = elements.to_vec();
    batch_inverse_skip_zeros_in_place(&mut out);
    out
}

/// In-place version of [`batch_inverse_skip_zeros`].
///
/// # Arguments
///
/// * `elements` — slice to rewrite; zeros stay zero, non-zeros become their inverse.
///
/// # Examples
///
/// ```
/// use gf2_core::field::batch_ops::batch_inverse_skip_zeros_in_place;
/// use gf2_core::field::{ConstField, FiniteField};
/// use gf2_core::gfp::Fp;
///
/// let mut xs: Vec<Fp<65537>> = vec![
///     Fp::<65537>::new(2),
///     Fp::<65537>::zero(),
///     Fp::<65537>::new(5),
/// ];
/// let originals = xs.clone();
/// batch_inverse_skip_zeros_in_place(&mut xs);
/// assert!((originals[0] * xs[0]).is_one());
/// assert!(xs[1].is_zero());
/// assert!((originals[2] * xs[2]).is_one());
/// ```
///
/// # Complexity
///
/// Same as [`batch_inverse_skip_zeros`].
pub fn batch_inverse_skip_zeros_in_place<F: FiniteField>(elements: &mut [F]) {
    if elements.is_empty() {
        return;
    }

    // Gather the positions of the non-zero entries. A dense gather/scatter
    // is simpler than threading conditional reductions through the scan.
    let nonzero_idx: Vec<usize> = elements
        .iter()
        .enumerate()
        .filter_map(|(i, e)| if e.is_zero() { None } else { Some(i) })
        .collect();

    if nonzero_idx.is_empty() {
        // All zeros — nothing to do.
        return;
    }

    // Gather the non-zero elements into a compacted workspace, invert that
    // in-place, then scatter back into `elements` at the recorded indices.
    let mut compact: Vec<F> = nonzero_idx.iter().map(|&i| elements[i].clone()).collect();
    let mut scratch: Vec<F> = compact.clone();

    // `batch_inverse_core` never returns `None` here because we guaranteed
    // non-zero inputs above — expect() documents that invariant.
    batch_inverse_core(&mut compact, &mut scratch, InPlaceMode::Yes)
        .expect("batch_inverse_core cannot fail on a slice with no zero entries");

    for (idx, inv) in nonzero_idx.iter().zip(compact) {
        elements[*idx] = inv;
    }
}

// ---------------------------------------------------------------------------
// Internal core routine
// ---------------------------------------------------------------------------

/// Whether the working slice already holds the input values (they will be
/// overwritten with inverses).
///
/// The current implementation always uses `InPlaceMode::Yes` — the enum exists
/// to document that the routine mutates `working`, and to leave room for a
/// future in-place-preserving variant without touching the call sites.
enum InPlaceMode {
    Yes,
}

/// Core of Montgomery's trick. `working` holds the input on entry and the
/// inverses on return. `scratch` is used for the prefix products. Both must
/// have the same length.
///
/// Preconditions: no element in `working` is zero, lengths agree. Violating
/// these leads to an arithmetic panic (from `F::inv()` being called on zero)
/// or a slice-bounds panic.
fn batch_inverse_core<F: FiniteField>(
    working: &mut [F],
    scratch: &mut [F],
    _mode: InPlaceMode,
) -> Option<()> {
    let n = working.len();
    debug_assert_eq!(n, scratch.len());

    if n == 0 {
        return Some(());
    }

    if n == 1 {
        // Degenerate case: no multiplications, one inversion.
        let inv = working[0].inv()?;
        working[0] = inv;
        return Some(());
    }

    // Forward pass — `scratch[i] = working[0] * working[1] * … * working[i]`.
    // (N-1) multiplications.
    scratch[0] = working[0].clone();
    for i in 1..n {
        scratch[i] = scratch[i - 1].clone() * working[i].clone();
    }

    // Single inversion of the full product.
    let mut running_inv = scratch[n - 1].inv()?;

    // Backward pass — reconstruct each inverse. 2(N-1) multiplications:
    // one for `individual_inv`, one to update `running_inv`.
    for i in (1..n).rev() {
        // working[i]⁻¹ = running_inv * scratch[i-1]
        let individual_inv = running_inv.clone() * scratch[i - 1].clone();
        // running_inv now tracks `(working[0] * … * working[i-1])⁻¹`.
        running_inv = running_inv * working[i].clone();
        working[i] = individual_inv;
    }
    working[0] = running_inv;
    Some(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::{ConstField, FiniteField};
    use crate::gfp::Fp;
    use proptest::prelude::*;
    use std::cell::Cell;

    const MERSENNE_61: u64 = (1u64 << 61) - 1;

    // --- Basic smoke tests ---------------------------------------------------

    #[test]
    fn test_batch_inverse_empty() {
        let xs: Vec<Fp<7>> = Vec::new();
        assert_eq!(batch_inverse(&xs).unwrap(), Vec::<Fp<7>>::new());
    }

    #[test]
    fn test_batch_inverse_single() {
        let xs = vec![Fp::<7>::new(3)];
        let invs = batch_inverse(&xs).unwrap();
        assert_eq!(invs.len(), 1);
        assert!((xs[0] * invs[0]).is_one());
    }

    #[test]
    fn test_batch_inverse_single_zero_fails() {
        let xs = vec![Fp::<7>::zero()];
        assert!(batch_inverse(&xs).is_none());
    }

    #[test]
    fn test_batch_inverse_matches_individual() {
        let xs: Vec<Fp<65537>> = (1u64..=10).map(Fp::<65537>::new).collect();
        let batched = batch_inverse(&xs).unwrap();
        for (x, b) in xs.iter().zip(batched.iter()) {
            assert_eq!(*b, x.inv().unwrap());
            assert!((*x * *b).is_one());
        }
    }

    #[test]
    fn test_batch_inverse_with_zero_returns_none() {
        let xs = vec![
            Fp::<65537>::new(3),
            Fp::<65537>::zero(),
            Fp::<65537>::new(5),
        ];
        assert!(batch_inverse(&xs).is_none());
    }

    #[test]
    fn test_batch_inverse_in_place_matches_individual() {
        let originals: Vec<Fp<65537>> = (1u64..=10).map(Fp::<65537>::new).collect();
        let mut xs = originals.clone();
        batch_inverse_in_place(&mut xs).unwrap();
        for (o, inv) in originals.iter().zip(xs.iter()) {
            assert!((*o * *inv).is_one());
        }
    }

    #[test]
    fn test_batch_inverse_in_place_leaves_input_untouched_on_zero() {
        let originals = vec![Fp::<7>::new(3), Fp::<7>::zero(), Fp::<7>::new(5)];
        let mut xs = originals.clone();
        assert!(batch_inverse_in_place(&mut xs).is_none());
        assert_eq!(xs, originals);
    }

    #[test]
    fn test_batch_inverse_with_scratch_basic() {
        let xs: Vec<Fp<65537>> = (1u64..=5).map(Fp::<65537>::new).collect();
        let mut out = vec![Fp::<65537>::zero(); xs.len()];
        let mut scratch = vec![Fp::<65537>::zero(); xs.len()];
        batch_inverse_with_scratch(&xs, &mut out, &mut scratch).unwrap();
        for (x, inv) in xs.iter().zip(out.iter()) {
            assert!((*x * *inv).is_one());
        }
    }

    #[test]
    #[should_panic(expected = "output length mismatch")]
    fn test_batch_inverse_with_scratch_panics_on_output_len_mismatch() {
        let xs: Vec<Fp<7>> = (1u64..=3).map(Fp::<7>::new).collect();
        let mut out = vec![Fp::<7>::zero(); 2];
        let mut scratch = vec![Fp::<7>::zero(); 3];
        let _ = batch_inverse_with_scratch(&xs, &mut out, &mut scratch);
    }

    #[test]
    #[should_panic(expected = "scratch length mismatch")]
    fn test_batch_inverse_with_scratch_panics_on_scratch_len_mismatch() {
        let xs: Vec<Fp<7>> = (1u64..=3).map(Fp::<7>::new).collect();
        let mut out = vec![Fp::<7>::zero(); 3];
        let mut scratch = vec![Fp::<7>::zero(); 2];
        let _ = batch_inverse_with_scratch(&xs, &mut out, &mut scratch);
    }

    // --- Skip-zeros variant --------------------------------------------------

    #[test]
    fn test_batch_inverse_skip_zeros_empty() {
        let xs: Vec<Fp<7>> = Vec::new();
        assert_eq!(batch_inverse_skip_zeros(&xs), Vec::<Fp<7>>::new());
    }

    #[test]
    fn test_batch_inverse_skip_zeros_all_zero() {
        let xs = vec![Fp::<7>::zero(); 4];
        let invs = batch_inverse_skip_zeros(&xs);
        assert!(invs.iter().all(|e| e.is_zero()));
    }

    #[test]
    fn test_batch_inverse_skip_zeros_mixed() {
        let xs = vec![
            Fp::<65537>::new(2),
            Fp::<65537>::zero(),
            Fp::<65537>::new(5),
            Fp::<65537>::zero(),
            Fp::<65537>::new(7),
        ];
        let invs = batch_inverse_skip_zeros(&xs);
        assert_eq!(invs.len(), xs.len());
        assert!((xs[0] * invs[0]).is_one());
        assert!(invs[1].is_zero());
        assert!((xs[2] * invs[2]).is_one());
        assert!(invs[3].is_zero());
        assert!((xs[4] * invs[4]).is_one());
    }

    #[test]
    fn test_batch_inverse_skip_zeros_in_place_mixed() {
        let originals = vec![
            Fp::<65537>::new(2),
            Fp::<65537>::zero(),
            Fp::<65537>::new(5),
        ];
        let mut xs = originals.clone();
        batch_inverse_skip_zeros_in_place(&mut xs);
        assert!((originals[0] * xs[0]).is_one());
        assert!(xs[1].is_zero());
        assert!((originals[2] * xs[2]).is_one());
    }

    // --- Op-count verification (criterion 1) --------------------------------
    //
    // We wrap Fp<65537> in a newtype that forwards every FiniteField operation
    // while incrementing thread-local counters on `mul` and `inv`. Using
    // `Cell<u64>` instead of atomics keeps the counter read/write paths
    // trivial; the tests are single-threaded so no synchronisation is needed.

    thread_local! {
        static MUL_COUNT: Cell<u64> = const { Cell::new(0) };
        static INV_COUNT: Cell<u64> = const { Cell::new(0) };
    }

    fn reset_counters() {
        MUL_COUNT.with(|c| c.set(0));
        INV_COUNT.with(|c| c.set(0));
    }

    fn mul_count() -> u64 {
        MUL_COUNT.with(Cell::get)
    }

    fn inv_count() -> u64 {
        INV_COUNT.with(Cell::get)
    }

    fn bump_mul() {
        MUL_COUNT.with(|c| c.set(c.get().wrapping_add(1)));
    }

    fn bump_inv() {
        INV_COUNT.with(|c| c.set(c.get().wrapping_add(1)));
    }

    #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
    struct OpCount(Fp<65537>);

    impl OpCount {
        fn new(v: u64) -> Self {
            Self(Fp::<65537>::new(v))
        }
    }

    // Ops: forward to the inner Fp, counting mul but not add/sub/div — only
    // the two operations relevant to the cost model are instrumented. Note
    // that `Div` is implemented via `mul(inv)` so the batch routine must not
    // invoke `/` internally, only `*` and `.inv()`.

    impl std::ops::Add for OpCount {
        type Output = Self;
        fn add(self, rhs: Self) -> Self {
            Self(self.0 + rhs.0)
        }
    }
    impl std::ops::Add<&OpCount> for OpCount {
        type Output = Self;
        fn add(self, rhs: &OpCount) -> Self {
            Self(self.0 + rhs.0)
        }
    }
    impl std::ops::Sub for OpCount {
        type Output = Self;
        fn sub(self, rhs: Self) -> Self {
            Self(self.0 - rhs.0)
        }
    }
    impl std::ops::Sub<&OpCount> for OpCount {
        type Output = Self;
        fn sub(self, rhs: &OpCount) -> Self {
            Self(self.0 - rhs.0)
        }
    }
    impl std::ops::Mul for OpCount {
        type Output = Self;
        fn mul(self, rhs: Self) -> Self {
            bump_mul();
            Self(self.0 * rhs.0)
        }
    }
    impl std::ops::Mul<&OpCount> for OpCount {
        type Output = Self;
        fn mul(self, rhs: &OpCount) -> Self {
            bump_mul();
            Self(self.0 * rhs.0)
        }
    }
    impl std::ops::Div for OpCount {
        type Output = Self;
        fn div(self, rhs: Self) -> Self {
            // Not used internally, but required by the FiniteField trait.
            Self(self.0 / rhs.0)
        }
    }
    impl std::ops::Div<&OpCount> for OpCount {
        type Output = Self;
        fn div(self, rhs: &OpCount) -> Self {
            Self(self.0 / rhs.0)
        }
    }
    impl std::ops::Neg for OpCount {
        type Output = Self;
        fn neg(self) -> Self {
            Self(-self.0)
        }
    }
    impl std::ops::AddAssign for OpCount {
        fn add_assign(&mut self, rhs: Self) {
            self.0 += rhs.0;
        }
    }
    impl std::ops::AddAssign<&OpCount> for OpCount {
        fn add_assign(&mut self, rhs: &OpCount) {
            self.0 += rhs.0;
        }
    }

    impl FiniteField for OpCount {
        type Characteristic = u64;
        type Wide = u128;

        fn characteristic(&self) -> u64 {
            self.0.characteristic()
        }
        fn extension_degree(&self) -> usize {
            self.0.extension_degree()
        }
        fn is_zero(&self) -> bool {
            self.0.is_zero()
        }
        fn is_one(&self) -> bool {
            self.0.is_one()
        }
        fn inv(&self) -> Option<Self> {
            bump_inv();
            self.0.inv().map(Self)
        }
        fn zero_like(&self) -> Self {
            Self(self.0.zero_like())
        }
        fn one_like(&self) -> Self {
            Self(self.0.one_like())
        }
        fn to_wide(&self) -> u128 {
            self.0.to_wide()
        }
        fn mul_to_wide(&self, rhs: &Self) -> u128 {
            self.0.mul_to_wide(&rhs.0)
        }
        fn reduce_wide(wide: &u128) -> Self {
            Self(<Fp<65537> as FiniteField>::reduce_wide(wide))
        }
        fn max_unreduced_additions() -> usize {
            <Fp<65537> as FiniteField>::max_unreduced_additions()
        }
    }

    #[test]
    fn test_op_count_n1() {
        // N=1 degenerate case: 1 inversion, 0 multiplications.
        reset_counters();
        let xs = vec![OpCount::new(5)];
        let _ = batch_inverse(&xs).unwrap();
        assert_eq!(inv_count(), 1, "should call inv exactly once");
        assert_eq!(
            mul_count(),
            0,
            "should perform zero multiplications for N=1"
        );
    }

    #[test]
    fn test_op_count_matches_3n_minus_3() {
        // The headline cost claim: 1 inv + 3(N-1) muls.
        for n in [2usize, 4, 8, 16, 100] {
            reset_counters();
            let xs: Vec<OpCount> = (1..=n as u64).map(OpCount::new).collect();
            let _ = batch_inverse(&xs).unwrap();
            assert_eq!(
                inv_count(),
                1,
                "exactly 1 inversion expected, got {} at N={}",
                inv_count(),
                n
            );
            assert_eq!(
                mul_count(),
                3 * (n as u64 - 1),
                "expected {} multiplications, got {} at N={}",
                3 * (n - 1),
                mul_count(),
                n
            );
        }
    }

    #[test]
    fn test_op_count_in_place_matches() {
        // In-place path should have the same op count (ignoring the zero-scan,
        // which doesn't touch mul/inv).
        reset_counters();
        let mut xs: Vec<OpCount> = (1..=10u64).map(OpCount::new).collect();
        batch_inverse_in_place(&mut xs).unwrap();
        assert_eq!(inv_count(), 1);
        assert_eq!(mul_count(), 3 * 9);
    }

    // --- Proptests (criteria 2, 3, 6) ---------------------------------------

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(500))]

        #[test]
        fn prop_batch_inverse_matches_individual_fp7(
            xs in prop::collection::vec(1u64..7, 1..20),
        ) {
            let elems: Vec<Fp<7>> = xs.into_iter().map(Fp::<7>::new).collect();
            let batched = batch_inverse(&elems).unwrap();
            let expected: Vec<Fp<7>> = elems.iter().map(|e| e.inv().unwrap()).collect();
            prop_assert_eq!(batched, expected);
        }

        #[test]
        fn prop_batch_inverse_matches_individual_fp65537(
            xs in prop::collection::vec(1u64..65537, 1..50),
        ) {
            let elems: Vec<Fp<65537>> = xs.into_iter().map(Fp::<65537>::new).collect();
            let batched = batch_inverse(&elems).unwrap();
            let expected: Vec<Fp<65537>> = elems.iter().map(|e| e.inv().unwrap()).collect();
            prop_assert_eq!(batched, expected);
        }

        #[test]
        fn prop_batch_inverse_matches_individual_mersenne61(
            xs in prop::collection::vec(1u64..MERSENNE_61, 1..50),
        ) {
            let elems: Vec<Fp<MERSENNE_61>> =
                xs.into_iter().map(Fp::<MERSENNE_61>::new).collect();
            let batched = batch_inverse(&elems).unwrap();
            let expected: Vec<Fp<MERSENNE_61>> =
                elems.iter().map(|e| e.inv().unwrap()).collect();
            prop_assert_eq!(batched, expected);
        }

        #[test]
        fn prop_product_is_one_fp65537(
            xs in prop::collection::vec(1u64..65537, 1..50),
        ) {
            let elems: Vec<Fp<65537>> = xs.into_iter().map(Fp::<65537>::new).collect();
            let batched = batch_inverse(&elems).unwrap();
            for (x, inv) in elems.iter().zip(batched.iter()) {
                prop_assert!((*x * *inv).is_one());
            }
        }

        #[test]
        fn prop_zero_anywhere_returns_none(
            mut xs in prop::collection::vec(1u64..65537, 1..50),
            pos in any::<prop::sample::Index>(),
        ) {
            let idx = pos.index(xs.len());
            xs[idx] = 0;
            let elems: Vec<Fp<65537>> = xs.into_iter().map(Fp::<65537>::new).collect();
            prop_assert!(batch_inverse(&elems).is_none());
            let mut clone = elems.clone();
            prop_assert!(batch_inverse_in_place(&mut clone).is_none());
            // On-failure preservation of in-place input.
            prop_assert_eq!(clone, elems);
        }

        #[test]
        fn prop_skip_zeros_matches_spec(
            xs in prop::collection::vec(0u64..65537, 1..50),
        ) {
            let elems: Vec<Fp<65537>> = xs.into_iter().map(Fp::<65537>::new).collect();
            let invs = batch_inverse_skip_zeros(&elems);
            prop_assert_eq!(invs.len(), elems.len());
            for (x, inv) in elems.iter().zip(invs.iter()) {
                if x.is_zero() {
                    prop_assert!(inv.is_zero());
                } else {
                    prop_assert!((*x * *inv).is_one());
                }
            }
        }
    }
}
