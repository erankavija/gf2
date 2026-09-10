//! Benchmarks comparing specialized prime-field reductions (Mersenne, Proth,
//! Goldilocks) against the Montgomery baseline.
//!
//! Success criteria targeted by this bench (revised 2026-04-18):
//!
//! * **AVX2 batch Mersenne31** multiplication (`fp_m31_batch_mul_simd`) ≥ 2×
//!   faster than scalar Montgomery batch. Measured ~4.0–4.4× at N=1024 on
//!   Zen 3. Requires the `simd` feature; this bench is feature-gated via
//!   `required-features = ["simd"]` in Cargo.toml so it cannot silently run
//!   against the scalar fallback.
//! * **Goldilocks** multiplication ≥ 1.5× faster than Montgomery. Measured
//!   ~1.99×.
//! * **Scalar Mersenne31** is not expected to beat Montgomery; modern x86
//!   REDC pipelines ~4 multiplies in ~2 ns, matching the specialized path's
//!   algorithmic floor. The scalar benches here are informational, not
//!   acceptance gates.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use gf2_core::field::two_adic::BABYBEAR_P;
use gf2_core::field::FieldVec;
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
/// BabyBear Proth prime: 15 * 2^27 + 1 = 2_013_265_921.
///
/// Rebinds the SSOT constant `BABYBEAR_P` (`crates/gf2-core/src/field/two_adic.rs`)
/// under the bench-local name `PROTH` to preserve the historical call-site
/// names throughout this benchmark file without duplicating the literal.
const PROTH: u64 = BABYBEAR_P;

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

fn bench_fp_generic_near_m31_fieldvec_mul_dispatch(c: &mut Criterion) {
    let a = FieldVec::from(
        (0..BATCH_LEN as u64)
            .map(|i| Fp::<M31_LIKE_GENERIC>::new(i * 17 + 1))
            .collect::<Vec<_>>(),
    );
    let b = FieldVec::from(
        (0..BATCH_LEN as u64)
            .map(|i| Fp::<M31_LIKE_GENERIC>::new(i * 23 + 5))
            .collect::<Vec<_>>(),
    );
    c.bench_function("fp_generic_near_m31_fieldvec_mul_dispatch", |bench| {
        bench.iter(|| black_box(&a).mul_vec(black_box(&b)))
    });
}

fn bench_fp_generic_near_m31_fieldvec_mul_scalar_loop(c: &mut Criterion) {
    let a: Vec<Fp<M31_LIKE_GENERIC>> = (0..BATCH_LEN as u64)
        .map(|i| Fp::<M31_LIKE_GENERIC>::new(i * 17 + 1))
        .collect();
    let b: Vec<Fp<M31_LIKE_GENERIC>> = (0..BATCH_LEN as u64)
        .map(|i| Fp::<M31_LIKE_GENERIC>::new(i * 23 + 5))
        .collect();
    let mut out = vec![Fp::<M31_LIKE_GENERIC>::new(0); BATCH_LEN];
    c.bench_function("fp_generic_near_m31_fieldvec_mul_scalar_loop", |bench| {
        bench.iter(|| {
            for i in 0..BATCH_LEN {
                out[i] = black_box(a[i]) * black_box(b[i]);
            }
            black_box(&out);
        })
    });
}

fn bench_fp_generic_near_m61_fieldvec_mul_dispatch(c: &mut Criterion) {
    let a = FieldVec::from(
        (0..BATCH_LEN as u64)
            .map(|i| Fp::<M61_LIKE_GENERIC>::new(i * 1_000_003 + 17))
            .collect::<Vec<_>>(),
    );
    let b = FieldVec::from(
        (0..BATCH_LEN as u64)
            .map(|i| Fp::<M61_LIKE_GENERIC>::new(i * 2_000_033 + 23))
            .collect::<Vec<_>>(),
    );
    c.bench_function("fp_generic_near_m61_fieldvec_mul_dispatch", |bench| {
        bench.iter(|| black_box(&a).mul_vec(black_box(&b)))
    });
}

fn bench_fp_generic_near_m61_fieldvec_mul_scalar_loop(c: &mut Criterion) {
    let a: Vec<Fp<M61_LIKE_GENERIC>> = (0..BATCH_LEN as u64)
        .map(|i| Fp::<M61_LIKE_GENERIC>::new(i * 1_000_003 + 17))
        .collect();
    let b: Vec<Fp<M61_LIKE_GENERIC>> = (0..BATCH_LEN as u64)
        .map(|i| Fp::<M61_LIKE_GENERIC>::new(i * 2_000_033 + 23))
        .collect();
    let mut out = vec![Fp::<M61_LIKE_GENERIC>::new(0); BATCH_LEN];
    c.bench_function("fp_generic_near_m61_fieldvec_mul_scalar_loop", |bench| {
        bench.iter(|| {
            for i in 0..BATCH_LEN {
                out[i] = black_box(a[i]) * black_box(b[i]);
            }
            black_box(&out);
        })
    });
}

// ---------------------------------------------------------------------------
// General 64-bit prime Montgomery batch — the C3 success-criterion leaf.
//
// This is the criterion-1.5x gate target for issue 86c09a51. It exercises
// the production generic-Fp SIMD path from cad241e6 (`fp_generic::detect`)
// rather than the stale WIP's separate Montgomery dispatch tree. The modulus
// is a ~61-bit generic prime that does not use the Mersenne/Proth storage
// specialisations, so operands and outputs are Montgomery-form `u64` words.
// ---------------------------------------------------------------------------

fn bench_fp_general_64bit_batch_mul_simd(c: &mut Criterion) {
    fn p_inv(p: u64) -> u64 {
        let mut inv: u64 = 1;
        for _ in 0..6 {
            inv = inv.wrapping_mul(2u64.wrapping_sub(p.wrapping_mul(inv)));
        }
        inv.wrapping_neg()
    }

    fn r2_mod_p(p: u64) -> u64 {
        let r = ((1u128 << 64) % p as u128) as u64;
        ((r as u128 * r as u128) % p as u128) as u64
    }

    fn scalar_redc(t: u128, p: u64, p_inv: u64) -> u64 {
        let m = (t as u64).wrapping_mul(p_inv);
        let u = ((t + m as u128 * p as u128) >> 64) as u64;
        if u >= p {
            u - p
        } else {
            u
        }
    }

    fn to_mont(x: u64, p: u64, p_inv: u64) -> u64 {
        scalar_redc(x as u128 * r2_mod_p(p) as u128, p, p_inv)
    }

    let p = M61_LIKE_GENERIC;
    let p_inv = p_inv(p);
    let a_raw: Vec<u64> = (0..BATCH_LEN as u64)
        .map(|i| to_mont((i.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 1) % p, p, p_inv))
        .collect();
    let b_raw: Vec<u64> = (0..BATCH_LEN as u64)
        .map(|i| to_mont((i.wrapping_mul(0xBF58_476D_1CE4_E5B9) ^ 7) % p, p, p_inv))
        .collect();
    let mut out_raw = vec![0u64; BATCH_LEN];

    let force_scalar = std::env::var("BENCH_FORCE_SCALAR").is_ok();
    let maybe_fns = if force_scalar {
        None
    } else {
        gf2_kernels_simd::fp_generic::detect()
    };

    c.bench_function("fp_general_64bit_batch_mul_simd", |bench| {
        bench.iter(|| {
            if let Some(fns) = maybe_fns {
                (fns.batch_mul_fn)(
                    black_box(&a_raw),
                    black_box(&b_raw),
                    p,
                    p_inv,
                    black_box(&mut out_raw),
                );
            } else {
                for i in 0..BATCH_LEN {
                    out_raw[i] = scalar_redc(a_raw[i] as u128 * b_raw[i] as u128, p, p_inv);
                }
                black_box(&out_raw);
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
    bench_fp_generic_near_m31_fieldvec_mul_dispatch,
    bench_fp_generic_near_m31_fieldvec_mul_scalar_loop,
    bench_fp_generic_near_m61_fieldvec_mul_dispatch,
    bench_fp_generic_near_m61_fieldvec_mul_scalar_loop,
    bench_fp_general_64bit_batch_mul_simd,
    bench_fp_m31_batch_dot_simd,
);
criterion_main!(specialized);
