//! Benchmarks [`SparseFieldMatrix::matmul`] on square operands over `Fp<7>`,
//! `Fp<65521>`, GF(2^8) and GF(2^32).

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use gf2_core::field::matrix::FieldMatrix;
use gf2_core::field::sparse_matrix::SparseFieldMatrix;
use gf2_core::field::FiniteField;
use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
use gf2_core::gfp::Fp;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

/// GF(2^8) with the AES polynomial (`@/citation/Nist2001`).
struct MmGf2m8Cfg;
impl Gf2mWideConfig<1> for MmGf2m8Cfg {
    const M: usize = 8;
    const MODULUS: [u64; 1] = [0x1B];
    const NAME: &'static str = "MmGf2m8Cfg";
}
type Gf2m8 = Gf2mWide<1, MmGf2m8Cfg>;

/// GF(2^32) in one storage word.
struct MmGf2m32Cfg;
impl Gf2mWideConfig<1> for MmGf2m32Cfg {
    const M: usize = 32;
    // x^32 + x^22 + x^2 + x + 1; the leading bit is implicit.
    const MODULUS: [u64; 1] = [(1u64 << 22) | 0b111];
    const NAME: &'static str = "MmGf2m32Cfg";
}
type Gf2m32 = Gf2mWide<1, MmGf2m32Cfg>;

/// `(n, density, label)` cells.
fn cells() -> Vec<(usize, f64, &'static str)> {
    let n1 = 1024usize;
    let n2 = 4096usize;
    let d1 = 1.0 / (n1 as f64);
    let d2 = (n2 as f64).log2() / (n2 as f64);
    vec![(n1, d1, "n1024_d1_over_n"), (n2, d2, "n4096_dlogn_over_n")]
}

fn random_sparse_fp<const P: u64>(
    rows: usize,
    cols: usize,
    density: f64,
    seed: u64,
) -> SparseFieldMatrix<Fp<P>> {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut m = FieldMatrix::<Fp<P>>::zeros(rows, cols);
    for r in 0..rows {
        for c in 0..cols {
            if rng.gen::<f64>() < density {
                let v = (rng.gen::<u64>() % (P - 1)) + 1;
                m.set(r, c, Fp::<P>::new(v));
            }
        }
    }
    SparseFieldMatrix::from_dense(&m)
}

fn random_sparse_gf2m8(
    rows: usize,
    cols: usize,
    density: f64,
    seed: u64,
) -> SparseFieldMatrix<Gf2m8> {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut m = FieldMatrix::<Gf2m8>::zeros(rows, cols);
    for r in 0..rows {
        for c in 0..cols {
            if rng.gen::<f64>() < density {
                let w = (rng.gen::<u64>() & 0xFF).max(1);
                m.set(r, c, Gf2m8::new([w]));
            }
        }
    }
    SparseFieldMatrix::from_dense(&m)
}

fn random_sparse_gf2m32(
    rows: usize,
    cols: usize,
    density: f64,
    seed: u64,
) -> SparseFieldMatrix<Gf2m32> {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut m = FieldMatrix::<Gf2m32>::zeros(rows, cols);
    for r in 0..rows {
        for c in 0..cols {
            if rng.gen::<f64>() < density {
                let w = (rng.gen::<u64>() & 0xFFFF_FFFF).max(1);
                m.set(r, c, Gf2m32::new([w]));
            }
        }
    }
    SparseFieldMatrix::from_dense(&m)
}

fn bench_with_label<F, Build>(c: &mut Criterion, label: &str, build: Build)
where
    F: FiniteField,
    Build: Fn(usize, usize, f64, u64) -> SparseFieldMatrix<F>,
{
    let group_name = format!("sparse_field_matmul/{label}");
    let mut group = c.benchmark_group(&group_name);
    group.sample_size(10);
    group.measurement_time(std::time::Duration::from_secs(5));
    for (n, density, cell_label) in cells() {
        let a = build(n, n, density, 0xA1 ^ n as u64);
        let b = build(n, n, density, 0xB2 ^ n as u64);
        group.bench_with_input(BenchmarkId::from_parameter(cell_label), &n, |bench, _| {
            bench.iter(|| {
                let c = black_box(&a).matmul(black_box(&b));
                black_box(c);
            });
        });
    }
    group.finish();
}

fn bench_fp7(c: &mut Criterion) {
    bench_with_label::<Fp<7>, _>(c, "Fp_7", random_sparse_fp::<7>);
}

fn bench_fp65521(c: &mut Criterion) {
    bench_with_label::<Fp<65521>, _>(c, "Fp_65521", random_sparse_fp::<65521>);
}

fn bench_gf2m8(c: &mut Criterion) {
    bench_with_label::<Gf2m8, _>(c, "Gf2m_u8_AES", random_sparse_gf2m8);
}

fn bench_gf2m32(c: &mut Criterion) {
    bench_with_label::<Gf2m32, _>(c, "Gf2m_u32_Conway", random_sparse_gf2m32);
}

criterion_group!(benches, bench_fp7, bench_fp65521, bench_gf2m8, bench_gf2m32);
criterion_main!(benches);
