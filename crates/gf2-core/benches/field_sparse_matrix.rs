//! Benchmarks [`SparseFieldMatrix`] `matvec` at densities 1% and 5% for
//! `n ∈ {256, 1024, 4096}` over `Fp<2^31-1>` and GF(2^8).

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use gf2_core::field::matrix::FieldMatrix;
use gf2_core::field::sparse_matrix::SparseFieldMatrix;
use gf2_core::field::FieldVec;
use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
use gf2_core::gfp::Fp;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

const MERSENNE_31: u64 = 2_147_483_647;

/// GF(2^8) with the AES polynomial `x^8 + x^4 + x^3 + x + 1`
/// (`@/citation/Nist2001`); the leading bit is implicit, so `MODULUS` is `0x1B`.
struct Gf2m8AesCfg;
impl Gf2mWideConfig<1> for Gf2m8AesCfg {
    const M: usize = 8;
    const MODULUS: [u64; 1] = [0x1B];
    const NAME: &'static str = "Gf2m8AesCfg";
}
type Gf2m8 = Gf2mWide<1, Gf2m8AesCfg>;

const SIZES: &[usize] = &[256, 1024, 4096];
const DENSITIES: &[f64] = &[0.01, 0.05];

fn random_sparse_fp<const P: u64>(
    rows: usize,
    cols: usize,
    density: f64,
    seed: u64,
) -> SparseFieldMatrix<Fp<P>> {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut m = FieldMatrix::<Fp<P>>::zeros(rows, cols);
    for r in 0..rows {
        for c in 0..cols {
            if rng.gen::<f64>() < density {
                let v = (rng.gen::<u64>() % (P - 1)) + 1;
                m.set(r, c, Fp::<P>::new(v));
            }
        }
    }
    SparseFieldMatrix::from_dense(&m)
}

fn random_sparse_gf2m8(
    rows: usize,
    cols: usize,
    density: f64,
    seed: u64,
) -> SparseFieldMatrix<Gf2m8> {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut m = FieldMatrix::<Gf2m8>::zeros(rows, cols);
    for r in 0..rows {
        for c in 0..cols {
            if rng.gen::<f64>() < density {
                let w = (rng.gen::<u64>() & 0xFF).max(1);
                m.set(r, c, Gf2m8::new([w]));
            }
        }
    }
    SparseFieldMatrix::from_dense(&m)
}

fn random_vec_fp<const P: u64>(n: usize, seed: u64) -> FieldVec<Fp<P>> {
    let mut rng = StdRng::seed_from_u64(seed);
    (0..n).map(|_| Fp::<P>::new(rng.gen::<u64>() % P)).collect()
}

fn random_vec_gf2m8(n: usize, seed: u64) -> FieldVec<Gf2m8> {
    let mut rng = StdRng::seed_from_u64(seed);
    (0..n)
        .map(|_| Gf2m8::new([rng.gen::<u64>() & 0xFF]))
        .collect()
}

fn bench_spmv_mersenne31(c: &mut Criterion) {
    let mut group = c.benchmark_group("field_sparse_matrix/spmv_mersenne31");
    for &density in DENSITIES {
        for &n in SIZES {
            let a = random_sparse_fp::<MERSENNE_31>(n, n, density, 0xA1 ^ n as u64);
            let x = random_vec_fp::<MERSENNE_31>(n, 0xA2 ^ n as u64);
            let id = BenchmarkId::new(format!("density_{density:.2}"), n);
            group.bench_with_input(id, &n, |bench, _| {
                bench.iter(|| {
                    let y = black_box(&a).matvec(black_box(&x));
                    black_box(y);
                });
            });
        }
    }
    group.finish();
}

fn bench_spmv_gf2m8(c: &mut Criterion) {
    let mut group = c.benchmark_group("field_sparse_matrix/spmv_gf2m8");
    for &density in DENSITIES {
        for &n in SIZES {
            let a = random_sparse_gf2m8(n, n, density, 0xB1 ^ n as u64);
            let x = random_vec_gf2m8(n, 0xB2 ^ n as u64);
            let id = BenchmarkId::new(format!("density_{density:.2}"), n);
            group.bench_with_input(id, &n, |bench, _| {
                bench.iter(|| {
                    let y = black_box(&a).matvec(black_box(&x));
                    black_box(y);
                });
            });
        }
    }
    group.finish();
}

criterion_group!(benches, bench_spmv_mersenne31, bench_spmv_gf2m8);
criterion_main!(benches);
