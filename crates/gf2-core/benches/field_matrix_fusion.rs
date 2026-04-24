//! Fused-vs-eager benchmark for the expression-template layer (issue
//! `7e6183bb`, story `d48a3cfd/T2`).
//!
//! Compares the canonical `A·B + C` fusion — one `gemm_with_beta` kernel
//! call, one allocation — against the eager two-step path:
//!
//! ```ignore
//! let t: FieldMatrix<F> = &a * &b;   // plain gemm, one alloc
//! let r: FieldMatrix<F> = &t + &c;   // axpy-linear, second alloc
//! ```
//!
//! Success criterion (issue 7e6183bb §5): the fused path must be
//! **measurably** faster and allocate less at n ∈ {256, 1024} for
//! Mersenne-31 (`Fp<2^31-1>`).
//!
//! ## Usage
//!
//! ```bash
//! cargo bench -p gf2-core --bench field_matrix_fusion
//! # Smoke-only.
//! cargo bench -p gf2-core --bench field_matrix_fusion -- --test
//! ```
//!
//! ## Results and allocation evidence
//!
//! See `benches/field_matrix_fusion_results.md` (checked into the repo)
//! for timing tables and the allocation-count breakdown. The companion
//! in-crate unit test `test_fused_path_allocates_fewer_matrices_than_eager`
//! (`crates/gf2-core/src/field/expr.rs`) asserts the allocation claim
//! directly via the `KernelCounts` trace counters without needing a
//! global allocator wrapper.

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
        // Throughput: n^3 field MACs per gemm.
        group.throughput(Throughput::Elements((n * n * n) as u64));
        let a = random_m31(n, n, 0xF1 ^ n as u64);
        let b = random_m31(n, n, 0xF2 ^ n as u64);
        let c_mat = random_m31(n, n, 0xF3 ^ n as u64);

        // Fused: (&a * &b + &c).into() → ONE gemm_with_beta kernel call.
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

        // Eager: `let t = &a * &b; let r = &t + &c;` → TWO kernel calls.
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
