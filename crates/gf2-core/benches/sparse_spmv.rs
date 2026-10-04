//! Benchmarks `SparseFieldMatrix::matvec` over four prime fields, GF(2^8) and
//! GF(2^16) at every `(density, size)` cell of `DENSITIES` × `SIZES`, with
//! fixtures from `gf2_core::bench_seed`.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use gf2_core::field::sparse_matrix::SparseFieldMatrix;
use gf2_core::field::vec::FieldVec;
use gf2_core::field::FiniteField;
use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
use gf2_core::gfp::Fp;

#[path = "common/seed.rs"]
mod seed;

use seed::{
    derive_seed, fp_sparse_from_seed, fp_vec_from_seed, gf2m_wide_1_sparse_from_seed,
    gf2m_wide_1_vec_from_seed, MASTER_SEED,
};

const PRIME_7: u64 = 7;
const PRIME_251: u64 = 251;
const PRIME_65521: u64 = 65521;
const MERSENNE_31: u64 = 2_147_483_647;

/// GF(2^8) with the AES polynomial (`@/citation/Nist2001`).
struct SpGf2m8Cfg;
impl Gf2mWideConfig<1> for SpGf2m8Cfg {
    const M: usize = 8;
    const MODULUS: [u64; 1] = [0x1B];
    const NAME: &'static str = "SpGf2m8Cfg";
}
type Gf2m8 = Gf2mWide<1, SpGf2m8Cfg>;

/// GF(2^16) with `x^16 + x^5 + x^3 + x^2 + 1`.
struct SpGf2m16Cfg;
impl Gf2mWideConfig<1> for SpGf2m16Cfg {
    const M: usize = 16;
    const MODULUS: [u64; 1] = [0x002D];
    const NAME: &'static str = "SpGf2m16Cfg";
}
type Gf2m16 = Gf2mWide<1, SpGf2m16Cfg>;

const SIZES: &[usize] = &[256, 1024, 4096];
const DENSITIES: &[(f64, &str)] = &[(0.01, "0.01"), (0.05, "0.05")];

fn random_sparse_fp<const P: u64>(
    rows: usize,
    cols: usize,
    density: f64,
    seed_val: u64,
) -> SparseFieldMatrix<Fp<P>> {
    fp_sparse_from_seed::<P>(rows, cols, density, seed_val)
}

fn random_sparse_gf2m_wide_1<C: Gf2mWideConfig<1>>(
    rows: usize,
    cols: usize,
    density: f64,
    seed_val: u64,
) -> SparseFieldMatrix<Gf2mWide<1, C>> {
    gf2m_wide_1_sparse_from_seed::<C>(rows, cols, density, seed_val)
}

fn fp_vec<const P: u64>(n: usize, seed_val: u64) -> FieldVec<Fp<P>> {
    fp_vec_from_seed::<P>(n, seed_val)
}

fn gf2m_vec<C: Gf2mWideConfig<1>>(n: usize, seed_val: u64) -> FieldVec<Gf2mWide<1, C>> {
    gf2m_wide_1_vec_from_seed::<C>(n, seed_val)
}

fn run_field<F, BuildSparse, BuildVec>(
    c: &mut Criterion,
    field_label: &str,
    sizes: &[usize],
    densities: &[(f64, &str)],
    build_sparse: BuildSparse,
    build_vec: BuildVec,
) where
    F: FiniteField,
    BuildSparse: Fn(usize, usize, f64, u64) -> SparseFieldMatrix<F>,
    BuildVec: Fn(usize, u64) -> FieldVec<F>,
{
    for (di, &(density, density_label)) in densities.iter().enumerate() {
        let group_name = format!("spmv/{field_label}/{density_label}");
        let mut group = c.benchmark_group(&group_name);
        group.sample_size(10);
        group.measurement_time(std::time::Duration::from_secs(5));
        for (si, &n) in sizes.iter().enumerate() {
            let row_seed = derive_seed(MASTER_SEED, "spmv", 11, si as u64, di as u64);
            let vec_seed = derive_seed(MASTER_SEED, "spmv_vec", 11, si as u64, di as u64);
            let a = build_sparse(n, n, density, row_seed);
            let x = build_vec(n, vec_seed);
            group.bench_with_input(BenchmarkId::from_parameter(n), &n, |bench, _| {
                bench.iter(|| {
                    let y = black_box(&a).matvec(black_box(&x));
                    black_box(y);
                });
            });
        }
        group.finish();
    }
}

fn bench_fp_7(c: &mut Criterion) {
    run_field::<Fp<PRIME_7>, _, _>(
        c,
        "Fp_7",
        SIZES,
        DENSITIES,
        random_sparse_fp::<PRIME_7>,
        fp_vec::<PRIME_7>,
    );
}

fn bench_fp_251(c: &mut Criterion) {
    run_field::<Fp<PRIME_251>, _, _>(
        c,
        "Fp_251",
        SIZES,
        DENSITIES,
        random_sparse_fp::<PRIME_251>,
        fp_vec::<PRIME_251>,
    );
}

fn bench_fp_65521(c: &mut Criterion) {
    run_field::<Fp<PRIME_65521>, _, _>(
        c,
        "Fp_65521",
        SIZES,
        DENSITIES,
        random_sparse_fp::<PRIME_65521>,
        fp_vec::<PRIME_65521>,
    );
}

fn bench_fp_m31(c: &mut Criterion) {
    run_field::<Fp<MERSENNE_31>, _, _>(
        c,
        "Fp_M31",
        SIZES,
        DENSITIES,
        random_sparse_fp::<MERSENNE_31>,
        fp_vec::<MERSENNE_31>,
    );
}

fn bench_gf2m8(c: &mut Criterion) {
    run_field::<Gf2m8, _, _>(
        c,
        "Gf2m8",
        SIZES,
        DENSITIES,
        random_sparse_gf2m_wide_1::<SpGf2m8Cfg>,
        gf2m_vec::<SpGf2m8Cfg>,
    );
}

fn bench_gf2m16(c: &mut Criterion) {
    run_field::<Gf2m16, _, _>(
        c,
        "Gf2m16",
        SIZES,
        DENSITIES,
        random_sparse_gf2m_wide_1::<SpGf2m16Cfg>,
        gf2m_vec::<SpGf2m16Cfg>,
    );
}

criterion_group! {
    name = spmv_benches;
    config = Criterion::default()
        .sample_size(10)
        .measurement_time(std::time::Duration::from_secs(5));
    targets =
        bench_fp_7,
        bench_fp_251,
        bench_fp_65521,
        bench_fp_m31,
        bench_gf2m8,
        bench_gf2m16
}
criterion_main!(spmv_benches);
