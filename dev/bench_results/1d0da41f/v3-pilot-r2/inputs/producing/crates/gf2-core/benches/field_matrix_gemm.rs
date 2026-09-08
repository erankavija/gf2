//! Micro-benchmarks for `FieldMatrix::gemm` at n = 64, 256, 1024.
//!
//! Matches the §5 success criterion of issue `91c06222` (epic `bb85c68a`):
//! measure the classical blocked gemm on a 32-bit Mersenne prime
//! (`Fp<2^31-1>`) and a binary field (`Gf2mWide<1, AES-GF(2^8)>`). Both
//! fields inherit SIMD via `FieldVec::dot_product` from epic `e095a100`.
//!
//! ## Usage
//!
//! ```bash
//! # Full run.
//! cargo bench -p gf2-core --bench field_matrix_gemm
//! # Smoke-only (verifies the bench harness compiles and executes).
//! cargo bench -p gf2-core --bench field_matrix_gemm -- --test
//! ```
//!
//! The benchmark reports (by default) wall-clock time per matrix
//! multiplication and criterion's derived throughput. Follow-up benchmark
//! work (including fflas-ffpack baseline comparison) lives in issue
//! `64c88ae4` and its children.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use gf2_core::field::matrix::FieldMatrix;
use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
use gf2_core::gfp::Fp;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

// Mersenne-31 prime.
const MERSENNE_31: u64 = 2_147_483_647;

/// GF(2^8) with AES irreducible `x^8 + x^4 + x^3 + x + 1`. Implicit leading
/// bit convention per `Gf2mWideConfig` ⇒ low byte is `0x1B`.
struct Gf2m8AesCfg;
impl Gf2mWideConfig<1> for Gf2m8AesCfg {
    const M: usize = 8;
    const MODULUS: [u64; 1] = [0x1B];
    const NAME: &'static str = "Gf2m8AesCfg";
}
type Gf2m8 = Gf2mWide<1, Gf2m8AesCfg>;

const SIZES: &[usize] = &[64, 256, 1024];

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

fn bench_gemm_fp_mersenne31(c: &mut Criterion) {
    let mut group = c.benchmark_group("field_matrix_gemm/fp_mersenne31");
    for &n in SIZES {
        // Throughput: n^3 finite-field MACs per gemm.
        group.throughput(Throughput::Elements((n * n * n) as u64));
        let a = random_fp_matrix::<MERSENNE_31>(n, n, 0xAA ^ n as u64);
        let b = random_fp_matrix::<MERSENNE_31>(n, n, 0xBB ^ n as u64);
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |bench, _| {
            bench.iter(|| {
                let out: FieldMatrix<_> = (black_box(&a) * black_box(&b)).into();
                black_box(out);
            });
        });
    }
    group.finish();
}

fn bench_gemm_gf2m8(c: &mut Criterion) {
    let mut group = c.benchmark_group("field_matrix_gemm/gf2m8_aes");
    for &n in SIZES {
        group.throughput(Throughput::Elements((n * n * n) as u64));
        let a = random_gf2m8_matrix(n, n, 0xCC ^ n as u64);
        let b = random_gf2m8_matrix(n, n, 0xDD ^ n as u64);
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |bench, _| {
            bench.iter(|| {
                let out: FieldMatrix<_> = (black_box(&a) * black_box(&b)).into();
                black_box(out);
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_gemm_fp_mersenne31, bench_gemm_gf2m8);
criterion_main!(benches);
