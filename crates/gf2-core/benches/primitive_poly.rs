//! Benchmarks for primitive polynomial verification and testing.
//!
//! This benchmark suite measures the performance of Phase 9 primitivity testing
//! operations, which are critical for ensuring correctness of GF(2^m) constructions.
//!
//! # Benchmark Groups
//!
//! 1. **Primitivity Verification**: Full primitive polynomial test (irreducibility + order)
//! 2. **Irreducibility Testing**: Rabin's irreducibility test only
//! 3. **Order Computation**: Testing multiplicative order of x
//!
//! # Sage Comparison Markers
//!
//! Benchmarks marked with `[SAGE_CMP]` have equivalent implementations in
//! `scripts/sage_benchmarks.py` for direct performance comparison.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use gf2_core::gf2m::Gf2mField;

/// Test polynomials for various degrees.
/// Format: (degree, polynomial_value, is_primitive, description)
const TEST_POLYNOMIALS: &[(usize, u64, bool, &str)] = &[
    // Small degrees (m=2..8)
    (2, 0b111, true, "GF(4): x^2 + x + 1"),
    (3, 0b1011, true, "GF(8): x^3 + x + 1"),
    (4, 0b10011, true, "GF(16): x^4 + x + 1"),
    (5, 0b100101, true, "GF(32): x^5 + x^2 + 1"),
    (8, 0b100011101, true, "GF(256): x^8 + x^4 + x^3 + x^2 + 1"),
    // DVB-T2 standard polynomials (m=10,14,16)
    (10, 0b10000001001, true, "DVB-T2 GF(1024): x^10 + x^3 + 1"),
    (
        14,
        0b100000000101011,
        true,
        "DVB-T2 GF(16384): x^14 + x^5 + x^3 + x + 1",
    ),
    (
        16,
        0b10000000000101101,
        true,
        "DVB-T2 GF(65536): x^16 + x^5 + x^3 + x^2 + 1",
    ),
    // Non-primitive but irreducible (for negative testing)
    (
        8,
        0b100011011,
        false,
        "AES polynomial (irreducible but NOT primitive)",
    ),
    (
        14,
        0b100000000100001,
        false,
        "x^14 + x^5 + 1 (irreducible but NOT primitive)",
    ),
];

/// Additional primitive polynomials for extended testing
const EXTENDED_PRIMITIVES: &[(usize, u64, &str)] = &[
    // Trinomials (hardware-efficient)
    (9, 0b1000010001, "x^9 + x^4 + 1"),
    (11, 0b100000000101, "x^11 + x^2 + 1"),
    (13, 0b10000000011011, "x^13 + x^4 + x^3 + x + 1"),
    (15, 0b1000000000000011, "x^15 + x + 1"),
    (17, 0b100000000000001001, "x^17 + x^3 + 1"),
    // Larger degrees for scaling tests
    (20, 0b100000000000000001001, "x^20 + x^3 + 1"),
    (
        24,
        0b1000000000000000000010000111,
        "x^24 + x^7 + x^2 + x + 1",
    ),
    (
        32,
        0b100000000000000000000000001100011,
        "x^32 + x^7 + x^5 + x^3 + x^2 + x + 1",
    ),
];

/// [SAGE_CMP] Benchmark full primitivity verification (irreducibility + order test)
///
/// This is the complete test: verify_primitive() which checks:
/// 1. Rabin irreducibility test
/// 2. Order verification (x has multiplicative order 2^m-1)
///
/// Compare with Sage: `GF(2^m, modulus=poly).is_primitive()`
fn bench_primitivity_verification(c: &mut Criterion) {
    let mut group = c.benchmark_group("primitivity_verification");
    group.sample_size(50); // Smaller sample for expensive tests

    for &(m, poly, _is_prim, desc) in TEST_POLYNOMIALS {
        let field = Gf2mField::new(m, poly);

        group.bench_with_input(
            BenchmarkId::new(format!("m={}", m), desc),
            &field,
            |b, field| {
                b.iter(|| black_box(field.verify_primitive()));
            },
        );
    }

    group.finish();
}

/// [SAGE_CMP] Benchmark Rabin irreducibility test only
///
/// Tests only irreducibility without order verification.
/// This is faster and useful for polynomial generation.
///
/// Compare with Sage: `poly.is_irreducible()`
fn bench_irreducibility_only(c: &mut Criterion) {
    let mut group = c.benchmark_group("irreducibility_rabin");
    group.sample_size(100);

    for &(m, poly, _, desc) in TEST_POLYNOMIALS {
        let field = Gf2mField::new(m, poly);

        group.bench_with_input(
            BenchmarkId::new(format!("m={}", m), desc),
            &field,
            |b, field| {
                b.iter(|| black_box(field.is_irreducible_rabin()));
            },
        );
    }

    group.finish();
}

/// Benchmark primitivity across a range of degrees
///
/// Tests scaling behavior from m=2 to m=32
fn bench_primitivity_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("primitivity_scaling");
    group.sample_size(30);

    // Combine test sets
    let mut all_tests: Vec<(usize, u64, &str)> = TEST_POLYNOMIALS
        .iter()
        .filter(|(_, _, is_prim, _)| *is_prim)
        .map(|(m, poly, _, desc)| (*m, *poly, *desc))
        .collect();

    all_tests.extend(EXTENDED_PRIMITIVES.iter().copied());

    for (m, poly, desc) in all_tests {
        let field = Gf2mField::new(m, poly);

        group.bench_with_input(
            BenchmarkId::new(
                "degree",
                format!("m={:02}_{}", m, desc.split(':').next().unwrap_or("")),
            ),
            &field,
            |b, field| {
                b.iter(|| black_box(field.verify_primitive()));
            },
        );
    }

    group.finish();
}

/// Benchmark irreducibility for various degrees
///
/// Measures Rabin test performance across different polynomial sizes
fn bench_irreducibility_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("irreducibility_scaling");
    group.sample_size(50);

    let mut all_tests: Vec<(usize, u64, &str)> = TEST_POLYNOMIALS
        .iter()
        .map(|(m, poly, _, desc)| (*m, *poly, *desc))
        .collect();

    all_tests.extend(EXTENDED_PRIMITIVES.iter().copied());

    for (i, (m, poly, _desc)) in all_tests.iter().enumerate() {
        let field = Gf2mField::new(*m, *poly);

        group.bench_with_input(
            BenchmarkId::new("degree", format!("m={:02}_poly{}", m, i)),
            &field,
            |b, field| {
                b.iter(|| black_box(field.is_irreducible_rabin()));
            },
        );
    }

    group.finish();
}

/// Benchmark detection of non-primitive polynomials
///
/// Tests performance when verification correctly rejects non-primitive polynomials.
/// This is important for polynomial generation where we try many candidates.
fn bench_nonprimitive_detection(c: &mut Criterion) {
    let mut group = c.benchmark_group("nonprimitive_detection");
    group.sample_size(50);

    // Test non-primitive polynomials
    let non_primitives = TEST_POLYNOMIALS
        .iter()
        .filter(|(_, _, is_prim, _)| !*is_prim);

    for &(m, poly, _, desc) in non_primitives {
        let field = Gf2mField::new(m, poly);

        group.bench_with_input(
            BenchmarkId::new(format!("m={}", m), desc),
            &field,
            |b, field| {
                b.iter(|| {
                    let result = field.verify_primitive();
                    black_box(result);
                    assert!(!result, "Should detect non-primitive polynomial");
                });
            },
        );
    }

    group.finish();
}

/// Benchmark standard field constructors with primitivity check
///
/// Measures overhead of verification in factory methods
fn bench_field_construction_with_verify(c: &mut Criterion) {
    let mut group = c.benchmark_group("field_construction");
    group.sample_size(100);

    // GF(256)
    group.bench_function("GF256_construct_verify", |b| {
        b.iter(|| {
            let field = Gf2mField::gf256();
            black_box(field.verify_primitive());
        });
    });

    // GF(65536)
    group.bench_function("GF65536_construct_verify", |b| {
        b.iter(|| {
            let field = Gf2mField::gf65536();
            black_box(field.verify_primitive());
        });
    });

    // Custom field construction
    group.bench_function("Custom_GF16384_construct_verify", |b| {
        b.iter(|| {
            let field = Gf2mField::new(14, 0b100000000101011);
            black_box(field.verify_primitive());
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_primitivity_verification,
    bench_irreducibility_only,
    bench_primitivity_scaling,
    bench_irreducibility_scaling,
    bench_nonprimitive_detection,
    bench_field_construction_with_verify,
);
criterion_main!(benches);
