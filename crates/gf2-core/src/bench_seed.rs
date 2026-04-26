//! Shared seed-derivation + matrix-fill helpers used by every gf2-core
//! `FieldMatrix` benchmark in this directory.
//!
//! Issue `6ed7f050` (story `64c88ae4`, epic `bb85c68a`). The reference
//! container harnesses (`benchmarks/reference/fflas_bench.cpp`,
//! `benchmarks/reference/m4ri_bench.c`) and the gf2-side criterion
//! benches in this directory must produce **byte-identical** input
//! matrices for the same `(tag, op_idx, size_idx, regime_idx)` cell so
//! that timing differences across libraries reflect *implementation*
//! and not *random sample*. To guarantee that, every bench file pulls
//! in this module via
//!
//! ```ignore
//! #[path = "common/seed.rs"]
//! mod seed;
//! ```
//!
//! and uses [`splitmix64`], [`derive_seed`], and [`MASTER_SEED`] verbatim.
//!
//! The algorithm is SplitMix64 with the constants from Sebastiano
//! Vigna's xoroshiro reference:
//!
//! ```text
//! splitmix64(state):
//!     state += 0x9E3779B97F4A7C15
//!     z = state
//!     z = (z ^ (z >> 30)) * 0xBF58476D1CE4E5B9
//!     z = (z ^ (z >> 27)) * 0x94D049BB133111EB
//!     return z ^ (z >> 31)
//!
//! derive_seed(master, tag, op_idx, size_idx, regime_idx):
//!     s = master
//!     for each byte b in tag: s ^= b; splitmix64(&s)
//!     s ^= op_idx;     splitmix64(&s)
//!     s ^= size_idx;   splitmix64(&s)
//!     s ^= regime_idx; splitmix64(&s)
//!     return splitmix64(&s)
//! ```
//!
//! See `benchmarks/reference/seed_helpers.h` for the canonical C/C++
//! definition; this module mirrors it bit-for-bit on `wrapping_*` Rust
//! arithmetic.

#![allow(dead_code)] // Different bench files use different subsets of these helpers.

use crate::field::matrix::FieldMatrix;
use crate::field::sparse_matrix::SparseFieldMatrix;
use crate::field::vec::FieldVec;
use crate::gf2m::{Gf2mWide, Gf2mWideConfig};
use crate::gfp::Fp;

/// Master seed pinned in `benchmarks/seeds/seed.txt`. Both the gf2-side
/// criterion harness and the reference container harness consume this
/// value so the two derivation streams line up byte-for-byte.
pub const MASTER_SEED: u64 = 0x6F73AC91D31E4A7C;

/// Per-row time cap mirroring the reference harness's
/// `kCellBudgetNs = 30 s`. Bench drivers are expected to keep
/// `criterion.measurement_time × sample_size` below this so a single
/// cell never exceeds the cap.
pub const CELL_BUDGET_NS: u64 = 30 * 1_000_000_000;

/// One step of SplitMix64, advancing `state` in place and returning the
/// 64 bits of stream output.
///
/// All wrapping multiplications mirror the unsigned-overflow semantics
/// the C reference relies on; the result is bit-identical to the
/// reference harness's `gf2_bench_splitmix64`.
///
/// # Examples
///
/// ```
/// use gf2_core::bench_seed::splitmix64;
///
/// // Pinned reference values from `benchmarks/reference/seed_helpers.h`'s
/// // identical algorithm, used as a hash check that the seed pipeline
/// // never drifts from the reference harness.
/// let mut st = 0u64;
/// assert_eq!(splitmix64(&mut st), 0xe220a8397b1dcdaf);
/// assert_eq!(splitmix64(&mut st), 0x6e789e6aa1b965f4);
/// assert_eq!(splitmix64(&mut st), 0x06c45d188009454f);
/// ```
#[inline]
pub fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Derive a 64-bit row seed for a `(tag, op_idx, size_idx, regime_idx)`
/// cell from the run's master seed. Identical to the C reference's
/// `gf2_bench_derive_seed`.
///
/// `tag` is mixed byte-by-byte so two benchmarks with different tags
/// (e.g. `"fgemm"` vs `"pluq"`) draw disjoint seed streams even at
/// identical `(op_idx, size_idx, regime_idx)` triples.
///
/// # Examples
///
/// ```
/// use gf2_core::bench_seed::{derive_seed, splitmix64, MASTER_SEED};
///
/// // Pinned hash check: master = 0, tag = "fgemm", first cell.
/// // The reference harness produces the same value bit-for-bit.
/// assert_eq!(derive_seed(0, "fgemm", 0, 0, 0), 0xa1f5_dbf0_5125_7436);
///
/// // Same cell with the project's pinned master seed.
/// assert_eq!(
///     derive_seed(MASTER_SEED, "fgemm", 0, 0, 0),
///     0x47e4_989d_742b_754f
/// );
///
/// // First four SplitMix64 outputs from that row seed — these are the
/// // first four matrix cells of the (Fp / Gf2m) matrix at this cell.
/// let mut st = derive_seed(MASTER_SEED, "fgemm", 0, 0, 0);
/// assert_eq!(splitmix64(&mut st), 0x350b_8ce7_e52d_880c);
/// assert_eq!(splitmix64(&mut st), 0x00b2_abfd_4b04_5d88);
/// assert_eq!(splitmix64(&mut st), 0x9573_9178_dbda_8b98);
/// assert_eq!(splitmix64(&mut st), 0x7313_2303_c672_288f);
/// ```
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

// ─── Field-uniform fillers ──────────────────────────────────────────────────
//
// Each filler runs `len` SplitMix64 steps off the row seed and reduces
// each draw modulo the field's cardinality, matching the reference
// harness's `fill_uniform<Field>` template (one splitmix call per
// element, `init(x, r % cardinality)`). The reduction is biased only
// for non-power-of-two fields, but the bias is identical on both sides.

/// Returns an `m × n` `Fp<P>` matrix drawn via SplitMix64 from `seed`.
/// The `(i, j)` element is the `(i*n + j + 1)`-th SplitMix64 output
/// reduced modulo `P` — matching `fill_uniform<Modular<...>>` in the
/// reference harness.
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

/// Returns a length-`n` `Fp<P>` vector drawn via SplitMix64 from `seed`.
pub fn fp_vec_from_seed<const P: u64>(n: usize, seed: u64) -> FieldVec<Fp<P>> {
    let mut st = seed;
    (0..n)
        .map(|_| Fp::<P>::new(splitmix64(&mut st) % P))
        .collect()
}

/// Returns an `m × n` `Gf2mWide<1, C>` matrix drawn via SplitMix64.
/// Each element is masked to `C::M` low bits — `Gf2mWide::new` only
/// reads those bits anyway, but masking keeps the bytewise stream
/// reproducible across configs that share `M`.
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

/// Returns a length-`n` `Gf2mWide<1, C>` vector drawn via SplitMix64.
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

// ─── Rank-deficient generators ─────────────────────────────────────────────
//
// The reference harness's `fill_rank_deficient<Field>` builds an `m×n`
// rank-`r` matrix as `L · R` where `L: m×r` is filled from
// `seed ^ 0xA5A5_A5A5_A5A5_A5A5` and `R: r×n` is filled from
// `seed ^ 0x5A5A_5A5A_5A5A_5A5A`. We reproduce that exactly so the
// resulting `A = L·R` matches byte-for-byte when both sides go through
// their respective reference matmul.
//
// The Rust gemm path used here is `gf2_core::field::matrix::gemm`,
// which routes through `gemm_into_view` and its blocked classical
// kernel — the same code path the dense gemm bench measures. The
// numerical *value* of `A` may differ from fflas-ffpack's because gemm
// accumulation order is implementation-defined over a finite ring; the
// **rank** is what the rank-deficient regime cares about, and that is
// invariant under reordering.

const RANK_DEF_L_SALT: u64 = 0xA5A5_A5A5_A5A5_A5A5;
const RANK_DEF_R_SALT: u64 = 0x5A5A_5A5A_5A5A_5A5A;

/// Returns an `m × n` `Fp<P>` matrix of rank exactly `r` (when `r ≤ min(m,n)`),
/// constructed as `L · R` per the reference harness's deficient regime.
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

/// Returns an `m × n` `Gf2mWide<1, C>` matrix of rank exactly `r`.
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

// ─── CSV emission ───────────────────────────────────────────────────────────

/// CSV row matching `benchmarks/README.md`'s ten-column schema.
///
/// `lib` is always `"gf2"` for rows produced by this harness.
/// `wall_ns` is mean wall-clock nanoseconds per iteration; `throughput_ops`
/// is the conventional op-count (`2·m·k·n` for gemm, `n³` for square
/// factorisations and charpoly, `nnz` for SpMV) divided by `wall_ns·1e-9`.
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
    /// `"uniform"` or `"deficient"` (rank = n/2) for factorization regimes; `"uniform"` for non-rank ops.
    pub rank_regime: &'a str,
    /// Per-cell row seed (output of [`derive_seed`]).
    pub seed: u64,
    /// Mean wall-clock nanoseconds per iteration.
    pub wall_ns: u64,
    /// Conventional op count divided by `wall_ns · 1e-9` (operations per second).
    pub throughput_ops: f64,
}

// ─── Sparse generators ────────────────────────────────────────────────────
//
// Bernoulli-support sampling: each cell is included with probability
// `density`, and included cells are filled with a non-zero value drawn
// from the same SplitMix64 state. The "non-zero" guarantee matters for
// the CSR conversion downstream (a zero-valued cell would silently
// become a structural zero, deflating the measured nnz).
//
// Centralised here so `crates/gf2-core/benches/sparse_spmv.rs` and
// `crates/gf2-core/examples/bench_csv_emitter.rs` cannot drift from
// each other — comparability of benchmark results across the criterion
// harness and the CSV emitter depends on identical input fixtures.

/// Builds an `rows × cols` sparse `Fp<P>` matrix with cells included
/// independently at probability `density`, each non-zero. Backed by
/// SplitMix64 from `seed_val`.
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

/// Builds an `rows × cols` sparse `Gf2mWide<1, C>` matrix with cells
/// included independently at probability `density`, each non-zero.
/// Backed by SplitMix64 from `seed_val`.
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

/// Conventional op-count helper for square `n × n` factorisations.
#[inline]
pub fn ops_cubic(n: usize) -> f64 {
    let nf = n as f64;
    nf * nf * nf
}

/// Conventional op-count helper for `m × k × n` gemm (`2 · m · k · n`).
#[inline]
pub fn ops_gemm(m: usize, k: usize, n: usize) -> f64 {
    2.0 * (m as f64) * (k as f64) * (n as f64)
}

/// Throughput in dimensionless "ops per second" for the CSV column.
#[inline]
pub fn tput(ops: f64, wall_ns: u64) -> f64 {
    if wall_ns == 0 {
        return f64::INFINITY;
    }
    ops / ((wall_ns as f64) * 1.0e-9)
}

/// Format a single CSV row in the canonical T1-compatible schema.
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

/// Header row matching `benchmarks/README.md`. Emitted once per CSV
/// file by the explicit-CSV binary; not used by the criterion drivers.
pub const CSV_HEADER: &str = "lib,operation,field,m,k,n,rank_regime,seed,wall_ns,throughput_ops\n";
