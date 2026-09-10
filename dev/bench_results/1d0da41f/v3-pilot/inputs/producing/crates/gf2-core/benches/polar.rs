use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use gf2_core::matrix::BitMatrix;
use gf2_core::BitVec;

fn benchmark_bit_reversal(c: &mut Criterion) {
    let mut group = c.benchmark_group("bit_reversal");

    for n in [64, 256, 1024, 4096, 16384].iter() {
        group.throughput(Throughput::Elements(*n as u64));

        let mut bv = BitVec::zeros(*n);
        for i in (0..*n).step_by(3) {
            bv.set(i, true);
        }

        group.bench_with_input(BenchmarkId::new("functional", n), n, |b, &n| {
            b.iter(|| black_box(&bv).bit_reversed(black_box(n)))
        });

        group.bench_with_input(BenchmarkId::new("into", n), n, |b, &n| {
            let mut bv_mut = bv.clone();
            b.iter(|| {
                bv_mut.bit_reverse_into(black_box(n));
                black_box(&bv_mut);
            })
        });
    }

    group.finish();
}

fn benchmark_polar_transform(c: &mut Criterion) {
    let mut group = c.benchmark_group("polar_transform");

    for n in [64, 256, 1024, 4096, 16384].iter() {
        group.throughput(Throughput::Elements(*n as u64));

        let mut bv = BitVec::zeros(*n);
        for i in (0..*n).step_by(3) {
            bv.set(i, true);
        }

        group.bench_with_input(BenchmarkId::new("forward_functional", n), n, |b, &n| {
            b.iter(|| black_box(&bv).polar_transform(black_box(n)))
        });

        group.bench_with_input(BenchmarkId::new("forward_into", n), n, |b, &n| {
            let mut bv_mut = bv.clone();
            b.iter(|| {
                bv_mut.polar_transform_into(black_box(n));
                black_box(&bv_mut);
            })
        });

        group.bench_with_input(BenchmarkId::new("inverse_functional", n), n, |b, &n| {
            b.iter(|| black_box(&bv).polar_transform_inverse(black_box(n)))
        });

        group.bench_with_input(BenchmarkId::new("inverse_into", n), n, |b, &n| {
            let mut bv_mut = bv.clone();
            b.iter(|| {
                bv_mut.polar_transform_inverse_into(black_box(n));
                black_box(&bv_mut);
            })
        });
    }

    group.finish();
}

fn benchmark_polar_transform_vs_naive(c: &mut Criterion) {
    let mut group = c.benchmark_group("polar_vs_naive");

    // Only test smaller sizes for naive matrix multiply
    for n in [64, 256, 1024].iter() {
        group.throughput(Throughput::Elements(*n as u64));

        let mut bv = BitVec::zeros(*n);
        for i in (0..*n).step_by(3) {
            bv.set(i, true);
        }

        group.bench_with_input(BenchmarkId::new("fht_optimized", n), n, |b, &n| {
            b.iter(|| black_box(&bv).polar_transform(black_box(n)))
        });

        // Naive: build full G_N matrix and multiply
        group.bench_with_input(BenchmarkId::new("naive_matrix", n), n, |b, &n| {
            // Build G_N via Kronecker product
            let mut g = BitMatrix::identity(1);
            let g2 = {
                let mut m = BitMatrix::zeros(2, 2);
                m.set(0, 0, true);
                m.set(1, 0, true);
                m.set(1, 1, true);
                m
            };

            let log_n = n.trailing_zeros();
            for _ in 0..log_n {
                g = kronecker_product(&g, &g2);
            }

            b.iter(|| {
                let mut result = BitVec::zeros(n);
                for row in 0..n {
                    let mut bit = false;
                    for col in 0..n {
                        if g.get(row, col) && bv.get(col) {
                            bit = !bit;
                        }
                    }
                    result.set(row, bit);
                }
                black_box(result)
            })
        });
    }

    group.finish();
}

fn kronecker_product(a: &BitMatrix, b: &BitMatrix) -> BitMatrix {
    let rows = a.rows() * b.rows();
    let cols = a.cols() * b.cols();
    let mut result = BitMatrix::zeros(rows, cols);

    for i in 0..a.rows() {
        for j in 0..a.cols() {
            if a.get(i, j) {
                for k in 0..b.rows() {
                    for l in 0..b.cols() {
                        if b.get(k, l) {
                            result.set(i * b.rows() + k, j * b.cols() + l, true);
                        }
                    }
                }
            }
        }
    }

    result
}

fn benchmark_polar_roundtrip(c: &mut Criterion) {
    let mut group = c.benchmark_group("polar_roundtrip");

    for n in [256, 1024, 4096, 16384].iter() {
        group.throughput(Throughput::Elements(*n as u64));

        let mut bv = BitVec::zeros(*n);
        for i in (0..*n).step_by(5) {
            bv.set(i, true);
        }

        group.bench_with_input(BenchmarkId::new("encode_decode", n), n, |b, &n| {
            b.iter(|| {
                let encoded = black_box(&bv).polar_transform(n);
                black_box(encoded).polar_transform_inverse(n)
            })
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    benchmark_bit_reversal,
    benchmark_polar_transform,
    benchmark_polar_transform_vs_naive,
    benchmark_polar_roundtrip
);
criterion_main!(benches);
