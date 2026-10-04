//! Benchmarks [`FieldMatrix::charpoly`], [`FieldMatrix::minpoly`] and
//! [`FieldMatrix::frobenius_form`] over prime fields and GF(2^8), and the
//! cubic and Keller–Gehrig characteristic-polynomial paths side by side.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use gf2_core::field::matrix::FieldMatrix;
use gf2_core::field::test_random_matrix::{random_fp, random_gf2m_wide_1};
use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};

const PRIME_65521: u64 = 65521;
const PRIME_251: u64 = 251;
const PRIME_31: u64 = 31;
const PRIME_7: u64 = 7;
const MERSENNE_31: u64 = 2_147_483_647;

/// GF(2^8) with the AES polynomial (`@/citation/Nist2001`).
struct CpBenchGf2m8Cfg;
impl Gf2mWideConfig<1> for CpBenchGf2m8Cfg {
    const M: usize = 8;
    const MODULUS: [u64; 1] = [0x1B];
    const NAME: &'static str = "CpBenchGf2m8Cfg";
}
type Gf2m8 = Gf2mWide<1, CpBenchGf2m8Cfg>;

const SIZES: &[usize] = &[32, 128, 512];

fn random_gf2m8(rows: usize, cols: usize, seed: u64) -> FieldMatrix<Gf2m8> {
    random_gf2m_wide_1::<CpBenchGf2m8Cfg>(rows, cols, seed)
}

fn bench_charpoly(c: &mut Criterion) {
    let mut group = c.benchmark_group("charpoly/charpoly");
    for &n in SIZES {
        let a_fp = random_fp::<PRIME_65521>(n, n, 0xCAFE);
        let a_gf = random_gf2m8(n, n, 0xCAFE);
        group.bench_with_input(BenchmarkId::new("Fp_65521", n), &n, |b, _| {
            b.iter(|| {
                let r = black_box(&a_fp).charpoly();
                black_box(r);
            });
        });
        group.bench_with_input(BenchmarkId::new("Gf2m8", n), &n, |b, _| {
            b.iter(|| {
                let r = black_box(&a_gf).charpoly();
                black_box(r);
            });
        });
    }
    group.finish();
}

fn bench_minpoly(c: &mut Criterion) {
    let mut group = c.benchmark_group("charpoly/minpoly");
    for &n in SIZES {
        let a_fp = random_fp::<PRIME_65521>(n, n, 0xBEEF);
        let a_gf = random_gf2m8(n, n, 0xBEEF);
        group.bench_with_input(BenchmarkId::new("Fp_65521", n), &n, |b, _| {
            b.iter(|| {
                let r = black_box(&a_fp).minpoly();
                black_box(r);
            });
        });
        group.bench_with_input(BenchmarkId::new("Gf2m8", n), &n, |b, _| {
            b.iter(|| {
                let r = black_box(&a_gf).minpoly();
                black_box(r);
            });
        });
    }
    group.finish();
}

fn bench_frobenius(c: &mut Criterion) {
    let mut group = c.benchmark_group("charpoly/frobenius");
    for &n in SIZES {
        let a_fp = random_fp::<PRIME_65521>(n, n, 0xC0DE);
        let a_gf = random_gf2m8(n, n, 0xC0DE);
        group.bench_with_input(BenchmarkId::new("Fp_65521", n), &n, |b, _| {
            b.iter(|| {
                let r = black_box(&a_fp).frobenius_form();
                black_box(r);
            });
        });
        group.bench_with_input(BenchmarkId::new("Gf2m8", n), &n, |b, _| {
            b.iter(|| {
                let r = black_box(&a_gf).frobenius_form();
                black_box(r);
            });
        });
    }
    group.finish();
}

fn bench_minpoly_reference_sweep(c: &mut Criterion) {
    let mut group = c.benchmark_group("charpoly/minpoly_ref");
    group.sample_size(10);
    const SIZES: &[usize] = &[64, 256];
    for &n in SIZES {
        let a = random_fp::<MERSENNE_31>(n, n, 0xBEEF_0001);
        group.bench_with_input(BenchmarkId::new("Fp_M31", n), &n, |b, _| {
            b.iter(|| black_box(black_box(&a).minpoly()));
        });
        let a = random_fp::<PRIME_65521>(n, n, 0xBEEF_0002);
        group.bench_with_input(BenchmarkId::new("Fp_65521", n), &n, |b, _| {
            b.iter(|| black_box(black_box(&a).minpoly()));
        });
        let a = random_fp::<PRIME_251>(n, n, 0xBEEF_0003);
        group.bench_with_input(BenchmarkId::new("Fp_251", n), &n, |b, _| {
            b.iter(|| black_box(black_box(&a).minpoly()));
        });
        let a = random_fp::<PRIME_7>(n, n, 0xBEEF_0004);
        group.bench_with_input(BenchmarkId::new("Fp_7", n), &n, |b, _| {
            b.iter(|| black_box(black_box(&a).minpoly()));
        });
        let a = random_fp::<PRIME_31>(n, n, 0xBEEF_0031);
        group.bench_with_input(BenchmarkId::new("Fp_31", n), &n, |b, _| {
            b.iter(|| black_box(black_box(&a).minpoly()));
        });
    }
    group.finish();
}

fn bench_charpoly_reference_sweep(c: &mut Criterion) {
    let mut group = c.benchmark_group("charpoly/charpoly_ref");
    group.sample_size(10);
    const SIZES: &[usize] = &[64, 256];
    for &n in SIZES {
        let a = random_fp::<MERSENNE_31>(n, n, 0xC4F0_0001);
        group.bench_with_input(BenchmarkId::new("Fp_M31", n), &n, |b, _| {
            b.iter(|| black_box(black_box(&a).charpoly()));
        });
        let a = random_fp::<PRIME_65521>(n, n, 0xC4F0_0002);
        group.bench_with_input(BenchmarkId::new("Fp_65521", n), &n, |b, _| {
            b.iter(|| black_box(black_box(&a).charpoly()));
        });
        let a = random_fp::<PRIME_251>(n, n, 0xC4F0_0003);
        group.bench_with_input(BenchmarkId::new("Fp_251", n), &n, |b, _| {
            b.iter(|| black_box(black_box(&a).charpoly()));
        });
        let a = random_fp::<PRIME_7>(n, n, 0xC4F0_0004);
        group.bench_with_input(BenchmarkId::new("Fp_7", n), &n, |b, _| {
            b.iter(|| black_box(black_box(&a).charpoly()));
        });
        let a = random_fp::<PRIME_31>(n, n, 0xC4F0_0031);
        group.bench_with_input(BenchmarkId::new("Fp_31", n), &n, |b, _| {
            b.iter(|| black_box(black_box(&a).charpoly()));
        });
    }
    group.finish();
}

fn bench_dispatch_arms<const P: u64>(
    c: &mut Criterion,
    group_name: &str,
    sizes: &[usize],
    include_dispatch: bool,
    kg_panic_msg: &'static str,
) {
    let mut group = c.benchmark_group(group_name);
    group.sample_size(10);
    for &n in sizes {
        let a = random_fp::<P>(n, n, 0xDEAD_BEEF);
        group.bench_with_input(BenchmarkId::new("cubic", n), &n, |b, _| {
            b.iter(|| {
                let r = black_box(&a).charpoly_cubic();
                black_box(r);
            });
        });
        group.bench_with_input(BenchmarkId::new("kg", n), &n, |b, _| {
            b.iter(|| {
                let r = black_box(&a)
                    .charpoly_keller_gehrig(0xC0FFEE)
                    .expect(kg_panic_msg);
                black_box(r);
            });
        });
        if include_dispatch {
            group.bench_with_input(BenchmarkId::new("dispatch", n), &n, |b, _| {
                b.iter(|| {
                    let r = black_box(&a).charpoly();
                    black_box(r);
                });
            });
        }
    }
    group.finish();
}

fn bench_dispatch_crossover(c: &mut Criterion) {
    bench_dispatch_arms::<MERSENNE_31>(
        c,
        "charpoly/dispatch",
        &[64, 128, 256, 512, 1024],
        true,
        "KG must converge on Fp<MERSENNE_31>",
    );
}

/// Keller–Gehrig requires `q > 2n²`: for `q = 65521` that holds at `n = 128`
/// (`2·128² = 32768`) and fails from `n = 181`.
fn bench_dispatch_crossover_fp65521(c: &mut Criterion) {
    bench_dispatch_arms::<PRIME_65521>(
        c,
        "charpoly/dispatch_fp65521",
        &[64, 128],
        false,
        "KG must converge on Fp<65521>",
    );
}

criterion_group! {
    name = charpoly_benches;
    config = Criterion::default()
        .sample_size(10)
        .measurement_time(std::time::Duration::from_secs(5));
    targets = bench_charpoly, bench_minpoly, bench_frobenius,
        bench_minpoly_reference_sweep,
        bench_charpoly_reference_sweep,
        bench_dispatch_crossover, bench_dispatch_crossover_fp65521
}
criterion_main!(charpoly_benches);
