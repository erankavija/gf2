//! Benchmarks for GF(2^m) polynomial arithmetic operations.
//!
//! These benchmarks measure the performance of polynomial operations over extension fields,
//! which are critical for BCH codes and other error-correcting codes.
//!
//! # Sage Comparison
//!
//! Benchmarks marked with `[SAGE_CMP]` have equivalent implementations in
//! `scripts/sage_benchmarks.py` for direct performance comparison against SageMath.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use gf2_core::gf2m::{Gf2mField, Gf2mPoly};

/// Helper to create a polynomial with random-ish coefficients of given degree
fn random_poly(field: &Gf2mField, degree: usize, seed: u32) -> Gf2mPoly {
    let mut coeffs = Vec::with_capacity(degree + 1);
    let modulus = ((1u64 << field.degree()) - 1) as u32;
    for i in 0..=degree {
        // Simple deterministic "random" values for reproducibility
        let value = ((seed.wrapping_mul(31).wrapping_add(i as u32)) % modulus) as u64;
        coeffs.push(field.element(if value == 0 { 1 } else { value }));
    }
    Gf2mPoly::new(coeffs)
}

fn bench_poly_addition(c: &mut Criterion) {
    let mut group = c.benchmark_group("polynomial_addition");

    for field_name in ["GF(256)", "GF(65536)"] {
        let field = match field_name {
            "GF(256)" => Gf2mField::gf256(),
            "GF(65536)" => Gf2mField::gf65536(),
            _ => unreachable!(),
        };

        for &degree in &[10, 50, 100, 500, 1000] {
            let p1 = random_poly(&field, degree, 42);
            let p2 = random_poly(&field, degree, 43);

            group.throughput(Throughput::Elements(degree as u64));
            group.bench_with_input(
                BenchmarkId::new(field_name, degree),
                &(&p1, &p2),
                |b, (p1, p2)| {
                    b.iter(|| black_box(*p1 + *p2));
                },
            );
        }
    }

    group.finish();
}

/// [SAGE_CMP] Benchmark polynomial multiplication
///
/// Compare with Sage: `poly1 * poly2` in polynomial ring over GF(2^m)
fn bench_poly_multiplication_schoolbook(c: &mut Criterion) {
    let mut group = c.benchmark_group("polynomial_multiplication_schoolbook");

    for field_name in ["GF(256)", "GF(65536)"] {
        let field = match field_name {
            "GF(256)" => Gf2mField::gf256(),
            "GF(65536)" => Gf2mField::gf65536(),
            _ => unreachable!(),
        };

        // Test various polynomial degrees
        for &degree in &[5, 10, 20, 50, 100, 200] {
            let p1 = random_poly(&field, degree, 42);
            let p2 = random_poly(&field, degree, 43);

            // Throughput is O(n²) coefficient multiplications
            group.throughput(Throughput::Elements((degree * degree) as u64));
            group.bench_with_input(
                BenchmarkId::new(field_name, degree),
                &(&p1, &p2),
                |b, (p1, p2)| {
                    b.iter(|| black_box(*p1 * *p2));
                },
            );
        }
    }

    group.finish();
}

fn bench_poly_division(c: &mut Criterion) {
    let mut group = c.benchmark_group("polynomial_division");

    for field_name in ["GF(256)", "GF(65536)"] {
        let field = match field_name {
            "GF(256)" => Gf2mField::gf256(),
            "GF(65536)" => Gf2mField::gf65536(),
            _ => unreachable!(),
        };

        // Divide polynomials of varying degrees
        for &dividend_deg in &[20, 50, 100, 200] {
            for &divisor_deg in &[5, 10, 20] {
                if divisor_deg >= dividend_deg {
                    continue;
                }

                let dividend = random_poly(&field, dividend_deg, 42);
                let divisor = random_poly(&field, divisor_deg, 43);

                group.throughput(Throughput::Elements(dividend_deg as u64));
                group.bench_with_input(
                    BenchmarkId::new(
                        format!("{}_dividend", field_name),
                        format!("{}÷{}", dividend_deg, divisor_deg),
                    ),
                    &(&dividend, &divisor),
                    |b, (dividend, divisor)| {
                        b.iter(|| black_box(dividend.div_rem(divisor)));
                    },
                );
            }
        }
    }

    group.finish();
}

fn bench_poly_gcd(c: &mut Criterion) {
    let mut group = c.benchmark_group("polynomial_gcd");

    for field_name in ["GF(256)", "GF(65536)"] {
        let field = match field_name {
            "GF(256)" => Gf2mField::gf256(),
            "GF(65536)" => Gf2mField::gf65536(),
            _ => unreachable!(),
        };

        // Test GCD on polynomials with known common factors
        for &degree in &[10, 20, 50, 100] {
            // Create polynomials with a common factor
            let common_factor = random_poly(&field, degree / 4, 40);
            let factor1 = random_poly(&field, degree / 2, 41);
            let factor2 = random_poly(&field, degree / 2, 42);

            let p1 = &common_factor * &factor1;
            let p2 = &common_factor * &factor2;

            group.throughput(Throughput::Elements(degree as u64));
            group.bench_with_input(
                BenchmarkId::new(field_name, degree),
                &(&p1, &p2),
                |b, (p1, p2)| {
                    b.iter(|| black_box(Gf2mPoly::gcd(p1, p2)));
                },
            );
        }
    }

    group.finish();
}

fn bench_poly_eval(c: &mut Criterion) {
    let mut group = c.benchmark_group("polynomial_evaluation");

    for field_name in ["GF(256)", "GF(65536)"] {
        let field = match field_name {
            "GF(256)" => Gf2mField::gf256(),
            "GF(65536)" => Gf2mField::gf65536(),
            _ => unreachable!(),
        };

        for &degree in &[10, 50, 100, 500, 1000] {
            let poly = random_poly(&field, degree, 42);
            let x = field.element(5);

            group.throughput(Throughput::Elements(degree as u64));
            group.bench_with_input(
                BenchmarkId::new(field_name, degree),
                &(&poly, &x),
                |b, &(poly, x)| {
                    b.iter(|| black_box(poly.eval(x)));
                },
            );
        }
    }

    group.finish();
}

fn bench_poly_eval_batch(c: &mut Criterion) {
    let mut group = c.benchmark_group("polynomial_evaluation_batch");

    for field_name in ["GF(256)", "GF(65536)"] {
        let field = match field_name {
            "GF(256)" => Gf2mField::gf256(),
            "GF(65536)" => Gf2mField::gf65536(),
            _ => unreachable!(),
        };

        // BCH syndrome computation pattern: evaluate at multiple points
        for &poly_degree in &[50, 100, 200] {
            for &num_points in &[4, 8, 16, 32] {
                let poly = random_poly(&field, poly_degree, 42);
                let points: Vec<_> = (1..=num_points).map(|i| field.element(i as u64)).collect();

                // Total operations: poly_degree * num_points
                group.throughput(Throughput::Elements((poly_degree * num_points) as u64));
                group.bench_with_input(
                    BenchmarkId::new(
                        format!("{}_deg{}", field_name, poly_degree),
                        format!("{}pts", num_points),
                    ),
                    &(&poly, &points),
                    |b, (poly, points)| {
                        b.iter(|| black_box(poly.eval_batch(points)));
                    },
                );
            }
        }
    }

    group.finish();
}

fn bench_minimal_polynomial(c: &mut Criterion) {
    let mut group = c.benchmark_group("minimal_polynomial");

    for field_name in ["GF(256)", "GF(65536)"] {
        let field = match field_name {
            "GF(256)" => Gf2mField::gf256(),
            "GF(65536)" => Gf2mField::gf65536(),
            _ => unreachable!(),
        };

        // Test minimal polynomial computation for various elements
        // Minimal polynomial degree divides m, so worst case is m
        for &elem_value in &[2, 3, 5, 7, 11, 13] {
            if elem_value >= (1 << field.degree()) {
                continue;
            }

            let element = field.element(elem_value);

            group.bench_with_input(
                BenchmarkId::new(field_name, format!("α^{}", elem_value)),
                &element,
                |b, elem| {
                    b.iter(|| black_box(elem.minimal_polynomial()));
                },
            );
        }
    }

    group.finish();
}

// Benchmark polynomial operations specifically sized for BCH(255,k) codes
fn bench_bch_syndrome_pattern(c: &mut Criterion) {
    let mut group = c.benchmark_group("bch_syndrome_simulation");
    group.sample_size(50);

    let field = Gf2mField::gf256();

    // BCH(255, k) with t=16 has syndrome polynomial of degree up to 255
    // and evaluates at 2t consecutive roots
    let message_poly = random_poly(&field, 254, 42); // degree 254 for 255-bit message

    // Typical BCH evaluation points: α, α², α³, ..., α^(2t)
    let t = 16;
    let num_syndromes = 2 * t;

    let mut eval_points = Vec::with_capacity(num_syndromes);
    let alpha = field.element(2); // primitive element
    let mut alpha_power = alpha.clone();
    for _ in 0..num_syndromes {
        eval_points.push(alpha_power.clone());
        alpha_power = &alpha_power * &alpha;
    }

    group.throughput(Throughput::Elements((254 * num_syndromes) as u64));
    group.bench_function("BCH(255)_t=16_syndrome_eval", |b| {
        b.iter(|| black_box(message_poly.eval_batch(&eval_points)));
    });

    group.finish();
}

/// [SAGE_CMP] Benchmark field element multiplication
///
/// Direct element-to-element multiplication in GF(2^m).
/// Compare with Sage: `a * b` where a, b ∈ GF(2^m)
fn bench_field_element_multiply(c: &mut Criterion) {
    let mut group = c.benchmark_group("field_element_multiply");
    group.sample_size(1000);

    // Test different field sizes
    let test_fields = [(8, "GF(256)"), (16, "GF(65536)")];

    for (m, name) in test_fields {
        let field = if m == 8 {
            Gf2mField::gf256()
        } else {
            Gf2mField::gf65536()
        };

        // Create test elements
        let a = field.element(42);
        let b = field.element(123);

        group.throughput(Throughput::Elements(1));
        group.bench_with_input(
            BenchmarkId::new(name, "single"),
            &(&field, &a, &b),
            |bench, (_f, a, b)| {
                bench.iter(|| black_box(*a * *b));
            },
        );
    }

    group.finish();
}

/// [SAGE_CMP] Benchmark batch field element multiplications
///
/// Measures throughput of repeated element multiplications.
/// Compare with Sage batch operations.
fn bench_field_element_multiply_batch(c: &mut Criterion) {
    let mut group = c.benchmark_group("field_element_multiply_batch");

    let test_fields = [(8, "GF(256)"), (16, "GF(65536)")];

    for (m, name) in test_fields {
        let field = if m == 8 {
            Gf2mField::gf256()
        } else {
            Gf2mField::gf65536()
        };

        // Create arrays of elements
        let count = 1000;
        let elements_a: Vec<_> = (0..count).map(|i| field.element(i)).collect();
        let elements_b: Vec<_> = (0..count).map(|i| field.element(i * 3 + 7)).collect();

        group.throughput(Throughput::Elements(count));
        group.bench_with_input(
            BenchmarkId::new(name, format!("{}ops", count)),
            &(&elements_a, &elements_b),
            |bench, (a, b)| {
                bench.iter(|| {
                    let mut results = Vec::with_capacity(count as usize);
                    for i in 0..count as usize {
                        results.push(&a[i] * &b[i]);
                    }
                    black_box(results)
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_poly_addition,
    bench_poly_multiplication_schoolbook,
    bench_poly_division,
    bench_poly_gcd,
    bench_poly_eval,
    bench_poly_eval_batch,
    bench_minimal_polynomial,
    bench_bch_syndrome_pattern,
    bench_field_element_multiply,
    bench_field_element_multiply_batch,
);
criterion_main!(benches);
