# Phase 9.4: Extended Performance Benchmarking - Implementation Plan

**Priority**: Medium  
**Goal**: Benchmark against performance-oriented C/C++ libraries beyond Sage

---

## Overview

Phase 9.4 extends benchmarking to specialized, production-grade C/C++ libraries that prioritize performance over generality. This establishes competitive positioning against libraries used in high-performance computing, cryptography, and coding theory applications.

### Why This Matters

SageMath (Phase 9.3) is a general-purpose computer algebra system optimized for correctness and flexibility, not raw performance. To claim true competitive performance, we must benchmark against:
- **Domain specialists**: Libraries focused solely on finite fields and GF(2) operations
- **Production systems**: Code used in real-world cryptographic and coding applications
- **Hardware-optimized**: Implementations with decades of low-level optimization

---

## Target Libraries

### Tier 1 (Essential)

#### NTL (Number Theory Library)
- **Language**: C++
- **Focus**: Polynomial arithmetic, finite fields, factorization
- **Why**: Gold standard for polynomial operations, 30+ years of optimization
- **Strengths**: GF(2^m) multiplication, polynomial GCD, irreducibility testing
- **Usage**: Cryptography research, production systems
- **Benchmarks**: 
  - GF(2^m) multiplication (m=8,16,32,64)
  - Polynomial GCD
  - Irreducibility testing (compare to our Rabin test)

#### M4RI (Methods of the Four Russians Inversion)
- **Language**: C
- **Focus**: Dense GF(2) matrices, M4RM multiplication
- **Why**: Backend for Sage's matrix operations, highly specialized
- **Strengths**: Matrix multiplication, Gaussian elimination, inversion
- **Usage**: Sage, GAP, coding theory research
- **Benchmarks**:
  - Matrix multiplication (1024x1024, 2048x2048)
  - Gaussian elimination
  - Our M4RM vs their M4RM implementation

### Tier 2 (Important)

#### FLINT (Fast Library for Number Theory)
- **Language**: C
- **Focus**: Modern number theory, polynomial arithmetic
- **Why**: Active development, SIMD optimizations, Sage backend
- **Strengths**: Polynomial factorization, GF(p^m) operations
- **Usage**: Modern Sage versions, research
- **Benchmarks**:
  - Polynomial multiplication over GF(2)[x]
  - GF(2^m) field operations
  - Polynomial factorization

#### GF-Complete
- **Language**: C (by James Plank)
- **Focus**: Galois Field arithmetic for erasure coding
- **Why**: Production erasure codes, hardware intrinsics (PCLMULQDQ)
- **Strengths**: GF(2^8), GF(2^16) optimized multiplication, table methods
- **Usage**: Real-world erasure coding systems, RAID-like applications
- **Benchmarks**:
  - GF(2^8) and GF(2^16) multiplication throughput
  - Table-based vs SIMD methods
  - Region operations (bulk data processing)

### Tier 3 (Nice-to-Have)

#### Magma
- **Language**: Proprietary (compiled)
- **Focus**: Computational algebra system
- **Why**: Industry performance leader, if accessible
- **Strengths**: Best-in-class algorithms across the board
- **Challenge**: Requires expensive license, may not be available

#### Catid's cm256/wirehair
- **Language**: C++
- **Focus**: Fast erasure coding (Cauchy Reed-Solomon)
- **Why**: Production-grade GF(2^8) operations
- **Benchmarks**: Real-world erasure coding throughput

---

## Benchmark Categories

### Category A: GF(2^m) Field Operations

**Operations**:
1. Multiplication (1M operations)
2. Inversion
3. Exponentiation
4. Primitive element finding

**Field Sizes**: m = 4, 8, 16, 32, 64

**Libraries**: NTL, FLINT, GF-Complete

**Expected Results**:
- Table methods competitive for m ≤ 16
- SIMD methods (PCLMULQDQ) competitive for m > 16
- Target: Within 2x of NTL for all operations

### Category B: Polynomial Arithmetic

**Operations**:
1. Multiplication (degrees: 50, 100, 200, 500)
2. GCD (degree 100 pairs)
3. Evaluation (Horner's method)

**Libraries**: NTL, FLINT

**Expected Results**:
- Karatsuba competitive for degree > 100
- Target: Within 1.5x of NTL for multiplication

### Category C: Matrix Operations

**Operations**:
1. M4RM multiplication (1024x1024, 2048x2048)
2. Gaussian elimination (1024x1024)
3. Matrix inversion (512x512)

**Libraries**: M4RI

**Expected Results**:
- Our M4RM should be comparable (same algorithm)
- Target: Within 1.2x of M4RI (they have decades of tuning)

### Category D: Primitive Polynomial Operations

**Operations**:
1. Primitivity verification (m = 4, 8, 12, 16)
2. Irreducibility testing
3. Polynomial generation (if supported)

**Libraries**: NTL, FLINT

**Expected Results**:
- Our order-based test already fast (Phase 9.3)
- Target: Match or exceed NTL

### Category E: Erasure Coding Workloads

**Operations**:
1. GF(2^8) bulk multiplication (1MB data)
2. Region operations
3. Systematic encoding

**Libraries**: GF-Complete, cm256

**Expected Results**:
- SIMD optimizations critical here
- Target: Within 1.5x for bulk operations

---

## Implementation Phases

### Phase 9.4.1: Setup & Infrastructure ✅ COMPLETE

**Objective**: Install libraries, create build system, verify functionality

**Tasks**:
- [x] Install NTL, M4RI, FLINT (GF-Complete deferred)
- [x] Create CMake build system for C/C++ benchmarks
- [x] Write benchmark programs
- [x] Document library versions (NTL 11.6.0, M4RI 20250128, FLINT 3.3.1)
- [x] Verify builds and execution

**Deliverables**:
- `scripts/setup_libraries.sh` - Install script
- `benchmarks-cpp/` - C/C++ benchmark directory
- `benchmarks-cpp/CMakeLists.txt` - Build configuration
- `benchmarks-cpp/test_*.cpp` - Verification tests

### Phase 9.4.2: Micro-Benchmarks ✅ COMPLETE

**Objective**: Implement and run micro-benchmarks for basic operations

**Results**:
- **NTL GF(2^m)**: We're 13-18x faster for m ≤ 16, 1.9x faster for m=32, 2x slower for m=64
- **M4RI matrices**: They're 5-7x faster for multiplication - significant optimization opportunity identified
- **FLINT polynomials**: Different domain (GF(2)[x] vs GF(2^m)[x]) - informative but not directly comparable
- **M4RI inversion**: Successfully benchmarked (0.50ms for 256x256, 8.61ms for 1024x1024)

**Tasks**:
- [x] Implement NTL benchmarks for GF(2^m) operations
- [x] Implement M4RI benchmarks for matrix operations
- [x] Implement FLINT benchmarks for polynomials
- [x] Run and collect timing data
- [x] Compare with our Phase 9.3 results
- [x] Update BENCHMARKS.md with C/C++ comparisons

**Deliverables**:
- `benchmarks-cpp/bench_field_ops.cpp` - Field operation benchmarks
- `benchmarks-cpp/bench_matrix.cpp` - Matrix benchmarks
- `benchmarks-cpp/bench_polynomial.cpp` - Polynomial benchmarks
- `docs/phase9_4_micro_results.md` - Initial results

### Phase 9.4.3: Specialized Benchmarks (Week 3)

**Objective**: Test domain-specific optimizations

**Focus**:
- GF-Complete region operations
- M4RI Gaussian elimination
- NTL polynomial GCD

**Tasks**:
- [ ] Implement GF-Complete erasure coding scenarios
- [ ] Benchmark M4RI elimination vs our implementation
- [ ] Test NTL's polynomial algorithms
- [ ] Identify performance gaps
- [ ] Document algorithmic differences

**Deliverables**:
- `benchmarks-cpp/bench_gf_complete.c` - GF-Complete benchmarks
- `benchmarks-cpp/bench_m4ri_elimination.c` - Elimination tests
- Analysis of algorithmic trade-offs

### Phase 9.4.4: Large-Scale Benchmarks (Week 4)

**Objective**: Test at production scales

**Focus**:
- Large matrices (2048x2048, 4096x4096)
- High-degree polynomials (degree 1000+)
- Bulk data processing (MB-scale)

**Tasks**:
- [ ] Run large matrix benchmarks
- [ ] Test polynomial operations at scale
- [ ] Memory usage analysis
- [ ] Scaling characteristics
- [ ] Cache behavior analysis

**Deliverables**:
- Large-scale timing results
- Memory footprint comparison
- Scaling analysis

### Phase 9.4.5: Analysis & Optimization (Week 5)

**Objective**: Identify gaps, optimize, document

**Tasks**:
- [ ] Analyze performance gaps
- [ ] Identify optimization opportunities
- [ ] Implement critical optimizations (if time permits)
- [ ] Update BENCHMARKS.md with full comparison
- [ ] Create competitive analysis document

**Deliverables**:
- Updated `docs/BENCHMARKS.md` with C/C++ comparisons
- `docs/COMPETITIVE_ANALYSIS.md` - Positioning vs specialists
- List of future optimization targets

---

## Benchmark Harness Design

### C/C++ Side

```cpp
// benchmarks-cpp/bench_field_ops.cpp
#include <NTL/GF2E.h>
#include <chrono>

using namespace NTL;

void bench_ntl_multiplication(int m, int iterations) {
    GF2X modulus = BuildIrred_GF2X(m);
    GF2E::init(modulus);
    
    GF2E a = random_GF2E();
    GF2E b = random_GF2E();
    
    auto start = chrono::high_resolution_clock::now();
    for (int i = 0; i < iterations; i++) {
        GF2E c = a * b;
    }
    auto end = chrono::high_resolution_clock::now();
    
    auto duration = chrono::duration_cast<chrono::nanoseconds>(end - start);
    cout << m << "," << duration.count() / iterations << endl;
}
```

### Python Comparison Script

```python
# scripts/compare_cpp_libraries.py
import subprocess
import pandas as pd

def run_cpp_benchmark(lib, operation, params):
    """Run C++ benchmark and parse output"""
    cmd = f"./benchmarks-cpp/bench_{lib} {operation} {params}"
    result = subprocess.check_output(cmd, shell=True)
    return parse_timing(result)

def compare_with_rust(lib, operation, m):
    """Compare C++ library with our Rust implementation"""
    cpp_time = run_cpp_benchmark(lib, operation, m)
    rust_time = get_rust_benchmark(operation, m)
    
    return {
        'library': lib,
        'operation': operation,
        'm': m,
        'cpp_time': cpp_time,
        'rust_time': rust_time,
        'ratio': rust_time / cpp_time
    }
```

---

## Success Criteria

### Performance Targets

| Category | Library | Target |
|----------|---------|--------|
| GF(2^m) multiplication | NTL | Within 2x |
| Polynomial multiplication | NTL/FLINT | Within 1.5x |
| Matrix M4RM | M4RI | Within 1.2x |
| Erasure coding ops | GF-Complete | Within 1.5x |
| Primitivity testing | NTL | Match or exceed |

### Competitive Positioning

**Success** means demonstrating:
1. Competitive performance across all categories
2. Superior performance in at least one category
3. Clear understanding of trade-offs and optimization opportunities
4. Documented path to further improvements

---

## Known Challenges

### Challenge 1: Library Installation Complexity
**Issue**: Multiple dependencies, version compatibility  
**Mitigation**: Containerized builds (Docker), clear documentation

### Challenge 2: Fair Comparison
**Issue**: Different algorithms, build optimizations  
**Mitigation**: Document algorithm differences, use same compiler flags where possible

### Challenge 3: API Differences
**Issue**: NTL uses different data structures than ours  
**Mitigation**: Exclude conversion overhead, focus on core operations

### Challenge 4: SIMD Capabilities
**Issue**: Libraries may use AVX-512, we use AVX2  
**Mitigation**: Document SIMD instruction sets used, test on same hardware

---

## Deliverables Summary

### Code
- [ ] `scripts/setup_libraries.sh` - Library installation
- [ ] `benchmarks-cpp/` directory with all C/C++ benchmarks
- [ ] `scripts/compare_cpp_libraries.py` - Comparison harness

### Documentation
- [ ] Updated `docs/BENCHMARKS.md` with C/C++ comparisons
- [ ] `docs/COMPETITIVE_ANALYSIS.md` - Strategic positioning
- [ ] `docs/OPTIMIZATION_OPPORTUNITIES.md` - Future work

### Analysis
- [ ] Performance comparison tables
- [ ] Algorithmic trade-off analysis
- [ ] Memory and scaling characteristics
- [ ] Recommendations for optimization priorities

---

## Timeline

**Total Duration**: 5 weeks

| Week | Phase | Deliverables |
|------|-------|--------------|
| 1 | Setup & Infrastructure | Build system, library installation |
| 2 | Micro-Benchmarks | Basic operation comparisons |
| 3 | Specialized Benchmarks | Domain-specific tests |
| 4 | Large-Scale Benchmarks | Production-scale testing |
| 5 | Analysis & Optimization | Documentation, optimization plan |

---

## Next Steps

1. **Assess library availability**: Check if NTL, M4RI, FLINT are accessible
2. **Start with Tier 1**: Focus on NTL and M4RI first
3. **Incremental approach**: One library at a time, validate before moving on
4. **Document everything**: Track versions, build flags, hardware specs

**Phase 9.4 Success**: Comprehensive understanding of competitive position vs production C/C++ libraries, with clear optimization roadmap.
