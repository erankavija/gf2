//! Benchmarks `FieldVec::dot_product` over `Fp<P>` at three prime sizes and
//! the scalar and SIMD dot products over GF(2^m).

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use gf2_core::field::FieldVec;
use gf2_core::gf2m::Gf2mField;
use gf2_core::gfp::Fp;

const SMALL_PRIME: u64 = 65521;

const MERSENNE_31: u64 = (1u64 << 31) - 1;

const LARGE_PRIME: u64 = 4_611_686_018_427_387_847; // largest prime < 2^62

const LENGTHS: &[usize] = &[100, 1_000, 10_000];

fn make_fp_vecs<const P: u64>(n: usize) -> (FieldVec<Fp<P>>, FieldVec<Fp<P>>) {
    let a: Vec<Fp<P>> = (0..n)
        .map(|i| Fp::<P>::new((i as u64 * 7 + 3) % P))
        .collect();
    let b: Vec<Fp<P>> = (0..n)
        .map(|i| Fp::<P>::new((i as u64 * 13 + 11) % P))
        .collect();
    (FieldVec::from(a), FieldVec::from(b))
}

fn bench_dot_fp_65521(c: &mut Criterion) {
    let mut group = c.benchmark_group("fieldvec_dot/fp_65521");

    for &n in LENGTHS {
        group.throughput(Throughput::Elements(n as u64));
        let (a, b) = make_fp_vecs::<SMALL_PRIME>(n);
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |bench, _| {
            bench.iter(|| black_box(&a).dot_product(black_box(&b)));
        });
    }

    group.finish();
}

fn bench_dot_fp_mersenne31(c: &mut Criterion) {
    let mut group = c.benchmark_group("fieldvec_dot/fp_mersenne31");

    for &n in LENGTHS {
        group.throughput(Throughput::Elements(n as u64));
        let (a, b) = make_fp_vecs::<MERSENNE_31>(n);
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |bench, _| {
            bench.iter(|| black_box(&a).dot_product(black_box(&b)));
        });
    }

    group.finish();
}

fn bench_dot_fp_large(c: &mut Criterion) {
    let mut group = c.benchmark_group("fieldvec_dot/fp_large");

    for &n in LENGTHS {
        group.throughput(Throughput::Elements(n as u64));
        let (a, b) = make_fp_vecs::<LARGE_PRIME>(n);
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |bench, _| {
            bench.iter(|| black_box(&a).dot_product(black_box(&b)));
        });
    }

    group.finish();
}

fn make_gf2m_vecs(
    m: usize,
    poly: u64,
    n: usize,
) -> (
    Gf2mField,
    FieldVec<gf2_core::gf2m::Gf2mElement>,
    FieldVec<gf2_core::gf2m::Gf2mElement>,
) {
    let field = Gf2mField::new(m, poly);
    let order = (1u64 << m) - 1; // max non-zero value
    let a_data: Vec<_> = (0..n)
        .map(|i| field.element((i as u64 % order) + 1))
        .collect();
    let b_data: Vec<_> = (0..n)
        .map(|i| field.element(((i as u64 * 3) % order) + 1))
        .collect();
    (field, FieldVec::from(a_data), FieldVec::from(b_data))
}

fn bench_dot_gf2m_4(c: &mut Criterion) {
    let mut group = c.benchmark_group("fieldvec_dot/gf2m_4");

    for &n in LENGTHS {
        group.throughput(Throughput::Elements(n as u64));
        let (_field, a, b) = make_gf2m_vecs(4, 0b10011, n);

        group.bench_with_input(BenchmarkId::new("scalar", n), &n, |bench, _| {
            bench.iter(|| black_box(&a).dot_product(black_box(&b)));
        });

        group.bench_with_input(BenchmarkId::new("simd", n), &n, |bench, _| {
            bench.iter(|| black_box(&a).simd_dot_product(black_box(&b)));
        });
    }

    group.finish();
}

fn bench_dot_gf2m_8(c: &mut Criterion) {
    let mut group = c.benchmark_group("fieldvec_dot/gf2m_8");

    for &n in LENGTHS {
        group.throughput(Throughput::Elements(n as u64));
        let (_field, a, b) = make_gf2m_vecs(8, 0x11b, n);

        group.bench_with_input(BenchmarkId::new("scalar", n), &n, |bench, _| {
            bench.iter(|| black_box(&a).dot_product(black_box(&b)));
        });

        group.bench_with_input(BenchmarkId::new("simd", n), &n, |bench, _| {
            bench.iter(|| black_box(&a).simd_dot_product(black_box(&b)));
        });
    }

    group.finish();
}

fn bench_dot_gf2m_12(c: &mut Criterion) {
    let mut group = c.benchmark_group("fieldvec_dot/gf2m_12");

    for &n in LENGTHS {
        group.throughput(Throughput::Elements(n as u64));
        let (_field, a, b) = make_gf2m_vecs(12, 0b1000001010011, n);

        group.bench_with_input(BenchmarkId::new("scalar", n), &n, |bench, _| {
            bench.iter(|| black_box(&a).dot_product(black_box(&b)));
        });

        group.bench_with_input(BenchmarkId::new("simd", n), &n, |bench, _| {
            bench.iter(|| black_box(&a).simd_dot_product(black_box(&b)));
        });
    }

    group.finish();
}

fn bench_dot_gf2m_16(c: &mut Criterion) {
    let mut group = c.benchmark_group("fieldvec_dot/gf2m_16");

    for &n in LENGTHS {
        group.throughput(Throughput::Elements(n as u64));
        let (_field, a, b) = make_gf2m_vecs(16, 0b10001000000001011, n);

        group.bench_with_input(BenchmarkId::new("scalar", n), &n, |bench, _| {
            bench.iter(|| black_box(&a).dot_product(black_box(&b)));
        });

        group.bench_with_input(BenchmarkId::new("simd", n), &n, |bench, _| {
            bench.iter(|| black_box(&a).simd_dot_product(black_box(&b)));
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_dot_fp_65521,
    bench_dot_fp_mersenne31,
    bench_dot_fp_large,
    bench_dot_gf2m_4,
    bench_dot_gf2m_8,
    bench_dot_gf2m_12,
    bench_dot_gf2m_16,
);
criterion_main!(benches);
