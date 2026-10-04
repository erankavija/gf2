//! Benchmarks classical `gemm` against `gemm_winograd` over `Fp<MERSENNE_31>`
//! and GF(2^8), and sweeps the Winograd recursion threshold at `n = 2048`
//! through [`gemm_winograd_with_threshold`].

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use gf2_core::field::matrix::{gemm, FieldMatrix};
use gf2_core::field::winograd::{gemm_winograd, gemm_winograd_with_threshold};
use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
use gf2_core::gfp::Fp;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

const MERSENNE_31: u64 = 2_147_483_647;

/// GF(2^8) with the AES polynomial `x^8 + x^4 + x^3 + x + 1` (`@/citation/Nist2001`).
struct StrassenGf2m8Cfg;
impl Gf2mWideConfig<1> for StrassenGf2m8Cfg {
    const M: usize = 8;
    const MODULUS: [u64; 1] = [0x1B];
    const NAME: &'static str = "StrassenGf2m8Cfg";
}
type Gf2m8 = Gf2mWide<1, StrassenGf2m8Cfg>;

// Every size exceeds the conservative `gemm.winograd_min_dim` of 128, so the
// Winograd recursion runs.
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
