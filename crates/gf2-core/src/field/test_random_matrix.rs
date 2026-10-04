//! Seeded random `FieldMatrix` / `FieldVec` builders shared by tests and
//! benches, compiled under `cfg(any(test, feature = "test-support"))`.

use crate::field::matrix::{gemm, FieldMatrix};
use crate::field::traits::FiniteField;
use crate::field::vec::FieldVec;
use crate::gf2m::{Gf2mWide, Gf2mWideConfig};
use crate::gfp::Fp;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

/// Returns a seeded random `rows × cols` matrix over `Fp<P>`.
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

/// Returns a seeded random length-`n` vector over `Fp<P>`.
pub fn random_fp_vec<const P: u64>(n: usize, seed: u64) -> FieldVec<Fp<P>> {
    let mut rng = StdRng::seed_from_u64(seed);
    (0..n).map(|_| Fp::<P>::new(rng.gen::<u64>() % P)).collect()
}

/// Returns a random full-rank `n × n` matrix over `Fp<P>`, resampling with
/// seeds `seed`, `seed.wrapping_add(1)`, … up to 16 times.
///
/// # Panics
///
/// Panics if all 16 samples are singular.
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

/// Returns the `m × n` product `F · G` of a random `m × rank` and a random
/// `rank × n` matrix over `Fp<P>`, of rank at most `rank`. `F` uses `seed`
/// and `G` uses `seed.wrapping_add(0x1234_5678)`.
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

/// Returns a seeded random `rows × cols` matrix over `Gf2mWide<1, C>`, each
/// entry drawn from the low `C::M` bits of a `u64`.
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

/// Returns a seeded random length-`n` vector over `Gf2mWide<1, C>`.
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
///
/// # Panics
///
/// Panics if all 16 samples are singular.
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

/// Returns a seeded `rows × cols` matrix over `Fp<P>` in which each entry is
/// independently non-zero with probability `density`, with non-zero values
/// drawn from `[1, P-1]`.
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

/// Column-by-column Gauss-Jordan RREF over `Fp<P>`, the reference for
/// `FieldMatrix::rref` tests: pivot columns are the leftmost
/// linearly-independent subset of the input's columns.
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
