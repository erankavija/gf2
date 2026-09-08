//! Benchmarks for Montgomery-form Fp<P> arithmetic.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use gf2_core::field::{ConstField, FiniteField};
use gf2_core::gfp::Fp;

fn bench_mul_gf7(c: &mut Criterion) {
    let a = Fp::<7>::new(3);
    let b = Fp::<7>::new(5);
    c.bench_function("fp7_mul", |bench| {
        bench.iter(|| black_box(a) * black_box(b))
    });
}

fn bench_add_gf7(c: &mut Criterion) {
    let a = Fp::<7>::new(3);
    let b = Fp::<7>::new(5);
    c.bench_function("fp7_add", |bench| {
        bench.iter(|| black_box(a) + black_box(b))
    });
}

fn bench_sub_gf7(c: &mut Criterion) {
    let a = Fp::<7>::new(3);
    let b = Fp::<7>::new(5);
    c.bench_function("fp7_sub", |bench| {
        bench.iter(|| black_box(a) - black_box(b))
    });
}

fn bench_inv_gf7(c: &mut Criterion) {
    let a = Fp::<7>::new(3);
    c.bench_function("fp7_inv", |bench| bench.iter(|| black_box(a).inv()));
}

fn bench_mul_gf65537(c: &mut Criterion) {
    let a = Fp::<65537>::new(12345);
    let b = Fp::<65537>::new(54321);
    c.bench_function("fp65537_mul", |bench| {
        bench.iter(|| black_box(a) * black_box(b))
    });
}

fn bench_add_gf65537(c: &mut Criterion) {
    let a = Fp::<65537>::new(12345);
    let b = Fp::<65537>::new(54321);
    c.bench_function("fp65537_add", |bench| {
        bench.iter(|| black_box(a) + black_box(b))
    });
}

fn bench_sub_gf65537(c: &mut Criterion) {
    let a = Fp::<65537>::new(12345);
    let b = Fp::<65537>::new(54321);
    c.bench_function("fp65537_sub", |bench| {
        bench.iter(|| black_box(a) - black_box(b))
    });
}

fn bench_inv_gf65537(c: &mut Criterion) {
    let a = Fp::<65537>::new(12345);
    c.bench_function("fp65537_inv", |bench| bench.iter(|| black_box(a).inv()));
}

const MERSENNE_61: u64 = (1u64 << 61) - 1;

fn bench_mul_mersenne61(c: &mut Criterion) {
    let a = Fp::<MERSENNE_61>::new(123_456_789);
    let b = Fp::<MERSENNE_61>::new(987_654_321);
    c.bench_function("fp_mersenne61_mul", |bench| {
        bench.iter(|| black_box(a) * black_box(b))
    });
}

fn bench_add_mersenne61(c: &mut Criterion) {
    let a = Fp::<MERSENNE_61>::new(123_456_789);
    let b = Fp::<MERSENNE_61>::new(987_654_321);
    c.bench_function("fp_mersenne61_add", |bench| {
        bench.iter(|| black_box(a) + black_box(b))
    });
}

fn bench_sub_mersenne61(c: &mut Criterion) {
    let a = Fp::<MERSENNE_61>::new(123_456_789);
    let b = Fp::<MERSENNE_61>::new(987_654_321);
    c.bench_function("fp_mersenne61_sub", |bench| {
        bench.iter(|| black_box(a) - black_box(b))
    });
}

fn bench_inv_mersenne61(c: &mut Criterion) {
    let a = Fp::<MERSENNE_61>::new(123_456_789);
    c.bench_function("fp_mersenne61_inv", |bench| {
        bench.iter(|| black_box(a).inv())
    });
}

fn bench_mul_chain_mersenne61(c: &mut Criterion) {
    let elements: Vec<_> = (1..=100u64)
        .map(|i| Fp::<MERSENNE_61>::new(i * 1_000_000))
        .collect();
    c.bench_function("fp_mersenne61_mul_chain_100", |bench| {
        bench.iter(|| {
            let mut acc = Fp::<MERSENNE_61>::one();
            for &e in &elements {
                acc = acc * black_box(e);
            }
            acc
        })
    });
}

// ---------------------------------------------------------------------------
// Naive baseline benchmarks for comparison
// ---------------------------------------------------------------------------

/// Naive modular multiplication using `%` — the baseline Montgomery replaces.
#[inline]
fn naive_mul(a: u64, b: u64, p: u64) -> u64 {
    ((a as u128 * b as u128) % p as u128) as u64
}

/// Naive modular exponentiation using `%`.
fn naive_mod_pow(mut base: u64, mut exp: u64, modulus: u64) -> u64 {
    let mut result = 1u64;
    base %= modulus;
    while exp > 0 {
        if exp & 1 == 1 {
            result = ((result as u128 * base as u128) % modulus as u128) as u64;
        }
        exp >>= 1;
        if exp > 0 {
            base = ((base as u128 * base as u128) % modulus as u128) as u64;
        }
    }
    result
}

/// Naive modular addition using `%`.
#[inline]
fn naive_add(a: u64, b: u64, p: u64) -> u64 {
    ((a as u128 + b as u128) % p as u128) as u64
}

/// Naive modular subtraction using `%`.
#[inline]
fn naive_sub(a: u64, b: u64, p: u64) -> u64 {
    ((a as u128 + p as u128 - b as u128) % p as u128) as u64
}

fn bench_naive_add_mersenne61(c: &mut Criterion) {
    let a = 123_456_789u64;
    let b = 987_654_321u64;
    // black_box(p) prevents the compiler from replacing `% p` with multiply-high+shift,
    // ensuring the benchmark measures actual division (what Montgomery eliminates).
    let p = MERSENNE_61;
    c.bench_function("naive_mersenne61_add", |bench| {
        bench.iter(|| naive_add(black_box(a), black_box(b), black_box(p)))
    });
}

fn bench_naive_sub_mersenne61(c: &mut Criterion) {
    let a = 123_456_789u64;
    let b = 987_654_321u64;
    let p = MERSENNE_61;
    c.bench_function("naive_mersenne61_sub", |bench| {
        bench.iter(|| naive_sub(black_box(a), black_box(b), black_box(p)))
    });
}

fn bench_naive_mul_mersenne61(c: &mut Criterion) {
    let a = 123_456_789u64;
    let b = 987_654_321u64;
    let p = MERSENNE_61;
    c.bench_function("naive_mersenne61_mul", |bench| {
        bench.iter(|| naive_mul(black_box(a), black_box(b), black_box(p)))
    });
}

fn bench_naive_inv_mersenne61(c: &mut Criterion) {
    let a = 123_456_789u64;
    let p = MERSENNE_61;
    let exp = p - 2;
    c.bench_function("naive_mersenne61_inv", |bench| {
        bench.iter(|| naive_mod_pow(black_box(a), black_box(exp), black_box(p)))
    });
}

fn bench_naive_mul_chain_mersenne61(c: &mut Criterion) {
    let elements: Vec<u64> = (1..=100u64).map(|i| i * 1_000_000).collect();
    let p = MERSENNE_61;
    c.bench_function("naive_mersenne61_mul_chain_100", |bench| {
        bench.iter(|| {
            let mut acc = 1u64;
            let p = black_box(p);
            for &e in &elements {
                acc = naive_mul(acc, black_box(e), p);
            }
            acc
        })
    });
}

criterion_group!(
    benches,
    bench_mul_gf7,
    bench_add_gf7,
    bench_sub_gf7,
    bench_inv_gf7,
    bench_mul_gf65537,
    bench_add_gf65537,
    bench_sub_gf65537,
    bench_inv_gf65537,
    bench_mul_mersenne61,
    bench_add_mersenne61,
    bench_sub_mersenne61,
    bench_inv_mersenne61,
    bench_mul_chain_mersenne61,
    bench_naive_add_mersenne61,
    bench_naive_sub_mersenne61,
    bench_naive_mul_mersenne61,
    bench_naive_inv_mersenne61,
    bench_naive_mul_chain_mersenne61,
);
criterion_main!(benches);
