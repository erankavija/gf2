//! Benchmarks for `Gf2mWide<4>::mul` (GF(2^256)).
//!
//! Two groups:
//!
//! 1. **scalar-baseline** — the pure-Rust `clmul_wide_slice::<4>` + Barrett
//!    reduction path. This is what `Gf2mWide::<4, _>::mul` executes when the
//!    `simd` feature is disabled or PCLMULQDQ is unavailable.
//!
//! 2. **simd-kernel** — the dispatched kernel returned by
//!    `gf2_kernels_simd::gf2m_wide::detect()` (AVX2+VPCLMULQDQ YMM on Zen 3,
//!    PCLMULQDQ scalar-XMM elsewhere, AVX-512VL+VPCLMULQDQ ZMM on capable
//!    hosts). The group name is suffixed with the kernel tag from
//!    `ClmulWide256Fns::name` so the benchmark output identifies which lane
//!    ran. Falls back to scalar if no PCLMULQDQ is present.
//!
//! # Running
//!
//! ```text
//! cargo bench -p gf2-core --bench gf2m_wide_mul -- --quick
//! ```

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use gf2_core::gf2m::barrett::BarrettReducerWide;
use gf2_core::gf2m::wide::clmul_wide_slice;
use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};

/// GF(2^256), Seroussi HPL-98-135 Table 1 row m = 256: `x^256 + x^10 + x^5 + x^2 + 1`.
///
/// Matches the canonical test config used throughout `Gf2mWide<4>` tests.
struct Gf2m256Config;

impl Gf2mWideConfig<4> for Gf2m256Config {
    const M: usize = 256;
    const MODULUS: [u64; 4] = [0x425, 0, 0, 0];
    const NAME: &'static str = "Gf2m256Config";
}

fn sample_operand(seed: u64) -> [u64; 4] {
    // Cheap deterministic fill — we just need bit-dense, irregular operands.
    let mut s = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(1);
    let mut out = [0u64; 4];
    for slot in &mut out {
        s = s
            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
            .wrapping_add(0xDEAD_BEEF);
        *slot = s;
    }
    out
}

/// Fresh Barrett reducer for GF(2^256). Allocated once per benchmark group
/// to avoid measuring cache warmup.
fn make_reducer() -> BarrettReducerWide<4> {
    BarrettReducerWide::<4>::new(Gf2m256Config::MODULUS, 256)
}

// ---------------------------------------------------------------------------
// Scalar baseline: clmul_wide_slice + Barrett (the non-SIMD path of Mul)
// ---------------------------------------------------------------------------

fn bench_scalar_clmul_plus_barrett(c: &mut Criterion) {
    let a = sample_operand(0x1234);
    let b = sample_operand(0xBEEF);
    let reducer = make_reducer();
    let mut product = [0u64; 8];

    c.bench_function("gf2m_wide4_scalar_clmul_barrett", |bench| {
        bench.iter(|| {
            // Zero the buffer — `clmul_wide_slice` XOR-accumulates.
            product.fill(0);
            clmul_wide_slice::<4>(black_box(&a), black_box(&b), &mut product);
            let reduced = reducer.reduce_slice(&product);
            black_box(reduced)
        })
    });
}

// ---------------------------------------------------------------------------
// End-to-end multiplication via the public `Mul` impl, which internally
// selects SIMD or scalar depending on the host.
// ---------------------------------------------------------------------------

fn bench_mul_ref_dispatched(c: &mut Criterion) {
    let a = Gf2mWide::<4, Gf2m256Config>::new(sample_operand(0x1234));
    let b = Gf2mWide::<4, Gf2m256Config>::new(sample_operand(0xBEEF));

    // Identify the lane the dispatcher chose. The `Mul` impl reaches into the
    // same `OnceLock`, so we re-read it here purely for the benchmark label.
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

// ---------------------------------------------------------------------------
// Direct SIMD-kernel benchmark (raw clmul only, no Barrett) — lets us see the
// pre-reduction SIMD speedup in isolation. The scalar baseline above
// includes Barrett, so compare against `gf2m_wide4_scalar_clmul_only` for a
// fair kernel-vs-kernel comparison.
// ---------------------------------------------------------------------------

fn bench_raw_kernels(c: &mut Criterion) {
    let a = sample_operand(0x1234);
    let b = sample_operand(0xBEEF);
    let mut product = [0u64; 8];

    c.bench_function("gf2m_wide4_scalar_clmul_only", |bench| {
        bench.iter(|| {
            product.fill(0);
            clmul_wide_slice::<4>(black_box(&a), black_box(&b), &mut product);
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

criterion_group!(
    benches,
    bench_scalar_clmul_plus_barrett,
    bench_mul_ref_dispatched,
    bench_raw_kernels,
);
criterion_main!(benches);
