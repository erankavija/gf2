//! Exact probabilities and exhaustive anchors for permanent-zero events.
//!
//! [`ExactProbability`] is the permanent module's single arbitrary-precision
//! count-over-total representation. The exhaustive enumerator deliberately
//! supports only the small campaign-anchor domain; compressed propagation uses
//! the same probability type at dimensions where matrix enumeration is
//! impossible.

use gf2_core::gfp::Fp;
use num_bigint::BigUint;

/// An exact event probability represented by arbitrary-precision counts.
///
/// `zero_count / matrix_count` is the rational probability of the relevant
/// permanent-zero or permanental-rank-deficiency event. The stored values are
/// deliberately not reduced so callers retain the complete raw outcome count.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactProbability {
    zero_count: BigUint,
    matrix_count: BigUint,
}

impl ExactProbability {
    /// Builds an exact probability from machine-sized counts.
    ///
    /// # Panics
    ///
    /// Panics when `matrix_count` is zero or `zero_count` exceeds it.
    ///
    #[must_use]
    pub fn from_counts(zero_count: u64, matrix_count: u64) -> Self {
        Self::from_big_counts(zero_count.into(), matrix_count.into())
    }

    /// Builds an exact probability from arbitrary-precision counts.
    ///
    /// The counts remain unreduced so the denominator continues to identify
    /// the complete enumerated or propagated sample space.
    ///
    /// # Panics
    ///
    /// Panics when `matrix_count` is zero or `zero_count` exceeds it.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_algebra::permanent::ExactProbability;
    /// use num_bigint::BigUint;
    ///
    /// let scale = BigUint::from(1_u8) << 200;
    /// let probability = ExactProbability::from_big_counts(&scale * 2_u8, &scale * 6_u8);
    /// assert_eq!(probability.reduced_decimal(), ("1".into(), "3".into()));
    /// ```
    #[must_use]
    pub fn from_big_counts(zero_count: BigUint, matrix_count: BigUint) -> Self {
        assert!(
            matrix_count != BigUint::from(0_u8),
            "an exact probability needs a nonzero total"
        );
        assert!(zero_count <= matrix_count, "zero count cannot exceed total");
        Self {
            zero_count,
            matrix_count,
        }
    }

    /// Returns the raw number of outcomes satisfying the exact event.
    #[must_use]
    pub fn zero_count(&self) -> &BigUint {
        &self.zero_count
    }

    /// Returns the raw number of outcomes in the unconditional sample space.
    #[must_use]
    pub fn matrix_count(&self) -> &BigUint {
        &self.matrix_count
    }

    /// Returns the same probability as a reduced numerator and denominator.
    #[must_use]
    pub fn reduced(&self) -> (BigUint, BigUint) {
        let divisor = greatest_common_divisor(self.zero_count.clone(), self.matrix_count.clone());
        (&self.zero_count / &divisor, &self.matrix_count / divisor)
    }

    /// Returns the reduced numerator and denominator as canonical base-ten
    /// integer strings.
    ///
    /// These strings are the serialization-safe exact representation. This
    /// API deliberately provides no floating-point or statistical rendering.
    #[must_use]
    pub fn reduced_decimal(&self) -> (String, String) {
        let (numerator, denominator) = self.reduced();
        (numerator.to_string(), denominator.to_string())
    }
}

/// Exhaustively counts zero permanents in one supported campaign anchor cell.
///
/// The supported cells are $𝔽_3$ through dimension four and
/// $𝔽_5$, $𝔽_7$ through dimension three.  The result retains the
/// integer zero count and the complete $q^{n^2}$ matrix count, so callers can
/// compare or serialize the value without rounding.
///
/// # Panics
///
/// Panics unless `field_order` is 3, 5, or 7 and `dimension` is in that
/// field's supported anchor range.  The two largest cells are intentionally
/// suitable only for the repository's explicitly invoked slow tier.
///
/// # Complexity
///
/// Enumerates $q^{n^2}$ matrices.  Each anchor permanent uses its fixed
/// small-dimensional expansion, so this routine needs constant auxiliary
/// storage.
#[must_use]
pub fn enumerate_permanent_zero_probability(
    field_order: u64,
    dimension: usize,
) -> ExactProbability {
    match field_order {
        3 if (1..=4).contains(&dimension) => enumerate::<3>(dimension),
        5 if (1..=3).contains(&dimension) => enumerate::<5>(dimension),
        7 if (1..=3).contains(&dimension) => enumerate::<7>(dimension),
        _ => panic!(
            "exact permanent anchors support q=3 with n<=4 and q=5,7 with n<=3; got q={field_order}, n={dimension}"
        ),
    }
}

/// Returns the exact finite-`n` probability that a uniform matrix over
/// $\mathbb{F}_q$ is singular.
///
/// The invertible matrices are the general linear group, so the singular count
/// is $q^{n^2} - \prod_{i=0}^{n-1}(q^n - q^i)$. This closed form is exact for
/// every prime power and is the authority the campaign's determinant evaluator
/// must reproduce; nothing here samples or enumerates.
///
/// # Panics
///
/// Panics when `field_order` is below two, when `dimension` is zero, or when
/// $q^{n^2}$ exceeds `u64`.
///
/// # Examples
///
/// ```
/// use gf2_algebra::permanent::determinant_singular_probability;
///
/// // Over F_3 the 2x2 singular matrices are 33 of the 81 matrices.
/// let probability = determinant_singular_probability(3, 2);
/// assert_eq!(probability.zero_count().to_string(), "33");
/// assert_eq!(probability.matrix_count().to_string(), "81");
/// ```
///
/// # Complexity
///
/// `O(n)` multiplications and `O(1)` auxiliary space.
#[must_use]
pub fn determinant_singular_probability(field_order: u64, dimension: usize) -> ExactProbability {
    assert!(field_order >= 2, "a field order is at least two");
    assert!(dimension > 0, "a matrix order is at least one");
    let exponent = u32::try_from(dimension * dimension).expect("anchor exponents fit u32");
    let total = field_order
        .checked_pow(exponent)
        .expect("the anchor matrix count fits u64");
    let order = field_order.pow(u32::try_from(dimension).expect("anchor orders fit u32"));
    let invertible = (0..dimension).fold(1_u64, |count, exponent| {
        count
            * (order - field_order.pow(u32::try_from(exponent).expect("anchor exponents fit u32")))
    });
    ExactProbability::from_counts(total - invertible, total)
}

fn enumerate<const Q: u64>(dimension: usize) -> ExactProbability {
    try_visit_permanent_anchor_matrices::<Q, std::convert::Infallible, _>(dimension, |_, _| Ok(()))
        .expect("an infallible visitor cannot fail")
}

/// Visits every matrix in one exact-anchor domain exactly once.
///
/// Matrices arrive in canonical row-major order, with the first entry changing
/// fastest in the base-`Q` enumeration. The second callback argument is the
/// independent fixed-expansion permanent used by
/// [`enumerate_permanent_zero_probability`]; it does not call any production
/// permanent implementation. Returning an error stops visitation immediately
/// and forwards that error to the caller.
///
/// This visitor is the shared exhaustive address source for validation. It
/// deliberately does not evaluate production backends or pool their outcomes,
/// keeping the oracle path independent from the implementation under test.
///
/// # Errors
///
/// Returns the first error produced by `visitor`.
///
/// # Panics
///
/// Panics unless `Q` is 3 with `dimension` in `1..=4`, or `Q` is 5 or 7 with
/// `dimension` in `1..=3`.
///
/// # Complexity
///
/// Visits `Q^(dimension^2)` matrices in constant auxiliary storage. The
/// independent oracle uses a fixed expansion of at most 24 terms per matrix.
pub fn try_visit_permanent_anchor_matrices<const Q: u64, E, F>(
    dimension: usize,
    mut visitor: F,
) -> Result<ExactProbability, E>
where
    F: FnMut(&[Fp<Q>], Fp<Q>) -> Result<(), E>,
{
    assert!(
        (Q == 3 && (1..=4).contains(&dimension))
            || (matches!(Q, 5 | 7) && (1..=3).contains(&dimension)),
        "exact permanent anchors support q=3 with n<=4 and q=5,7 with n<=3; got q={Q}, n={dimension}"
    );
    let entry_count = dimension * dimension;
    let matrix_count = Q.pow(entry_count as u32);
    let mut residues = [0_u64; 16];
    let mut entries = [Fp::<Q>::new(0); 16];
    let mut zero_count = 0_u64;

    for encoded in 0..matrix_count {
        let mut remaining = encoded;
        for (residue, entry) in residues[..entry_count]
            .iter_mut()
            .zip(&mut entries[..entry_count])
        {
            *residue = remaining % Q;
            *entry = Fp::new(*residue);
            remaining /= Q;
        }
        let permanent = Fp::new(permanent_mod_prime(&residues[..entry_count], dimension, Q));
        if permanent == Fp::new(0) {
            zero_count += 1;
        }
        visitor(&entries[..entry_count], permanent)?;
    }

    Ok(ExactProbability::from_counts(zero_count, matrix_count))
}

// The anchor domain ends at n=4.  Keeping these fixed expansions here avoids
// creating a second, general permanent or enumeration implementation beside the
// production algorithms.
fn permanent_mod_prime(entries: &[u64], dimension: usize, field_order: u64) -> u64 {
    match dimension {
        1 => entries[0],
        2 => (entries[0] * entries[3] + entries[1] * entries[2]) % field_order,
        3 => sum_permutations(entries, &PERMUTATIONS_3, 3, field_order),
        4 => sum_permutations(entries, &PERMUTATIONS_4, 4, field_order),
        _ => unreachable!("the exact-anchor domain is bounded by n=4"),
    }
}

fn sum_permutations(
    entries: &[u64],
    permutations: &[[usize; 4]],
    dimension: usize,
    field_order: u64,
) -> u64 {
    permutations.iter().fold(0, |sum, permutation| {
        let mut term = 1_u64;
        for (row, &column) in permutation.iter().enumerate() {
            if column == usize::MAX {
                break;
            }
            term = (term * entries[row * dimension + column]) % field_order;
        }
        (sum + term) % field_order
    })
}

const PERMUTATIONS_3: [[usize; 4]; 6] = [
    [0, 1, 2, usize::MAX],
    [0, 2, 1, usize::MAX],
    [1, 0, 2, usize::MAX],
    [1, 2, 0, usize::MAX],
    [2, 0, 1, usize::MAX],
    [2, 1, 0, usize::MAX],
];

const PERMUTATIONS_4: [[usize; 4]; 24] = [
    [0, 1, 2, 3],
    [0, 1, 3, 2],
    [0, 2, 1, 3],
    [0, 2, 3, 1],
    [0, 3, 1, 2],
    [0, 3, 2, 1],
    [1, 0, 2, 3],
    [1, 0, 3, 2],
    [1, 2, 0, 3],
    [1, 2, 3, 0],
    [1, 3, 0, 2],
    [1, 3, 2, 0],
    [2, 0, 1, 3],
    [2, 0, 3, 1],
    [2, 1, 0, 3],
    [2, 1, 3, 0],
    [2, 3, 0, 1],
    [2, 3, 1, 0],
    [3, 0, 1, 2],
    [3, 0, 2, 1],
    [3, 1, 0, 2],
    [3, 1, 2, 0],
    [3, 2, 0, 1],
    [3, 2, 1, 0],
];

fn greatest_common_divisor(mut left: BigUint, mut right: BigUint) -> BigUint {
    while right != BigUint::from(0_u8) {
        let remainder = &left % &right;
        left = right;
        right = remainder;
    }
    left
}
