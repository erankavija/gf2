//! Seed derivation and matrix fills shared by the gf2-core benchmarks and the
//! reference harnesses under `benchmarks/reference/`, so both sides draw the
//! inputs of a `(tag, op_idx, size_idx, regime_idx)` cell from one seed.
//! `benchmarks/reference/seed_helpers.h` is the C definition of
//! [`splitmix64`] and [`derive_seed`].

#![allow(dead_code)]

use crate::field::matrix::FieldMatrix;
use crate::field::sparse_matrix::SparseFieldMatrix;
use crate::field::vec::FieldVec;
use crate::gf2m::{Gf2mWide, Gf2mWideConfig};
use crate::gfp::Fp;
use crate::matrix::BitMatrix;
use crate::sparse::SpBitMatrix;
use crate::BitVec;

/// Master seed pinned in `benchmarks/seeds/seed.txt`.
pub const MASTER_SEED: u64 = 0x6F73AC91D31E4A7C;

/// Per-cell time cap, equal to the reference harnesses' `kCellBudgetNs`.
pub const CELL_BUDGET_NS: u64 = 30 * 1_000_000_000;

/// Advances `state` by one SplitMix64 step (`@/citation/Vigna2015`) and
/// returns the output word.
#[inline]
pub fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Derives the row seed of a `(tag, op_idx, size_idx, regime_idx)` cell from
/// `master`, as `gf2_bench_derive_seed` does in the C reference.
#[inline]
pub fn derive_seed(master: u64, tag: &str, op_idx: u64, size_idx: u64, regime_idx: u64) -> u64 {
    let mut s = master;
    for b in tag.as_bytes() {
        s ^= u64::from(*b);
        let _ = splitmix64(&mut s);
    }
    s ^= op_idx;
    let _ = splitmix64(&mut s);
    s ^= size_idx;
    let _ = splitmix64(&mut s);
    s ^= regime_idx;
    let _ = splitmix64(&mut s);
    splitmix64(&mut s)
}

/// `rows × cols` matrix whose `(i, j)` element is the `(i*cols + j + 1)`-th
/// SplitMix64 output of `seed` reduced modulo `P`. Compiles only for
/// `2 <= P <= 2^63` (const assertion of [`Fp`]).
pub fn fp_matrix_from_seed<const P: u64>(
    rows: usize,
    cols: usize,
    seed: u64,
) -> FieldMatrix<Fp<P>> {
    let mut st = seed;
    let mut m = FieldMatrix::<Fp<P>>::zeros(rows, cols);
    for r in 0..rows {
        for c in 0..cols {
            let raw = splitmix64(&mut st);
            m.set(r, c, Fp::<P>::new(raw % P));
        }
    }
    m
}

/// Length-`n` vector of successive SplitMix64 outputs of `seed` reduced
/// modulo `P`. Compiles only for `2 <= P <= 2^63` (const assertion of
/// [`Fp`]).
pub fn fp_vec_from_seed<const P: u64>(n: usize, seed: u64) -> FieldVec<Fp<P>> {
    let mut st = seed;
    (0..n)
        .map(|_| Fp::<P>::new(splitmix64(&mut st) % P))
        .collect()
}

/// `rows × cols` matrix of successive SplitMix64 outputs of `seed` in
/// row-major order, each masked to its `C::M` low bits.
pub fn gf2m_wide_1_matrix_from_seed<C: Gf2mWideConfig<1>>(
    rows: usize,
    cols: usize,
    seed: u64,
) -> FieldMatrix<Gf2mWide<1, C>> {
    let mask: u64 = if C::M >= 64 {
        u64::MAX
    } else {
        (1u64 << C::M) - 1
    };
    let mut st = seed;
    let mut m = FieldMatrix::<Gf2mWide<1, C>>::zeros(rows, cols);
    for r in 0..rows {
        for c in 0..cols {
            let raw = splitmix64(&mut st);
            m.set(r, c, Gf2mWide::<1, C>::new([raw & mask]));
        }
    }
    m
}

/// Length-`n` vector of successive SplitMix64 outputs of `seed`, each masked
/// to its `C::M` low bits.
pub fn gf2m_wide_1_vec_from_seed<C: Gf2mWideConfig<1>>(
    n: usize,
    seed: u64,
) -> FieldVec<Gf2mWide<1, C>> {
    let mask: u64 = if C::M >= 64 {
        u64::MAX
    } else {
        (1u64 << C::M) - 1
    };
    let mut st = seed;
    (0..n)
        .map(|_| Gf2mWide::<1, C>::new([splitmix64(&mut st) & mask]))
        .collect()
}

const RANK_DEF_L_SALT: u64 = 0xA5A5_A5A5_A5A5_A5A5;
const RANK_DEF_R_SALT: u64 = 0x5A5A_5A5A_5A5A_5A5A;

/// `m × n` matrix of rank at most `r`: the product `L · R` of the `m × r` and
/// `r × n` fills of [`fp_matrix_from_seed`] under two salts of `seed`.
/// Compiles only for `2 <= P <= 2^63` (const assertion of [`Fp`]).
pub fn fp_rank_deficient_from_seed<const P: u64>(
    m: usize,
    n: usize,
    r: usize,
    seed: u64,
) -> FieldMatrix<Fp<P>> {
    let l = fp_matrix_from_seed::<P>(m, r, seed ^ RANK_DEF_L_SALT);
    let rmat = fp_matrix_from_seed::<P>(r, n, seed ^ RANK_DEF_R_SALT);
    crate::field::matrix::gemm(&l, &rmat)
}

/// `m × n` matrix of rank at most `r`, built as
/// [`fp_rank_deficient_from_seed`] builds its product.
pub fn gf2m_wide_1_rank_deficient_from_seed<C: Gf2mWideConfig<1>>(
    m: usize,
    n: usize,
    r: usize,
    seed: u64,
) -> FieldMatrix<Gf2mWide<1, C>> {
    let l = gf2m_wide_1_matrix_from_seed::<C>(m, r, seed ^ RANK_DEF_L_SALT);
    let rmat = gf2m_wide_1_matrix_from_seed::<C>(r, n, seed ^ RANK_DEF_R_SALT);
    crate::field::matrix::gemm(&l, &rmat)
}

/// Uniform `m × n` matrix over GF(2) determined by `seed`.
pub fn bitmatrix_from_seed(m: usize, n: usize, seed: u64) -> BitMatrix {
    BitMatrix::random_seeded(m, n, seed)
}

/// Uniform `n`-bit vector determined by `seed`.
pub fn bitvec_from_seed(n: usize, seed: u64) -> BitVec {
    BitVec::random_seeded(n, seed)
}

/// `m × n` matrix over GF(2) of rank at most `r`, built as
/// [`fp_rank_deficient_from_seed`] builds its product.
pub fn bitmatrix_rank_deficient_from_seed(m: usize, n: usize, r: usize, seed: u64) -> BitMatrix {
    let l = bitmatrix_from_seed(m, r, seed ^ RANK_DEF_L_SALT);
    let rmat = bitmatrix_from_seed(r, n, seed ^ RANK_DEF_R_SALT);
    crate::alg::m4rm::multiply(&l, &rmat)
}

/// `m × n` sparse matrix over GF(2): one SplitMix64 draw of `seed` per cell
/// in row-major order, the cell set when the draw is below
/// `(density · 2^64) as u64`. The cast saturates: `density <= 0` or NaN sets
/// no cell, and `density >= 1` sets every cell whose draw is not `u64::MAX`.
pub fn bitmatrix_sparse_from_seed(m: usize, n: usize, density: f64, seed: u64) -> SpBitMatrix {
    let mut st = seed;
    let mut dense = BitMatrix::zeros(m, n);
    let threshold = (density * (u64::MAX as f64 + 1.0)) as u64;
    for r in 0..m {
        for c in 0..n {
            if splitmix64(&mut st) < threshold {
                dense.set(r, c, true);
            }
        }
    }
    SpBitMatrix::from_dense(&dense)
}

/// One data row of the CSV schema in `benchmarks/README.md`, without its
/// `lib` column.
pub struct CsvRow<'a> {
    /// Operation tag (`"fgemm"`, `"pluq"`, `"echelon"`, `"invert"`, `"solve"`, `"charpoly"`, `"spmv"`, …).
    pub operation: &'a str,
    /// Field label (e.g. `"Fp_M31"`, `"Gf2m8"`).
    pub field: &'a str,
    /// Output / input row count.
    pub m: usize,
    /// Inner / shared dimension.
    pub k: usize,
    /// Output column count.
    pub n: usize,
    /// `"uniform"` or `"deficient"`.
    pub rank_regime: &'a str,
    /// Per-cell row seed (output of [`derive_seed`]).
    pub seed: u64,
    /// Mean wall-clock nanoseconds per iteration.
    pub wall_ns: u64,
    /// Op count per second, as [`tput`] computes it.
    pub throughput_ops: f64,
}

/// `rows × cols` sparse matrix: one SplitMix64 draw of `seed_val` per cell in
/// row-major order under the threshold of [`bitmatrix_sparse_from_seed`]; a
/// cell below it takes the next draw reduced into `1..P`. Compiles only for
/// `2 <= P <= 2^63` (const assertion of [`Fp`]).
pub fn fp_sparse_from_seed<const P: u64>(
    rows: usize,
    cols: usize,
    density: f64,
    seed_val: u64,
) -> SparseFieldMatrix<Fp<P>> {
    let mut st = seed_val;
    let mut m = FieldMatrix::<Fp<P>>::zeros(rows, cols);
    let threshold = (density * (u64::MAX as f64 + 1.0)) as u64;
    for r in 0..rows {
        for c in 0..cols {
            let draw = splitmix64(&mut st);
            if draw < threshold {
                let v_raw = splitmix64(&mut st);
                // Avoid zero so the included cell shows up in CSR.
                let v = (v_raw % (P - 1)) + 1;
                m.set(r, c, Fp::<P>::new(v));
            }
        }
    }
    SparseFieldMatrix::from_dense(&m)
}

/// [`fp_sparse_from_seed`] over `Gf2mWide<1, C>`: a cell below the threshold
/// takes the next draw masked to its `C::M` low bits, with 0 replaced by 1.
pub fn gf2m_wide_1_sparse_from_seed<C: Gf2mWideConfig<1>>(
    rows: usize,
    cols: usize,
    density: f64,
    seed_val: u64,
) -> SparseFieldMatrix<Gf2mWide<1, C>> {
    let mask: u64 = if C::M >= 64 {
        u64::MAX
    } else {
        (1u64 << C::M) - 1
    };
    let mut st = seed_val;
    let mut m = FieldMatrix::<Gf2mWide<1, C>>::zeros(rows, cols);
    let threshold = (density * (u64::MAX as f64 + 1.0)) as u64;
    for r in 0..rows {
        for c in 0..cols {
            let draw = splitmix64(&mut st);
            if draw < threshold {
                let v_raw = splitmix64(&mut st) & mask;
                let v = if v_raw == 0 { 1 } else { v_raw };
                m.set(r, c, Gf2mWide::<1, C>::new([v]));
            }
        }
    }
    SparseFieldMatrix::from_dense(&m)
}

/// Op count `n³` of a square `n × n` factorisation.
#[inline]
pub fn ops_cubic(n: usize) -> f64 {
    let nf = n as f64;
    nf * nf * nf
}

/// Op count `2 · m · k · n` of an `m × k × n` gemm.
#[inline]
pub fn ops_gemm(m: usize, k: usize, n: usize) -> f64 {
    2.0 * (m as f64) * (k as f64) * (n as f64)
}

/// Op count `n⁴` of a square `n × n` operation of that order.
#[inline]
pub fn ops_quartic(n: usize) -> f64 {
    let nf = n as f64;
    nf * nf * nf * nf
}

/// `ops` per second; infinite when `wall_ns == 0`.
#[inline]
pub fn tput(ops: f64, wall_ns: u64) -> f64 {
    if wall_ns == 0 {
        return f64::INFINITY;
    }
    ops / ((wall_ns as f64) * 1.0e-9)
}

/// Formats `row` under [`CSV_HEADER`] with `lib = "gf2"`.
pub fn format_csv_row(row: &CsvRow<'_>) -> String {
    format!(
        "gf2,{op},{field},{m},{k},{n},{regime},{seed},{wall_ns},{tput:.6e}\n",
        op = row.operation,
        field = row.field,
        m = row.m,
        k = row.k,
        n = row.n,
        regime = row.rank_regime,
        seed = row.seed,
        wall_ns = row.wall_ns,
        tput = row.throughput_ops,
    )
}

/// Header row of the CSV schema in `benchmarks/README.md`.
pub const CSV_HEADER: &str = "lib,operation,field,m,k,n,rank_regime,seed,wall_ns,throughput_ops\n";
