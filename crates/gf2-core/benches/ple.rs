//! Benchmarks the PLE decomposition and the row-echelon, RREF and LU
//! operations derived from it over `Fp<MERSENNE_31>` and a GF(2^8)
//! `Gf2mWide<1>` configuration.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use gf2_core::field::matrix::FieldMatrix;
use gf2_core::field::test_random_matrix::{random_fp, random_gf2m_wide_1};
use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};

const MERSENNE_31: u64 = 2_147_483_647;

/// GF(2^8) with the AES polynomial (`@/citation/Nist2001`).
struct PleBenchGf2m8Cfg;
impl Gf2mWideConfig<1> for PleBenchGf2m8Cfg {
    const M: usize = 8;
    const MODULUS: [u64; 1] = [0x1B];
    const NAME: &'static str = "PleBenchGf2m8Cfg";
}
type Gf2m8 = Gf2mWide<1, PleBenchGf2m8Cfg>;

const SIZES: &[usize] = &[64, 256, 1024];

fn random_gf2m8(rows: usize, cols: usize, seed: u64) -> FieldMatrix<Gf2m8> {
    random_gf2m_wide_1::<PleBenchGf2m8Cfg>(rows, cols, seed)
}

fn bench_ple(c: &mut Criterion) {
    let mut group = c.benchmark_group("ple/ple");
    for &n in SIZES {
        let a_fp = random_fp::<MERSENNE_31>(n, n, 0xCAFE);
        let a_gf = random_gf2m8(n, n, 0xCAFE);
        group.bench_with_input(BenchmarkId::new("Fp_M31", n), &n, |b, _| {
            b.iter(|| {
                let r = black_box(&a_fp).ple();
                black_box(r);
            });
        });
        group.bench_with_input(BenchmarkId::new("Gf2m8", n), &n, |b, _| {
            b.iter(|| {
                let r = black_box(&a_gf).ple();
                black_box(r);
            });
        });
    }
    group.finish();
}

fn bench_row_echelon(c: &mut Criterion) {
    let mut group = c.benchmark_group("ple/row_echelon");
    for &n in SIZES {
        let a_fp = random_fp::<MERSENNE_31>(n, n, 0xBEEF);
        let a_gf = random_gf2m8(n, n, 0xBEEF);
        group.bench_with_input(BenchmarkId::new("Fp_M31", n), &n, |b, _| {
            b.iter(|| {
                let r = black_box(&a_fp).row_echelon();
                black_box(r);
            });
        });
        group.bench_with_input(BenchmarkId::new("Gf2m8", n), &n, |b, _| {
            b.iter(|| {
                let r = black_box(&a_gf).row_echelon();
                black_box(r);
            });
        });
    }
    group.finish();
}

fn bench_rref(c: &mut Criterion) {
    let mut group = c.benchmark_group("ple/rref");
    for &n in SIZES {
        let a_fp = random_fp::<MERSENNE_31>(n, n, 0xFEED);
        let a_gf = random_gf2m8(n, n, 0xFEED);
        group.bench_with_input(BenchmarkId::new("Fp_M31", n), &n, |b, _| {
            b.iter(|| {
                let r = black_box(&a_fp).rref();
                black_box(r);
            });
        });
        group.bench_with_input(BenchmarkId::new("Gf2m8", n), &n, |b, _| {
            b.iter(|| {
                let r = black_box(&a_gf).rref();
                black_box(r);
            });
        });
    }
    group.finish();
}

fn bench_lu(c: &mut Criterion) {
    let mut group = c.benchmark_group("ple/lu");
    for &n in SIZES {
        let a_fp = random_fp::<MERSENNE_31>(n, n, 0xDEAD);
        let a_gf = random_gf2m8(n, n, 0xDEAD);
        group.bench_with_input(BenchmarkId::new("Fp_M31", n), &n, |b, _| {
            b.iter(|| {
                let r = black_box(&a_fp).lu();
                black_box(r);
            });
        });
        group.bench_with_input(BenchmarkId::new("Gf2m8", n), &n, |b, _| {
            b.iter(|| {
                let r = black_box(&a_gf).lu();
                black_box(r);
            });
        });
    }
    group.finish();
}

criterion_group! {
    name = ple_benches;
    config = Criterion::default().sample_size(10);
    targets = bench_ple, bench_row_echelon, bench_rref, bench_lu
}
criterion_main!(ple_benches);
