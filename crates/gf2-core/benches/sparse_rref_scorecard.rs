//! Benchmarks `SparseFieldMatrix::rref` over GF(2^8) and GF(2^16) at
//! `n ∈ {256, 1024}` with ten expected non-zeros per row.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use gf2_core::field::matrix::FieldMatrix;
use gf2_core::field::sparse_matrix::SparseFieldMatrix;
use gf2_core::field::FiniteField;
use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

/// GF(2^8) with the AES polynomial `x^8 + x^4 + x^3 + x + 1` (`@/citation/Nist2001`).
struct ScorecardGf2m8Cfg;
impl Gf2mWideConfig<1> for ScorecardGf2m8Cfg {
    const M: usize = 8;
    const MODULUS: [u64; 1] = [0x1B];
    const NAME: &'static str = "ScorecardGf2m8Cfg";
}
type Gf2m8 = Gf2mWide<1, ScorecardGf2m8Cfg>;

/// GF(2^16) with `x^16 + x^5 + x^3 + x^2 + 1`.
struct ScorecardGf2m16Cfg;
impl Gf2mWideConfig<1> for ScorecardGf2m16Cfg {
    const M: usize = 16;
    // x^5 + x^3 + x^2 + 1; the leading bit is implicit.
    const MODULUS: [u64; 1] = [0b101101];
    const NAME: &'static str = "ScorecardGf2m16Cfg";
}
type Gf2m16 = Gf2mWide<1, ScorecardGf2m16Cfg>;

/// `(n, density, label)` cells.
fn cells() -> Vec<(usize, f64, &'static str)> {
    vec![
        (256, 3.906_250e-2, "n256_d3.906e-2_csr"),
        (1024, 9.765_625e-3, "n1024_d9.766e-3_csr"),
    ]
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

fn random_sparse_gf2m16(
    rows: usize,
    cols: usize,
    density: f64,
    seed: u64,
) -> SparseFieldMatrix<Gf2m16> {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut m = FieldMatrix::<Gf2m16>::zeros(rows, cols);
    for r in 0..rows {
        for c in 0..cols {
            if rng.gen::<f64>() < density {
                let w = (rng.gen::<u64>() & 0xFFFF).max(1);
                m.set(r, c, Gf2m16::new([w]));
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
    let group_name = format!("sparse_rref_scorecard/{label}");
    let mut group = c.benchmark_group(&group_name);
    group.sample_size(10);
    group.measurement_time(std::time::Duration::from_secs(5));
    for (n, density, cell_label) in cells() {
        let a = build(n, n, density, 0xC0DE ^ n as u64);
        group.bench_with_input(BenchmarkId::from_parameter(cell_label), &n, |bench, _| {
            bench.iter(|| {
                let r = black_box(&a).rref();
                black_box(r);
            });
        });
    }
    group.finish();
}

fn bench_gf2m8(c: &mut Criterion) {
    bench_with_label::<Gf2m8, _>(c, "Gf2m_u8_AES", random_sparse_gf2m8);
}

fn bench_gf2m16(c: &mut Criterion) {
    bench_with_label::<Gf2m16, _>(c, "Gf2m_u16_Conway", random_sparse_gf2m16);
}

criterion_group!(benches, bench_gf2m8, bench_gf2m16);
criterion_main!(benches);
