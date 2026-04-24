//! Strassen–Winograd threshold sweep and classical-vs-Winograd comparison.
//!
//! Issue `ad597ede`, story `d48a3cfd/T3`. Measures:
//!
//! 1. Classical `gemm` vs `gemm_winograd` at `n ∈ {256, 512, 1024, 2048,
//!    4096}` for both `Fp<2^31 - 1>` (Mersenne-31) and `Gf2mWide<1, AES>`
//!    GF(2^8).
//! 2. Threshold sweep at `n = 2048` over Mersenne-31: records the Winograd
//!    runtime for per-recursion thresholds `∈ {32, 64, 128, 256, 512, 1024}`
//!    — the winning value is recorded in
//!    `benches/strassen_threshold_results.md` and fed back to the
//!    `FiniteField::WINOGRAD_THRESHOLD` default (see
//!    `crates/gf2-core/src/field/traits.rs`).
//!
//! The recorded results live in `benches/strassen_threshold_results.md`.
//!
//! No recursion or helper logic is duplicated here; the sweep invokes
//! [`gemm_winograd_with_threshold`] which is the same recursion used by
//! production.
//!
//! ## Usage
//!
//! ```bash
//! cargo bench -p gf2-core --bench strassen_threshold --features rand
//! # Smoke run:
//! cargo bench -p gf2-core --bench strassen_threshold --features rand -- --test
//! ```
//!
//! The `n = 4096` case is expensive (≈ minutes per sample on a
//! commodity core); the `threshold_sweep` group is the most useful
//! artefact for retuning the crossover.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use gf2_core::field::matrix::{gemm, FieldMatrix};
use gf2_core::field::winograd::{gemm_winograd, gemm_winograd_with_threshold};
use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
use gf2_core::gfp::Fp;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

// Mersenne-31 prime.
const MERSENNE_31: u64 = 2_147_483_647;

/// GF(2^8) with AES irreducible `x^8 + x^4 + x^3 + x + 1`. Implicit leading
/// bit convention per `Gf2mWideConfig` ⇒ low byte is `0x1B`.
struct StrassenGf2m8Cfg;
impl Gf2mWideConfig<1> for StrassenGf2m8Cfg {
    const M: usize = 8;
    const MODULUS: [u64; 1] = [0x1B];
    const NAME: &'static str = "StrassenGf2m8Cfg";
}
type Gf2m8 = Gf2mWide<1, StrassenGf2m8Cfg>;

// Compare at `n ≥ 256` so the Winograd peel actually fires (default
// threshold = 128). n = 4096 takes minutes per sample on a scalar path;
// Criterion owners may filter it with a bench name filter for faster
// iteration.
const COMPARE_SIZES: &[usize] = &[256, 512, 1024, 2048, 4096];

fn random_fp_matrix<const P: u64>(rows: usize, cols: usize, seed: u64) -> FieldMatrix<Fp<P>> {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut m = FieldMatrix::<Fp<P>>::zeros(rows, cols);
    for r in 0..rows {
        for c in 0..cols {
            m.set(r, c, Fp::<P>::new(rng.gen::<u64>() % P));
        }
    }
    m
}

fn random_gf2m8_matrix(rows: usize, cols: usize, seed: u64) -> FieldMatrix<Gf2m8> {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut m = FieldMatrix::<Gf2m8>::zeros(rows, cols);
    for r in 0..rows {
        for c in 0..cols {
            m.set(r, c, Gf2m8::new([rng.gen::<u64>() & 0xFF]));
        }
    }
    m
}

fn bench_gemm_vs_winograd_fp(c: &mut Criterion) {
    let mut group = c.benchmark_group("strassen_threshold/fp_mersenne31");
    for &n in COMPARE_SIZES {
        group.throughput(Throughput::Elements((n * n * n) as u64));
        let a = random_fp_matrix::<MERSENNE_31>(n, n, 0xAA ^ n as u64);
        let b = random_fp_matrix::<MERSENNE_31>(n, n, 0xBB ^ n as u64);
        group.bench_with_input(BenchmarkId::new("classical", n), &n, |bench, _| {
            bench.iter(|| {
                let out = gemm(black_box(&a), black_box(&b));
                black_box(out);
            });
        });
        group.bench_with_input(BenchmarkId::new("winograd", n), &n, |bench, _| {
            bench.iter(|| {
                let out = gemm_winograd(black_box(&a), black_box(&b));
                black_box(out);
            });
        });
    }
    group.finish();
}

fn bench_gemm_vs_winograd_gf2m8(c: &mut Criterion) {
    let mut group = c.benchmark_group("strassen_threshold/gf2m8_aes");
    for &n in COMPARE_SIZES {
        group.throughput(Throughput::Elements((n * n * n) as u64));
        let a = random_gf2m8_matrix(n, n, 0xCC ^ n as u64);
        let b = random_gf2m8_matrix(n, n, 0xDD ^ n as u64);
        group.bench_with_input(BenchmarkId::new("classical", n), &n, |bench, _| {
            bench.iter(|| {
                let out = gemm(black_box(&a), black_box(&b));
                black_box(out);
            });
        });
        group.bench_with_input(BenchmarkId::new("winograd", n), &n, |bench, _| {
            bench.iter(|| {
                let out = gemm_winograd(black_box(&a), black_box(&b));
                black_box(out);
            });
        });
    }
    group.finish();
}

/// Threshold sweep at `n = 2048` over Mersenne-31. The winning threshold
/// is recorded in `benches/strassen_threshold_results.md` and fed back
/// to the `FiniteField::WINOGRAD_THRESHOLD` default. The sweep routes
/// through the production recursion via
/// [`gemm_winograd_with_threshold`], so no helper duplication.
fn bench_threshold_sweep(c: &mut Criterion) {
    let mut group = c.benchmark_group("strassen_threshold/sweep_fp_mersenne31_n2048");
    let n = 2048;
    let a = random_fp_matrix::<MERSENNE_31>(n, n, 0xEE);
    let b = random_fp_matrix::<MERSENNE_31>(n, n, 0xFF);
    for &threshold in &[32usize, 64, 128, 256, 512, 1024] {
        group.bench_with_input(
            BenchmarkId::from_parameter(threshold),
            &threshold,
            |bench, &t| {
                bench.iter(|| {
                    let out = gemm_winograd_with_threshold(black_box(&a), black_box(&b), t);
                    black_box(out);
                });
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_gemm_vs_winograd_fp,
    bench_gemm_vs_winograd_gf2m8,
    bench_threshold_sweep
);
criterion_main!(benches);
