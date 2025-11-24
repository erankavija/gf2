use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
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

// Benchmark table generation (need to expose this function)
fn bench_table_generation(c: &mut Criterion) {
    let b = random_matrix(1024, 1024, 42);

    c.bench_function("gray_table_generation_k8", |bench| {
        bench.iter(|| {
            // Simulate building a gray table
            // Since build_gray_table is private, we measure the pattern
            let k_block = 8;
            let table_size = 1usize << k_block;
            let stride_words = 1024_usize.div_ceil(64);
            let mut table = vec![vec![0u64; stride_words]; table_size];

            // Binary enumeration (current implementation)
            for (idx, entry) in table.iter_mut().enumerate() {
                for bit in 0..k_block {
                    if (idx & (1 << bit)) != 0 && bit < b.rows() {
                        let row_words = b.row_words(bit);
                        // Simulate XOR
                        for (dst, &src) in entry.iter_mut().zip(row_words) {
                            *dst ^= src;
                        }
                    }
                }
            }
            black_box(&table);
        });
    });
}

// Benchmark bit extraction pattern
fn bench_bit_extraction(c: &mut Criterion) {
    let a = random_matrix(1024, 1024, 42);

    c.bench_function("extract_bits_k8", |bench| {
        bench.iter(|| {
            let mut sum = 0usize;
            for row in 0..1024 {
                // Extract 8 bits
                let mut result = 0usize;
                for bit_idx in 0..8 {
                    let col = bit_idx;
                    if col < a.cols() && a.get(row, col) {
                        result |= 1usize << bit_idx;
                    }
                }
                sum = sum.wrapping_add(result);
            }
            black_box(sum);
        });
    });
}

// Benchmark overall M4RM multiplication for different sizes
fn bench_m4rm_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("m4rm_multiply");

    for size in [256, 512, 1024].iter() {
        let a = random_matrix(*size, *size, 42);
        let b = random_matrix(*size, *size, 43);

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |bench, _| {
            bench.iter(|| {
                let _c = gf2_core::alg::m4rm::multiply(black_box(&a), black_box(&b));
            });
        });
    }

    group.finish();
}

// Benchmark just the table lookup + XOR accumulation pattern
fn bench_table_lookup(c: &mut Criterion) {
    let stride_words = 1024_usize.div_ceil(64);
    let table_size = 256; // k_block = 8

    // Create a dummy table
    let table: Vec<Vec<u64>> = (0..table_size)
        .map(|_| (0..stride_words).map(|_| rand::random()).collect())
        .collect();

    let mut result_row = vec![0u64; stride_words];

    c.bench_function("table_lookup_and_xor", |bench| {
        bench.iter(|| {
            // Simulate 1024 table lookups (one per row of A)
            for idx in 0..1024 {
                let table_idx = idx % table_size;
                for (dst, &src) in result_row.iter_mut().zip(&table[table_idx]) {
                    *dst ^= src;
                }
            }
            black_box(&result_row);
        });
    });
}

criterion_group!(
    benches,
    bench_table_generation,
    bench_bit_extraction,
    bench_table_lookup,
    bench_m4rm_sizes
);
criterion_main!(benches);
