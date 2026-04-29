use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use gf2_core::alg::m4rm::multiply as m4rm_multiply;
use gf2_core::kernels::scalar::SCALAR_BACKEND;
use gf2_core::kernels::Backend;
use gf2_core::matrix::BitMatrix;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

const FALLBACK_ROWS: usize = 512;
const FALLBACK_INNER: usize = 1;
const FALLBACK_COLS: usize = 8_192;

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

fn row_xor_fallback_inputs() -> (BitMatrix, BitMatrix) {
    // Production-path proof for this benchmark:
    // - choose_k_block(k=1, n=8192) must return 1 because the M4RM selector
    //   cannot choose any k_block > k.
    // - n=8192 is 128 words, well above the >=8-word SIMD dispatch threshold.
    assert_eq!(FALLBACK_INNER, 1);
    assert!(FALLBACK_COLS.div_ceil(64) >= 8);

    let mut lhs = BitMatrix::zeros(FALLBACK_ROWS, FALLBACK_INNER);
    for row in 0..FALLBACK_ROWS {
        lhs.set(row, 0, true);
    }

    let rhs = random_matrix(FALLBACK_INNER, FALLBACK_COLS, 0x5223_bb04);
    (lhs, rhs)
}

#[inline(never)]
fn scalar_backend_row_xor_mul(lhs: &BitMatrix, rhs: &BitMatrix) -> BitMatrix {
    assert_eq!(lhs.cols(), rhs.rows());

    let mut out = BitMatrix::zeros(lhs.rows(), rhs.cols());
    if lhs.rows() == 0 || lhs.cols() == 0 || rhs.cols() == 0 {
        return out;
    }

    for row in 0..lhs.rows() {
        let lhs_row = lhs.row_words(row);
        let out_row = out.row_words_mut(row);

        for (word_idx, &word) in lhs_row.iter().enumerate() {
            let mut bits = word;
            while bits != 0 {
                let bit = bits.trailing_zeros() as usize;
                let rhs_row = (word_idx << 6) + bit;
                if rhs_row < lhs.cols() {
                    SCALAR_BACKEND.xor(out_row, rhs.row_words(rhs_row));
                }
                bits &= bits - 1;
            }
        }
    }

    out
}

fn bench_matmul_square(c: &mut Criterion) {
    let mut group = c.benchmark_group("matmul_square");

    for size in [64, 128, 256, 512, 1024, 2048, 4096].iter() {
        let a = random_matrix(*size, *size, 42);
        let b = random_matrix(*size, *size, 43);

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |bench, _| {
            bench.iter(|| {
                let _result = m4rm_multiply(black_box(&a), black_box(&b));
            });
        });
    }

    group.finish();
}

fn bench_matmul_square_strassen_compare(c: &mut Criterion) {
    let mut group = c.benchmark_group("matmul_square_strassen_compare");
    group.sample_size(10);

    for size in [1024, 2048, 4096, 8192].iter() {
        let a = random_matrix(*size, *size, 0x59c4_87c3);
        let b = random_matrix(*size, *size, 0x59c4_87c4);

        group.bench_with_input(BenchmarkId::new("m4rm_base", size), size, |bench, _| {
            bench.iter(|| {
                let _result = m4rm_multiply(black_box(&a), black_box(&b));
            });
        });

        group.bench_with_input(BenchmarkId::new("auto_dispatch", size), size, |bench, _| {
            bench.iter(|| {
                let _result = black_box(&a) * black_box(&b);
            });
        });

        if *size >= 2048 {
            group.bench_with_input(
                BenchmarkId::new("strassen_forced_1", size),
                size,
                |bench, _| {
                    bench.iter(|| {
                        let _result =
                            black_box(&a).strassen_mul_for_test(black_box(&b), size / 2, 1);
                    });
                },
            );
        }
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
                let _result = m4rm_multiply(black_box(&a), black_box(&b));
            });
        });
    }

    group.finish();
}

fn bench_row_xor_fallback(c: &mut Criterion) {
    let mut group = c.benchmark_group("matmul_row_xor_fallback");
    let (lhs, rhs) = row_xor_fallback_inputs();

    group.bench_function(BenchmarkId::new("scalar_backend", "512x1x8192"), |bench| {
        bench.iter(|| {
            let _result = scalar_backend_row_xor_mul(black_box(&lhs), black_box(&rhs));
        });
    });

    group.bench_function(BenchmarkId::new("dispatch_helper", "512x1x8192"), |bench| {
        bench.iter(|| {
            let _result = black_box(&lhs).mul_row_xor_for_test(black_box(&rhs));
        });
    });

    group.bench_function(BenchmarkId::new("production_mul", "512x1x8192"), |bench| {
        bench.iter(|| {
            let _result = m4rm_multiply(black_box(&lhs), black_box(&rhs));
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_matmul_square,
    bench_matmul_square_strassen_compare,
    bench_matmul_rectangular,
    bench_row_xor_fallback
);
criterion_main!(benches);
