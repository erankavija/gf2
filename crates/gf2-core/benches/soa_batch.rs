//! Benchmarks for [`BatchExtField`] quadratic and cubic SoA multiplication.
//!
//! Compares SoA batched Karatsuba multiplication against AoS sequential
//! baselines that call `QuadraticExt::mul` / `CubicExt::mul` once per
//! element. The cubic group is the C5 criterion leaf for jit:33d3f5b7.
//!
//! Regenerate the numbers cited in `gf2_core::gfpn::batch` module docs
//! with:
//!
//! ```text
//! cargo bench -p gf2-core --bench soa_batch
//! ```

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use gf2_core::gfp::Fp;
use gf2_core::gfpn::{BatchExtField, CubicExt, ExtConfig, QuadraticExt};

struct QuadCfg;
impl ExtConfig for QuadCfg {
    type BaseField = Fp<65537>;
    const NON_RESIDUE: Fp<65537> = Fp::<65537>::new(3);
}
type Fq2 = QuadraticExt<QuadCfg>;

struct CubicCfg;
impl ExtConfig for CubicCfg {
    type BaseField = Fp<65537>;
    const NON_RESIDUE: Fp<65537> = Fp::<65537>::new(3);
}
type Fq3 = CubicExt<CubicCfg>;

fn make_fq2_inputs(n: usize, seed: u64) -> Vec<Fq2> {
    // Deterministic pseudo-random pattern: good enough for a throughput
    // benchmark; no need for cryptographic randomness.
    (0..n)
        .map(|i| {
            let a = ((i as u64).wrapping_mul(2_654_435_761).wrapping_add(seed)) % 65537;
            let b = ((i as u64).wrapping_mul(40_503).wrapping_add(seed * 7)) % 65537;
            Fq2::new(Fp::new(a), Fp::new(b))
        })
        .collect()
}

fn make_fq3_inputs(n: usize, seed: u64) -> Vec<Fq3> {
    (0..n)
        .map(|i| {
            let a = ((i as u64).wrapping_mul(2_654_435_761).wrapping_add(seed)) % 65537;
            let b = ((i as u64).wrapping_mul(40_503).wrapping_add(seed * 7)) % 65537;
            let c = ((i as u64)
                .wrapping_mul(1_103_515_245)
                .wrapping_add(seed * 13))
                % 65537;
            Fq3::new(Fp::new(a), Fp::new(b), Fp::new(c))
        })
        .collect()
}

fn bench_quadratic_soa_vs_sequential(c: &mut Criterion) {
    let mut group = c.benchmark_group("soa_batch_mul_fq2_fp65537");

    for &n in &[64usize, 256, 1000] {
        let xs = make_fq2_inputs(n, 1);
        let ys = make_fq2_inputs(n, 2);

        group.throughput(Throughput::Elements(n as u64));

        // AoS sequential baseline: scalar QuadraticExt::mul per pair.
        group.bench_with_input(
            BenchmarkId::new("sequential_aos", n),
            &(xs.clone(), ys.clone()),
            |bench, (xs, ys)| {
                bench.iter(|| {
                    let out: Vec<Fq2> = xs.iter().zip(ys.iter()).map(|(x, y)| *x * *y).collect();
                    black_box(out)
                });
            },
        );

        // SoA batch multiplication (excludes AoS↔SoA conversion cost: we
        // transpose once outside the timed loop, matching the target use
        // case where data already lives in SoA form).
        let bxs = BatchExtField::<Fp<65537>, 2>::from_quadratic::<QuadCfg>(&xs);
        let bys = BatchExtField::<Fp<65537>, 2>::from_quadratic::<QuadCfg>(&ys);
        group.bench_with_input(
            BenchmarkId::new("batch_soa", n),
            &(bxs, bys),
            |bench, (bxs, bys)| {
                bench.iter(|| black_box(bxs.batch_mul_quadratic::<QuadCfg>(black_box(bys))));
            },
        );

        // SoA batch multiplication including AoS↔SoA conversions — shows
        // the end-to-end cost for data that originates in AoS form.
        group.bench_with_input(
            BenchmarkId::new("batch_soa_with_transpose", n),
            &(xs.clone(), ys.clone()),
            |bench, (xs, ys)| {
                bench.iter(|| {
                    let bxs = BatchExtField::<Fp<65537>, 2>::from_quadratic::<QuadCfg>(xs);
                    let bys = BatchExtField::<Fp<65537>, 2>::from_quadratic::<QuadCfg>(ys);
                    let bzs = bxs.batch_mul_quadratic::<QuadCfg>(&bys);
                    black_box(bzs.to_quadratic::<QuadCfg>())
                });
            },
        );
    }

    group.finish();
}

fn bench_cubic_soa_vs_sequential(c: &mut Criterion) {
    let mut group = c.benchmark_group("soa_batch_mul_fq3_fp65537");

    for &n in &[64usize, 256, 1000] {
        let xs = make_fq3_inputs(n, 11);
        let ys = make_fq3_inputs(n, 23);

        group.throughput(Throughput::Elements(n as u64));

        group.bench_with_input(
            BenchmarkId::new("sequential_aos", n),
            &(xs.clone(), ys.clone()),
            |bench, (xs, ys)| {
                bench.iter(|| {
                    let out: Vec<Fq3> = xs.iter().zip(ys.iter()).map(|(x, y)| *x * *y).collect();
                    black_box(out)
                });
            },
        );

        let bxs = BatchExtField::<Fp<65537>, 3>::from_cubic::<CubicCfg>(&xs);
        let bys = BatchExtField::<Fp<65537>, 3>::from_cubic::<CubicCfg>(&ys);
        group.bench_with_input(
            BenchmarkId::new("batch_soa", n),
            &(bxs, bys),
            |bench, (bxs, bys)| {
                bench.iter(|| black_box(bxs.batch_mul_cubic::<CubicCfg>(black_box(bys))));
            },
        );

        group.bench_with_input(
            BenchmarkId::new("batch_soa_with_transpose", n),
            &(xs.clone(), ys.clone()),
            |bench, (xs, ys)| {
                bench.iter(|| {
                    let bxs = BatchExtField::<Fp<65537>, 3>::from_cubic::<CubicCfg>(xs);
                    let bys = BatchExtField::<Fp<65537>, 3>::from_cubic::<CubicCfg>(ys);
                    let bzs = bxs.batch_mul_cubic::<CubicCfg>(&bys);
                    black_box(bzs.to_cubic::<CubicCfg>())
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_quadratic_soa_vs_sequential,
    bench_cubic_soa_vs_sequential
);
criterion_main!(benches);
