//! Benchmarks for [`gf2_core::field::FieldPoly`] batch evaluation.
//!
//! Compares the subproduct-tree [`FieldPoly::batch_evaluate`] against the
//! naive per-point Horner baseline (`points.iter().map(|p| self.eval(p)).collect()`)
//! on `Fp<65537>` for the matrix
//! `n ∈ {16, 64, 256, 1024} × k ∈ {16, 64, 256, 1024}`.
//!
//! The success criterion from the issue spec is that the subproduct-tree
//! path beats `k` individual Horner folds at `k >= 16 && n >= 16`.
//! The measured results are committed into the module docstring of
//! [`gf2_core::field::poly`] so callers can consult them without
//! re-running the benchmark.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use gf2_core::field::poly::batch_evaluate_subproduct;
use gf2_core::field::FieldPoly;
use gf2_core::gfp::Fp;

type F = Fp<65537>;

/// Build a deterministic polynomial of length `n` (degree `n - 1`) with
/// non-zero leading coefficient. Pattern matters only for coverage; the
/// benchmark measures arithmetic cost, not data-dependent branches.
fn make_poly(n: usize) -> FieldPoly<F> {
    let modulus: u64 = 65537;
    let mut coeffs: Vec<F> = (0..n)
        .map(|i| {
            let v = ((i as u64).wrapping_mul(2_654_435_761) % (modulus - 1)) + 1;
            F::new(v)
        })
        .collect();
    // Guarantee a non-zero leading coefficient so the polynomial has the
    // nominal degree and we don't accidentally evaluate a trimmed-down
    // version below the target size.
    *coeffs.last_mut().unwrap() = F::new(1);
    FieldPoly::new(coeffs)
}

/// Build `k` deterministic evaluation points. Pattern chosen to avoid
/// clustering on repeated residues: the subproduct tree's reduction cost
/// is effectively insensitive to duplication (each node is a linear
/// modulus) but we keep the points distinct for realism.
fn make_points(k: usize) -> Vec<F> {
    let modulus: u64 = 65537;
    (0..k)
        .map(|i| F::new(((i as u64).wrapping_mul(1_000_003) % (modulus - 1)) + 1))
        .collect()
}

fn bench_batch_evaluate(c: &mut Criterion) {
    let mut group = c.benchmark_group("field_poly_batch_evaluate_fp65537");
    // `batch_evaluate` includes a subproduct-tree build; keep sample size
    // modest so the full matrix fits well under the 60s test-suite
    // budget when run via `--quick`.
    group.sample_size(20);

    let ns = [16usize, 64, 256, 1024];
    let ks = [16usize, 64, 256, 1024];

    for &n in &ns {
        let poly = make_poly(n);
        for &k in &ks {
            let points = make_points(k);
            let id_fmt = format!("n{n}_k{k}");

            // The "subproduct" arm bypasses the SUBPRODUCT_THRESHOLD gate
            // so we always measure the fast-path cost even on sizes where
            // `batch_evaluate` would fall back to the naive loop. The
            // "naive" arm is the literal spec from the issue: k
            // independent Horner folds collected into a `Vec<F>`.
            group.bench_with_input(
                BenchmarkId::new("subproduct", &id_fmt),
                &(&poly, &points),
                |b, (p, xs)| {
                    b.iter(|| black_box(batch_evaluate_subproduct(black_box(*p), black_box(*xs))));
                },
            );

            group.bench_with_input(
                BenchmarkId::new("naive", &id_fmt),
                &(&poly, &points),
                |b, (p, xs)| {
                    b.iter(|| {
                        let out: Vec<F> = xs.iter().map(|x| p.eval(x)).collect();
                        black_box(out)
                    });
                },
            );
        }
    }

    group.finish();
}

criterion_group!(benches, bench_batch_evaluate);
criterion_main!(benches);
