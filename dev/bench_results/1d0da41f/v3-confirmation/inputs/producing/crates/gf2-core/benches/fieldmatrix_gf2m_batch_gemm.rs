//! GF(2^m) `FieldMatrix::gemm` benchmark for `jit:577b9e7f`.
//!
//! Compares the production matrix path, which routes supported single-word
//! GF(2^m) dot products through the batched carry-less-multiply hook, against
//! an eager scalar reference that multiplies one field element at a time inside
//! the innermost loop. Run with:
//!
//! ```bash
//! RUSTFLAGS="-C target-cpu=native" cargo bench -p gf2-core \
//!     --bench fieldmatrix_gf2m_batch_gemm --features rand,simd
//! ```

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use gf2_core::field::matrix::{gemm, FieldMatrix};
use gf2_core::field::ConstField;
use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
use rand::{Rng, SeedableRng};

struct Gf2m8Cfg;
impl Gf2mWideConfig<1> for Gf2m8Cfg {
    const M: usize = 8;
    const MODULUS: [u64; 1] = [0x1B];
    const NAME: &'static str = "bench-gf2m8";
}
type Gf2m8 = Gf2mWide<1, Gf2m8Cfg>;

struct Gf2m16Cfg;
impl Gf2mWideConfig<1> for Gf2m16Cfg {
    const M: usize = 16;
    const MODULUS: [u64; 1] = [0x100B];
    const NAME: &'static str = "bench-gf2m16";
}
type Gf2m16 = Gf2mWide<1, Gf2m16Cfg>;

struct Gf2m32Cfg;
impl Gf2mWideConfig<1> for Gf2m32Cfg {
    const M: usize = 32;
    const MODULUS: [u64; 1] = [0x0040_0007];
    const NAME: &'static str = "bench-gf2m32";
}
type Gf2m32 = Gf2mWide<1, Gf2m32Cfg>;

trait FromRawU64 {
    const MASK: u64;
    fn from_raw_u64(value: u64) -> Self;
}

impl FromRawU64 for Gf2m8 {
    const MASK: u64 = 0xFF;
    fn from_raw_u64(value: u64) -> Self {
        Self::from_u64(value)
    }
}

impl FromRawU64 for Gf2m16 {
    const MASK: u64 = 0xFFFF;
    fn from_raw_u64(value: u64) -> Self {
        Self::from_u64(value)
    }
}

impl FromRawU64 for Gf2m32 {
    const MASK: u64 = 0xFFFF_FFFF;
    fn from_raw_u64(value: u64) -> Self {
        Self::from_u64(value)
    }
}

fn random_matrix<F>(rows: usize, cols: usize, seed: u64) -> FieldMatrix<F>
where
    F: ConstField + FromRawU64,
{
    let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
    let mut m = FieldMatrix::<F>::zeros(rows, cols);
    for r in 0..rows {
        for c in 0..cols {
            m.set(r, c, F::from_raw_u64(rng.gen::<u64>() & F::MASK));
        }
    }
    m
}

fn scalar_gemm<F>(a: &FieldMatrix<F>, b: &FieldMatrix<F>) -> FieldMatrix<F>
where
    F: ConstField,
{
    assert_eq!(a.cols(), b.rows());
    let mut out = FieldMatrix::<F>::zeros(a.rows(), b.cols());
    for i in 0..a.rows() {
        for j in 0..b.cols() {
            let mut acc = F::zero();
            for k in 0..a.cols() {
                acc += a.get(i, k) * b.get(k, j);
            }
            out.set(i, j, acc);
        }
    }
    out
}

fn bench_field<F>(c: &mut Criterion, label: &str)
where
    F: ConstField + FromRawU64,
{
    let mut group = c.benchmark_group(format!("fieldmatrix_gf2m_batch_gemm/{label}"));
    group.sample_size(10);
    group.measurement_time(std::time::Duration::from_secs(1));
    group.warm_up_time(std::time::Duration::from_millis(300));

    for &(m, k, n, shape) in &[
        (64, 64, 64, "square64"),
        (128, 8, 128, "rect_k8"),
        (128, 32, 128, "rect_k32"),
    ] {
        group.throughput(Throughput::Elements((m * k * n) as u64));
        let a = random_matrix::<F>(m, k, 0x577B_9E7F ^ (m as u64) ^ ((k as u64) << 16));
        let b = random_matrix::<F>(k, n, 0x06E9_42CC ^ (n as u64) ^ ((k as u64) << 16));

        group.bench_with_input(
            BenchmarkId::new("scalar_eager", shape),
            &shape,
            |bench, _| {
                bench.iter(|| black_box(scalar_gemm(black_box(&a), black_box(&b))));
            },
        );
        group.bench_with_input(BenchmarkId::new("batch_gemm", shape), &shape, |bench, _| {
            bench.iter(|| black_box(gemm(black_box(&a), black_box(&b))));
        });
    }
    group.finish();
}

fn bench_gf2m8(c: &mut Criterion) {
    bench_field::<Gf2m8>(c, "gf2m8");
}

fn bench_gf2m16(c: &mut Criterion) {
    bench_field::<Gf2m16>(c, "gf2m16");
}

fn bench_gf2m32(c: &mut Criterion) {
    bench_field::<Gf2m32>(c, "gf2m32");
}

criterion_group!(benches, bench_gf2m8, bench_gf2m16, bench_gf2m32);
criterion_main!(benches);
