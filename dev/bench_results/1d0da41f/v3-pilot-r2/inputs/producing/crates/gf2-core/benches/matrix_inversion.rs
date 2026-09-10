use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use gf2_core::alg::gauss::invert;
use gf2_core::matrix::BitMatrix;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

fn random_invertible_matrix(n: usize, seed: u64) -> BitMatrix {
    let mut rng = StdRng::seed_from_u64(seed);

    // Try up to 10 times to generate an invertible matrix
    for _ in 0..10 {
        let mut m = BitMatrix::zeros(n, n);
        for r in 0..n {
            for c in 0..n {
                if rng.gen_bool(0.5) {
                    m.set(r, c, true);
                }
            }
        }

        // Quick check: try to invert
        if invert(&m).is_some() {
            return m;
        }
    }

    // Fallback: identity + small perturbation (guaranteed invertible)
    let mut m = BitMatrix::identity(n);
    if n > 1 {
        m.set(0, 1, true); // Make it non-trivial
    }
    m
}

fn bench_inversion(c: &mut Criterion) {
    let mut group = c.benchmark_group("matrix_inversion");

    for size in [64, 128, 256, 512, 1024].iter() {
        let m = random_invertible_matrix(*size, 42 + *size as u64);

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |bench, _| {
            bench.iter(|| {
                let _inv = invert(black_box(&m));
            });
        });
    }

    group.finish();
}

fn bench_inversion_success_rate(c: &mut Criterion) {
    // Measure how often random matrices are invertible
    let mut group = c.benchmark_group("inversion_success_rate");

    for size in [64, 128, 256].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |bench, &n| {
            let mut rng = StdRng::seed_from_u64(12345);
            bench.iter(|| {
                let mut m = BitMatrix::zeros(n, n);
                for r in 0..n {
                    for c in 0..n {
                        if rng.gen_bool(0.5) {
                            m.set(r, c, true);
                        }
                    }
                }
                let result = invert(black_box(&m));
                black_box(result.is_some())
            });
        });
    }

    group.finish();
}

criterion_group!(benches, bench_inversion, bench_inversion_success_rate);
criterion_main!(benches);
