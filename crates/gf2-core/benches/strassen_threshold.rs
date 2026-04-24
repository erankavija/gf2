//! Strassen–Winograd threshold sweep and classical-vs-Winograd comparison.
//!
//! Issue `ad597ede`, story `d48a3cfd/T3`. Measures:
//!
//! 1. Classical `gemm` vs `gemm_winograd` at `n ∈ {256, 512, 1024, 2048,
//!    4096}` for both `Fp<2^31 - 1>` (Mersenne-31) and `Gf2mWide<1, AES>`
//!    GF(2^8).
//! 2. Threshold sweep at `n = 2048` over Mersenne-31: records the Winograd
//!    runtime for `WINO_THRESHOLD ∈ {32, 64, 128, 256, 512}` — the winning
//!    value feeds the committed `pub const WINO_THRESHOLD` in
//!    `gf2-core::field::winograd`.
//!
//! The recorded results live in `benches/strassen_threshold_results.md`.
//!
//! ## Usage
//!
//! ```bash
//! cargo bench -p gf2-core --bench strassen_threshold --features rand
//! # Smoke run:
//! cargo bench -p gf2-core --bench strassen_threshold --features rand -- --test
//! ```
//!
//! The `n = 4096` case is expensive (≈ minutes per sample on a
//! commodity core); the `threshold_sweep` group is the most useful
//! artefact for retuning the crossover.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use gf2_core::field::matrix::{gemm, FieldMatrix};
use gf2_core::field::winograd::gemm_winograd;
use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
use gf2_core::gfp::Fp;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

// Mersenne-31 prime.
const MERSENNE_31: u64 = 2_147_483_647;

/// GF(2^8) with AES irreducible `x^8 + x^4 + x^3 + x + 1`. Implicit leading
/// bit convention per `Gf2mWideConfig` ⇒ low byte is `0x1B`.
struct StrassenGf2m8Cfg;
impl Gf2mWideConfig<1> for StrassenGf2m8Cfg {
    const M: usize = 8;
    const MODULUS: [u64; 1] = [0x1B];
    const NAME: &'static str = "StrassenGf2m8Cfg";
}
type Gf2m8 = Gf2mWide<1, StrassenGf2m8Cfg>;

// Compare at `n ≥ 256` so the Winograd peel actually fires (default
// threshold = 128). n = 4096 takes minutes per sample on a scalar path;
// Criterion owners may comment it out for faster iteration.
const COMPARE_SIZES: &[usize] = &[256, 512, 1024, 2048, 4096];

// Threshold-sweep: we can't reassign the crate's `WINO_THRESHOLD` at
// runtime without recompiling, so instead we emulate it by directly
// calling a small recursive helper parameterised on the threshold and
// comparing its runtime. The wrapper routes through the same
// `gemm_winograd` call chain except with a different fallback size.
fn winograd_with_threshold<F: gf2_core::field::FiniteField>(
    a: &FieldMatrix<F>,
    b: &FieldMatrix<F>,
    threshold: usize,
) -> FieldMatrix<F> {
    // Delegate degenerate cases.
    let (m, k) = a.shape();
    let n = b.cols();
    if m == 0 || k == 0 || n == 0 || m.min(k).min(n) < threshold {
        return gemm(a, b);
    }
    // Pad odd dims, peel, recurse.
    let m_even = m + (m & 1);
    let k_even = k + (k & 1);
    let n_even = n + (n & 1);
    let zero = a.get(0, 0).zero_like();
    let a_padded = pad_to(a, m_even, k_even, &zero);
    let b_padded = pad_to(b, k_even, n_even, &zero);

    let mh = m_even / 2;
    let kh = k_even / 2;
    let nh = n_even / 2;

    let a11 = submat(&a_padded, 0, 0, mh, kh, &zero);
    let a12 = submat(&a_padded, 0, kh, mh, kh, &zero);
    let a21 = submat(&a_padded, mh, 0, mh, kh, &zero);
    let a22 = submat(&a_padded, mh, kh, mh, kh, &zero);
    let b11 = submat(&b_padded, 0, 0, kh, nh, &zero);
    let b12 = submat(&b_padded, 0, nh, kh, nh, &zero);
    let b21 = submat(&b_padded, kh, 0, kh, nh, &zero);
    let b22 = submat(&b_padded, kh, nh, kh, nh, &zero);

    let s1 = add_m(&a21, &a22);
    let s2 = sub_m(&s1, &a11);
    let s3 = sub_m(&a11, &a21);
    let s4 = sub_m(&a12, &s2);

    let t1 = sub_m(&b12, &b11);
    let t2 = sub_m(&b22, &t1);
    let t3 = sub_m(&b22, &b12);
    let t4 = sub_m(&t2, &b21);

    let m1 = winograd_with_threshold(&a11, &b11, threshold);
    let m2 = winograd_with_threshold(&a12, &b21, threshold);
    let m3 = winograd_with_threshold(&s4, &b22, threshold);
    let m4 = winograd_with_threshold(&a22, &t4, threshold);
    let m5 = winograd_with_threshold(&s1, &t1, threshold);
    let m6 = winograd_with_threshold(&s2, &t2, threshold);
    let m7 = winograd_with_threshold(&s3, &t3, threshold);

    let c11 = add_m(&m1, &m2);
    let u2 = add_m(&m1, &m6);
    let u3 = add_m(&u2, &m7);
    let u4 = add_m(&u2, &m5);
    let c12 = add_m(&u4, &m3);
    let c21 = sub_m(&u3, &m4);
    let c22 = add_m(&u3, &m5);

    let c_padded = assemble(&c11, &c12, &c21, &c22, &zero);
    if (m_even, n_even) == (m, n) {
        c_padded
    } else {
        slice_to(&c_padded, m, n, &zero)
    }
}

fn pad_to<F: gf2_core::field::FiniteField>(
    src: &FieldMatrix<F>,
    rows: usize,
    cols: usize,
    zero: &F,
) -> FieldMatrix<F> {
    let (sr, sc) = src.shape();
    if (sr, sc) == (rows, cols) {
        return src.clone();
    }
    let mut out = FieldMatrix::<F>::new(rows, cols, zero.clone());
    for r in 0..sr {
        for c in 0..sc {
            out.set(r, c, src.get(r, c));
        }
    }
    out
}

fn slice_to<F: gf2_core::field::FiniteField>(
    src: &FieldMatrix<F>,
    rows: usize,
    cols: usize,
    zero: &F,
) -> FieldMatrix<F> {
    let mut out = FieldMatrix::<F>::new(rows, cols, zero.clone());
    for r in 0..rows {
        for c in 0..cols {
            out.set(r, c, src.get(r, c));
        }
    }
    out
}

fn submat<F: gf2_core::field::FiniteField>(
    src: &FieldMatrix<F>,
    ro: usize,
    co: usize,
    rows: usize,
    cols: usize,
    zero: &F,
) -> FieldMatrix<F> {
    let mut out = FieldMatrix::<F>::new(rows, cols, zero.clone());
    for r in 0..rows {
        for c in 0..cols {
            out.set(r, c, src.get(ro + r, co + c));
        }
    }
    out
}

fn add_m<F: gf2_core::field::FiniteField>(
    a: &FieldMatrix<F>,
    b: &FieldMatrix<F>,
) -> FieldMatrix<F> {
    let (rows, cols) = a.shape();
    let zero = a.get(0, 0).zero_like();
    let mut out = FieldMatrix::<F>::new(rows, cols, zero);
    for r in 0..rows {
        for c in 0..cols {
            out.set(r, c, a.get(r, c) + b.get(r, c));
        }
    }
    out
}

fn sub_m<F: gf2_core::field::FiniteField>(
    a: &FieldMatrix<F>,
    b: &FieldMatrix<F>,
) -> FieldMatrix<F> {
    let (rows, cols) = a.shape();
    let zero = a.get(0, 0).zero_like();
    let mut out = FieldMatrix::<F>::new(rows, cols, zero);
    for r in 0..rows {
        for c in 0..cols {
            out.set(r, c, a.get(r, c) - b.get(r, c));
        }
    }
    out
}

fn assemble<F: gf2_core::field::FiniteField>(
    c11: &FieldMatrix<F>,
    c12: &FieldMatrix<F>,
    c21: &FieldMatrix<F>,
    c22: &FieldMatrix<F>,
    zero: &F,
) -> FieldMatrix<F> {
    let (mh, nh) = c11.shape();
    let m = 2 * mh;
    let n = 2 * nh;
    let mut out = FieldMatrix::<F>::new(m, n, zero.clone());
    for r in 0..mh {
        for c in 0..nh {
            out.set(r, c, c11.get(r, c));
            out.set(r, nh + c, c12.get(r, c));
            out.set(mh + r, c, c21.get(r, c));
            out.set(mh + r, nh + c, c22.get(r, c));
        }
    }
    out
}

fn random_fp_matrix<const P: u64>(rows: usize, cols: usize, seed: u64) -> FieldMatrix<Fp<P>> {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut m = FieldMatrix::<Fp<P>>::zeros(rows, cols);
    for r in 0..rows {
        for c in 0..cols {
            m.set(r, c, Fp::<P>::new(rng.gen::<u64>() % P));
        }
    }
    m
}

fn random_gf2m8_matrix(rows: usize, cols: usize, seed: u64) -> FieldMatrix<Gf2m8> {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut m = FieldMatrix::<Gf2m8>::zeros(rows, cols);
    for r in 0..rows {
        for c in 0..cols {
            m.set(r, c, Gf2m8::new([rng.gen::<u64>() & 0xFF]));
        }
    }
    m
}

fn bench_gemm_vs_winograd_fp(c: &mut Criterion) {
    let mut group = c.benchmark_group("strassen_threshold/fp_mersenne31");
    for &n in COMPARE_SIZES {
        group.throughput(Throughput::Elements((n * n * n) as u64));
        let a = random_fp_matrix::<MERSENNE_31>(n, n, 0xAA ^ n as u64);
        let b = random_fp_matrix::<MERSENNE_31>(n, n, 0xBB ^ n as u64);
        group.bench_with_input(BenchmarkId::new("classical", n), &n, |bench, _| {
            bench.iter(|| {
                let out = gemm(black_box(&a), black_box(&b));
                black_box(out);
            });
        });
        group.bench_with_input(BenchmarkId::new("winograd", n), &n, |bench, _| {
            bench.iter(|| {
                let out = gemm_winograd(black_box(&a), black_box(&b));
                black_box(out);
            });
        });
    }
    group.finish();
}

fn bench_gemm_vs_winograd_gf2m8(c: &mut Criterion) {
    let mut group = c.benchmark_group("strassen_threshold/gf2m8_aes");
    for &n in COMPARE_SIZES {
        group.throughput(Throughput::Elements((n * n * n) as u64));
        let a = random_gf2m8_matrix(n, n, 0xCC ^ n as u64);
        let b = random_gf2m8_matrix(n, n, 0xDD ^ n as u64);
        group.bench_with_input(BenchmarkId::new("classical", n), &n, |bench, _| {
            bench.iter(|| {
                let out = gemm(black_box(&a), black_box(&b));
                black_box(out);
            });
        });
        group.bench_with_input(BenchmarkId::new("winograd", n), &n, |bench, _| {
            bench.iter(|| {
                let out = gemm_winograd(black_box(&a), black_box(&b));
                black_box(out);
            });
        });
    }
    group.finish();
}

/// Threshold sweep at `n = 2048` over Mersenne-31. The winning threshold
/// is recorded in `benches/strassen_threshold_results.md` and fed back
/// to the committed `WINO_THRESHOLD` constant.
fn bench_threshold_sweep(c: &mut Criterion) {
    let mut group = c.benchmark_group("strassen_threshold/sweep_fp_mersenne31_n2048");
    let n = 2048;
    let a = random_fp_matrix::<MERSENNE_31>(n, n, 0xEE);
    let b = random_fp_matrix::<MERSENNE_31>(n, n, 0xFF);
    for &threshold in &[32usize, 64, 128, 256, 512, 1024] {
        group.bench_with_input(
            BenchmarkId::from_parameter(threshold),
            &threshold,
            |bench, &t| {
                bench.iter(|| {
                    let out = winograd_with_threshold(black_box(&a), black_box(&b), t);
                    black_box(out);
                });
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_gemm_vs_winograd_fp,
    bench_gemm_vs_winograd_gf2m8,
    bench_threshold_sweep
);
criterion_main!(benches);
