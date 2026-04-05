//! Benchmarks for `FieldVec::dot_product` across different field types and sizes.
//!
//! Measures throughput of the delayed-reduction dot product for `Fp<P>` at several
//! prime sizes and vector lengths, plus GF(2^m) baselines for comparison.
//!
//! ## Measured comparison: gf2-core vs fflas-ffpack
//!
//! Measured on Linux 6.19 x86_64 (GCC 15.2, fflas-ffpack with OpenBLAS).
//! Build the C++ harness with `./benches/build_fflas_bench.sh`.
//!
//! ### n = 1000 (ns/elem, lower is better)
//!
//! | Prime         | gf2-core `dot_product` | fflas `fdot<int64_t>` | fflas `fdot<double>` |
//! |---------------|------------------------|-----------------------|----------------------|
//! | p = 65521     | 1.72                   | 0.49                  | 0.11 (BLAS ddot)     |
//! | p = 2^31 - 1  | 1.77                   | 1.11                  | n/a                  |
//! | p ~ 2^62      | 2.72                   | **not supported**     | n/a                  |
//!
//! ### All sizes (ns/elem, gf2-core / fflas-ffpack int64)
//!
//! | Prime         |  n=100       |  n=1000      |  n=10000     |
//! |---------------|--------------|--------------|--------------|
//! | p = 65521     | 1.75 / 0.53  | 1.72 / 0.49  | 1.63 / 0.36  |
//! | p = 2^31 - 1  | 1.80 / 1.21  | 1.77 / 1.11  | 1.72 / 1.09  |
//! | p ~ 2^62      | 2.41 / ---   | 2.72 / ---   | 2.36 / ---   |
//!
//! **Key findings:**
//! - For small primes, fflas-ffpack's `Modular<double>` delegates to BLAS `ddot`,
//!   giving ~15x throughput advantage.  Its `Modular<int64_t>` path is ~3.5x faster.
//! - For 31-bit primes, the gap narrows to ~1.6x (both use delayed reduction).
//! - For primes near 2^62, fflas-ffpack's `Modular<int64_t>` max cardinality is
//!   ~2^31 — it simply cannot represent these fields.  gf2-core handles them
//!   natively via Montgomery multiplication with chunked delayed reduction.
//!
//! References:
//! - Dumas, Giorgi, Pernet. "Dense Linear Algebra over Word-Size Prime Fields:
//!   the FFLAS and FFPACK Packages." ACM TOMS 35(3), 2008.
//! - FFLAS-FFPACK source: <https://github.com/linbox-team/fflas-ffpack>
//! - C++ harness: `benches/fflas_fdot_bench.cpp`

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use gf2_core::field::FieldVec;
use gf2_core::gf2m::Gf2mField;
use gf2_core::gfp::Fp;

// ---------------------------------------------------------------------------
// Prime constants
// ---------------------------------------------------------------------------

/// Small prime fitting in 16 bits. fflas-ffpack would use Modular<double> + BLAS ddot.
const SMALL_PRIME: u64 = 65521;

/// Mersenne prime 2^31 - 1. fflas-ffpack uses Modular<int64_t> with delayed reduction.
const MERSENNE_31: u64 = (1u64 << 31) - 1;

/// Large prime near 2^62. Stresses the delayed-reduction chunking logic since
/// kmax is small (fewer products before reduction is required).
const LARGE_PRIME: u64 = 4_611_686_018_427_387_847; // largest prime < 2^62

// ---------------------------------------------------------------------------
// Vector lengths
// ---------------------------------------------------------------------------

const LENGTHS: &[usize] = &[100, 1_000, 10_000];

// ---------------------------------------------------------------------------
// Helper: build deterministic FieldVec<Fp<P>>
// ---------------------------------------------------------------------------

fn make_fp_vecs<const P: u64>(n: usize) -> (FieldVec<Fp<P>>, FieldVec<Fp<P>>) {
    let a: Vec<Fp<P>> = (0..n)
        .map(|i| Fp::<P>::new((i as u64 * 7 + 3) % P))
        .collect();
    let b: Vec<Fp<P>> = (0..n)
        .map(|i| Fp::<P>::new((i as u64 * 13 + 11) % P))
        .collect();
    (FieldVec::from(a), FieldVec::from(b))
}

// ---------------------------------------------------------------------------
// Fp<65521> benchmarks (small prime)
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Fp<2^31-1> benchmarks (Mersenne-31)
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Fp<~2^62> benchmarks (large prime)
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// GF(2^m) helpers and benchmarks
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// GF(2^12) benchmarks (scalar vs SIMD)
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// GF(2^16) benchmarks (scalar vs SIMD)
// ---------------------------------------------------------------------------

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
