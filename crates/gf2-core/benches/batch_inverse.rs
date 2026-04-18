//! Benchmarks for [`gf2_core::field::batch_ops::batch_inverse`].
//!
//! Compares Montgomery's batch-inversion trick against the baseline of
//! inverting each element individually with `Fp<65537>::inv`. The target
//! set by the issue spec is a ≥5× speed-up at `N = 100`.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use gf2_core::field::batch_ops::batch_inverse;
use gf2_core::field::FiniteField;
use gf2_core::gfp::Fp;

type F = Fp<65537>;

fn make_inputs(n: usize) -> Vec<F> {
    // Fill deterministically with non-zero residues of a linear sequence. The
    // pattern itself doesn't matter — all we need is coverage over the field
    // that stays non-zero so the batch path runs end to end.
    let modulus: u64 = 65537;
    (0..n)
        .map(|i| {
            let v = ((i as u64 * 2_654_435_761) % (modulus - 1)) + 1;
            F::new(v)
        })
        .collect()
}

fn bench_batch_vs_individual(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_inverse_fp65537");

    for &n in &[16usize, 100, 1000] {
        let inputs = make_inputs(n);

        group.throughput(Throughput::Elements(n as u64));

        group.bench_with_input(BenchmarkId::new("batch", n), &inputs, |bench, xs| {
            bench.iter(|| black_box(batch_inverse(black_box(xs)).unwrap()));
        });

        group.bench_with_input(BenchmarkId::new("individual", n), &inputs, |bench, xs| {
            bench.iter(|| {
                let out: Vec<F> = xs.iter().map(|e| e.inv().unwrap()).collect();
                black_box(out)
            });
        });
    }

    group.finish();
}

criterion_group!(benches, bench_batch_vs_individual);
criterion_main!(benches);
