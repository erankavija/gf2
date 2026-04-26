//! Characteristic polynomial / minimal polynomial / Frobenius form —
//! Criterion benches.
//!
//! Issue `f01298db`. Measures the public entry points
//! [`FieldMatrix::charpoly`], [`FieldMatrix::minpoly`], and
//! [`FieldMatrix::frobenius_form`] at `n ∈ {32, 128, 512}` for
//! `Fp<65521>` and a small `Gf2mWide<8>` configuration.
//!
//! ```text
//! charpoly/charpoly/Fp_65521/32
//! charpoly/minpoly/Gf2m8/128
//! charpoly/frobenius/Fp_65521/512
//! ```
//!
//! ## Usage
//!
//! ```bash
//! cargo bench -p gf2-core --bench charpoly --features rand
//! cargo bench -p gf2-core --bench charpoly --features rand -- --test
//! cargo bench -p gf2-core --bench charpoly --features rand -- charpoly/charpoly/Fp_65521/32
//! ```

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use gf2_core::field::matrix::FieldMatrix;
use gf2_core::field::test_random_matrix::{random_fp, random_gf2m_wide_1};
use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};

const PRIME_65521: u64 = 65521;

/// GF(2^8) AES irreducible.
struct CpBenchGf2m8Cfg;
impl Gf2mWideConfig<1> for CpBenchGf2m8Cfg {
    const M: usize = 8;
    const MODULUS: [u64; 1] = [0x1B];
    const NAME: &'static str = "CpBenchGf2m8Cfg";
}
type Gf2m8 = Gf2mWide<1, CpBenchGf2m8Cfg>;

const SIZES: &[usize] = &[32, 128, 512];

fn random_gf2m8(rows: usize, cols: usize, seed: u64) -> FieldMatrix<Gf2m8> {
    random_gf2m_wide_1::<CpBenchGf2m8Cfg>(rows, cols, seed)
}

fn bench_charpoly(c: &mut Criterion) {
    let mut group = c.benchmark_group("charpoly/charpoly");
    for &n in SIZES {
        let a_fp = random_fp::<PRIME_65521>(n, n, 0xCAFE);
        let a_gf = random_gf2m8(n, n, 0xCAFE);
        group.bench_with_input(BenchmarkId::new("Fp_65521", n), &n, |b, _| {
            b.iter(|| {
                let r = black_box(&a_fp).charpoly();
                black_box(r);
            });
        });
        group.bench_with_input(BenchmarkId::new("Gf2m8", n), &n, |b, _| {
            b.iter(|| {
                let r = black_box(&a_gf).charpoly();
                black_box(r);
            });
        });
    }
    group.finish();
}

fn bench_minpoly(c: &mut Criterion) {
    let mut group = c.benchmark_group("charpoly/minpoly");
    for &n in SIZES {
        let a_fp = random_fp::<PRIME_65521>(n, n, 0xBEEF);
        let a_gf = random_gf2m8(n, n, 0xBEEF);
        group.bench_with_input(BenchmarkId::new("Fp_65521", n), &n, |b, _| {
            b.iter(|| {
                let r = black_box(&a_fp).minpoly();
                black_box(r);
            });
        });
        group.bench_with_input(BenchmarkId::new("Gf2m8", n), &n, |b, _| {
            b.iter(|| {
                let r = black_box(&a_gf).minpoly();
                black_box(r);
            });
        });
    }
    group.finish();
}

fn bench_frobenius(c: &mut Criterion) {
    let mut group = c.benchmark_group("charpoly/frobenius");
    for &n in SIZES {
        let a_fp = random_fp::<PRIME_65521>(n, n, 0xC0DE);
        let a_gf = random_gf2m8(n, n, 0xC0DE);
        group.bench_with_input(BenchmarkId::new("Fp_65521", n), &n, |b, _| {
            b.iter(|| {
                let r = black_box(&a_fp).frobenius_form();
                black_box(r);
            });
        });
        group.bench_with_input(BenchmarkId::new("Gf2m8", n), &n, |b, _| {
            b.iter(|| {
                let r = black_box(&a_gf).frobenius_form();
                black_box(r);
            });
        });
    }
    group.finish();
}

criterion_group! {
    name = charpoly_benches;
    config = Criterion::default()
        .sample_size(10)
        .measurement_time(std::time::Duration::from_secs(5));
    targets = bench_charpoly, bench_minpoly, bench_frobenius
}
criterion_main!(charpoly_benches);
