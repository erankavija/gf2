use criterion::{black_box, criterion_group, criterion_main, Criterion};
use gf2_core::alg::m4rm::multiply;
use gf2_core::matrix::BitMatrix;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

fn random_matrix(rows: usize, cols: usize, seed: u64) -> BitMatrix {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut m = BitMatrix::zeros(rows, cols);
    for r in 0..rows {
        for c in 0..cols {
            if rng.gen_bool(0.5) {
                m.set(r, c, true);
            }
        }
    }
    m
}

fn bench_m4rm_1024(c: &mut Criterion) {
    let a = random_matrix(1024, 1024, 42);
    let b = random_matrix(1024, 1024, 43);

    c.bench_function("m4rm_1024x1024", |bench| {
        bench.iter(|| {
            let _result = multiply(black_box(&a), black_box(&b));
        });
    });
}

criterion_group!(benches, bench_m4rm_1024);
criterion_main!(benches);
