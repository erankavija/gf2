//! Benchmarks for sparse matrix operations over GF(2).
//!
//! # Sage Comparison
//!
//! Benchmarks marked with `[SAGE_CMP]` have equivalent implementations in
//! `scripts/sage_benchmarks.py` for comparison with SageMath sparse matrices.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use gf2_core::matrix::BitMatrix;
use gf2_core::sparse::{SpBitMatrix, SpBitMatrixDual};
use gf2_core::BitVec;
use rand::SeedableRng;

/// [SAGE_CMP] Benchmark sparse matrix-vector multiplication
///
/// Compare with Sage: `sparse_matrix * vector` over GF(2)
fn bench_sparse_matvec(c: &mut Criterion) {
    let mut group = c.benchmark_group("sparse_matvec");

    for &density in &[0.01, 0.05, 0.10] {
        for &size in &[100, 500, 1000] {
            let mut rng = rand::rngs::StdRng::seed_from_u64(42);
            let m = BitMatrix::random_with_probability(size, size, density, &mut rng);
            let s = SpBitMatrix::from_dense(&m);
            let x = BitVec::random(size, &mut rng);

            group.bench_with_input(
                BenchmarkId::new(format!("density_{:.2}", density), size),
                &(&s, &x),
                |b, (s, x)| b.iter(|| black_box(s.matvec(x))),
            );
        }
    }
    group.finish();
}

fn bench_dense_vs_sparse(c: &mut Criterion) {
    let mut group = c.benchmark_group("dense_vs_sparse_1pct");

    let size = 500;
    let density = 0.01;
    let mut rng = rand::rngs::StdRng::seed_from_u64(42);
    let m = BitMatrix::random_with_probability(size, size, density, &mut rng);
    let s = SpBitMatrix::from_dense(&m);
    let x = BitVec::random(size, &mut rng);

    group.bench_function("sparse_matvec", |b| b.iter(|| black_box(s.matvec(&x))));

    // Compare to dense matrix-vector via manual iteration
    group.bench_function("dense_manual_matvec", |b| {
        b.iter(|| {
            let mut y = BitVec::with_capacity(size);
            for r in 0..size {
                let mut acc = false;
                for c in 0..size {
                    if m.get(r, c) {
                        acc ^= x.get(c);
                    }
                }
                y.push_bit(acc);
            }
            black_box(y)
        })
    });

    group.finish();
}

/// [SAGE_CMP] Benchmark sparse matrix transpose
///
/// Compare with Sage: `matrix.transpose()` for sparse GF(2) matrices
fn bench_sparse_transpose(c: &mut Criterion) {
    let mut group = c.benchmark_group("sparse_transpose");

    for &density in &[0.01, 0.05] {
        for &size in &[100, 500, 1000] {
            let mut rng = rand::rngs::StdRng::seed_from_u64(42);
            let m = BitMatrix::random_with_probability(size, size, density, &mut rng);
            let s = SpBitMatrix::from_dense(&m);

            group.bench_with_input(
                BenchmarkId::new(format!("density_{:.2}", density), size),
                &s,
                |b, s| b.iter(|| black_box(s.transpose())),
            );
        }
    }
    group.finish();
}

fn bench_dual_col_iter_vs_transpose(c: &mut Criterion) {
    let mut group = c.benchmark_group("dual_col_access");

    let size = 500;
    let density = 0.01;
    let mut rng = rand::rngs::StdRng::seed_from_u64(42);
    let m = BitMatrix::random_with_probability(size, size, density, &mut rng);

    let single = SpBitMatrix::from_dense(&m);
    let dual = SpBitMatrixDual::from_dense(&m);

    // Single CSR: transpose on every column access
    group.bench_function("single_csr_transpose_per_col", |b| {
        b.iter(|| {
            let mut sum = 0;
            for c in 0..size {
                for _r in single.col_iter(c) {
                    sum += 1;
                }
            }
            black_box(sum)
        })
    });

    // Dual: direct column access via CSC
    group.bench_function("dual_direct_col_access", |b| {
        b.iter(|| {
            let mut sum = 0;
            for c in 0..size {
                for _r in dual.col_iter(c) {
                    sum += 1;
                }
            }
            black_box(sum)
        })
    });

    group.finish();
}

fn bench_bidirectional_sweep(c: &mut Criterion) {
    let mut group = c.benchmark_group("bidirectional_sweep");

    let size = 500;
    let density = 0.01;
    let mut rng = rand::rngs::StdRng::seed_from_u64(42);
    let m = BitMatrix::random_with_probability(size, size, density, &mut rng);

    let dual = SpBitMatrixDual::from_dense(&m);

    group.bench_function("alternating_row_col_sweeps", |b| {
        b.iter(|| {
            let mut sum = 0;
            // Row sweep
            for r in 0..dual.rows() {
                for _c in dual.row_iter(r) {
                    sum += 1;
                }
            }
            // Column sweep
            for c in 0..dual.cols() {
                for _r in dual.col_iter(c) {
                    sum += 1;
                }
            }
            black_box(sum)
        })
    });

    group.finish();
}

fn bench_dual_matvec_transpose(c: &mut Criterion) {
    let mut group = c.benchmark_group("dual_transpose_matvec");

    let size = 500;
    let density = 0.01;
    let mut rng = rand::rngs::StdRng::seed_from_u64(42);
    let m = BitMatrix::random_with_probability(size, size, density, &mut rng);

    let dual = SpBitMatrixDual::from_dense(&m);
    let x = BitVec::random(size, &mut rng);

    group.bench_function("matvec", |b| b.iter(|| black_box(dual.matvec(&x))));

    group.bench_function("matvec_transpose", |b| {
        b.iter(|| black_box(dual.matvec_transpose(&x)))
    });

    group.finish();
}

fn deterministic_ldpc_like(rows: usize, cols: usize, row_weight: usize) -> SpBitMatrix {
    let mut entries = Vec::with_capacity(rows * row_weight);
    for r in 0..rows {
        let base = r.wrapping_mul(1_315_423_911usize) ^ rows.rotate_left(7);
        for k in 0..row_weight {
            let stride = 2 * k + 1;
            let col = base
                .wrapping_add(k.wrapping_mul(97_531))
                .wrapping_add(r.wrapping_mul(stride))
                % cols;
            entries.push((r, col));
        }
    }
    SpBitMatrix::from_coo_deduplicated(rows, cols, &entries)
}

fn deterministic_bitvec(len: usize) -> BitVec {
    let mut x = BitVec::with_capacity(len);
    for i in 0..len {
        x.push_bit(((i.wrapping_mul(0x9E37_79B1) ^ (i >> 3)) & 7) < 3);
    }
    x
}

/// LDPC-sized opt-in block-CSR matvec benchmark.
///
/// Compare:
/// - `csr`: existing caller-visible scalar CSR path, unchanged.
/// - `block_csr_prefetch`: transformed block-CSR layout with predecoded bit
///   gathers and software prefetch.
/// - `block_csr_no_prefetch`: same layout with the prefetch distance set to 0,
///   used to isolate the prefetch hint from the cache-layout transform.
fn bench_ldpc_block_csr_matvec(c: &mut Criterion) {
    let mut group = c.benchmark_group("sparse_matvec_ldpc_block_csr");
    group.sample_size(10);
    group.measurement_time(std::time::Duration::from_secs(3));

    for &(rows, cols, row_weight) in &[
        (4096usize, 8192usize, 6usize),
        (8192, 16384, 6),
        (4096, 32768, 32),
    ] {
        let csr = deterministic_ldpc_like(rows, cols, row_weight);
        let block = csr.to_default_block_csr();
        let x = deterministic_bitvec(cols);
        let case = format!("{rows}x{cols}_w{row_weight}");

        group.bench_with_input(
            BenchmarkId::new("csr", &case),
            &(&csr, &x),
            |b, (csr, x)| b.iter(|| black_box(csr.matvec(x))),
        );
        group.bench_with_input(
            BenchmarkId::new("block_csr_prefetch", &case),
            &(&block, &x),
            |b, (block, x)| b.iter(|| black_box(block.matvec(x))),
        );
        group.bench_with_input(
            BenchmarkId::new("block_csr_no_prefetch", &case),
            &(&block, &x),
            |b, (block, x)| b.iter(|| black_box(block.matvec_with_prefetch_distance(x, 0))),
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_sparse_matvec,
    bench_dense_vs_sparse,
    bench_sparse_transpose,
    bench_dual_col_iter_vs_transpose,
    bench_bidirectional_sweep,
    bench_dual_matvec_transpose,
    bench_ldpc_block_csr_matvec
);
criterion_main!(benches);
