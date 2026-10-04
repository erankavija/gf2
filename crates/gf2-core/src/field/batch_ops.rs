//! Montgomery's batch inversion trick (`@/citation/Montgomery1987`) for
//! [`FiniteField`] elements: `N` inverses from one inversion and `3(N − 1)`
//! multiplications, through one forward pass of prefix products and one
//! backward pass.

use crate::field::FiniteField;

/// Batch-inverts a slice of finite field elements using Montgomery's trick.
///
/// Returns `Some(v)` where `v[i] = elements[i]⁻¹`, or `None` if any input
/// element is zero.
///
/// # Complexity
///
/// One inversion and `3(N − 1)` multiplications for `N ≥ 1`, with `O(N)`
/// memory for the output vector and a scratch buffer;
/// [`batch_inverse_with_scratch`] reuses caller buffers.
pub fn batch_inverse<F: FiniteField>(elements: &[F]) -> Option<Vec<F>> {
    if elements.is_empty() {
        return Some(Vec::new());
    }

    // The initial contents of `output` and `scratch` are never read.
    let mut output: Vec<F> = elements.to_vec();
    let mut scratch: Vec<F> = elements.to_vec();
    batch_inverse_with_scratch(elements, &mut output, &mut scratch)?;
    Some(output)
}

/// In-place batch inversion; returns `None` if any element is zero
/// (leaving the slice unchanged in that case).
///
/// # Complexity
///
/// `O(N)` time; `O(N)` scratch for the prefix products.
pub fn batch_inverse_in_place<F: FiniteField>(elements: &mut [F]) -> Option<()> {
    if elements.is_empty() {
        return Some(());
    }

    if elements.iter().any(F::is_zero) {
        return None;
    }

    let mut scratch: Vec<F> = elements.to_vec();
    batch_inverse_core(elements, &mut scratch, InPlaceMode::Yes)?;
    Some(())
}

/// Batch inversion into caller-provided `output` and `scratch` buffers,
/// allocating none of its own.
///
/// Returns `None` if any input is zero, leaving the contents of `output`
/// unspecified. The contents of `scratch` on return are unspecified.
///
/// # Panics
///
/// Panics if `output.len() != elements.len()` or `scratch.len() != elements.len()`.
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

    output.clone_from_slice(elements);
    batch_inverse_core(output, scratch, InPlaceMode::Yes)?;
    Some(())
}

/// Batch-inverts, mapping each zero input to zero and each non-zero input to
/// its inverse.
///
/// # Complexity
///
/// `O(N)` time, one inversion, `3(K − 1)` multiplications over `K` non-zero
/// inputs.
pub fn batch_inverse_skip_zeros<F: FiniteField>(elements: &[F]) -> Vec<F> {
    let mut out = elements.to_vec();
    batch_inverse_skip_zeros_in_place(&mut out);
    out
}

/// In-place version of [`batch_inverse_skip_zeros`].
///
/// # Complexity
///
/// Same as [`batch_inverse_skip_zeros`].
pub fn batch_inverse_skip_zeros_in_place<F: FiniteField>(elements: &mut [F]) {
    if elements.is_empty() {
        return;
    }

    // Gather and scatter, to keep conditional reductions out of the scan.
    let nonzero_idx: Vec<usize> = elements
        .iter()
        .enumerate()
        .filter_map(|(i, e)| if e.is_zero() { None } else { Some(i) })
        .collect();

    if nonzero_idx.is_empty() {
        return;
    }

    let mut compact: Vec<F> = nonzero_idx.iter().map(|&i| elements[i].clone()).collect();
    let mut scratch: Vec<F> = compact.clone();

    batch_inverse_core(&mut compact, &mut scratch, InPlaceMode::Yes)
        .expect("batch_inverse_core cannot fail on a slice with no zero entries");

    for (idx, inv) in nonzero_idx.iter().zip(compact) {
        elements[*idx] = inv;
    }
}

/// Marks that `batch_inverse_core` overwrites its working slice with the inverses.
enum InPlaceMode {
    Yes,
}

/// Core of Montgomery's trick. `working` holds the input on entry and the
/// inverses on return; `scratch`, of the same length, holds the prefix
/// products.
///
/// Returns `None` with `working` unchanged if an element is zero.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::{ConstField, FiniteField};
    use crate::gfp::Fp;
    use proptest::prelude::*;
    use std::cell::Cell;

    const MERSENNE_61: u64 = (1u64 << 61) - 1;

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

    // Thread-local `Cell` counters: each test runs on one thread.

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

    // Only `mul` and `inv` are counted; a `/` in the routine would bypass both
    // counters.

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
        // The zero scan touches neither counter.
        reset_counters();
        let mut xs: Vec<OpCount> = (1..=10u64).map(OpCount::new).collect();
        batch_inverse_in_place(&mut xs).unwrap();
        assert_eq!(inv_count(), 1);
        assert_eq!(mul_count(), 3 * 9);
    }

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
