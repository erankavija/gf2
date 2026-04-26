//! Characteristic polynomial / minimal polynomial / Frobenius form —
//! Criterion benches.
//!
//! Issues `f01298db` (cubic baseline) and `1454ec2d` (sub-cubic
//! Keller–Gehrig variant + dispatch crossover sweep).
//!
//! Measures the public entry points [`FieldMatrix::charpoly`],
//! [`FieldMatrix::minpoly`], and [`FieldMatrix::frobenius_form`] at
//! `n ∈ {32, 128, 512}` for `Fp<65521>` and a small `Gf2mWide<8>`
//! configuration. Adds a `charpoly/dispatch/...` group at
//! `n ∈ {64, 128, 256, 512, 1024}` on `Fp<MERSENNE_31>` that benches
//! the cubic and Keller–Gehrig paths side-by-side; the empirical
//! crossover for the `[aspirational]` success criterion of issue
//! `1454ec2d` is read off this group.
//!
//! ```text
//! charpoly/charpoly/Fp_65521/32
//! charpoly/minpoly/Gf2m8/128
//! charpoly/frobenius/Fp_65521/512
//! charpoly/dispatch/cubic/256
//! charpoly/dispatch/kg/256
//! ```
//!
//! ## Usage
//!
//! ```bash
//! cargo bench -p gf2-core --bench charpoly --features rand
//! cargo bench -p gf2-core --bench charpoly --features rand -- --test
//! cargo bench -p gf2-core --bench charpoly --features rand -- charpoly/charpoly/Fp_65521/32
//! cargo bench -p gf2-core --bench charpoly --features rand -- charpoly/dispatch
//! ```

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use gf2_core::field::matrix::FieldMatrix;
use gf2_core::field::test_random_matrix::{random_fp, random_gf2m_wide_1};
use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};

const PRIME_65521: u64 = 65521;
const MERSENNE_31: u64 = 2_147_483_647;

/// GF(2^8) AES irreducible.
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

/// Dispatch-crossover bench (issue `1454ec2d`): runs the cubic and
/// Keller–Gehrig paths side-by-side at
/// `n ∈ {64, 128, 256, 512, 1024}` on `Fp<MERSENNE_31>`.
///
/// Empirically the cubic path is currently ~173× faster than KG at
/// `n = 256` (see `crates/gf2-core/src/field/charpoly.rs` module docs);
/// public [`FieldMatrix::charpoly`] therefore always selects cubic
/// under default dispatch (`KG_DISPATCH_MIN_N == usize::MAX`). The
/// `dispatch` arm of this bench measures the public surface (i.e. the
/// cubic baseline today) and is kept alongside the explicit `cubic`
/// and `kg` arms so a future tuning of `KG_DISPATCH_MIN_N` can be
/// validated against the same fixtures.
///
/// Compiled and skip-runnable via `--test` so the bench harness stays
/// healthy without paying the full `n = 1024` measurement cost.
fn bench_dispatch_crossover(c: &mut Criterion) {
    let sizes: &[usize] = &[64, 128, 256, 512, 1024];
    let mut group = c.benchmark_group("charpoly/dispatch");
    group.sample_size(10);
    for &n in sizes {
        let a = random_fp::<MERSENNE_31>(n, n, 0xDEAD_BEEF);
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
                    .expect("KG must converge on Fp<MERSENNE_31>");
                black_box(r);
            });
        });
        // Public dispatch — picks one of the two paths above based on
        // the runtime decision tree.
        group.bench_with_input(BenchmarkId::new("dispatch", n), &n, |b, _| {
            b.iter(|| {
                let r = black_box(&a).charpoly();
                black_box(r);
            });
        });
    }
    group.finish();
}

criterion_group! {
    name = charpoly_benches;
    config = Criterion::default()
        .sample_size(10)
        .measurement_time(std::time::Duration::from_secs(5));
    targets = bench_charpoly, bench_minpoly, bench_frobenius, bench_dispatch_crossover
}
criterion_main!(charpoly_benches);
