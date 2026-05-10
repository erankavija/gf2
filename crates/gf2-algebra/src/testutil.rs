//! Test-only helpers shared across the crate's test modules.
//!
//! Centralises the deterministic pseudo-random matrix generators used by the
//! `permanent_*` algorithm family's cross-check tests. All helpers route
//! through the workspace SSOT RNG [`gf2_core::rng::Lcg`] so that seed values
//! reproduce bit-identical streams across modules.

use gf2_core::gfp::Fp;
use gf2_core::rng::Lcg;

/// Generate a deterministic pseudo-random `n × n` matrix of [`Fp<P>`] elements,
/// row-major.
///
/// Internally constructs a fresh [`Lcg`] from `seed` and draws `n * n` words,
/// reducing each modulo `P` to obtain a canonical `Fp<P>` value. The output
/// layout matches the row-major convention used by every `permanent_*` kernel
/// in this crate.
///
/// # Arguments
///
/// * `n`    — matrix dimension; result has length `n * n`.
/// * `seed` — seed for the workspace SSOT [`Lcg`] RNG.
///
/// # Examples
///
/// ```ignore
/// // Test-only helper — usage example for crate-internal tests:
/// // let mat = crate::testutil::random_matrix::<3>(4, 0xdead_beef);
/// // assert_eq!(mat.len(), 16);
/// ```
///
/// # Complexity
///
/// `O(n^2)` — one [`Lcg::next_u64`] call per entry.
pub fn random_matrix<const P: u64>(n: usize, seed: u64) -> Vec<Fp<P>> {
    let mut rng = Lcg::new(seed);
    (0..n * n)
        .map(|_| Fp::<P>::new(rng.next_u64() % P))
        .collect()
}

/// Same as [`random_matrix`] but draws from an existing [`Lcg`] stream rather
/// than reseeding, so callers can produce multiple independent matrices from a
/// single deterministic stream.
///
/// # Arguments
///
/// * `rng` — mutable [`Lcg`] state to draw from; advanced by `n * n` words.
/// * `n`   — matrix dimension; result has length `n * n`.
///
/// # Examples
///
/// ```ignore
/// // Test-only helper — usage example for crate-internal tests:
/// // let mut rng = gf2_core::rng::Lcg::new(0xfeed_face);
/// // let m1 = crate::testutil::random_matrix_with_rng::<3>(&mut rng, 4);
/// // let m2 = crate::testutil::random_matrix_with_rng::<3>(&mut rng, 4);
/// // assert_ne!(m1, m2); // independent draws
/// ```
///
/// # Complexity
///
/// `O(n^2)` — one [`Lcg::next_u64`] call per entry.
pub fn random_matrix_with_rng<const P: u64>(rng: &mut Lcg, n: usize) -> Vec<Fp<P>> {
    (0..n * n)
        .map(|_| Fp::<P>::new(rng.next_u64() % P))
        .collect()
}
