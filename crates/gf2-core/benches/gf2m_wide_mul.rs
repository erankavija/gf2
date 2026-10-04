//! Benchmarks `Gf2mWide` multiplication in GF(2^256) and GF(2^571): the
//! portable carry-less multiply with Barrett reduction, the dispatched `Mul`
//! impl, and the raw carry-less-multiply kernels.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use gf2_core::gf2m::barrett::BarrettReducerWide;
use gf2_core::gf2m::wide::clmul_wide_slice_portable;
use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};

/// GF(2^256), `@/citation/Seroussi1998` Table 1 row m = 256: `x^256 + x^10 + x^5 + x^2 + 1`.
struct Gf2m256Config;

impl Gf2mWideConfig<4> for Gf2m256Config {
    const M: usize = 256;
    const MODULUS: [u64; 4] = [0x425, 0, 0, 0];
    const NAME: &'static str = "Gf2m256Config";
}

/// GF(2^571), `@/citation/Nist2013` B-571/K-571 polynomial: `x^571 + x^10 + x^5 + x^2 + 1`.
struct Gf2m571Config;

impl Gf2mWideConfig<9> for Gf2m571Config {
    const M: usize = 571;
    const MODULUS: [u64; 9] = [0x425, 0, 0, 0, 0, 0, 0, 0, 0];
    const NAME: &'static str = "Gf2m571Config";
}

fn sample_operand<const N: usize>(seed: u64) -> [u64; N] {
    let mut s = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(1);
    let mut out = [0u64; N];
    for slot in &mut out {
        s = s
            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
            .wrapping_add(0xDEAD_BEEF);
        *slot = s;
    }
    out
}

fn sample_operand_m571(seed: u64) -> [u64; 9] {
    let mut out = sample_operand::<9>(seed);
    out[8] &= (1u64 << 59) - 1;
    out
}

fn make_reducer() -> BarrettReducerWide<4> {
    BarrettReducerWide::<4>::new(Gf2m256Config::MODULUS, 256)
}

fn make_reducer_m571() -> BarrettReducerWide<9> {
    BarrettReducerWide::<9>::new(Gf2m571Config::MODULUS, 571)
}

fn bench_scalar_clmul_plus_barrett(c: &mut Criterion) {
    let a = sample_operand::<4>(0x1234);
    let b = sample_operand::<4>(0xBEEF);
    let reducer = make_reducer();
    let mut product = [0u64; 8];

    c.bench_function("gf2m_wide4_scalar_clmul_barrett", |bench| {
        bench.iter(|| {
            // `clmul_wide_slice_portable` XOR-accumulates.
            product.fill(0);
            clmul_wide_slice_portable::<4>(black_box(&a), black_box(&b), &mut product);
            let reduced = reducer.reduce_slice(&product);
            black_box(reduced)
        })
    });
}

fn bench_mul_ref_dispatched(c: &mut Criterion) {
    let a = Gf2mWide::<4, Gf2m256Config>::new(sample_operand::<4>(0x1234));
    let b = Gf2mWide::<4, Gf2m256Config>::new(sample_operand::<4>(0xBEEF));

    #[cfg(feature = "simd")]
    let lane_name = gf2_kernels_simd::gf2m_wide::detect()
        .map(|f| f.name)
        .unwrap_or("scalar-fallback");
    #[cfg(not(feature = "simd"))]
    let lane_name = "scalar-fallback";

    let id = format!("gf2m_wide4_mul_dispatched[{lane_name}]");
    c.bench_function(&id, |bench| {
        bench.iter(|| black_box(black_box(a) * black_box(b)))
    });
}

fn bench_raw_kernels(c: &mut Criterion) {
    let a = sample_operand::<4>(0x1234);
    let b = sample_operand::<4>(0xBEEF);
    let mut product = [0u64; 8];

    c.bench_function("gf2m_wide4_scalar_clmul_only", |bench| {
        bench.iter(|| {
            product.fill(0);
            clmul_wide_slice_portable::<4>(black_box(&a), black_box(&b), &mut product);
            black_box(&product);
        })
    });

    #[cfg(feature = "simd")]
    {
        if let Some(fns) = gf2_kernels_simd::gf2m_wide::detect() {
            let id = format!("gf2m_wide4_simd_clmul_only[{}]", fns.name);
            c.bench_function(&id, |bench| {
                bench.iter(|| {
                    product.fill(0);
                    (fns.clmul)(black_box(&a), black_box(&b), &mut product);
                    black_box(&product);
                })
            });
        }
    }
}

fn bench_scalar_clmul_plus_barrett_m571(c: &mut Criterion) {
    let a = sample_operand_m571(0x5710_1234);
    let b = sample_operand_m571(0x5710_BEEF);
    let reducer = make_reducer_m571();
    let mut product = [0u64; 18];

    c.bench_function("gf2m_wide9_m571_scalar_clmul_barrett", |bench| {
        bench.iter(|| {
            product.fill(0);
            clmul_wide_slice_portable::<9>(black_box(&a), black_box(&b), &mut product);
            let reduced = reducer.reduce_slice(&product);
            black_box(reduced)
        })
    });
}

fn bench_mul_ref_dispatched_m571(c: &mut Criterion) {
    let a = Gf2mWide::<9, Gf2m571Config>::new(sample_operand_m571(0x5710_1234));
    let b = Gf2mWide::<9, Gf2m571Config>::new(sample_operand_m571(0x5710_BEEF));

    #[cfg(feature = "simd")]
    let lane_name = gf2_kernels_simd::gf2m_wide::detect_571()
        .map(|f| f.name)
        .unwrap_or("scalar-fallback");
    #[cfg(not(feature = "simd"))]
    let lane_name = "scalar-fallback";

    let id = format!("gf2m_wide9_m571_mul_dispatched[{lane_name}]");
    c.bench_function(&id, |bench| {
        bench.iter(|| black_box(black_box(a) * black_box(b)))
    });
}

fn bench_raw_kernels_m571(c: &mut Criterion) {
    let a = sample_operand_m571(0x5710_1234);
    let b = sample_operand_m571(0x5710_BEEF);
    let mut product = [0u64; 18];

    c.bench_function("gf2m_wide9_m571_scalar_clmul_only", |bench| {
        bench.iter(|| {
            product.fill(0);
            clmul_wide_slice_portable::<9>(black_box(&a), black_box(&b), &mut product);
            black_box(&product);
        })
    });

    #[cfg(feature = "simd")]
    {
        if let Some(fns) = gf2_kernels_simd::gf2m_wide::detect_571() {
            let id = format!("gf2m_wide9_m571_simd_clmul_only[{}]", fns.name);
            c.bench_function(&id, |bench| {
                bench.iter(|| {
                    product.fill(0);
                    (fns.clmul)(black_box(&a), black_box(&b), &mut product);
                    black_box(&product);
                })
            });
        }
    }
}
criterion_group!(
    benches,
    bench_scalar_clmul_plus_barrett,
    bench_mul_ref_dispatched,
    bench_raw_kernels,
    bench_scalar_clmul_plus_barrett_m571,
    bench_mul_ref_dispatched_m571,
    bench_raw_kernels_m571,
);
criterion_main!(benches);
