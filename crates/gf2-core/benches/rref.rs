use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use gf2_core::alg::rref::{rref, rref_with_block_size_for_test};
use gf2_core::matrix::BitMatrix;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

#[path = "common/seed.rs"]
mod seed;

use seed::{bitmatrix_from_seed, bitmatrix_rank_deficient_from_seed, derive_seed};

/// Generate a random matrix for RREF benchmarking
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

fn bench_rref_standard_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("rref_standard");

    // Standard square matrix sizes
    for size in [256, 512, 1024, 2048].iter() {
        let m = random_matrix(*size, *size, 42 + *size as u64);

        group.bench_with_input(
            BenchmarkId::new("square", format!("{}x{}", size, size)),
            size,
            |bench, _| {
                bench.iter(|| {
                    let _result = rref(black_box(&m), false);
                });
            },
        );
    }

    group.finish();
}

fn bench_gf2_echelon_target_rows(c: &mut Criterion) {
    let mut group = c.benchmark_group("gf2_echelon_target_rows");
    group.sample_size(10);

    const MASTER: u64 = 0x6F73_AC91_D31E_4A7C;
    for (si, &n) in [64usize, 256, 1024].iter().enumerate() {
        for (ri, &(regime, deficient)) in
            [("uniform", false), ("deficient", true)].iter().enumerate()
        {
            let seed = derive_seed(MASTER, "echelon", 13, si as u64, ri as u64);
            let matrix = if deficient {
                bitmatrix_rank_deficient_from_seed(n, n, n / 2, seed)
            } else {
                bitmatrix_from_seed(n, n, seed)
            };

            group.bench_with_input(
                BenchmarkId::new("baseline_block1", format!("{n}/{regime}")),
                &matrix,
                |bench, matrix| {
                    bench.iter(|| {
                        black_box(rref_with_block_size_for_test(black_box(matrix), false, 1));
                    });
                },
            );

            group.bench_with_input(
                BenchmarkId::new("production_blocked", format!("{n}/{regime}")),
                &matrix,
                |bench, matrix| {
                    bench.iter(|| {
                        black_box(rref(black_box(matrix), false));
                    });
                },
            );
        }
    }

    group.finish();
}

fn bench_rref_rectangular(c: &mut Criterion) {
    let mut group = c.benchmark_group("rref_rectangular");

    // Rectangular matrices (common in coding theory)
    let test_cases = vec![
        (100, 200, "100x200"),
        (500, 1000, "500x1000"),
        (1000, 2000, "1000x2000"),
    ];

    for (rows, cols, label) in test_cases {
        let m = random_matrix(rows, cols, 42 + rows as u64);

        group.bench_with_input(
            BenchmarkId::new("rect", label),
            &(rows, cols),
            |bench, _| {
                bench.iter(|| {
                    let _result = rref(black_box(&m), false);
                });
            },
        );
    }

    group.finish();
}

fn bench_rref_dvb_t2(c: &mut Criterion) {
    let mut group = c.benchmark_group("rref_dvb_t2");

    // DVB-T2 LDPC matrix sizes
    // Note: These are large and slow - sample size will be reduced
    group.sample_size(10); // Reduce from default 100

    // DVB-T2 Short Rate 3/5: 6,480 × 16,200
    // M4RI baseline: 142.29 ms
    // Target: <1 second (7x slower than M4RI is acceptable)
    let m_short_35 = random_matrix(6480, 16200, 42);
    group.bench_function("dvb_t2_short_rate_3_5", |bench| {
        bench.iter(|| {
            let _result = rref(black_box(&m_short_35), true);
        });
    });

    // DVB-T2 Short Rate 1/2: 9,000 × 16,200
    // M4RI baseline: 241.18 ms
    // Target: <1 second (4x slower than M4RI is acceptable)
    let m_short_12 = random_matrix(9000, 16200, 43);
    group.bench_function("dvb_t2_short_rate_1_2", |bench| {
        bench.iter(|| {
            let _result = rref(black_box(&m_short_12), true);
        });
    });

    // DVB-T2 Normal is too large for regular benchmarking
    // Will be tested separately in integration tests

    group.finish();
}

criterion_group!(
    benches,
    bench_gf2_echelon_target_rows,
    bench_rref_standard_sizes,
    bench_rref_rectangular,
    bench_rref_dvb_t2
);
criterion_main!(benches);
