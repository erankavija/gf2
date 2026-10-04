//! Benchmarks `FieldMatrix::charpoly` and `minpoly` on uniform matrices at
//! `n ∈ {32, 128, 512}` over four prime fields, GF(2^8) and GF(2^16), with
//! inputs derived from the shared bench seed.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use gf2_core::field::matrix::FieldMatrix;
use gf2_core::field::FiniteField;
use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};

#[path = "common/seed.rs"]
mod seed;

use seed::{derive_seed, fp_matrix_from_seed, gf2m_wide_1_matrix_from_seed, MASTER_SEED};

const PRIME_7: u64 = 7;
const PRIME_251: u64 = 251;
const PRIME_65521: u64 = 65521;
const MERSENNE_31: u64 = 2_147_483_647;

/// GF(2^8) with the AES polynomial (`@/citation/Nist2001`).
struct CpGf2m8Cfg;
impl Gf2mWideConfig<1> for CpGf2m8Cfg {
    const M: usize = 8;
    const MODULUS: [u64; 1] = [0x1B];
    const NAME: &'static str = "CpGf2m8Cfg";
}
type Gf2m8 = Gf2mWide<1, CpGf2m8Cfg>;

/// GF(2^16) with the Conway polynomial (`@/citation/Lubeck2024`).
struct CpGf2m16Cfg;
impl Gf2mWideConfig<1> for CpGf2m16Cfg {
    const M: usize = 16;
    const MODULUS: [u64; 1] = [0x002D];
    const NAME: &'static str = "CpGf2m16Cfg";
}
type Gf2m16 = Gf2mWide<1, CpGf2m16Cfg>;

const SIZES: &[usize] = &[32, 128, 512];

fn run_field<F, FillUniform>(
    c: &mut Criterion,
    field_label: &str,
    sizes: &[usize],
    fill_uniform: FillUniform,
) where
    F: FiniteField,
    FillUniform: Fn(usize, usize, u64) -> FieldMatrix<F> + Copy,
{
    {
        let group_name = format!("charpoly/{field_label}");
        let mut group = c.benchmark_group(&group_name);
        group.sample_size(10);
        group.measurement_time(std::time::Duration::from_secs(5));
        for (si, &n) in sizes.iter().enumerate() {
            let row_seed = derive_seed(MASTER_SEED, "charpoly", 5, si as u64, 0);
            let a = fill_uniform(n, n, row_seed);
            group.bench_with_input(BenchmarkId::from_parameter(n), &n, |bench, _| {
                bench.iter(|| {
                    let r = black_box(&a).charpoly();
                    black_box(r);
                });
            });
        }
        group.finish();
    }

    {
        let group_name = format!("minpoly/{field_label}");
        let mut group = c.benchmark_group(&group_name);
        group.sample_size(10);
        group.measurement_time(std::time::Duration::from_secs(5));
        for (si, &n) in sizes.iter().enumerate() {
            let row_seed = derive_seed(MASTER_SEED, "minpoly", 10, si as u64, 0);
            let a = fill_uniform(n, n, row_seed);
            group.bench_with_input(BenchmarkId::from_parameter(n), &n, |bench, _| {
                bench.iter(|| {
                    let r = black_box(&a).minpoly();
                    black_box(r);
                });
            });
        }
        group.finish();
    }
}

fn bench_fp_7(c: &mut Criterion) {
    run_field::<gf2_core::gfp::Fp<PRIME_7>, _>(c, "Fp_7", SIZES, fp_matrix_from_seed::<PRIME_7>);
}

fn bench_fp_251(c: &mut Criterion) {
    run_field::<gf2_core::gfp::Fp<PRIME_251>, _>(
        c,
        "Fp_251",
        SIZES,
        fp_matrix_from_seed::<PRIME_251>,
    );
}

fn bench_fp_65521(c: &mut Criterion) {
    run_field::<gf2_core::gfp::Fp<PRIME_65521>, _>(
        c,
        "Fp_65521",
        SIZES,
        fp_matrix_from_seed::<PRIME_65521>,
    );
}

fn bench_fp_m31(c: &mut Criterion) {
    run_field::<gf2_core::gfp::Fp<MERSENNE_31>, _>(
        c,
        "Fp_M31",
        SIZES,
        fp_matrix_from_seed::<MERSENNE_31>,
    );
}

fn bench_gf2m8(c: &mut Criterion) {
    run_field::<Gf2m8, _>(
        c,
        "Gf2m8",
        SIZES,
        gf2m_wide_1_matrix_from_seed::<CpGf2m8Cfg>,
    );
}

fn bench_gf2m16(c: &mut Criterion) {
    run_field::<Gf2m16, _>(
        c,
        "Gf2m16",
        SIZES,
        gf2m_wide_1_matrix_from_seed::<CpGf2m16Cfg>,
    );
}

criterion_group! {
    name = charpoly_benches;
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
criterion_main!(charpoly_benches);
