use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use gf2_core::alg::m4rm::{multiply, multiply_with_table_schedule_for_test};
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

fn bench_gray_schedule(c: &mut Criterion) {
    let mut group = c.benchmark_group("m4rm_gray_schedule");
    group.sample_size(10);

    for size in [1024, 2048, 4096] {
        let a = random_matrix(size, size, 0x380e_041a);
        let b = random_matrix(size, size, 0x380e_041b);

        group.bench_with_input(
            BenchmarkId::new("production_auto", size),
            &size,
            |bench, _| {
                bench.iter(|| {
                    let _result = multiply(black_box(&a), black_box(&b));
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("legacy_64k_max8", size),
            &size,
            |bench, _| {
                bench.iter(|| {
                    let _result = multiply_with_table_schedule_for_test(
                        black_box(&a),
                        black_box(&b),
                        64 * 1024,
                        8,
                    );
                });
            },
        );

        for (label, target_bytes) in [
            ("gray_64k_max10", 64 * 1024),
            ("gray_128k_max10", 128 * 1024),
            ("gray_256k_max10", 256 * 1024),
            ("gray_512k_max10", 512 * 1024),
        ] {
            group.bench_with_input(BenchmarkId::new(label, size), &size, |bench, _| {
                bench.iter(|| {
                    let _result = multiply_with_table_schedule_for_test(
                        black_box(&a),
                        black_box(&b),
                        target_bytes,
                        10,
                    );
                });
            });
        }
    }

    group.finish();
}

criterion_group!(benches, bench_gray_schedule);
criterion_main!(benches);
