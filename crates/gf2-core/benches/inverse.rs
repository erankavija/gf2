//! Matrix inversion / solve / determinant — Criterion benches.
//!
//! Issue `ae1d1e88`. Measures the public entry points
//! [`FieldMatrix::inv`], [`FieldMatrix::solve`], and
//! [`FieldMatrix::det`] at `n ∈ {64, 256, 1024}` for `Fp<MERSENNE_31>`
//! and a small `Gf2mWide<8>` configuration.
//!
//! ```text
//! inverse/inv/Fp_M31/64
//! inverse/solve/Gf2m8/256
//! inverse/det/Fp_M31/1024
//! ```
//!
//! ## Usage
//!
//! ```bash
//! cargo bench -p gf2-core --bench inverse --features rand
//! cargo bench -p gf2-core --bench inverse --features rand -- --test
//! cargo bench -p gf2-core --bench inverse --features rand -- inverse/inv/Fp_M31/64
//! ```

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use gf2_core::field::matrix::FieldMatrix;
use gf2_core::field::vec::FieldVec;
use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
use gf2_core::gfp::Fp;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

const MERSENNE_31: u64 = 2_147_483_647;

/// GF(2^8) AES irreducible.
struct InvBenchGf2m8Cfg;
impl Gf2mWideConfig<1> for InvBenchGf2m8Cfg {
    const M: usize = 8;
    const MODULUS: [u64; 1] = [0x1B];
    const NAME: &'static str = "InvBenchGf2m8Cfg";
}
type Gf2m8 = Gf2mWide<1, InvBenchGf2m8Cfg>;

const SIZES: &[usize] = &[64, 256, 1024];

// ─── Random matrix builders ──────────────────────────────────────────────────

fn random_fp<const P: u64>(rows: usize, cols: usize, seed: u64) -> FieldMatrix<Fp<P>> {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut m = FieldMatrix::<Fp<P>>::zeros(rows, cols);
    for r in 0..rows {
        for c in 0..cols {
            m.set(r, c, Fp::<P>::new(rng.gen::<u64>() % P));
        }
    }
    m
}

fn random_gf2m8(rows: usize, cols: usize, seed: u64) -> FieldMatrix<Gf2m8> {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut m = FieldMatrix::<Gf2m8>::zeros(rows, cols);
    for r in 0..rows {
        for c in 0..cols {
            m.set(r, c, Gf2m8::new([rng.gen::<u64>() & 0xFF]));
        }
    }
    m
}

/// Returns a random `n × n` matrix that is full-rank.
fn random_fp_invertible<const P: u64>(n: usize, seed: u64) -> FieldMatrix<Fp<P>> {
    for k in 0..16u64 {
        let m = random_fp::<P>(n, n, seed.wrapping_add(k));
        if m.rank() == n {
            return m;
        }
    }
    panic!("random_fp_invertible: failed to find an invertible matrix");
}

fn random_gf2m8_invertible(n: usize, seed: u64) -> FieldMatrix<Gf2m8> {
    for k in 0..16u64 {
        let m = random_gf2m8(n, n, seed.wrapping_add(k));
        if m.rank() == n {
            return m;
        }
    }
    panic!("random_gf2m8_invertible: failed to find an invertible matrix");
}

fn random_fp_vec<const P: u64>(n: usize, seed: u64) -> FieldVec<Fp<P>> {
    let mut rng = StdRng::seed_from_u64(seed);
    (0..n).map(|_| Fp::<P>::new(rng.gen::<u64>() % P)).collect()
}

fn random_gf2m8_vec(n: usize, seed: u64) -> FieldVec<Gf2m8> {
    let mut rng = StdRng::seed_from_u64(seed);
    (0..n)
        .map(|_| Gf2m8::new([rng.gen::<u64>() & 0xFF]))
        .collect()
}

// ─── Benches ────────────────────────────────────────────────────────────────

fn bench_inv(c: &mut Criterion) {
    let mut group = c.benchmark_group("inverse/inv");
    for &n in SIZES {
        let a_fp = random_fp_invertible::<MERSENNE_31>(n, 0xCAFE);
        let a_gf = random_gf2m8_invertible(n, 0xCAFE);
        group.bench_with_input(BenchmarkId::new("Fp_M31", n), &n, |b, _| {
            b.iter(|| {
                let r = black_box(&a_fp).inv();
                black_box(r);
            });
        });
        group.bench_with_input(BenchmarkId::new("Gf2m8", n), &n, |b, _| {
            b.iter(|| {
                let r = black_box(&a_gf).inv();
                black_box(r);
            });
        });
    }
    group.finish();
}

fn bench_solve(c: &mut Criterion) {
    let mut group = c.benchmark_group("inverse/solve");
    for &n in SIZES {
        let a_fp = random_fp_invertible::<MERSENNE_31>(n, 0xBEEF);
        let a_gf = random_gf2m8_invertible(n, 0xBEEF);
        let b_fp = random_fp_vec::<MERSENNE_31>(n, 0xF00D);
        let b_gf = random_gf2m8_vec(n, 0xF00D);
        group.bench_with_input(BenchmarkId::new("Fp_M31", n), &n, |b, _| {
            b.iter(|| {
                let r = black_box(&a_fp).solve(black_box(&b_fp));
                black_box(r);
            });
        });
        group.bench_with_input(BenchmarkId::new("Gf2m8", n), &n, |b, _| {
            b.iter(|| {
                let r = black_box(&a_gf).solve(black_box(&b_gf));
                black_box(r);
            });
        });
    }
    group.finish();
}

fn bench_det(c: &mut Criterion) {
    let mut group = c.benchmark_group("inverse/det");
    for &n in SIZES {
        let a_fp = random_fp::<MERSENNE_31>(n, n, 0xC0DE);
        let a_gf = random_gf2m8(n, n, 0xC0DE);
        group.bench_with_input(BenchmarkId::new("Fp_M31", n), &n, |b, _| {
            b.iter(|| {
                let r = black_box(&a_fp).det();
                black_box(r);
            });
        });
        group.bench_with_input(BenchmarkId::new("Gf2m8", n), &n, |b, _| {
            b.iter(|| {
                let r = black_box(&a_gf).det();
                black_box(r);
            });
        });
    }
    group.finish();
}

criterion_group! {
    name = inverse_benches;
    config = Criterion::default()
        .sample_size(10)
        .measurement_time(std::time::Duration::from_secs(5));
    targets = bench_inv, bench_solve, bench_det
}
criterion_main!(inverse_benches);
