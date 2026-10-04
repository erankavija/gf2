//! Benchmarks `gemm` on square and skinny-output rectangular shapes over prime
//! fields, GF(2^8) and GF(2^16), with inputs derived from the shared bench
//! seed.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use gf2_core::field::matrix::{gemm, FieldMatrix};
use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};

#[path = "common/seed.rs"]
mod seed;

use seed::{derive_seed, fp_matrix_from_seed, gf2m_wide_1_matrix_from_seed, MASTER_SEED};

const PRIME_7: u64 = 7;
const PRIME_11: u64 = 11;
const PRIME_13: u64 = 13;
const PRIME_17: u64 = 17;
const PRIME_19: u64 = 19;
const PRIME_23: u64 = 23;
const PRIME_29: u64 = 29;
const PRIME_31: u64 = 31;
const PRIME_127: u64 = 127;
const PRIME_241: u64 = 241;
const PRIME_251: u64 = 251;
const PRIME_65521: u64 = 65521;
const PRIME_65537: u64 = 65537;
const MERSENNE_31: u64 = 2_147_483_647;

const SQUARE_SIZES_SMALL_PRIME: &[usize] = &[256, 1024];

const SQUARE_SIZES_SMALL_PRIME_N64_TO_4096: &[usize] = &[64, 256, 1024, 4096];

const SQUARE_SIZES_GF31_SMALL_N: &[usize] = &[64, 256, 1024, 4096];

// 257 is the smallest prime above 251, 8191 = 2^13 - 1, and 32749 is the
// largest prime below 2^15.
const PRIME_257: u64 = 257;
const PRIME_8191: u64 = 8191;
const PRIME_32749: u64 = 32749;
const SQUARE_SIZES_MEDIUM: &[usize] = &[64, 256, 1024];

/// GF(2^8) with the AES polynomial `x^8 + x^4 + x^3 + x + 1` (`@/citation/Nist2001`).
struct GemmGf2m8Cfg;
impl Gf2mWideConfig<1> for GemmGf2m8Cfg {
    const M: usize = 8;
    const MODULUS: [u64; 1] = [0x1B];
    const NAME: &'static str = "GemmGf2m8Cfg";
}
type Gf2m8 = Gf2mWide<1, GemmGf2m8Cfg>;

/// GF(2^16) with the Conway polynomial `x^16 + x^5 + x^3 + x^2 + 1` (`@/citation/Lubeck2024`).
struct GemmGf2m16Cfg;
impl Gf2mWideConfig<1> for GemmGf2m16Cfg {
    const M: usize = 16;
    const MODULUS: [u64; 1] = [0x002D];
    const NAME: &'static str = "GemmGf2m16Cfg";
}
type Gf2m16 = Gf2mWide<1, GemmGf2m16Cfg>;

// Order matters: a size's index is its `derive_seed` salt.
const SQUARE_SIZES: &[usize] = &[64, 256, 1024, 4096];

/// `(m, k, n)` shapes with a skinny output: `1024^0.5 = 32`, `1024^0.3 = 8`.
const RECT_SHAPES: &[(usize, usize, usize)] = &[(1024, 1024, 32), (1024, 1024, 8)];

fn bench_square<F, FillFn>(
    c: &mut Criterion,
    group_name: &str,
    field_label: &str,
    sizes: &[usize],
    mut fill: FillFn,
) where
    F: gf2_core::field::FiniteField,
    FillFn: FnMut(usize, usize, u64) -> FieldMatrix<F>,
{
    let mut group = c.benchmark_group(group_name);
    group.sample_size(10);
    group.measurement_time(std::time::Duration::from_secs(5));
    for (si, &n) in sizes.iter().enumerate() {
        group.throughput(Throughput::Elements(seed::ops_gemm(n, n, n) as u64));
        let seed_a = derive_seed(MASTER_SEED, "fgemm", 0, si as u64, 0);
        let seed_b = derive_seed(MASTER_SEED, "fgemm_b", 0, si as u64, 0);
        let a = fill(n, n, seed_a);
        let b = fill(n, n, seed_b);
        group.bench_with_input(BenchmarkId::new(field_label, n), &n, |bench, _| {
            bench.iter(|| {
                let out = gemm(black_box(&a), black_box(&b));
                black_box(out);
            });
        });
    }
    group.finish();
}

fn bench_rect<F, FillFn>(
    c: &mut Criterion,
    group_name: &str,
    field_label: &str,
    shapes: &[(usize, usize, usize)],
    mut fill: FillFn,
) where
    F: gf2_core::field::FiniteField,
    FillFn: FnMut(usize, usize, u64) -> FieldMatrix<F>,
{
    let mut group = c.benchmark_group(group_name);
    group.sample_size(10);
    group.measurement_time(std::time::Duration::from_secs(5));
    for (si, &(m, k, n)) in shapes.iter().enumerate() {
        group.throughput(Throughput::Elements(seed::ops_gemm(m, k, n) as u64));
        let size_idx = (SQUARE_SIZES.len() + si) as u64;
        let seed_a = derive_seed(MASTER_SEED, "fgemm_rect", 0, size_idx, 0);
        let seed_b = derive_seed(MASTER_SEED, "fgemm_rect_b", 0, size_idx, 0);
        let a = fill(m, k, seed_a);
        let b = fill(k, n, seed_b);
        let id = BenchmarkId::new(field_label, format!("{m}x{k}x{n}"));
        group.bench_with_input(id, &(m, k, n), |bench, _| {
            bench.iter(|| {
                let out = gemm(black_box(&a), black_box(&b));
                black_box(out);
            });
        });
    }
    group.finish();
}

fn bench_gemm_fp_7(c: &mut Criterion) {
    bench_square::<gf2_core::gfp::Fp<PRIME_7>, _>(
        c,
        "gemm/Fp_7",
        "Fp_7",
        SQUARE_SIZES,
        fp_matrix_from_seed::<PRIME_7>,
    );
    bench_rect::<gf2_core::gfp::Fp<PRIME_7>, _>(
        c,
        "gemm_rect/Fp_7",
        "Fp_7",
        RECT_SHAPES,
        fp_matrix_from_seed::<PRIME_7>,
    );
}

fn bench_gemm_fp_11(c: &mut Criterion) {
    bench_square::<gf2_core::gfp::Fp<PRIME_11>, _>(
        c,
        "gemm/Fp_11",
        "Fp_11",
        SQUARE_SIZES_SMALL_PRIME,
        fp_matrix_from_seed::<PRIME_11>,
    );
}

fn bench_gemm_fp_13(c: &mut Criterion) {
    bench_square::<gf2_core::gfp::Fp<PRIME_13>, _>(
        c,
        "gemm/Fp_13",
        "Fp_13",
        SQUARE_SIZES_SMALL_PRIME,
        fp_matrix_from_seed::<PRIME_13>,
    );
}

fn bench_gemm_fp_17(c: &mut Criterion) {
    bench_square::<gf2_core::gfp::Fp<PRIME_17>, _>(
        c,
        "gemm/Fp_17",
        "Fp_17",
        SQUARE_SIZES_SMALL_PRIME,
        fp_matrix_from_seed::<PRIME_17>,
    );
}

fn bench_gemm_fp_19(c: &mut Criterion) {
    bench_square::<gf2_core::gfp::Fp<PRIME_19>, _>(
        c,
        "gemm/Fp_19",
        "Fp_19",
        SQUARE_SIZES_SMALL_PRIME,
        fp_matrix_from_seed::<PRIME_19>,
    );
}

fn bench_gemm_fp_23(c: &mut Criterion) {
    bench_square::<gf2_core::gfp::Fp<PRIME_23>, _>(
        c,
        "gemm/Fp_23",
        "Fp_23",
        SQUARE_SIZES_SMALL_PRIME,
        fp_matrix_from_seed::<PRIME_23>,
    );
}

fn bench_gemm_fp_29(c: &mut Criterion) {
    bench_square::<gf2_core::gfp::Fp<PRIME_29>, _>(
        c,
        "gemm/Fp_29",
        "Fp_29",
        SQUARE_SIZES_SMALL_PRIME,
        fp_matrix_from_seed::<PRIME_29>,
    );
}

fn bench_gemm_fp_31(c: &mut Criterion) {
    bench_square::<gf2_core::gfp::Fp<PRIME_31>, _>(
        c,
        "gemm/Fp_31",
        "Fp_31",
        SQUARE_SIZES_GF31_SMALL_N,
        fp_matrix_from_seed::<PRIME_31>,
    );
}

fn bench_gemm_fp_127(c: &mut Criterion) {
    bench_square::<gf2_core::gfp::Fp<PRIME_127>, _>(
        c,
        "gemm/Fp_127",
        "Fp_127",
        SQUARE_SIZES_SMALL_PRIME_N64_TO_4096,
        fp_matrix_from_seed::<PRIME_127>,
    );
}

fn bench_gemm_fp_241(c: &mut Criterion) {
    bench_square::<gf2_core::gfp::Fp<PRIME_241>, _>(
        c,
        "gemm/Fp_241",
        "Fp_241",
        SQUARE_SIZES_SMALL_PRIME_N64_TO_4096,
        fp_matrix_from_seed::<PRIME_241>,
    );
}

fn bench_gemm_fp_251(c: &mut Criterion) {
    let route_a = std::env::var("GF2_GF251_ROUTE_A")
        .map(|v| v == "1")
        .unwrap_or(false);
    let route_c = std::env::var("GF2_GF251_ROUTE_C")
        .map(|v| v == "1")
        .unwrap_or(false);
    gf2_core::gfp::simd_ops::set_route_a_gf251_enabled(route_a);
    gf2_core::gfp::simd_ops::set_route_c_gf251_enabled(route_c);
    bench_square::<gf2_core::gfp::Fp<PRIME_251>, _>(
        c,
        "gemm/Fp_251",
        "Fp_251",
        SQUARE_SIZES,
        fp_matrix_from_seed::<PRIME_251>,
    );
    bench_rect::<gf2_core::gfp::Fp<PRIME_251>, _>(
        c,
        "gemm_rect/Fp_251",
        "Fp_251",
        RECT_SHAPES,
        fp_matrix_from_seed::<PRIME_251>,
    );
    // The route toggles are process-wide: restore the default for later groups.
    gf2_core::gfp::simd_ops::set_route_a_gf251_enabled(false);
    gf2_core::gfp::simd_ops::set_route_c_gf251_enabled(false);
}

fn bench_gemm_fp_65521(c: &mut Criterion) {
    bench_square::<gf2_core::gfp::Fp<PRIME_65521>, _>(
        c,
        "gemm/Fp_65521",
        "Fp_65521",
        SQUARE_SIZES,
        fp_matrix_from_seed::<PRIME_65521>,
    );
    bench_rect::<gf2_core::gfp::Fp<PRIME_65521>, _>(
        c,
        "gemm_rect/Fp_65521",
        "Fp_65521",
        RECT_SHAPES,
        fp_matrix_from_seed::<PRIME_65521>,
    );
}

fn bench_gemm_fp_65537(c: &mut Criterion) {
    bench_square::<gf2_core::gfp::Fp<PRIME_65537>, _>(
        c,
        "gemm/Fp_65537",
        "Fp_65537",
        SQUARE_SIZES,
        fp_matrix_from_seed::<PRIME_65537>,
    );
    bench_rect::<gf2_core::gfp::Fp<PRIME_65537>, _>(
        c,
        "gemm_rect/Fp_65537",
        "Fp_65537",
        RECT_SHAPES,
        fp_matrix_from_seed::<PRIME_65537>,
    );
}

fn bench_gemm_fp_m31(c: &mut Criterion) {
    bench_square::<gf2_core::gfp::Fp<MERSENNE_31>, _>(
        c,
        "gemm/Fp_M31",
        "Fp_M31",
        SQUARE_SIZES,
        fp_matrix_from_seed::<MERSENNE_31>,
    );
    bench_rect::<gf2_core::gfp::Fp<MERSENNE_31>, _>(
        c,
        "gemm_rect/Fp_M31",
        "Fp_M31",
        RECT_SHAPES,
        fp_matrix_from_seed::<MERSENNE_31>,
    );
}

fn bench_gemm_fp_257(c: &mut Criterion) {
    bench_square::<gf2_core::gfp::Fp<PRIME_257>, _>(
        c,
        "gemm/Fp_257",
        "Fp_257",
        SQUARE_SIZES_MEDIUM,
        fp_matrix_from_seed::<PRIME_257>,
    );
}

fn bench_gemm_fp_8191(c: &mut Criterion) {
    bench_square::<gf2_core::gfp::Fp<PRIME_8191>, _>(
        c,
        "gemm/Fp_8191",
        "Fp_8191",
        SQUARE_SIZES_MEDIUM,
        fp_matrix_from_seed::<PRIME_8191>,
    );
}

fn bench_gemm_fp_32749(c: &mut Criterion) {
    bench_square::<gf2_core::gfp::Fp<PRIME_32749>, _>(
        c,
        "gemm/Fp_32749",
        "Fp_32749",
        SQUARE_SIZES_MEDIUM,
        fp_matrix_from_seed::<PRIME_32749>,
    );
}

fn bench_gemm_gf2m8(c: &mut Criterion) {
    bench_square::<Gf2m8, _>(
        c,
        "gemm/Gf2m8",
        "Gf2m8",
        SQUARE_SIZES,
        gf2m_wide_1_matrix_from_seed::<GemmGf2m8Cfg>,
    );
    bench_rect::<Gf2m8, _>(
        c,
        "gemm_rect/Gf2m8",
        "Gf2m8",
        RECT_SHAPES,
        gf2m_wide_1_matrix_from_seed::<GemmGf2m8Cfg>,
    );
}

fn bench_gemm_gf2m16(c: &mut Criterion) {
    bench_square::<Gf2m16, _>(
        c,
        "gemm/Gf2m16",
        "Gf2m16",
        SQUARE_SIZES,
        gf2m_wide_1_matrix_from_seed::<GemmGf2m16Cfg>,
    );
    bench_rect::<Gf2m16, _>(
        c,
        "gemm_rect/Gf2m16",
        "Gf2m16",
        RECT_SHAPES,
        gf2m_wide_1_matrix_from_seed::<GemmGf2m16Cfg>,
    );
}

criterion_group! {
    name = gemm_benches;
    config = Criterion::default()
        .sample_size(10)
        .measurement_time(std::time::Duration::from_secs(5));
    targets =
        bench_gemm_fp_7,
        bench_gemm_fp_11,
        bench_gemm_fp_13,
        bench_gemm_fp_17,
        bench_gemm_fp_19,
        bench_gemm_fp_23,
        bench_gemm_fp_29,
        bench_gemm_fp_31,
        bench_gemm_fp_127,
        bench_gemm_fp_241,
        bench_gemm_fp_251,
        bench_gemm_fp_257,
        bench_gemm_fp_8191,
        bench_gemm_fp_32749,
        bench_gemm_fp_65521,
        bench_gemm_fp_65537,
        bench_gemm_fp_m31,
        bench_gemm_gf2m8,
        bench_gemm_gf2m16
}
criterion_main!(gemm_benches);
