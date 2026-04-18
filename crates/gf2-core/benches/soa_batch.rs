//! Benchmark for [`BatchExtField<Fp<P>, 2>::batch_mul_quadratic`].
//!
//! Compares the SoA batched Karatsuba multiplication against an AoS
//! sequential baseline that calls `QuadraticExt::mul` once per element.
//! Target: ≥3× speedup at `N = 1000` GF(p²) elements over `Fp<65537>`
//! with `β = 3`.
//!
//! Regenerate the numbers cited in `gf2_core::gfpn::batch` module docs
//! with:
//!
//! ```text
//! cargo bench -p gf2-core --bench soa_batch
//! ```

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use gf2_core::gfp::Fp;
use gf2_core::gfpn::{BatchExtField, ExtConfig, QuadraticExt};

struct Cfg;
impl ExtConfig for Cfg {
    type BaseField = Fp<65537>;
    const NON_RESIDUE: Fp<65537> = Fp::<65537>::new(3);
}
type Fq2 = QuadraticExt<Cfg>;

fn make_inputs(n: usize, seed: u64) -> Vec<Fq2> {
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

fn bench_soa_vs_sequential(c: &mut Criterion) {
    let mut group = c.benchmark_group("soa_batch_mul_fq2_fp65537");

    for &n in &[64usize, 256, 1000] {
        let xs = make_inputs(n, 1);
        let ys = make_inputs(n, 2);

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
        let bxs = BatchExtField::<Fp<65537>, 2>::from_quadratic::<Cfg>(&xs);
        let bys = BatchExtField::<Fp<65537>, 2>::from_quadratic::<Cfg>(&ys);
        group.bench_with_input(
            BenchmarkId::new("batch_soa", n),
            &(bxs, bys),
            |bench, (bxs, bys)| {
                bench.iter(|| black_box(bxs.batch_mul_quadratic::<Cfg>(black_box(bys))));
            },
        );

        // SoA batch multiplication including AoS↔SoA conversions — shows
        // the end-to-end cost for data that originates in AoS form.
        group.bench_with_input(
            BenchmarkId::new("batch_soa_with_transpose", n),
            &(xs.clone(), ys.clone()),
            |bench, (xs, ys)| {
                bench.iter(|| {
                    let bxs = BatchExtField::<Fp<65537>, 2>::from_quadratic::<Cfg>(xs);
                    let bys = BatchExtField::<Fp<65537>, 2>::from_quadratic::<Cfg>(ys);
                    let bzs = bxs.batch_mul_quadratic::<Cfg>(&bys);
                    black_box(bzs.to_quadratic::<Cfg>())
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_soa_vs_sequential);
criterion_main!(benches);
