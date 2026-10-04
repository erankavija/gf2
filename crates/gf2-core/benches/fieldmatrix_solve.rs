//! Benchmarks `FieldMatrix` `inv`, `solve` and `det` on uniform and
//! rank-`n / 2` matrices derived from the shared bench seed.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use gf2_core::field::matrix::FieldMatrix;
use gf2_core::field::vec::FieldVec;
use gf2_core::field::FiniteField;
use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};

#[path = "common/seed.rs"]
mod seed;

use seed::{
    derive_seed, fp_matrix_from_seed, fp_rank_deficient_from_seed, fp_vec_from_seed,
    gf2m_wide_1_matrix_from_seed, gf2m_wide_1_rank_deficient_from_seed, gf2m_wide_1_vec_from_seed,
    MASTER_SEED,
};

const PRIME_7: u64 = 7;
const PRIME_31: u64 = 31;
const PRIME_251: u64 = 251;
const PRIME_65521: u64 = 65521;
const MERSENNE_31: u64 = 2_147_483_647;

/// GF(2^8) with the AES polynomial (`@/citation/Nist2001`).
struct SolveGf2m8Cfg;
impl Gf2mWideConfig<1> for SolveGf2m8Cfg {
    const M: usize = 8;
    const MODULUS: [u64; 1] = [0x1B];
    const NAME: &'static str = "SolveGf2m8Cfg";
}
type Gf2m8 = Gf2mWide<1, SolveGf2m8Cfg>;

/// GF(2^16) with the Conway polynomial (`@/citation/Lubeck2024`).
struct SolveGf2m16Cfg;
impl Gf2mWideConfig<1> for SolveGf2m16Cfg {
    const M: usize = 16;
    const MODULUS: [u64; 1] = [0x002D];
    const NAME: &'static str = "SolveGf2m16Cfg";
}
type Gf2m16 = Gf2mWide<1, SolveGf2m16Cfg>;

/// Setting `GF2_BENCH_SKIP_4096` to any value drops the `n = 4096` cells.
fn bench_sizes() -> &'static [usize] {
    if std::env::var("GF2_BENCH_SKIP_4096").is_ok() {
        &[64, 256, 1024]
    } else {
        &[64, 256, 1024, 4096]
    }
}

const REGIMES: &[(&str, u64)] = &[("uniform", 0), ("deficient", 1)];

fn build_matrix<F, FillUniform, FillDeficient>(
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

#[allow(clippy::too_many_arguments)]
fn run_field<F, FillUniform, FillDeficient, FillVec>(
    c: &mut Criterion,
    field_label: &str,
    sizes: &[usize],
    fill_uniform: FillUniform,
    fill_deficient: FillDeficient,
    fill_vec: FillVec,
) where
    F: FiniteField,
    FillUniform: Fn(usize, usize, u64) -> FieldMatrix<F> + Copy,
    FillDeficient: Fn(usize, usize, usize, u64) -> FieldMatrix<F> + Copy,
    FillVec: Fn(usize, u64) -> FieldVec<F> + Copy,
{
    for &(regime, regime_idx) in REGIMES {
        let group_name = format!("invert/{field_label}/{regime}");
        let mut group = c.benchmark_group(&group_name);
        group.sample_size(10);
        group.measurement_time(std::time::Duration::from_secs(5));
        for (si, &n) in sizes.iter().enumerate() {
            let a = build_matrix::<F, _, _>(
                n,
                si,
                regime_idx,
                "invert",
                3,
                fill_uniform,
                fill_deficient,
            );
            group.bench_with_input(BenchmarkId::from_parameter(n), &n, |bench, _| {
                bench.iter(|| {
                    let r = black_box(&a).inv();
                    black_box(r);
                });
            });
        }
        group.finish();
    }

    for &(regime, regime_idx) in REGIMES {
        let group_name = format!("solve/{field_label}/{regime}");
        let mut group = c.benchmark_group(&group_name);
        group.sample_size(10);
        group.measurement_time(std::time::Duration::from_secs(5));
        for (si, &n) in sizes.iter().enumerate() {
            let a = build_matrix::<F, _, _>(
                n,
                si,
                regime_idx,
                "solve",
                4,
                fill_uniform,
                fill_deficient,
            );
            let b_seed = derive_seed(MASTER_SEED, "solve_rhs", 4, si as u64, regime_idx);
            let bvec = fill_vec(n, b_seed);
            group.bench_with_input(BenchmarkId::from_parameter(n), &n, |bench, _| {
                bench.iter(|| {
                    let r = black_box(&a).solve(black_box(&bvec));
                    black_box(r);
                });
            });
        }
        group.finish();
    }

    for &(regime, regime_idx) in REGIMES {
        let group_name = format!("det/{field_label}/{regime}");
        let mut group = c.benchmark_group(&group_name);
        group.sample_size(10);
        group.measurement_time(std::time::Duration::from_secs(5));
        for (si, &n) in sizes.iter().enumerate() {
            let a =
                build_matrix::<F, _, _>(n, si, regime_idx, "det", 9, fill_uniform, fill_deficient);
            group.bench_with_input(BenchmarkId::from_parameter(n), &n, |bench, _| {
                bench.iter(|| {
                    let r = black_box(&a).det();
                    black_box(r);
                });
            });
        }
        group.finish();
    }
}

fn bench_fp_7(c: &mut Criterion) {
    run_field::<gf2_core::gfp::Fp<PRIME_7>, _, _, _>(
        c,
        "Fp_7",
        bench_sizes(),
        fp_matrix_from_seed::<PRIME_7>,
        fp_rank_deficient_from_seed::<PRIME_7>,
        fp_vec_from_seed::<PRIME_7>,
    );
}

fn bench_fp_31(c: &mut Criterion) {
    run_field::<gf2_core::gfp::Fp<PRIME_31>, _, _, _>(
        c,
        "Fp_31",
        bench_sizes(),
        fp_matrix_from_seed::<PRIME_31>,
        fp_rank_deficient_from_seed::<PRIME_31>,
        fp_vec_from_seed::<PRIME_31>,
    );
}

fn bench_fp_251(c: &mut Criterion) {
    run_field::<gf2_core::gfp::Fp<PRIME_251>, _, _, _>(
        c,
        "Fp_251",
        bench_sizes(),
        fp_matrix_from_seed::<PRIME_251>,
        fp_rank_deficient_from_seed::<PRIME_251>,
        fp_vec_from_seed::<PRIME_251>,
    );
}

fn bench_fp_65521(c: &mut Criterion) {
    run_field::<gf2_core::gfp::Fp<PRIME_65521>, _, _, _>(
        c,
        "Fp_65521",
        bench_sizes(),
        fp_matrix_from_seed::<PRIME_65521>,
        fp_rank_deficient_from_seed::<PRIME_65521>,
        fp_vec_from_seed::<PRIME_65521>,
    );
}

fn bench_fp_m31(c: &mut Criterion) {
    run_field::<gf2_core::gfp::Fp<MERSENNE_31>, _, _, _>(
        c,
        "Fp_M31",
        bench_sizes(),
        fp_matrix_from_seed::<MERSENNE_31>,
        fp_rank_deficient_from_seed::<MERSENNE_31>,
        fp_vec_from_seed::<MERSENNE_31>,
    );
}

fn bench_gf2m8(c: &mut Criterion) {
    run_field::<Gf2m8, _, _, _>(
        c,
        "Gf2m8",
        bench_sizes(),
        gf2m_wide_1_matrix_from_seed::<SolveGf2m8Cfg>,
        gf2m_wide_1_rank_deficient_from_seed::<SolveGf2m8Cfg>,
        gf2m_wide_1_vec_from_seed::<SolveGf2m8Cfg>,
    );
}

fn bench_gf2m16(c: &mut Criterion) {
    run_field::<Gf2m16, _, _, _>(
        c,
        "Gf2m16",
        bench_sizes(),
        gf2m_wide_1_matrix_from_seed::<SolveGf2m16Cfg>,
        gf2m_wide_1_rank_deficient_from_seed::<SolveGf2m16Cfg>,
        gf2m_wide_1_vec_from_seed::<SolveGf2m16Cfg>,
    );
}

criterion_group! {
    name = solve_benches;
    config = Criterion::default()
        .sample_size(10)
        .measurement_time(std::time::Duration::from_secs(5));
    targets =
        bench_fp_7,
        bench_fp_31,
        bench_fp_251,
        bench_fp_65521,
        bench_fp_m31,
        bench_gf2m8,
        bench_gf2m16
}
criterion_main!(solve_benches);
