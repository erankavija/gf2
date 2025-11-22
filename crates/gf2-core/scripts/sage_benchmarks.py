#!/usr/bin/env sage
"""
Sage benchmark script for comparing GF(2^m) operations with gf2-core.

This script benchmarks:
1. Primitive polynomial verification (is_primitive)
2. Irreducibility testing (is_irreducible)  
3. Polynomial multiplication
4. Field element operations

Output is JSON format for automated comparison with Rust benchmarks.
"""

import json
import time
from sage.all import GF, ZZ, PolynomialRing, Matrix, random_matrix

def poly_from_int(m, poly_int):
    """Convert integer representation to Sage polynomial over GF(2)."""
    R = PolynomialRing(GF(2), 'x')
    x = R.gen()
    poly = R(0)
    for i in range(m + 1):
        if poly_int & (1 << i):
            poly += x**i
    return poly

def int_from_poly(poly):
    """Convert Sage polynomial to integer representation."""
    result = 0
    for i, coeff in enumerate(poly.list()):
        if coeff:
            result |= (1 << i)
    return result

def time_operation(func, iterations=100):
    """Time an operation over multiple iterations."""
    # Warmup
    for _ in range(min(10, iterations // 10)):
        func()
    
    # Actual timing
    start = time.perf_counter()
    for _ in range(iterations):
        func()
    end = time.perf_counter()
    
    return (end - start) / iterations

# Test polynomials matching Rust benchmarks
TEST_POLYNOMIALS = [
    (2, 0b111, True, "GF(4): x^2 + x + 1"),
    (3, 0b1011, True, "GF(8): x^3 + x + 1"),
    (4, 0b10011, True, "GF(16): x^4 + x + 1"),
    (5, 0b100101, True, "GF(32): x^5 + x^2 + 1"),
    (8, 0b100011101, True, "GF(256): x^8 + x^4 + x^3 + x^2 + 1"),
    (10, 0b10000001001, True, "DVB-T2 GF(1024): x^10 + x^3 + 1"),
    (14, 0b100000000101011, True, "DVB-T2 GF(16384): x^14 + x^5 + x^3 + x + 1"),
    (16, 0b10000000000101101, True, "DVB-T2 GF(65536): x^16 + x^5 + x^3 + x^2 + 1"),
    (8, 0b100011011, False, "AES polynomial (irreducible but NOT primitive)"),
    (14, 0b100000000100001, False, "x^14 + x^5 + 1 (irreducible but NOT primitive)"),
]

EXTENDED_PRIMITIVES = [
    (9, 0b1000010001, "x^9 + x^4 + 1"),
    (11, 0b100000000101, "x^11 + x^2 + 1"),
    (13, 0b10000000011011, "x^13 + x^4 + x^3 + x + 1"),
    (15, 0b1000000000000011, "x^15 + x + 1"),
    (17, 0b100000000000001001, "x^17 + x^3 + 1"),
    (20, 0b100000000000000001001, "x^20 + x^3 + 1"),
]

def bench_primitivity_verification():
    """Benchmark full primitivity verification."""
    results = {}
    
    print("Benchmarking primitivity verification...")
    for m, poly_int, is_prim, desc in TEST_POLYNOMIALS:
        print(f"  m={m}: {desc}")
        
        # Create polynomial
        poly = poly_from_int(m, poly_int)
        
        # Check if irreducible first (Sage requires this)
        if not poly.is_irreducible():
            # For non-primitive cases, still time is_primitive
            def test_func():
                try:
                    F = GF(2**m, name='a', modulus=poly)
                    # In Sage, is_primitive() on polynomial checks if it generates the field
                    return poly.is_primitive()
                except:
                    return False
        else:
            # For primitive cases
            def test_func():
                F = GF(2**m, name='a', modulus=poly)
                # Check if the generator is primitive
                a = F.gen()
                return a.multiplicative_order() == 2**m - 1
        
        # Time it (fewer iterations for larger m)
        iterations = max(10, min(1000, 10000 // (2**m)))
        avg_time = time_operation(test_func, iterations=iterations)
        
        results[f"m{m}_{desc.split(':')[0]}"] = {
            "degree": m,
            "is_primitive": is_prim,
            "time_ns": avg_time * 1e9,
            "description": desc
        }
    
    return results

def bench_irreducibility():
    """Benchmark irreducibility testing only."""
    results = {}
    
    print("Benchmarking irreducibility testing...")
    for m, poly_int, _, desc in TEST_POLYNOMIALS:
        print(f"  m={m}: {desc}")
        
        poly = poly_from_int(m, poly_int)
        
        def test_func():
            return poly.is_irreducible()
        
        iterations = max(10, min(1000, 10000 // (2**m)))
        avg_time = time_operation(test_func, iterations=iterations)
        
        results[f"m{m}_{desc.split(':')[0]}"] = {
            "degree": m,
            "time_ns": avg_time * 1e9,
            "description": desc
        }
    
    return results

def bench_field_operations():
    """Benchmark basic field arithmetic operations."""
    results = {}
    
    print("Benchmarking field operations...")
    
    test_fields = [
        (8, 0b100011101, "GF(256)"),
        (16, 0b10000000000101101, "GF(65536)"),
    ]
    
    for m, poly_int, name in test_fields:
        print(f"  {name}")
        poly = poly_from_int(m, poly_int)
        F = GF(2**m, name='a', modulus=poly)
        a = F.gen()
        
        # Element multiplication
        b = a**5
        c = a**7
        
        def mult_func():
            return b * c
        
        mult_time = time_operation(mult_func, iterations=10000)
        
        results[f"{name}_multiply"] = {
            "degree": m,
            "operation": "field_element_multiply",
            "time_ns": mult_time * 1e9
        }
        
        # Batch multiplication (1000 operations)
        elements_a = [F.random_element() for _ in range(1000)]
        elements_b = [F.random_element() for _ in range(1000)]
        
        def batch_mult_func():
            return [elements_a[i] * elements_b[i] for i in range(1000)]
        
        batch_time = time_operation(batch_mult_func, iterations=100)
        
        results[f"{name}_multiply_batch"] = {
            "degree": m,
            "operation": "field_element_multiply_batch",
            "time_ns": batch_time * 1e9,
            "batch_size": 1000
        }
    
    return results

def bench_polynomial_multiplication():
    """Benchmark polynomial multiplication in GF(2^m)[x]."""
    results = {}
    
    print("Benchmarking polynomial multiplication...")
    
    test_configs = [
        (8, 0b100011101, "GF(256)", [10, 50, 100, 200]),
        (16, 0b10000000000101101, "GF(65536)", [10, 50, 100, 200]),
    ]
    
    for m, poly_int, name, degrees in test_configs:
        print(f"  {name}")
        poly = poly_from_int(m, poly_int)
        F = GF(2**m, name='a', modulus=poly)
        R = PolynomialRing(F, 'x')
        
        for deg in degrees:
            print(f"    degree {deg}")
            # Create random polynomials
            p1 = R.random_element(degree=deg)
            p2 = R.random_element(degree=deg)
            
            def mult_func():
                return p1 * p2
            
            # Fewer iterations for larger degrees
            iterations = max(10, min(1000, 10000 // (deg * deg)))
            mult_time = time_operation(mult_func, iterations=iterations)
            
            results[f"{name}_deg{deg}"] = {
                "field_degree": m,
                "poly_degree": deg,
                "time_ns": mult_time * 1e9,
                "field": name
            }
    
    return results

def bench_sparse_matrices():
    """Benchmark sparse matrix operations over GF(2)."""
    results = {}
    
    print("Benchmarking sparse matrix operations...")
    
    test_configs = [
        (100, 0.01, "100x100_1pct"),
        (500, 0.01, "500x500_1pct"),
        (1000, 0.01, "1000x1000_1pct"),
        (500, 0.05, "500x500_5pct"),
    ]
    
    for size, density, name in test_configs:
        print(f"  {name}")
        
        # Create random sparse matrix over GF(2)
        # Sage's random_matrix doesn't have exact density control, so we approximate
        num_entries = int(size * size * density)
        M = Matrix(GF(2), size, size, sparse=True)
        
        # Fill with random 1s to approximate density
        import random
        random.seed(42)
        for _ in range(num_entries):
            i = random.randint(0, size-1)
            j = random.randint(0, size-1)
            M[i, j] = 1
        
        # Create random vector
        from sage.modules.free_module_element import vector
        v = vector(GF(2), [GF(2).random_element() for _ in range(size)])
        
        # Benchmark matrix-vector multiplication
        def matvec_func():
            return M * v
        
        iterations = max(10, min(100, 10000 // size))
        matvec_time = time_operation(matvec_func, iterations=iterations)
        
        results[f"{name}_matvec"] = {
            "size": size,
            "density": density,
            "operation": "sparse_matvec",
            "time_ns": matvec_time * 1e9
        }
        
        # Benchmark transpose
        def transpose_func():
            return M.transpose()
        
        transpose_time = time_operation(transpose_func, iterations=iterations)
        
        results[f"{name}_transpose"] = {
            "size": size,
            "density": density,
            "operation": "sparse_transpose",
            "time_ns": transpose_time * 1e9
        }
    
    return results

def main():
    """Run all benchmarks and output JSON."""
    print("="*60)
    print("Sage GF(2^m) Performance Benchmarks")
    print("="*60)
    print()
    
    results = {
        "sage_version": "10.7",
        "timestamp": time.time(),
        "benchmarks": {}
    }
    
    # Run benchmark suites
    results["benchmarks"]["primitivity_verification"] = bench_primitivity_verification()
    print()
    results["benchmarks"]["irreducibility"] = bench_irreducibility()
    print()
    results["benchmarks"]["field_operations"] = bench_field_operations()
    print()
    results["benchmarks"]["polynomial_multiplication"] = bench_polynomial_multiplication()
    print()
    results["benchmarks"]["sparse_matrices"] = bench_sparse_matrices()
    
    # Output JSON
    output_file = "/tmp/sage_benchmark_results.json"
    with open(output_file, 'w') as f:
        json.dump(results, f, indent=2)
    
    print()
    print("="*60)
    print(f"Results written to: {output_file}")
    print("="*60)
    
    # Print summary
    print("\n### Summary ###")
    print("\nPrimitivity Verification Times:")
    for key, data in results["benchmarks"]["primitivity_verification"].items():
        print(f"  m={data['degree']:2d}: {data['time_ns']/1000:8.2f} µs - {data['description']}")
    
    print("\nIrreducibility Test Times:")
    for key, data in results["benchmarks"]["irreducibility"].items():
        print(f"  m={data['degree']:2d}: {data['time_ns']/1000:8.2f} µs")
    
    print("\nField Element Multiplication:")
    for key, data in results["benchmarks"]["field_operations"].items():
        if "multiply" in key and "batch" not in key:
            print(f"  {key}: {data['time_ns']:8.2f} ns")
    
    print("\nPolynomial Multiplication (selected degrees):")
    for key, data in results["benchmarks"]["polynomial_multiplication"].items():
        if data['poly_degree'] in [10, 100, 200]:
            print(f"  {data['field']} deg={data['poly_degree']}: {data['time_ns']/1000:8.2f} µs")
    
    print("\nSparse Matrix Operations:")
    for key, data in results["benchmarks"]["sparse_matrices"].items():
        if data['operation'] == 'sparse_matvec':
            print(f"  {key}: {data['time_ns']/1000:8.2f} µs")

if __name__ == "__main__":
    main()
