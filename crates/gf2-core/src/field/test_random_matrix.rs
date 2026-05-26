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

use crate::field::matrix::{gemm, FieldMatrix};
use crate::field::traits::FiniteField;
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

/// Returns a rank-deficient `m × n` matrix over `Fp<P>` with rank exactly
/// `rank` (must satisfy `rank < m.min(n)`). Constructed as an outer product
/// `F · G` where `F` is `m × rank` and `G` is `rank × n`, both random.
///
/// # Arguments
///
/// - `m`, `n` — matrix dimensions.
/// - `rank` — desired rank; must be `< m.min(n)` for the matrix to be
///   rank-deficient. The caller is responsible for the precondition.
/// - `seed` — deterministic seed. `F` uses `seed`; `G` uses
///   `seed.wrapping_add(0x1234_5678)` to keep the two draws independent.
///
/// # Examples
///
/// ```
/// use gf2_core::field::test_random_matrix::random_fp_rank_deficient;
/// let a = random_fp_rank_deficient::<7>(8, 8, 4, 42);
/// assert_eq!(a.rows(), 8);
/// assert_eq!(a.cols(), 8);
/// ```
pub fn random_fp_rank_deficient<const P: u64>(
    m: usize,
    n: usize,
    rank: usize,
    seed: u64,
) -> FieldMatrix<Fp<P>> {
    let f = random_fp::<P>(m, rank, seed);
    let g = random_fp::<P>(rank, n, seed.wrapping_add(0x1234_5678));
    gemm(&f, &g)
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

// ─── Sparse (density-threshold) Fp builder ───────────────────────────────────

/// Returns a sparse `m × n` matrix over `Fp<P>` where each entry is
/// independently non-zero with probability `density`. Non-zero values
/// are sampled uniformly from `[1, P-1]`. Deterministic in `seed`.
///
/// Used by RREF/PLE tests in `ple.rs` and `sparse_matrix.rs`; this is
/// the single source of truth for the generator (jit:bd9c6e13 SSOT fix).
///
/// # Arguments
///
/// - `rows`, `cols` — matrix dimensions.
/// - `density` — Bernoulli probability that each entry is non-zero
///   (0.0 = all-zero, 1.0 = all non-zero).
/// - `seed` — deterministic seed for `StdRng`.
///
/// # Examples
///
/// ```
/// use gf2_core::field::test_random_matrix::dense_random_fp_sparse;
/// let m = dense_random_fp_sparse::<7>(4, 5, 0.3, 42);
/// assert_eq!(m.rows(), 4);
/// assert_eq!(m.cols(), 5);
/// ```
pub fn dense_random_fp_sparse<const P: u64>(
    rows: usize,
    cols: usize,
    density: f64,
    seed: u64,
) -> FieldMatrix<Fp<P>> {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut m = FieldMatrix::<Fp<P>>::zeros(rows, cols);
    for r in 0..rows {
        for c in 0..cols {
            if rng.gen::<f64>() < density {
                let v = (rng.gen::<u64>() % (P - 1)) + 1;
                m.set(r, c, Fp::<P>::new(v));
            }
        }
    }
    m
}

// ─── Canonical RREF oracle ────────────────────────────────────────────────────

/// Textbook column-by-column Gauss-Jordan RREF over `Fp<P>`.
///
/// Produces the canonical RREF by construction: pivot columns are the
/// leftmost linearly-independent subset of the input's columns.
/// Used as a byte-equality reference for `FieldMatrix::rref` tests in
/// `ple.rs` and `sparse_matrix.rs`; this is the single source of truth
/// for the oracle (jit:bd9c6e13 SSOT fix, was duplicated as
/// `direct_rref_oracle_fp` in `ple.rs` and `direct_rref_reference_fp`
/// in `sparse_matrix.rs`).
///
/// # Arguments
///
/// - `a` — input matrix (not mutated).
///
/// # Examples
///
/// ```
/// use gf2_core::field::test_random_matrix::{dense_random_fp_sparse, direct_rref_oracle_fp};
/// let a = dense_random_fp_sparse::<7>(4, 5, 0.4, 1);
/// let rref = direct_rref_oracle_fp(&a);
/// assert_eq!(rref.rows(), 4);
/// assert_eq!(rref.cols(), 5);
/// ```
pub fn direct_rref_oracle_fp<const P: u64>(a: &FieldMatrix<Fp<P>>) -> FieldMatrix<Fp<P>> {
    let (m, n) = a.shape();
    let mut e = a.clone();
    let zero = Fp::<P>::new(0);
    let one = Fp::<P>::new(1);
    let mut next_pivot_row = 0usize;
    for col in 0..n {
        if next_pivot_row >= m {
            break;
        }
        let mut pivot_row: Option<usize> = None;
        for i in next_pivot_row..m {
            if e.get(i, col) != zero {
                pivot_row = Some(i);
                break;
            }
        }
        let Some(p) = pivot_row else {
            continue;
        };
        if p != next_pivot_row {
            for c in 0..n {
                let tmp = e.get(next_pivot_row, c);
                e.set(next_pivot_row, c, e.get(p, c));
                e.set(p, c, tmp);
            }
        }
        let piv = e.get(next_pivot_row, col);
        if piv != one {
            let inv = piv.inv().unwrap();
            for c in 0..n {
                let v = e.get(next_pivot_row, c) * inv;
                e.set(next_pivot_row, c, v);
            }
        }
        for k in 0..m {
            if k == next_pivot_row {
                continue;
            }
            let factor = e.get(k, col);
            if factor == zero {
                continue;
            }
            for c in 0..n {
                let v = e.get(k, c) - factor * e.get(next_pivot_row, c);
                e.set(k, c, v);
            }
        }
        next_pivot_row += 1;
    }
    e
}
