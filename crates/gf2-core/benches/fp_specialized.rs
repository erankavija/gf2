//! Benchmarks comparing specialized prime-field reductions (Mersenne, Proth,
//! Goldilocks) against the Montgomery baseline.
//!
//! Success criteria targeted by this bench:
//!
//! * Mersenne `Fp<2^31 - 1>` multiplication ≥ 2× faster than Montgomery.
//! * Goldilocks multiplication ≥ 1.5× faster than Montgomery.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use gf2_core::field::{ConstField, FiniteField};
use gf2_core::gfp::specialized::{
    batch_dot_mersenne31, batch_mul_mersenne31, goldilocks_reduce_fast, mersenne_reduce,
    proth_reduce, GoldilocksFp, GOLDILOCKS_PRIME,
};
use gf2_core::gfp::Fp;

// ---------------------------------------------------------------------------
// Mersenne 2^31 - 1
// ---------------------------------------------------------------------------

const M31: u64 = (1u64 << 31) - 1;
const M61: u64 = (1u64 << 61) - 1;
/// BabyBear Proth prime: 15 * 2^27 + 1 = 2013265921.
const PROTH: u64 = 15 * (1u64 << 27) + 1;

fn bench_fp_mersenne31_mul(c: &mut Criterion) {
    let a = Fp::<M31>::new(123_456_789);
    let b = Fp::<M31>::new(987_654_321 % M31);
    c.bench_function("fp_mersenne31_mul_specialized", |bench| {
        bench.iter(|| black_box(a) * black_box(b))
    });
}

fn bench_naive_mul_m31(c: &mut Criterion) {
    let a = 123_456_789u64;
    let b = 987_654_321u64 % M31;
    // black_box(p) forces runtime division instead of strength reduction.
    let p = M31;
    c.bench_function("naive_mersenne31_mul_mod", |bench| {
        bench.iter(|| {
            let x = black_box(a) as u128 * black_box(b) as u128;
            (x % black_box(p) as u128) as u64
        })
    });
}

fn bench_fp_mersenne31_add(c: &mut Criterion) {
    let a = Fp::<M31>::new(123_456_789);
    let b = Fp::<M31>::new(987_654_321 % M31);
    c.bench_function("fp_mersenne31_add_specialized", |bench| {
        bench.iter(|| black_box(a) + black_box(b))
    });
}

/// Synthetic Montgomery baseline of similar magnitude — uses the nearby
/// prime `2^31 - 19 = 2147483629` (prime, not Mersenne) so the Montgomery
/// code path is forced and the workloads are size-matched.
const M31_LIKE_GENERIC: u64 = 2_147_483_629;

fn bench_fp_montgomery_like_mul(c: &mut Criterion) {
    let a = Fp::<M31_LIKE_GENERIC>::new(123_456_789);
    let b = Fp::<M31_LIKE_GENERIC>::new(987_654_321 % M31_LIKE_GENERIC);
    c.bench_function("fp_generic_near_m31_mul_montgomery", |bench| {
        bench.iter(|| black_box(a) * black_box(b))
    });
}

fn bench_fp_mersenne31_inv(c: &mut Criterion) {
    let a = Fp::<M31>::new(123_456_789);
    c.bench_function("fp_mersenne31_inv_specialized", |bench| {
        bench.iter(|| black_box(a).inv())
    });
}

fn bench_fp_mersenne31_mul_chain(c: &mut Criterion) {
    let xs: Vec<_> = (1..=100u64).map(|i| Fp::<M31>::new(i * 12345)).collect();
    c.bench_function("fp_mersenne31_mul_chain_100", |bench| {
        bench.iter(|| {
            let mut acc = Fp::<M31>::one();
            for &e in &xs {
                acc = acc * black_box(e);
            }
            acc
        })
    });
}

// ---------------------------------------------------------------------------
// Mersenne 2^61 - 1 — the existing Fp<M61> is already covered by the
// Montgomery bench; here we measure with the specialized path enabled.
// ---------------------------------------------------------------------------

fn bench_fp_mersenne61_mul_specialized(c: &mut Criterion) {
    let a = Fp::<M61>::new(123_456_789);
    let b = Fp::<M61>::new(987_654_321);
    c.bench_function("fp_mersenne61_mul_specialized", |bench| {
        bench.iter(|| black_box(a) * black_box(b))
    });
}

/// Montgomery baseline of the same ~61-bit magnitude — the nearby prime
/// `2^61 - 45 = 2305843009213693907` forces the Montgomery code path.
const M61_LIKE_GENERIC: u64 = 2_305_843_009_213_693_907;

fn bench_fp_m61_like_mul_montgomery(c: &mut Criterion) {
    let a = Fp::<M61_LIKE_GENERIC>::new(123_456_789);
    let b = Fp::<M61_LIKE_GENERIC>::new(987_654_321);
    c.bench_function("fp_generic_near_m61_mul_montgomery", |bench| {
        bench.iter(|| black_box(a) * black_box(b))
    });
}

// ---------------------------------------------------------------------------
// Proth 3·2^32 + 1
// ---------------------------------------------------------------------------

fn bench_fp_proth_mul(c: &mut Criterion) {
    let a = Fp::<PROTH>::new(123_456_789);
    let b = Fp::<PROTH>::new(987_654_321);
    c.bench_function("fp_proth_mul_specialized", |bench| {
        bench.iter(|| black_box(a) * black_box(b))
    });
}

// ---------------------------------------------------------------------------
// Goldilocks
// ---------------------------------------------------------------------------

fn bench_goldilocks_mul(c: &mut Criterion) {
    let a = GoldilocksFp::new(123_456_789_012_345);
    let b = GoldilocksFp::new(987_654_321_098_765);
    c.bench_function("goldilocks_mul_specialized", |bench| {
        bench.iter(|| black_box(a) * black_box(b))
    });
}

fn bench_goldilocks_add(c: &mut Criterion) {
    let a = GoldilocksFp::new(123_456_789_012_345);
    let b = GoldilocksFp::new(987_654_321_098_765);
    c.bench_function("goldilocks_add_specialized", |bench| {
        bench.iter(|| black_box(a) + black_box(b))
    });
}

fn bench_goldilocks_inv(c: &mut Criterion) {
    let a = GoldilocksFp::new(123_456_789_012_345);
    c.bench_function("goldilocks_inv_specialized", |bench| {
        bench.iter(|| black_box(a).inv())
    });
}

/// Naive Goldilocks multiplication baseline using `% p` directly.
fn bench_goldilocks_naive_mul(c: &mut Criterion) {
    let a: u64 = 123_456_789_012_345;
    let b: u64 = 987_654_321_098_765;
    let p = GOLDILOCKS_PRIME;
    c.bench_function("goldilocks_mul_naive_mod", |bench| {
        bench.iter(|| {
            let x = (black_box(a) as u128) * (black_box(b) as u128);
            (x % black_box(p) as u128) as u64
        })
    });
}

/// Montgomery baseline at an "almost Goldilocks" size — the largest prime
/// that fits in `Fp<P>`'s `P ≤ 2^63` bound, to anchor the speed-up claim.
const NEAR_GOLDILOCKS_MONTGOMERY: u64 = 9_223_372_036_854_775_783; // 2^63 - 25 (prime)

fn bench_fp_near_goldilocks_montgomery_mul(c: &mut Criterion) {
    let a = Fp::<NEAR_GOLDILOCKS_MONTGOMERY>::new(123_456_789_012_345);
    let b = Fp::<NEAR_GOLDILOCKS_MONTGOMERY>::new(987_654_321_098_765);
    c.bench_function("fp_near_goldilocks_mul_montgomery", |bench| {
        bench.iter(|| black_box(a) * black_box(b))
    });
}

// ---------------------------------------------------------------------------
// Raw reducer benches (isolate the reduction itself, no field wrapping)
// ---------------------------------------------------------------------------

fn bench_mersenne31_reducer(c: &mut Criterion) {
    let x = 0x1234_5678_9abc_def0u128;
    c.bench_function("mersenne31_reduce_raw", |bench| {
        bench.iter(|| mersenne_reduce::<31>(black_box(x)))
    });
}

fn bench_mersenne61_reducer(c: &mut Criterion) {
    let x = 0x1234_5678_9abc_def0u128;
    c.bench_function("mersenne61_reduce_raw", |bench| {
        bench.iter(|| mersenne_reduce::<61>(black_box(x)))
    });
}

fn bench_proth_reducer(c: &mut Criterion) {
    let x = 0x1234_5678_9abc_def0u128;
    c.bench_function("proth_reduce_raw", |bench| {
        bench.iter(|| proth_reduce::<15, 27>(black_box(x)))
    });
}

fn bench_goldilocks_reducer(c: &mut Criterion) {
    let x = 0x1234_5678_9abc_def0_1111_2222_3333_4444u128;
    c.bench_function("goldilocks_reduce_raw", |bench| {
        bench.iter(|| goldilocks_reduce_fast(black_box(x)))
    });
}

// ---------------------------------------------------------------------------
// SIMD batch Mersenne31 — the success-criterion benchmark. The target is
// ≥ 2× throughput over the scalar Montgomery batch for the same length,
// and ≥ 2× over the scalar specialized batch.
// ---------------------------------------------------------------------------

/// Length used for batch benches. Large enough to saturate AVX2 but not so
/// large that cache misses dominate.
const BATCH_LEN: usize = 1024;

fn bench_fp_m31_batch_mul_simd(c: &mut Criterion) {
    let m31 = M31 as u32;
    let a: Vec<u32> = (0..BATCH_LEN as u32).map(|i| (i * 17 + 1) % m31).collect();
    let b: Vec<u32> = (0..BATCH_LEN as u32).map(|i| (i * 23 + 5) % m31).collect();
    let mut out = vec![0u32; BATCH_LEN];
    c.bench_function("fp_m31_batch_mul_simd", |bench| {
        bench.iter(|| {
            batch_mul_mersenne31(black_box(&a), black_box(&b), black_box(&mut out));
        })
    });
}

fn bench_fp_m31_batch_mul_scalar_specialized(c: &mut Criterion) {
    let a: Vec<Fp<M31>> = (0..BATCH_LEN as u64)
        .map(|i| Fp::<M31>::new(i * 17 + 1))
        .collect();
    let b: Vec<Fp<M31>> = (0..BATCH_LEN as u64)
        .map(|i| Fp::<M31>::new(i * 23 + 5))
        .collect();
    let mut out: Vec<Fp<M31>> = vec![Fp::<M31>::new(0); BATCH_LEN];
    c.bench_function("fp_m31_batch_mul_scalar_specialized", |bench| {
        bench.iter(|| {
            for i in 0..BATCH_LEN {
                out[i] = black_box(a[i]) * black_box(b[i]);
            }
        })
    });
}

fn bench_fp_m31_batch_mul_scalar_montgomery(c: &mut Criterion) {
    // Use a nearby non-Mersenne prime (2^31 - 19) so Fp<P>'s Montgomery
    // storage path is forced — this gives an apples-to-apples scalar
    // Montgomery batch baseline at the same operand magnitude.
    let a: Vec<Fp<M31_LIKE_GENERIC>> = (0..BATCH_LEN as u64)
        .map(|i| Fp::<M31_LIKE_GENERIC>::new(i * 17 + 1))
        .collect();
    let b: Vec<Fp<M31_LIKE_GENERIC>> = (0..BATCH_LEN as u64)
        .map(|i| Fp::<M31_LIKE_GENERIC>::new(i * 23 + 5))
        .collect();
    let mut out: Vec<Fp<M31_LIKE_GENERIC>> = vec![Fp::<M31_LIKE_GENERIC>::new(0); BATCH_LEN];
    c.bench_function("fp_m31_batch_mul_scalar_montgomery", |bench| {
        bench.iter(|| {
            for i in 0..BATCH_LEN {
                out[i] = black_box(a[i]) * black_box(b[i]);
            }
        })
    });
}

fn bench_fp_m31_batch_dot_simd(c: &mut Criterion) {
    let m31 = M31 as u32;
    let a: Vec<u32> = (0..BATCH_LEN as u32).map(|i| (i * 17 + 1) % m31).collect();
    let b: Vec<u32> = (0..BATCH_LEN as u32).map(|i| (i * 23 + 5) % m31).collect();
    c.bench_function("fp_m31_batch_dot_simd", |bench| {
        bench.iter(|| batch_dot_mersenne31(black_box(&a), black_box(&b)))
    });
}

criterion_group!(
    specialized,
    bench_fp_mersenne31_mul,
    bench_fp_mersenne31_add,
    bench_fp_mersenne31_inv,
    bench_fp_mersenne31_mul_chain,
    bench_fp_montgomery_like_mul,
    bench_naive_mul_m31,
    bench_fp_mersenne61_mul_specialized,
    bench_fp_m61_like_mul_montgomery,
    bench_fp_proth_mul,
    bench_goldilocks_mul,
    bench_goldilocks_add,
    bench_goldilocks_inv,
    bench_goldilocks_naive_mul,
    bench_fp_near_goldilocks_montgomery_mul,
    bench_mersenne31_reducer,
    bench_mersenne61_reducer,
    bench_proth_reducer,
    bench_goldilocks_reducer,
    bench_fp_m31_batch_mul_simd,
    bench_fp_m31_batch_mul_scalar_specialized,
    bench_fp_m31_batch_mul_scalar_montgomery,
    bench_fp_m31_batch_dot_simd,
);
criterion_main!(specialized);
