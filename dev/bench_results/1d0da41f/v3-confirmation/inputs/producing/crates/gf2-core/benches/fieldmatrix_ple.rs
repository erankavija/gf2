//! `FieldMatrix` PLE / row_echelon / RREF / rank / nullspace —
//! Criterion benchmarks at every (operation, field, size, regime) cell
//! of the `64c88ae4` story matrix.
//!
//! Issue `6ed7f050`. Sibling of the reference container harness's
//! `bench_pluq` and `bench_echelon` calls in
//! `benchmarks/reference/fflas_bench.cpp`. Matrices are byte-identical
//! to the reference fixtures via the shared SplitMix64 seed derivation.
//!
//! ## Coverage
//!
//! - **Sizes**: `n ∈ {64, 256, 1024, 4096}`. Per the R1 amendment of
//!   `a03b2556`, the n=4096 reference cell is deferred for the heavier
//!   non-fgemm ops; gf2 keeps its 4096 cells but criterion's
//!   `sample_size = 10` and the 30 s `seed::CELL_BUDGET_NS` cap stop
//!   the harness from running away on slower hosts.
//! - **Regimes**: `uniform` (i.i.d. via SplitMix64) and `deficient`
//!   (rank exactly `n / 2`, generated as `L · R`).
//! - **Operations**: [`FieldMatrix::ple`], [`FieldMatrix::row_echelon`],
//!   [`FieldMatrix::rref`], [`FieldMatrix::rank`],
//!   [`FieldMatrix::nullspace`].
//! - **Fields**: `Fp<7>`, `Fp<251>`, `Fp<65521>`, `Fp<2^31-1>`,
//!   `Gf2mWide<1, M=8 AES>`, `Gf2mWide<1, M=16 Conway>`.
//!
//! ## Usage
//!
//! ```bash
//! cargo bench -p gf2-core --bench fieldmatrix_ple --features rand
//! cargo bench -p gf2-core --bench fieldmatrix_ple --features rand -- --test
//! cargo bench -p gf2-core --bench fieldmatrix_ple --features rand -- pluq/Fp_M31/uniform/256
//! ```

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use gf2_core::field::matrix::FieldMatrix;
use gf2_core::field::FiniteField;
use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};

#[path = "common/seed.rs"]
mod seed;

use seed::{
    derive_seed, fp_matrix_from_seed, fp_rank_deficient_from_seed, gf2m_wide_1_matrix_from_seed,
    gf2m_wide_1_rank_deficient_from_seed, MASTER_SEED,
};

const PRIME_7: u64 = 7;
const PRIME_31: u64 = 31;
const PRIME_127: u64 = 127;
const PRIME_241: u64 = 241;
const PRIME_251: u64 = 251;
const PRIME_65521: u64 = 65521;
const MERSENNE_31: u64 = 2_147_483_647;

/// GF(2^8) AES irreducible.
struct PleGf2m8Cfg;
impl Gf2mWideConfig<1> for PleGf2m8Cfg {
    const M: usize = 8;
    const MODULUS: [u64; 1] = [0x1B];
    const NAME: &'static str = "PleGf2m8Cfg";
}
type Gf2m8 = Gf2mWide<1, PleGf2m8Cfg>;

/// GF(2^16) Conway polynomial.
struct PleGf2m16Cfg;
impl Gf2mWideConfig<1> for PleGf2m16Cfg {
    const M: usize = 16;
    const MODULUS: [u64; 1] = [0x002D];
    const NAME: &'static str = "PleGf2m16Cfg";
}
type Gf2m16 = Gf2mWide<1, PleGf2m16Cfg>;

const SIZES: &[usize] = &[64, 256, 1024, 4096];

const REGIMES: &[(&str, u64)] = &[("uniform", 0), ("deficient", 1)];

/// Build the input matrix for a given `(field, size, regime)` cell.
/// `op_idx` is the seed-derivation index from the reference harness's
/// per-tag op enumeration (1=pluq, 2=echelon, 3=invert, 4=solve).
fn build<F, FillUniform, FillDeficient>(
    n: usize,
    si: usize,
    regime_idx: u64,
    tag: &str,
    op_idx: u64,
    fill_uniform: FillUniform,
    fill_deficient: FillDeficient,
) -> FieldMatrix<F>
where
    F: FiniteField,
    FillUniform: Fn(usize, usize, u64) -> FieldMatrix<F>,
    FillDeficient: Fn(usize, usize, usize, u64) -> FieldMatrix<F>,
{
    let row_seed = derive_seed(MASTER_SEED, tag, op_idx, si as u64, regime_idx);
    if regime_idx == 0 {
        fill_uniform(n, n, row_seed)
    } else {
        fill_deficient(n, n, n / 2, row_seed)
    }
}

/// Generic driver covering `ple`, `row_echelon`, `rref`, `rank`, `nullspace`
/// over one field. Each `(op, regime)` pair becomes its own criterion group.
#[allow(clippy::too_many_arguments)]
fn run_field<F, FillUniform, FillDeficient>(
    c: &mut Criterion,
    field_label: &str,
    sizes: &[usize],
    fill_uniform: FillUniform,
    fill_deficient: FillDeficient,
) where
    F: FiniteField,
    FillUniform: Fn(usize, usize, u64) -> FieldMatrix<F> + Copy,
    FillDeficient: Fn(usize, usize, usize, u64) -> FieldMatrix<F> + Copy,
{
    // ── ple ────────────────────────────────────────────────────────────────
    for &(regime, regime_idx) in REGIMES {
        let group_name = format!("pluq/{field_label}/{regime}");
        let mut group = c.benchmark_group(&group_name);
        group.sample_size(10);
        group.measurement_time(std::time::Duration::from_secs(5));
        for (si, &n) in sizes.iter().enumerate() {
            let a = build::<F, _, _>(n, si, regime_idx, "pluq", 1, fill_uniform, fill_deficient);
            group.bench_with_input(BenchmarkId::from_parameter(n), &n, |bench, _| {
                bench.iter(|| {
                    let r = black_box(&a).ple();
                    black_box(r);
                });
            });
        }
        group.finish();
    }

    // ── row_echelon ───────────────────────────────────────────────────────
    for &(regime, regime_idx) in REGIMES {
        let group_name = format!("echelon/{field_label}/{regime}");
        let mut group = c.benchmark_group(&group_name);
        group.sample_size(10);
        group.measurement_time(std::time::Duration::from_secs(5));
        for (si, &n) in sizes.iter().enumerate() {
            let a = build::<F, _, _>(
                n,
                si,
                regime_idx,
                "echelon",
                2,
                fill_uniform,
                fill_deficient,
            );
            group.bench_with_input(BenchmarkId::from_parameter(n), &n, |bench, _| {
                bench.iter(|| {
                    let r = black_box(&a).row_echelon();
                    black_box(r);
                });
            });
        }
        group.finish();
    }

    // ── rref ──────────────────────────────────────────────────────────────
    for &(regime, regime_idx) in REGIMES {
        let group_name = format!("rref/{field_label}/{regime}");
        let mut group = c.benchmark_group(&group_name);
        group.sample_size(10);
        group.measurement_time(std::time::Duration::from_secs(5));
        for (si, &n) in sizes.iter().enumerate() {
            let a = build::<F, _, _>(n, si, regime_idx, "rref", 6, fill_uniform, fill_deficient);
            group.bench_with_input(BenchmarkId::from_parameter(n), &n, |bench, _| {
                bench.iter(|| {
                    let r = black_box(&a).rref();
                    black_box(r);
                });
            });
        }
        group.finish();
    }

    // ── rank ──────────────────────────────────────────────────────────────
    for &(regime, regime_idx) in REGIMES {
        let group_name = format!("rank/{field_label}/{regime}");
        let mut group = c.benchmark_group(&group_name);
        group.sample_size(10);
        group.measurement_time(std::time::Duration::from_secs(5));
        for (si, &n) in sizes.iter().enumerate() {
            let a = build::<F, _, _>(n, si, regime_idx, "rank", 7, fill_uniform, fill_deficient);
            group.bench_with_input(BenchmarkId::from_parameter(n), &n, |bench, _| {
                bench.iter(|| {
                    let r = black_box(&a).rank();
                    black_box(r);
                });
            });
        }
        group.finish();
    }

    // ── nullspace ─────────────────────────────────────────────────────────
    //
    // Uniform inputs are full-rank with overwhelming probability so the
    // nullspace is empty; the timing then reflects the rank-detection
    // cost. The deficient regime exercises the actual basis-extraction
    // path, which is the cell that matters for downstream comparison.
    for &(regime, regime_idx) in REGIMES {
        let group_name = format!("nullspace/{field_label}/{regime}");
        let mut group = c.benchmark_group(&group_name);
        group.sample_size(10);
        group.measurement_time(std::time::Duration::from_secs(5));
        for (si, &n) in sizes.iter().enumerate() {
            let a = build::<F, _, _>(
                n,
                si,
                regime_idx,
                "nullspace",
                8,
                fill_uniform,
                fill_deficient,
            );
            group.bench_with_input(BenchmarkId::from_parameter(n), &n, |bench, _| {
                bench.iter(|| {
                    let r = black_box(&a).nullspace();
                    black_box(r);
                });
            });
        }
        group.finish();
    }
}

fn bench_fp_7(c: &mut Criterion) {
    run_field::<gf2_core::gfp::Fp<PRIME_7>, _, _>(
        c,
        "Fp_7",
        SIZES,
        fp_matrix_from_seed::<PRIME_7>,
        fp_rank_deficient_from_seed::<PRIME_7>,
    );
}

fn bench_fp_31(c: &mut Criterion) {
    run_field::<gf2_core::gfp::Fp<PRIME_31>, _, _>(
        c,
        "Fp_31",
        SIZES,
        fp_matrix_from_seed::<PRIME_31>,
        fp_rank_deficient_from_seed::<PRIME_31>,
    );
}

fn bench_fp_127(c: &mut Criterion) {
    run_field::<gf2_core::gfp::Fp<PRIME_127>, _, _>(
        c,
        "Fp_127",
        SIZES,
        fp_matrix_from_seed::<PRIME_127>,
        fp_rank_deficient_from_seed::<PRIME_127>,
    );
}

fn bench_fp_241(c: &mut Criterion) {
    run_field::<gf2_core::gfp::Fp<PRIME_241>, _, _>(
        c,
        "Fp_241",
        SIZES,
        fp_matrix_from_seed::<PRIME_241>,
        fp_rank_deficient_from_seed::<PRIME_241>,
    );
}

fn bench_fp_251(c: &mut Criterion) {
    run_field::<gf2_core::gfp::Fp<PRIME_251>, _, _>(
        c,
        "Fp_251",
        SIZES,
        fp_matrix_from_seed::<PRIME_251>,
        fp_rank_deficient_from_seed::<PRIME_251>,
    );
}

fn bench_fp_65521(c: &mut Criterion) {
    run_field::<gf2_core::gfp::Fp<PRIME_65521>, _, _>(
        c,
        "Fp_65521",
        SIZES,
        fp_matrix_from_seed::<PRIME_65521>,
        fp_rank_deficient_from_seed::<PRIME_65521>,
    );
}

fn bench_fp_m31(c: &mut Criterion) {
    run_field::<gf2_core::gfp::Fp<MERSENNE_31>, _, _>(
        c,
        "Fp_M31",
        SIZES,
        fp_matrix_from_seed::<MERSENNE_31>,
        fp_rank_deficient_from_seed::<MERSENNE_31>,
    );
}

fn bench_gf2m8(c: &mut Criterion) {
    run_field::<Gf2m8, _, _>(
        c,
        "Gf2m8",
        SIZES,
        gf2m_wide_1_matrix_from_seed::<PleGf2m8Cfg>,
        gf2m_wide_1_rank_deficient_from_seed::<PleGf2m8Cfg>,
    );
}

fn bench_gf2m16(c: &mut Criterion) {
    run_field::<Gf2m16, _, _>(
        c,
        "Gf2m16",
        SIZES,
        gf2m_wide_1_matrix_from_seed::<PleGf2m16Cfg>,
        gf2m_wide_1_rank_deficient_from_seed::<PleGf2m16Cfg>,
    );
}

criterion_group! {
    name = ple_benches;
    config = Criterion::default()
        .sample_size(10)
        .measurement_time(std::time::Duration::from_secs(5));
    targets =
        bench_fp_7,
        bench_fp_31,
        bench_fp_127,
        bench_fp_241,
        bench_fp_251,
        bench_fp_65521,
        bench_fp_m31,
        bench_gf2m8,
        bench_gf2m16
}
criterion_main!(ple_benches);
