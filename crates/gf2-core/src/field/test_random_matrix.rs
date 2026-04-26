//! Shared random `FieldMatrix` / `FieldVec` builders for tests and
//! benches.
//!
//! Issue `ae1d1e88` (R1 review). The PLE story (`c3f8c1cb`), triangular
//! story (`83b1ad8b`), and inverse story (`ae1d1e88`) each grew their
//! own copy of the same deterministic random builders inside private
//! `#[cfg(test)] mod tests` blocks (and in their corresponding
//! `benches/*.rs` files). The reviewer flagged the duplication as an
//! SSOT violation; per the project's standing rule, SSOT fixes land in
//! the same task that surfaces them.
//!
//! This module is the single source of truth for those builders. It is
//! gated behind `#[cfg(any(test, feature = "test-support"))]` so it
//! adds zero compile-time cost to non-test, non-benchmark consumers.
//! Benches reach it through the `dev-dependency` self-import that
//! enables `test-support`.
//!
//! ## What's exported
//!
//! - [`random_fp`] — uniform random `m × n` over `Fp<P>`.
//! - [`random_fp_invertible`] — random square `Fp<P>` resampled until
//!   `rank == n`.
//! - [`random_gf2m_wide_1`] — uniform random `m × n` over `Gf2mWide<1, C>`
//!   for any `Gf2mWideConfig<1>` (covers `M ∈ {8, 16}` used by tests
//!   and benches via masking on the low `M` bits).
//! - [`random_gf2m_wide_1_invertible`] — random square `Gf2mWide<1, C>`
//!   resampled until full rank.
//! - [`random_fp_vec`] / [`random_gf2m_wide_1_vec`] — vector counterparts.
//!
//! All builders take a deterministic `u64` seed; identical seeds
//! produce identical matrices on identical platforms (StdRng is
//! platform-stable for our `cargo test` matrix).

use crate::field::matrix::FieldMatrix;
use crate::field::vec::FieldVec;
use crate::gf2m::{Gf2mWide, Gf2mWideConfig};
use crate::gfp::Fp;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

// ─── Fp builders ─────────────────────────────────────────────────────────────

/// Returns an `m × n` matrix of uniform random elements over `Fp<P>`,
/// reduced modulo `P`. Deterministic in `seed`.
pub fn random_fp<const P: u64>(rows: usize, cols: usize, seed: u64) -> FieldMatrix<Fp<P>> {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut m = FieldMatrix::<Fp<P>>::zeros(rows, cols);
    for r in 0..rows {
        for c in 0..cols {
            m.set(r, c, Fp::<P>::new(rng.gen::<u64>() % P));
        }
    }
    m
}

/// Returns a uniform random length-`n` vector over `Fp<P>`.
pub fn random_fp_vec<const P: u64>(n: usize, seed: u64) -> FieldVec<Fp<P>> {
    let mut rng = StdRng::seed_from_u64(seed);
    (0..n).map(|_| Fp::<P>::new(rng.gen::<u64>() % P)).collect()
}

/// Returns a random `n × n` matrix over `Fp<P>` that is full-rank
/// (`rank == n`). Resamples up to `attempts` times before panicking;
/// for any reasonable `P` and `n ≥ 1` the singularity probability is
/// `~1/P`, so `attempts = 16` is dramatic overkill.
///
/// The seed schedule starts at `seed`, then `seed.wrapping_add(1)`,
/// `seed.wrapping_add(2)`, … so callers using disjoint base seeds get
/// disjoint resample sequences.
pub fn random_fp_invertible<const P: u64>(n: usize, seed: u64) -> FieldMatrix<Fp<P>> {
    for k in 0..16u64 {
        let m = random_fp::<P>(n, n, seed.wrapping_add(k));
        if m.rank() == n {
            return m;
        }
    }
    panic!(
        "random_fp_invertible: failed to find an invertible n={} matrix \
         over Fp<{}> after 16 attempts (seed={})",
        n, P, seed
    );
}

// ─── Gf2mWide<1, C> builders ─────────────────────────────────────────────────

/// Returns an `m × n` matrix of uniform random elements over
/// `Gf2mWide<1, C>`. The low `C::M` bits are kept; this matches every
/// in-tree config (`M ∈ {8, 16}`) since the upper bits are always
/// masked out by `Gf2mWide::new`.
///
/// Generic so all per-module configs (PLE/triangular/inverse tests
/// each define their own marker struct to avoid trait-coherence
/// conflicts) can share a single builder.
pub fn random_gf2m_wide_1<C: Gf2mWideConfig<1>>(
    rows: usize,
    cols: usize,
    seed: u64,
) -> FieldMatrix<Gf2mWide<1, C>> {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut m = FieldMatrix::<Gf2mWide<1, C>>::zeros(rows, cols);
    let mask: u64 = if C::M >= 64 {
        u64::MAX
    } else {
        (1u64 << C::M) - 1
    };
    for r in 0..rows {
        for c in 0..cols {
            m.set(r, c, Gf2mWide::<1, C>::new([rng.gen::<u64>() & mask]));
        }
    }
    m
}

/// Returns a uniform random length-`n` vector over `Gf2mWide<1, C>`.
pub fn random_gf2m_wide_1_vec<C: Gf2mWideConfig<1>>(
    n: usize,
    seed: u64,
) -> FieldVec<Gf2mWide<1, C>> {
    let mut rng = StdRng::seed_from_u64(seed);
    let mask: u64 = if C::M >= 64 {
        u64::MAX
    } else {
        (1u64 << C::M) - 1
    };
    (0..n)
        .map(|_| Gf2mWide::<1, C>::new([rng.gen::<u64>() & mask]))
        .collect()
}

/// Returns a random full-rank `n × n` matrix over `Gf2mWide<1, C>`,
/// resampling up to 16 times.
pub fn random_gf2m_wide_1_invertible<C: Gf2mWideConfig<1>>(
    n: usize,
    seed: u64,
) -> FieldMatrix<Gf2mWide<1, C>> {
    for k in 0..16u64 {
        let m = random_gf2m_wide_1::<C>(n, n, seed.wrapping_add(k));
        if m.rank() == n {
            return m;
        }
    }
    panic!(
        "random_gf2m_wide_1_invertible: failed to find invertible n={} \
         matrix over {} after 16 attempts (seed={})",
        n,
        C::NAME,
        seed
    );
}
