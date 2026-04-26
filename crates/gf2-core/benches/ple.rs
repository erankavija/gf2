//! PLE decomposition + derived operations — Criterion benches.
//!
//! Issue `c3f8c1cb`. Measures the public PLE entry points at
//! `n ∈ {64, 256, 1024}` for `Fp<MERSENNE_31>` and a small
//! `Gf2mWide<8>` configuration. Each operation lives in its own
//! Criterion group so individual cases can be filtered:
//!
//! ```text
//! ple/ple/Fp_M31/64
//! ple/row_echelon/Fp_M31/256
//! ple/rref/Gf2m8/1024
//! ple/lu/Fp_M31/1024
//! ```
//!
//! ## Usage
//!
//! ```bash
//! cargo bench -p gf2-core --bench ple --features rand
//! cargo bench -p gf2-core --bench ple --features rand -- --test
//! cargo bench -p gf2-core --bench ple --features rand -- ple/ple/Fp_M31/256
//! ```

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use gf2_core::field::matrix::FieldMatrix;
use gf2_core::field::test_random_matrix::{random_fp, random_gf2m_wide_1};
use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};

const MERSENNE_31: u64 = 2_147_483_647;

/// GF(2^8) AES irreducible.
struct PleBenchGf2m8Cfg;
impl Gf2mWideConfig<1> for PleBenchGf2m8Cfg {
    const M: usize = 8;
    const MODULUS: [u64; 1] = [0x1B];
    const NAME: &'static str = "PleBenchGf2m8Cfg";
}
type Gf2m8 = Gf2mWide<1, PleBenchGf2m8Cfg>;

const SIZES: &[usize] = &[64, 256, 1024];

// ─── Random matrix builders ──────────────────────────────────────────────────
//
// Thin local alias that monomorphises the shared generic helpers in
// `gf2_core::field::test_random_matrix` to this bench's `Gf2m8` config.

fn random_gf2m8(rows: usize, cols: usize, seed: u64) -> FieldMatrix<Gf2m8> {
    random_gf2m_wide_1::<PleBenchGf2m8Cfg>(rows, cols, seed)
}

// ─── Benches ────────────────────────────────────────────────────────────────

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
