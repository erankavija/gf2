//! Benchmarks for [`gf2_core::field::FieldPoly`] batch evaluation.
//!
//! Benchmarks four implementations on `Fp<65537>` across the matrix
//! `n ∈ {16, 64, 256, 1024, 4096} × k ∈ {16, 64, 256, 1024, 4096}`:
//!
//! - **`dispatcher`** — the public [`FieldPoly::batch_evaluate`] API,
//!   which routes to the naive per-point Horner path below
//!   `SUBPRODUCT_THRESHOLD = 4096` and to the
//!   schoolbook-[`FieldPoly::div_rem`] subproduct tree above it.
//! - **`subproduct`** — the raw subproduct-tree path
//!   ([`batch_evaluate_subproduct`], bypassing the threshold gate).
//!   Always uses schoolbook [`FieldPoly::div_rem`] for per-node
//!   reductions.
//! - **`subproduct_auto`** — the [`TwoAdicField`]-specialised
//!   subproduct-tree path
//!   ([`batch_evaluate_subproduct_auto`]) that routes per-node
//!   reductions through [`FieldPoly::div_rem_auto`], picking up the
//!   Newton-iteration fast-division primitive (issue `ae0c7e1f`,
//!   `DIV_REM_THRESHOLD = 2048`) above the fast-division threshold.
//! - **`naive`** — the literal comparison point from the issue scope:
//!   `points.iter().map(|x| poly.eval(x)).collect()` — `k` independent
//!   Horner folds collected into a `Vec<F>`.
//!
//! This harness drove the tuning of `SUBPRODUCT_THRESHOLD` under
//! issue `046f95c1`. On the cheap-scalar `Fp<65537>` field naive
//! Horner wins on every cell except the (`n = 4096`, `k = 4096`)
//! corner, where `subproduct_auto` pulls ahead at `0.89×` of naive
//! (54.29 ms vs 60.89 ms on the latest rerun; the `dispatcher`
//! at that cell lands at ~80 ms because it routes through the
//! schoolbook `subproduct` arm, not the `_auto` winner). That
//! crossover pins the threshold at the smallest power of two above
//! the measured boundary (`4096`).
//!
//! The measured results are committed into the module docstring of
//! [`gf2_core::field::poly`] so callers can consult them without
//! re-running the benchmark.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use gf2_core::field::poly::{batch_evaluate_subproduct, batch_evaluate_subproduct_auto};
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

    let ns = [16usize, 64, 256, 1024, 4096];
    let ks = [16usize, 64, 256, 1024, 4096];

    for &n in &ns {
        let poly = make_poly(n);
        for &k in &ks {
            let points = make_points(k);
            let id_fmt = format!("n{n}_k{k}");

            // "dispatcher" arm: the public FieldPoly::batch_evaluate
            // API — what ordinary callers exercise. Below
            // SUBPRODUCT_THRESHOLD (4096) it delegates to naive
            // Horner; at or above the threshold it uses
            // batch_evaluate_subproduct (schoolbook FieldPoly::div_rem).
            // TwoAdicField callers who want the div_rem_auto-backed
            // dispatcher should call FieldPoly::batch_evaluate_auto
            // (see the "subproduct_auto" arm below for the underlying
            // free function).
            group.bench_with_input(
                BenchmarkId::new("dispatcher", &id_fmt),
                &(&poly, &points),
                |b, (p, xs)| {
                    b.iter(|| black_box(p.batch_evaluate(xs)));
                },
            );

            // "subproduct" arm: bypasses the SUBPRODUCT_THRESHOLD gate
            // so we always measure the fast-path cost, even on sizes
            // where `batch_evaluate` currently falls back to the naive
            // loop.
            group.bench_with_input(
                BenchmarkId::new("subproduct", &id_fmt),
                &(&poly, &points),
                |b, (p, xs)| {
                    b.iter(|| black_box(batch_evaluate_subproduct(black_box(*p), black_box(*xs))));
                },
            );

            // "subproduct_auto" arm: the TwoAdicField-specialised
            // variant that routes per-node reductions through
            // `FieldPoly::div_rem_auto`. Measures the Newton-iteration
            // fast-division speedup wired into the subproduct tree by
            // issue `046f95c1`.
            group.bench_with_input(
                BenchmarkId::new("subproduct_auto", &id_fmt),
                &(&poly, &points),
                |b, (p, xs)| {
                    b.iter(|| {
                        black_box(batch_evaluate_subproduct_auto(
                            black_box(*p),
                            black_box(*xs),
                        ))
                    });
                },
            );

            // "naive" arm: the literal comparison point from the issue
            // scope — k independent Horner folds collected into a Vec.
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

/// Build `k` degree-8 polynomials with uniform random-looking coefficients
/// over `Fp<65537>`. Each polynomial uses a distinct seed so every member
/// of the batch is unique and no artificial cancellation occurs. Uses the
/// shared workspace LCG (`gf2_core::rng::Lcg`) so the bench stays in sync
/// with the SSOT deterministic RNG.
fn make_batch(k: usize) -> Vec<FieldPoly<F>> {
    let modulus: u64 = 65537;
    (0..k)
        .map(|seed| {
            // Advance the shared LCG once from the per-polynomial seed so
            // each batch element starts from an independent stream.
            let mut rng = gf2_core::rng::Lcg::new((seed as u64).wrapping_add(1));
            rng.next_u64();
            let coeffs: Vec<F> = (0..=8)
                .map(|_| F::new((rng.next_u64() >> 33) % (modulus - 1) + 1))
                .collect();
            FieldPoly::new(coeffs)
        })
        .collect()
}

fn bench_batch_mul(c: &mut Criterion) {
    let mut group = c.benchmark_group("field_poly_batch_mul_fp65537");
    // Keep sample size small so the k=128 cells stay inside the 60s budget
    // when run via `--quick`. Criterion's default 100-sample target would
    // overrun on k=128 balanced-tree runs.
    group.sample_size(20);

    for &k in &[8usize, 32, 128] {
        let polys = make_batch(k);
        let sample = F::new(1);

        group.bench_with_input(BenchmarkId::new("balanced_tree", k), &polys, |b, ps| {
            b.iter(|| black_box(FieldPoly::batch_mul(black_box(ps))));
        });

        group.bench_with_input(BenchmarkId::new("left_fold", k), &polys, |b, ps| {
            b.iter(|| {
                let linear = ps.iter().fold(FieldPoly::one_like(&sample), |a, b| &a * b);
                black_box(linear)
            });
        });
    }

    group.finish();
}

/// Build a deterministic polynomial of length `n` for the NTT /
/// Karatsuba shoot-out. Coefficients are drawn from the shared workspace
/// LCG (`gf2_core::rng::Lcg`) so the bench is reproducible across runs
/// and stays in sync with the SSOT deterministic RNG.
fn make_ntt_poly(n: usize, seed: u64) -> FieldPoly<F> {
    let modulus: u64 = 65537;
    let mut rng = gf2_core::rng::Lcg::new(seed | 1);
    let coeffs: Vec<F> = (0..n)
        .map(|_| F::new((rng.next_u64() >> 33) % modulus))
        .collect();
    FieldPoly::new(coeffs)
}

fn bench_ntt_vs_karatsuba(c: &mut Criterion) {
    use gf2_core::field::poly::mul_fast;

    let mut group = c.benchmark_group("field_poly_mul_fp65537");
    group.sample_size(20);

    for &n in &[64usize, 128, 256, 512, 1024] {
        let a = make_ntt_poly(n, 0xa5a5_5a5a_a5a5_5a5a);
        let b = make_ntt_poly(n, 0x5a5a_a5a5_5a5a_a5a5);

        // "karatsuba" — the existing `Mul` operator dispatch, which
        // routes through the schoolbook / Karatsuba code path.
        group.bench_with_input(BenchmarkId::new("karatsuba", n), &(&a, &b), |bh, (p, q)| {
            bh.iter(|| black_box(black_box(*p).mul(black_box(*q))));
        });

        // "ntt" — the unconditional NTT path via `mul_ntt`. The
        // threshold gate in `mul_fast` is benchmarked separately below.
        group.bench_with_input(BenchmarkId::new("ntt", n), &(&a, &b), |bh, (p, q)| {
            bh.iter(|| black_box(black_box(*p).mul_ntt(black_box(*q))));
        });

        // "mul_fast" — the tuned dispatcher; should track the faster of
        // the two arms above on every `n`.
        group.bench_with_input(BenchmarkId::new("mul_fast", n), &(&a, &b), |bh, (p, q)| {
            bh.iter(|| black_box(mul_fast(black_box(*p), black_box(*q))));
        });
    }

    group.finish();
}

/// Build `n` deterministic distinct evaluation points for interpolation benchmarks.
/// Uses a stride coprime to 65537 so no two points collide for n ≤ 65536.
fn make_interp_points(n: usize) -> Vec<(F, F)> {
    let modulus: u64 = 65537;
    (0..n)
        .map(|i| {
            let x = ((i as u64).wrapping_mul(1_000_003) % (modulus - 1)) + 1;
            let y = ((i as u64).wrapping_mul(999_983) % (modulus - 1)) + 1;
            (F::new(x), F::new(y))
        })
        .collect()
}

fn bench_interpolate(c: &mut Criterion) {
    use gf2_core::field::poly_interpolate::{interpolate, interpolate_fast};

    let mut group = c.benchmark_group("field_poly_interpolate_fp65537");
    group.sample_size(10);

    for &n in &[4usize, 8, 16, 32, 64, 128, 256, 512, 1024, 2048] {
        let points = make_interp_points(n);

        group.bench_with_input(BenchmarkId::new("naive", n), &points, |b, pts| {
            b.iter(|| black_box(interpolate(black_box(pts)).unwrap()));
        });

        group.bench_with_input(BenchmarkId::new("fast", n), &points, |b, pts| {
            b.iter(|| black_box(interpolate_fast(black_box(pts)).unwrap()));
        });
    }

    group.finish();
}

/// Build a deterministic pair of polynomials for the `div_rem` shoot-out.
/// Lengths are `(n, m)` where `n = dividend.len()` and `m = divisor.len()`.
/// The divisor is forced to have a non-zero leading coefficient so the
/// Newton-iteration path hits its `k = n − m` precision target; the
/// dividend is a randomly-looking `Fp<65537>` polynomial of length `n`.
fn make_div_rem_pair(n: usize, m: usize) -> (FieldPoly<F>, FieldPoly<F>) {
    let modulus: u64 = 65537;
    let mut rng_a = gf2_core::rng::Lcg::new(0xa5a5_5a5a_a5a5_5a5a);
    let mut a_coeffs: Vec<F> = (0..n)
        .map(|_| F::new((rng_a.next_u64() >> 33) % (modulus - 1) + 1))
        .collect();
    *a_coeffs.last_mut().unwrap() = F::new(1);

    let mut rng_b = gf2_core::rng::Lcg::new(0x5a5a_a5a5_5a5a_a5a5);
    let mut b_coeffs: Vec<F> = (0..m)
        .map(|_| F::new((rng_b.next_u64() >> 33) % (modulus - 1) + 1))
        .collect();
    *b_coeffs.last_mut().unwrap() = F::new(1);

    (FieldPoly::new(a_coeffs), FieldPoly::new(b_coeffs))
}

fn bench_div_rem(c: &mut Criterion) {
    let mut group = c.benchmark_group("field_poly_div_rem_fp65537");
    // Keep sample count small so the largest (n = 1024, m = 512) cell fits
    // comfortably inside the 60s test-suite budget under `--quick`.
    group.sample_size(10);

    // Sizes bracket the schoolbook / fast crossover on `Fp<65537>` (Zen 3).
    // The (n = 128 … 1024) block is the required task arm set; the
    // (n = 2048, m = 1024) row is appended so the crossover is visible in
    // a single `--quick` run — the tuned `DIV_REM_THRESHOLD` sits exactly
    // at that row's `n`. See the module docstring in
    // `crates/gf2-core/src/field/poly.rs` for the committed numbers.
    let sizes = [
        (128usize, 64usize),
        (256, 128),
        (512, 256),
        (1024, 512),
        (2048, 1024),
    ];

    for &(n, m) in &sizes {
        let (dividend, divisor) = make_div_rem_pair(n, m);
        let id_fmt = format!("n{n}_m{m}");

        // "schoolbook" arm — the existing O((n − m) · m) long division.
        group.bench_with_input(
            BenchmarkId::new("schoolbook", &id_fmt),
            &(&dividend, &divisor),
            |b, (a, d)| {
                b.iter(|| black_box(black_box(*a).div_rem(black_box(*d))));
            },
        );

        // "fast" arm — Newton-iteration `div_rem_fast`, which routes its
        // internal multiplications through `mul_fast` (Karatsuba below
        // NTT_THRESHOLD, NTT above).
        group.bench_with_input(
            BenchmarkId::new("fast", &id_fmt),
            &(&dividend, &divisor),
            |b, (a, d)| {
                b.iter(|| black_box(black_box(*a).div_rem_fast(black_box(*d))));
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_batch_evaluate,
    bench_batch_mul,
    bench_ntt_vs_karatsuba,
    bench_interpolate,
    bench_div_rem,
);
criterion_main!(benches);
