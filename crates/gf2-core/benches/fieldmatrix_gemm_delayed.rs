//! Focused `FieldMatrix::gemm` baseline comparison for issue `e7ab802d`.
//!
//! This bench keeps the full `64c88ae4` sweep (`fieldmatrix_gemm`) untouched
//! and adds fast development cells that compare:
//!
//! - `eager_scalar`: classical triple loop, reducing after every field MAC.
//! - `delayed_blocked`: production `gemm`, cache-blocked over output tiles and
//!   using `dot_product_slices` delayed product-sum reduction.
//!
//! The 64×64 cells are intended for quick pre/post checks. Larger 64c88ae4
//! cells, especially 1024/4096 and rectangular sweeps, are deferred to the
//! existing full harness/nightly runs.
//!
//! ```bash
//! cargo bench -p gf2-core --bench fieldmatrix_gemm_delayed --features rand
//! cargo bench -p gf2-core --bench fieldmatrix_gemm_delayed --features rand -- --test
//! cargo bench -p gf2-core --bench fieldmatrix_gemm_delayed --features rand -- Fp_7/64
//! ```

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use gf2_core::field::matrix::{gemm, FieldMatrix};
use gf2_core::gfp::Fp;
use rand::{Rng, SeedableRng};

const SIZES: &[usize] = &[64, 128];

fn random_fp_matrix<const P: u64>(rows: usize, cols: usize, seed: u64) -> FieldMatrix<Fp<P>> {
    let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
    let mut m = FieldMatrix::<Fp<P>>::zeros(rows, cols);
    for r in 0..rows {
        for c in 0..cols {
            m.set(r, c, Fp::<P>::new(rng.gen::<u64>() % P));
        }
    }
    m
}

fn eager_scalar_gemm<const P: u64>(
    a: &FieldMatrix<Fp<P>>,
    b: &FieldMatrix<Fp<P>>,
) -> FieldMatrix<Fp<P>> {
    assert_eq!(a.cols(), b.rows());
    let mut out = FieldMatrix::<Fp<P>>::zeros(a.rows(), b.cols());
    for i in 0..a.rows() {
        for j in 0..b.cols() {
            let mut acc = Fp::<P>::new(0);
            for k in 0..a.cols() {
                acc += a.get(i, k) * b.get(k, j);
            }
            out.set(i, j, acc);
        }
    }
    out
}

fn bench_prime<const P: u64>(c: &mut Criterion, label: &str) {
    let mut group = c.benchmark_group(format!("fieldmatrix_gemm_delayed/{label}"));
    group.sample_size(10);
    group.measurement_time(std::time::Duration::from_secs(1));
    group.warm_up_time(std::time::Duration::from_millis(300));

    for &n in SIZES {
        group.throughput(Throughput::Elements((n * n * n) as u64));
        let a = random_fp_matrix::<P>(n, n, 0xE7AB_802D ^ n as u64);
        let b = random_fp_matrix::<P>(n, n, 0x64C8_8AE4 ^ n as u64);

        group.bench_with_input(BenchmarkId::new("eager_scalar", n), &n, |bench, _| {
            bench.iter(|| {
                let out = eager_scalar_gemm::<P>(black_box(&a), black_box(&b));
                black_box(out);
            });
        });

        group.bench_with_input(BenchmarkId::new("delayed_blocked", n), &n, |bench, _| {
            bench.iter(|| {
                let out = gemm(black_box(&a), black_box(&b));
                black_box(out);
            });
        });
    }
    group.finish();
}

fn bench_fp7(c: &mut Criterion) {
    bench_prime::<7>(c, "Fp_7");
}

fn bench_fp251(c: &mut Criterion) {
    bench_prime::<251>(c, "Fp_251");
}

fn bench_fp65521(c: &mut Criterion) {
    bench_prime::<65521>(c, "Fp_65521");
}

criterion_group!(benches, bench_fp7, bench_fp251, bench_fp65521);
criterion_main!(benches);
