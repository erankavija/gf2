use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use gf2::alg::m4rm::multiply;
use gf2::matrix::BitMatrix;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

fn random_matrix(rows: usize, cols: usize, seed: u64) -> BitMatrix {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut m = BitMatrix::new_zero(rows, cols);

    for r in 0..rows {
        for c in 0..cols {
            if rng.gen_bool(0.5) {
                m.set(r, c, true);
            }
        }
    }

    m
}

fn bench_matmul_square(c: &mut Criterion) {
    let mut group = c.benchmark_group("matmul_square");

    for size in [64, 128, 256, 512, 1024].iter() {
        let a = random_matrix(*size, *size, 42);
        let b = random_matrix(*size, *size, 43);

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |bench, _| {
            bench.iter(|| {
                let _result = multiply(black_box(&a), black_box(&b));
            });
        });
    }

    group.finish();
}

fn bench_matmul_rectangular(c: &mut Criterion) {
    let mut group = c.benchmark_group("matmul_rectangular");

    let configs = [(100, 200, 100), (256, 128, 256), (512, 256, 512)];

    for (m, k, n) in configs.iter() {
        let a = random_matrix(*m, *k, 100);
        let b = random_matrix(*k, *n, 101);

        let label = format!("{}x{}x{}", m, k, n);
        group.bench_with_input(BenchmarkId::new("dims", &label), &label, |bench, _| {
            bench.iter(|| {
                let _result = multiply(black_box(&a), black_box(&b));
            });
        });
    }

    group.finish();
}

criterion_group!(benches, bench_matmul_square, bench_matmul_rectangular);
criterion_main!(benches);
