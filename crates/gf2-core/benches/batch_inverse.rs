//! Benchmarks [`gf2_core::field::batch_ops::batch_inverse`] against
//! per-element `Fp<65537>::inv`.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use gf2_core::field::batch_ops::batch_inverse;
use gf2_core::field::FiniteField;
use gf2_core::gfp::Fp;

type F = Fp<65537>;

fn make_inputs(n: usize) -> Vec<F> {
    // Non-zero residues: `batch_inverse` returns `None` on a zero input.
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
