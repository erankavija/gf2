//! Block-recursive triangular primitives — `trsm`, `trmm`, `trtri`, `trtrm`.
//!
//! Issues `83b1ad8b` (initial harness) and `73ec5da3` (TRSM coverage +
//! `TRI_BASE_THRESHOLD` sweep). Measures five primitives (`trsm_upper`,
//! `trsm_lower`, `trmm_upper`, `trtri_upper`, `trtrm`) at
//! `n ∈ {64, 256, 1024}` for `Fp<7>`, `Fp<MERSENNE_31>`, and
//! `Gf2mWide<1, AES>`. Each primitive lives in its own Criterion group so
//! individual cases can be filtered:
//!
//! ```text
//! triangular/trsm_upper/Fp_7/64
//! triangular/trsm_upper/Fp_M31/256
//! triangular/trsm_lower/Fp_M31/1024
//! triangular/trtri_upper/Gf2m8/1024
//! triangular/trtrm/Fp_M31/1024
//! ```
//!
//! ## Usage
//!
//! ```bash
//! cargo bench -p gf2-core --bench triangular --features rand
//! # Smoke run:
//! cargo bench -p gf2-core --bench triangular --features rand -- --test
//! # Filter to a single primitive at a single size:
//! cargo bench -p gf2-core --bench triangular --features rand -- triangular/trsm_upper/Fp_M31/256
//! ```
//!
//! All benches use the default per-field `TRI_BASE_THRESHOLD` (currently
//! 8, selected by Criterion sweep in jit:73ec5da3). The threshold is
//! wired through the `FiniteField` trait so any future override
//! propagates here without bench code changes.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use gf2_core::field::matrix::FieldMatrix;
use gf2_core::field::triangular::{trmm_upper, trsm_lower, trsm_upper, trtri_upper, trtrm};
use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
use gf2_core::gfp::Fp;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

const MERSENNE_31: u64 = 2_147_483_647;

/// GF(2^8) with AES irreducible.
struct TriBenchGf2m8Cfg;
impl Gf2mWideConfig<1> for TriBenchGf2m8Cfg {
    const M: usize = 8;
    const MODULUS: [u64; 1] = [0x1B];
    const NAME: &'static str = "TriBenchGf2m8Cfg";
}
type Gf2m8 = Gf2mWide<1, TriBenchGf2m8Cfg>;

const SIZES: &[usize] = &[64, 256, 1024];

// ─── Random matrix builders ────────────────────────────────────────────────

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

fn random_upper_fp<const P: u64>(n: usize, seed: u64) -> FieldMatrix<Fp<P>> {
    let mut m = random_fp_matrix::<P>(n, n, seed);
    for r in 0..n {
        for c in 0..r {
            m.set(r, c, Fp::<P>::new(0));
        }
        if m.get(r, r) == Fp::<P>::new(0) {
            m.set(r, r, Fp::<P>::new(1));
        }
    }
    m
}

fn random_upper_gf2m8(n: usize, seed: u64) -> FieldMatrix<Gf2m8> {
    let mut m = random_gf2m8_matrix(n, n, seed);
    for r in 0..n {
        for c in 0..r {
            m.set(r, c, Gf2m8::new([0]));
        }
        if m.get(r, r) == Gf2m8::new([0]) {
            m.set(r, r, Gf2m8::new([1]));
        }
    }
    m
}

fn random_lower_fp<const P: u64>(n: usize, seed: u64) -> FieldMatrix<Fp<P>> {
    let mut m = random_fp_matrix::<P>(n, n, seed);
    for r in 0..n {
        for c in (r + 1)..n {
            m.set(r, c, Fp::<P>::new(0));
        }
        if m.get(r, r) == Fp::<P>::new(0) {
            m.set(r, r, Fp::<P>::new(1));
        }
    }
    m
}

fn random_lower_gf2m8(n: usize, seed: u64) -> FieldMatrix<Gf2m8> {
    let mut m = random_gf2m8_matrix(n, n, seed);
    for r in 0..n {
        for c in (r + 1)..n {
            m.set(r, c, Gf2m8::new([0]));
        }
        if m.get(r, r) == Gf2m8::new([0]) {
            m.set(r, r, Gf2m8::new([1]));
        }
    }
    m
}

fn random_strict_lower_fp<const P: u64>(n: usize, seed: u64) -> FieldMatrix<Fp<P>> {
    // Strictly lower (the diagonal is implicit unit for trtrm).
    let mut m = random_fp_matrix::<P>(n, n, seed);
    for r in 0..n {
        for c in r..n {
            m.set(r, c, Fp::<P>::new(0));
        }
    }
    m
}

fn random_strict_lower_gf2m8(n: usize, seed: u64) -> FieldMatrix<Gf2m8> {
    let mut m = random_gf2m8_matrix(n, n, seed);
    for r in 0..n {
        for c in r..n {
            m.set(r, c, Gf2m8::new([0]));
        }
    }
    m
}

// ─── Per-primitive benches ─────────────────────────────────────────────────

fn bench_trsm_upper(c: &mut Criterion) {
    let mut g = c.benchmark_group("triangular/trsm_upper");
    for &n in SIZES {
        // Fp<7>
        let a = random_upper_fp::<7>(n, 0xA1A1 + n as u64);
        let b = random_fp_matrix::<7>(n, n, 0xA2A2 + n as u64);
        g.bench_with_input(BenchmarkId::new("Fp_7", n), &n, |bencher, _| {
            bencher.iter_with_setup(
                || b.clone(),
                |mut b_local| {
                    trsm_upper(black_box(&a).submat(.., ..), b_local.submat_mut(.., ..));
                },
            );
        });
        // Fp<251> — small-prime byte-lane AVX2 cell (issue `40195c09`).
        let a251 = random_upper_fp::<251>(n, 0xA3A3 + n as u64);
        let b251 = random_fp_matrix::<251>(n, n, 0xA4A4 + n as u64);
        g.bench_with_input(BenchmarkId::new("Fp_251", n), &n, |bencher, _| {
            bencher.iter_with_setup(
                || b251.clone(),
                |mut b_local| {
                    trsm_upper(black_box(&a251).submat(.., ..), b_local.submat_mut(.., ..));
                },
            );
        });
        // Fp<65521> — largest medium-prime u16-lane AVX2 cell.
        let a65 = random_upper_fp::<65521>(n, 0xA5A5 + n as u64);
        let b65 = random_fp_matrix::<65521>(n, n, 0xA6A6 + n as u64);
        g.bench_with_input(BenchmarkId::new("Fp_65521", n), &n, |bencher, _| {
            bencher.iter_with_setup(
                || b65.clone(),
                |mut b_local| {
                    trsm_upper(black_box(&a65).submat(.., ..), b_local.submat_mut(.., ..));
                },
            );
        });
        // Fp<MERSENNE_31>
        let a31 = random_upper_fp::<MERSENNE_31>(n, 0xB1B1 + n as u64);
        let b31 = random_fp_matrix::<MERSENNE_31>(n, n, 0xB2B2 + n as u64);
        g.bench_with_input(BenchmarkId::new("Fp_M31", n), &n, |bencher, _| {
            bencher.iter_with_setup(
                || b31.clone(),
                |mut b_local| {
                    trsm_upper(black_box(&a31).submat(.., ..), b_local.submat_mut(.., ..));
                },
            );
        });
        // Gf2m8
        let a8 = random_upper_gf2m8(n, 0xC1C1 + n as u64);
        let b8 = random_gf2m8_matrix(n, n, 0xC2C2 + n as u64);
        g.bench_with_input(BenchmarkId::new("Gf2m8", n), &n, |bencher, _| {
            bencher.iter_with_setup(
                || b8.clone(),
                |mut b_local| {
                    trsm_upper(black_box(&a8).submat(.., ..), b_local.submat_mut(.., ..));
                },
            );
        });
    }
    g.finish();
}

fn bench_trsm_lower(c: &mut Criterion) {
    let mut g = c.benchmark_group("triangular/trsm_lower");
    for &n in SIZES {
        // Fp<7>
        let a = random_lower_fp::<7>(n, 0xA1A1 + n as u64);
        let b = random_fp_matrix::<7>(n, n, 0xA2A2 + n as u64);
        g.bench_with_input(BenchmarkId::new("Fp_7", n), &n, |bencher, _| {
            bencher.iter_with_setup(
                || b.clone(),
                |mut b_local| {
                    trsm_lower(black_box(&a).submat(.., ..), b_local.submat_mut(.., ..));
                },
            );
        });
        // Fp<251> — small-prime byte-lane AVX2 cell (issue `40195c09`).
        let a251 = random_lower_fp::<251>(n, 0xA3A3 + n as u64);
        let b251 = random_fp_matrix::<251>(n, n, 0xA4A4 + n as u64);
        g.bench_with_input(BenchmarkId::new("Fp_251", n), &n, |bencher, _| {
            bencher.iter_with_setup(
                || b251.clone(),
                |mut b_local| {
                    trsm_lower(black_box(&a251).submat(.., ..), b_local.submat_mut(.., ..));
                },
            );
        });
        // Fp<65521> — largest medium-prime u16-lane AVX2 cell.
        let a65 = random_lower_fp::<65521>(n, 0xA5A5 + n as u64);
        let b65 = random_fp_matrix::<65521>(n, n, 0xA6A6 + n as u64);
        g.bench_with_input(BenchmarkId::new("Fp_65521", n), &n, |bencher, _| {
            bencher.iter_with_setup(
                || b65.clone(),
                |mut b_local| {
                    trsm_lower(black_box(&a65).submat(.., ..), b_local.submat_mut(.., ..));
                },
            );
        });
        // Fp<MERSENNE_31>
        let a31 = random_lower_fp::<MERSENNE_31>(n, 0xB1B1 + n as u64);
        let b31 = random_fp_matrix::<MERSENNE_31>(n, n, 0xB2B2 + n as u64);
        g.bench_with_input(BenchmarkId::new("Fp_M31", n), &n, |bencher, _| {
            bencher.iter_with_setup(
                || b31.clone(),
                |mut b_local| {
                    trsm_lower(black_box(&a31).submat(.., ..), b_local.submat_mut(.., ..));
                },
            );
        });
        // Gf2m8
        let a8 = random_lower_gf2m8(n, 0xC1C1 + n as u64);
        let b8 = random_gf2m8_matrix(n, n, 0xC2C2 + n as u64);
        g.bench_with_input(BenchmarkId::new("Gf2m8", n), &n, |bencher, _| {
            bencher.iter_with_setup(
                || b8.clone(),
                |mut b_local| {
                    trsm_lower(black_box(&a8).submat(.., ..), b_local.submat_mut(.., ..));
                },
            );
        });
    }
    g.finish();
}

fn bench_trmm_upper(c: &mut Criterion) {
    let mut g = c.benchmark_group("triangular/trmm_upper");
    for &n in SIZES {
        let a = random_upper_fp::<7>(n, 0xD1D1 + n as u64);
        let b = random_fp_matrix::<7>(n, n, 0xD2D2 + n as u64);
        g.bench_with_input(BenchmarkId::new("Fp_7", n), &n, |bencher, _| {
            bencher.iter_with_setup(
                || b.clone(),
                |mut b_local| {
                    trmm_upper(black_box(&a).submat(.., ..), b_local.submat_mut(.., ..));
                },
            );
        });
        let a31 = random_upper_fp::<MERSENNE_31>(n, 0xE1E1 + n as u64);
        let b31 = random_fp_matrix::<MERSENNE_31>(n, n, 0xE2E2 + n as u64);
        g.bench_with_input(BenchmarkId::new("Fp_M31", n), &n, |bencher, _| {
            bencher.iter_with_setup(
                || b31.clone(),
                |mut b_local| {
                    trmm_upper(black_box(&a31).submat(.., ..), b_local.submat_mut(.., ..));
                },
            );
        });
        let a8 = random_upper_gf2m8(n, 0xF1F1 + n as u64);
        let b8 = random_gf2m8_matrix(n, n, 0xF2F2 + n as u64);
        g.bench_with_input(BenchmarkId::new("Gf2m8", n), &n, |bencher, _| {
            bencher.iter_with_setup(
                || b8.clone(),
                |mut b_local| {
                    trmm_upper(black_box(&a8).submat(.., ..), b_local.submat_mut(.., ..));
                },
            );
        });
    }
    g.finish();
}

fn bench_trtri_upper(c: &mut Criterion) {
    let mut g = c.benchmark_group("triangular/trtri_upper");
    for &n in SIZES {
        let a = random_upper_fp::<7>(n, 0x1111 + n as u64);
        g.bench_with_input(BenchmarkId::new("Fp_7", n), &n, |bencher, _| {
            bencher.iter_with_setup(
                || a.clone(),
                |mut a_local| {
                    trtri_upper(a_local.submat_mut(.., ..));
                    black_box(&a_local);
                },
            );
        });
        let a31 = random_upper_fp::<MERSENNE_31>(n, 0x2222 + n as u64);
        g.bench_with_input(BenchmarkId::new("Fp_M31", n), &n, |bencher, _| {
            bencher.iter_with_setup(
                || a31.clone(),
                |mut a_local| {
                    trtri_upper(a_local.submat_mut(.., ..));
                    black_box(&a_local);
                },
            );
        });
        let a8 = random_upper_gf2m8(n, 0x3333 + n as u64);
        g.bench_with_input(BenchmarkId::new("Gf2m8", n), &n, |bencher, _| {
            bencher.iter_with_setup(
                || a8.clone(),
                |mut a_local| {
                    trtri_upper(a_local.submat_mut(.., ..));
                    black_box(&a_local);
                },
            );
        });
    }
    g.finish();
}

fn bench_trtrm(c: &mut Criterion) {
    let mut g = c.benchmark_group("triangular/trtrm");
    for &n in SIZES {
        let l = random_strict_lower_fp::<7>(n, 0x4444 + n as u64);
        let u = random_upper_fp::<7>(n, 0x5555 + n as u64);
        g.bench_with_input(BenchmarkId::new("Fp_7", n), &n, |bencher, _| {
            bencher.iter_with_setup(
                || l.clone(),
                |mut l_local| {
                    trtrm(l_local.submat_mut(.., ..), black_box(&u).submat(.., ..));
                    black_box(&l_local);
                },
            );
        });
        let l31 = random_strict_lower_fp::<MERSENNE_31>(n, 0x6666 + n as u64);
        let u31 = random_upper_fp::<MERSENNE_31>(n, 0x7777 + n as u64);
        g.bench_with_input(BenchmarkId::new("Fp_M31", n), &n, |bencher, _| {
            bencher.iter_with_setup(
                || l31.clone(),
                |mut l_local| {
                    trtrm(l_local.submat_mut(.., ..), black_box(&u31).submat(.., ..));
                    black_box(&l_local);
                },
            );
        });
        let l8 = random_strict_lower_gf2m8(n, 0x8888 + n as u64);
        let u8 = random_upper_gf2m8(n, 0x9999 + n as u64);
        g.bench_with_input(BenchmarkId::new("Gf2m8", n), &n, |bencher, _| {
            bencher.iter_with_setup(
                || l8.clone(),
                |mut l_local| {
                    trtrm(l_local.submat_mut(.., ..), black_box(&u8).submat(.., ..));
                    black_box(&l_local);
                },
            );
        });
    }
    g.finish();
}

criterion_group!(
    benches,
    bench_trsm_upper,
    bench_trsm_lower,
    bench_trmm_upper,
    bench_trtri_upper,
    bench_trtrm,
);
criterion_main!(benches);
