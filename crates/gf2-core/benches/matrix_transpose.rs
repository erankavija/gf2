//! Benchmarks for [`BitMatrix::transpose`] across the PPC-spiral B1 design
//! sizes.
//!
//! The B1 kernel under `dev/plans/gf2_core_ppc_spiral.md` § Tier B is a
//! 64×64 bit-block transpose with an AVX2 PSHUFB lane and a Hacker's
//! Delight scalar fallback. Per the Tier-B "PPC walk" rule, this bench
//! exists as the V0 baseline so subsequent SIMD edits can be measured
//! against a pinned criterion baseline (see
//! `dev/scripts/ppc-baselines.json` entry `B1`).
//!
//! Sizes follow the manifest's `design_size_class` (1024, 4096) and
//! extend down to 64/256 to capture the pure register-tile regime where
//! cache and memory bandwidth do not dominate.
//!
//! Run:
//!
//! ```text
//! cargo bench -p gf2-core --bench matrix_transpose
//! ```
//!
//! Pin V0:
//!
//! ```text
//! cargo bench -p gf2-core --bench matrix_transpose -- \
//!     --save-baseline ppc-v0-2026-04-27
//! ```

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use gf2_core::BitMatrix;

/// Benchmark dense bit-matrix transpose across PPC B1 design sizes.
fn bench_transpose(c: &mut Criterion) {
    let mut group = c.benchmark_group("matrix_transpose");

    // Tier B1 design sizes per dev/scripts/ppc-baselines.json.
    // 64 and 256 are below the manifest size class but kept so the bench
    // also covers the register-tile / single-quadrant regimes where the
    // PSHUFB path is expected to dominate. The 1024 and 4096 leaves are
    // the criterion-1.5x gate's measurement points.
    for &size in &[64usize, 256, 1024, 4096] {
        let m = BitMatrix::random_seeded(size, size, 0x42);

        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            b.iter(|| black_box(m.transpose()))
        });
    }
    group.finish();
}

/// Benchmark bit-matrix transpose at word boundaries.
///
/// These leaves are not in the gate manifest but stress edge cases that
/// most often surface lane-tail or 64-word-block bugs in SIMD kernels.
fn bench_transpose_word_boundaries(c: &mut Criterion) {
    let mut group = c.benchmark_group("matrix_transpose_word_boundaries");

    for &size in &[63usize, 64, 65, 127, 128, 129] {
        let m = BitMatrix::random_seeded(size, size, 0x42);

        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            b.iter(|| black_box(m.transpose()))
        });
    }
    group.finish();
}

/// Benchmark rectangular (non-square) transposes — common in coding-theory
/// generator/parity matrices.
fn bench_transpose_rectangular(c: &mut Criterion) {
    let mut group = c.benchmark_group("matrix_transpose_rectangular");

    for &(rows, cols) in &[(256usize, 1024), (1024, 256), (512, 4096), (4096, 512)] {
        let m = BitMatrix::random_seeded(rows, cols, 0x42);

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}x{}", rows, cols)),
            &(rows, cols),
            |b, _| b.iter(|| black_box(m.transpose())),
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_transpose,
    bench_transpose_word_boundaries,
    bench_transpose_rectangular,
);
criterion_main!(benches);
