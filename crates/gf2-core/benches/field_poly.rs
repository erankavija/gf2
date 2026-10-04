//! Benchmarks [`gf2_core::field::FieldPoly`] on `Fp<65537>`: batch evaluation
//! (dispatcher, subproduct-tree and per-point Horner arms), batch
//! multiplication, NTT against Karatsuba multiplication, interpolation and
//! `div_rem`.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use gf2_core::field::poly::{batch_evaluate_subproduct, batch_evaluate_subproduct_auto};
use gf2_core::field::FieldPoly;
use gf2_core::gfp::Fp;

type F = Fp<65537>;

fn make_poly(n: usize) -> FieldPoly<F> {
    let modulus: u64 = 65537;
    let mut coeffs: Vec<F> = (0..n)
        .map(|i| {
            let v = ((i as u64).wrapping_mul(2_654_435_761) % (modulus - 1)) + 1;
            F::new(v)
        })
        .collect();
    // A non-zero leading coefficient keeps the degree at `n - 1`.
    *coeffs.last_mut().unwrap() = F::new(1);
    FieldPoly::new(coeffs)
}

fn make_points(k: usize) -> Vec<F> {
    let modulus: u64 = 65537;
    (0..k)
        .map(|i| F::new(((i as u64).wrapping_mul(1_000_003) % (modulus - 1)) + 1))
        .collect()
}

fn bench_batch_evaluate(c: &mut Criterion) {
    let mut group = c.benchmark_group("field_poly_batch_evaluate_fp65537");
    group.sample_size(20);

    let ns = [16usize, 64, 256, 1024, 4096, 8192];
    let ks = [16usize, 64, 256, 1024, 4096, 8192];

    for &n in &ns {
        let poly = make_poly(n);
        for &k in &ks {
            let points = make_points(k);
            let id_fmt = format!("n{n}_k{k}");

            group.bench_with_input(
                BenchmarkId::new("dispatcher", &id_fmt),
                &(&poly, &points),
                |b, (p, xs)| {
                    b.iter(|| black_box(p.batch_evaluate(xs)));
                },
            );

            group.bench_with_input(
                BenchmarkId::new("subproduct", &id_fmt),
                &(&poly, &points),
                |b, (p, xs)| {
                    b.iter(|| black_box(batch_evaluate_subproduct(black_box(*p), black_box(*xs))));
                },
            );

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

fn make_batch(k: usize) -> Vec<FieldPoly<F>> {
    let modulus: u64 = 65537;
    (0..k)
        .map(|seed| {
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

        group.bench_with_input(BenchmarkId::new("karatsuba", n), &(&a, &b), |bh, (p, q)| {
            bh.iter(|| black_box(black_box(*p).mul(black_box(*q))));
        });

        group.bench_with_input(BenchmarkId::new("ntt", n), &(&a, &b), |bh, (p, q)| {
            bh.iter(|| black_box(black_box(*p).mul_ntt(black_box(*q))));
        });

        group.bench_with_input(BenchmarkId::new("mul_fast", n), &(&a, &b), |bh, (p, q)| {
            bh.iter(|| black_box(mul_fast(black_box(*p), black_box(*q))));
        });
    }

    group.finish();
}

/// The x stride is odd, so the points are distinct for `n ≤ 65536`.
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
    group.sample_size(10);

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

        group.bench_with_input(
            BenchmarkId::new("schoolbook", &id_fmt),
            &(&dividend, &divisor),
            |b, (a, d)| {
                b.iter(|| black_box(black_box(*a).div_rem(black_box(*d))));
            },
        );

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
