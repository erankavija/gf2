//! Benchmark Arc vs Rc clone overhead for GF(2^m) fields.
//!
//! Phase 15: Validates that Arc has negligible overhead compared to Rc.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use gf2_core::gf2m::Gf2mField;

fn bench_field_clone(c: &mut Criterion) {
    let field = Gf2mField::gf256();

    c.bench_function("field_clone_gf256", |b| {
        b.iter(|| black_box(field.clone()))
    });
}

fn bench_field_clone_with_tables(c: &mut Criterion) {
    let field = Gf2mField::gf256().with_tables();

    c.bench_function("field_clone_gf256_with_tables", |b| {
        b.iter(|| black_box(field.clone()))
    });
}

fn bench_field_element_creation(c: &mut Criterion) {
    let field = Gf2mField::gf256().with_tables();

    c.bench_function("element_creation_gf256", |b| {
        b.iter(|| {
            let a = black_box(field.element(42));
            a
        })
    });
}

fn bench_field_multiplication(c: &mut Criterion) {
    let field = Gf2mField::gf256().with_tables();
    let a = field.element(42);
    let b = field.element(137);

    c.bench_function("multiply_gf256", |bencher| {
        bencher.iter(|| black_box(&a * &b))
    });
}

criterion_group!(
    benches,
    bench_field_clone,
    bench_field_clone_with_tables,
    bench_field_element_creation,
    bench_field_multiplication
);
criterion_main!(benches);
