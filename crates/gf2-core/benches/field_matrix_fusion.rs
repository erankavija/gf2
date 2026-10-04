//! Benchmarks the fused `(&a * &b + &c).into()` expression against the eager
//! two-step product then sum on `Fp<2^31-1>`.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use gf2_core::field::matrix::FieldMatrix;
use gf2_core::gfp::Fp;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

const MERSENNE_31: u64 = 2_147_483_647;
type M31 = Fp<MERSENNE_31>;

const SIZES: &[usize] = &[256, 1024];

fn random_m31(rows: usize, cols: usize, seed: u64) -> FieldMatrix<M31> {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut m = FieldMatrix::<M31>::zeros(rows, cols);
    for r in 0..rows {
        for c in 0..cols {
            m.set(r, c, M31::new(rng.gen::<u64>() % MERSENNE_31));
        }
    }
    m
}

fn bench_fused_vs_eager_product_plus(c: &mut Criterion) {
    let mut group = c.benchmark_group("field_matrix_fusion/product_plus");
    for &n in SIZES {
        group.throughput(Throughput::Elements((n * n * n) as u64));
        let a = random_m31(n, n, 0xF1 ^ n as u64);
        let b = random_m31(n, n, 0xF2 ^ n as u64);
        let c_mat = random_m31(n, n, 0xF3 ^ n as u64);

        group.bench_with_input(
            BenchmarkId::new("fused_gemm_with_beta", n),
            &n,
            |bench, _| {
                bench.iter(|| {
                    let out: FieldMatrix<M31> =
                        (black_box(&a) * black_box(&b) + black_box(&c_mat)).into();
                    black_box(out);
                });
            },
        );

        group.bench_with_input(BenchmarkId::new("eager_two_step", n), &n, |bench, _| {
            bench.iter(|| {
                let t: FieldMatrix<M31> = (black_box(&a) * black_box(&b)).into();
                let r: FieldMatrix<M31> = (black_box(&t) + black_box(&c_mat)).into();
                black_box(r);
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_fused_vs_eager_product_plus);
criterion_main!(benches);
