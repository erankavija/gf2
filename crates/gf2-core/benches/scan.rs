use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use gf2_core::BitVec;

/// Create a sparse bit vector with a single bit set at position `pos`.
fn create_sparse_bitvec(num_bits: usize, pos: usize) -> BitVec {
    let mut bv = BitVec::with_capacity(num_bits);
    bv.resize(num_bits, false);
    if pos < num_bits {
        bv.set(pos, true);
    }
    bv
}

/// Create a dense bit vector with all bits set except one at position `pos`.
fn create_dense_bitvec(num_bits: usize, pos: usize) -> BitVec {
    let mut bv = BitVec::with_capacity(num_bits);
    bv.resize(num_bits, true);
    if pos < num_bits {
        bv.set(pos, false);
    }
    bv
}

fn bench_find_first_one_sparse(c: &mut Criterion) {
    let mut group = c.benchmark_group("find_first_one_sparse");
    
    // Test different positions of the first set bit
    for &(size_kb, bit_pos) in &[(1, 0), (1, 63), (1, 64), (64, 0), (64, 4096), (256, 0), (256, 16384)] {
        let size_bits = size_kb * 1024 * 8;
        let bv = create_sparse_bitvec(size_bits, bit_pos);
        group.throughput(Throughput::Bytes((size_kb * 1024) as u64));
        
        let id = format!("{}KB_pos{}", size_kb, bit_pos);
        group.bench_with_input(BenchmarkId::from_parameter(id), &bv, |b, bv| {
            b.iter(|| black_box(bv.find_first_one()));
        });
    }
    
    group.finish();
}

fn bench_find_first_one_dense(c: &mut Criterion) {
    let mut group = c.benchmark_group("find_first_one_dense");
    
    // Test with all bits set (worst case - none found)
    for &size_kb in &[1, 64, 256] {
        let size_bits = size_kb * 1024 * 8;
        let bv = create_sparse_bitvec(size_bits, size_bits); // No bit set
        group.throughput(Throughput::Bytes((size_kb * 1024) as u64));
        
        group.bench_with_input(BenchmarkId::from_parameter(size_kb), &bv, |b, bv| {
            b.iter(|| black_box(bv.find_first_one()));
        });
    }
    
    group.finish();
}

fn bench_find_first_zero_sparse(c: &mut Criterion) {
    let mut group = c.benchmark_group("find_first_zero_sparse");
    
    // Test different positions of the first clear bit
    for &(size_kb, bit_pos) in &[(1, 0), (1, 63), (1, 64), (64, 0), (64, 4096), (256, 0), (256, 16384)] {
        let size_bits = size_kb * 1024 * 8;
        let bv = create_dense_bitvec(size_bits, bit_pos);
        group.throughput(Throughput::Bytes((size_kb * 1024) as u64));
        
        let id = format!("{}KB_pos{}", size_kb, bit_pos);
        group.bench_with_input(BenchmarkId::from_parameter(id), &bv, |b, bv| {
            b.iter(|| black_box(bv.find_first_zero()));
        });
    }
    
    group.finish();
}

fn bench_find_first_zero_dense(c: &mut Criterion) {
    let mut group = c.benchmark_group("find_first_zero_dense");
    
    // Test with all bits clear (worst case - none found)
    for &size_kb in &[1, 64, 256] {
        let size_bits = size_kb * 1024 * 8;
        let bv = create_dense_bitvec(size_bits, size_bits); // No bit clear
        group.throughput(Throughput::Bytes((size_kb * 1024) as u64));
        
        group.bench_with_input(BenchmarkId::from_parameter(size_kb), &bv, |b, bv| {
            b.iter(|| black_box(bv.find_first_zero()));
        });
    }
    
    group.finish();
}

criterion_group!(
    benches,
    bench_find_first_one_sparse,
    bench_find_first_one_dense,
    bench_find_first_zero_sparse,
    bench_find_first_zero_dense
);
criterion_main!(benches);
