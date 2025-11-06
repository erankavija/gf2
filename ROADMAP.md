# Development Roadmap

This document outlines the phased development plan for the `gf2` workspace. The early phases focus on the `gf2-core` primitives, while later phases inform the `gf2-coding` applications layer.

## Phase 1: Scalar BitVec Baseline ✅ (Current)

**Status**: Complete

### Deliverables
- [x] Core `BitVec` type with `Vec<u64>` backing storage
- [x] Documented invariants (little-endian bit order, tail masking)
- [x] Complete API implementation:
  - Construction: `new()`, `with_capacity()`, `from_bytes_le()`
  - Access: `get()`, `set()`, `len()`, `is_empty()`
  - Stack ops: `push_bit()`, `pop_bit()`
  - Bitwise ops: `bit_and_into()`, `bit_or_into()`, `bit_xor_into()`, `not_into()`
  - Shifts: `shift_left()`, `shift_right()` with word-level optimization
  - Queries: `count_ones()`, `find_first_set()`, `find_last_set()`
  - Utilities: `clear()`, `resize()`, `to_bytes_le()`
- [x] Comprehensive test coverage:
  - Unit tests for all methods
  - Edge cases (0, 1, 63, 64, 65 bits)
  - Boundary conditions near word edges
- [x] Property-based testing with `proptest`:
  - Round-trip conversions
  - Equivalence against reference implementation
  - Random inputs and operations
- [x] Kernel module scaffolding:
  - `Kernel` trait for bulk operations
  - Scalar baseline implementation
  - CPU feature detection stubs (x86, AArch64)
  - Runtime dispatch framework
- [x] Benchmarking infrastructure (Criterion):
  - XOR operations (1 KiB, 64 KiB, 1 MiB)
  - Population count (1 KiB, 64 KiB, 1 MiB)
  - Shifts (64 KiB, various amounts)
- [x] CI/CD (GitHub Actions):
  - Multiple OS (Ubuntu, macOS)
  - Multiple Rust versions (stable, nightly)
  - Format, clippy, tests, docs, benchmarks
- [x] Documentation:
  - README with examples and design notes
  - API documentation with doctests
  - This roadmap

### Performance Characteristics
- Tight word-level loops with minimal branching
- Shifts optimized with whole-word moves + residual carry
- Bit scanning via `trailing_zeros` / `leading_zeros`
- Suitable baseline for small to medium bit vectors

---

## Phase 2: Optimized Wide Buffers

**Status**: Planned  
**Target**: Q1 2026

### Goals
Optimize bulk operations on large bit vectors without SIMD, focusing on cache efficiency and instruction-level parallelism.

### Tasks
- [ ] Add `BitSlice` type for non-owning views:
  - `&BitSlice` and `&mut BitSlice` analogous to `&[T]` / `&mut [T]`
  - Implement `Deref<Target = BitSlice>` for `BitVec`
  - Support subslicing with range indexing
- [ ] Implement optimized scalar kernels:
  - Loop unrolling (4x or 8x) for AND/OR/XOR/NOT
  - Prefetch hints for large buffers (>L1 cache)
  - Streaming stores for write-heavy workloads
- [ ] Add `from_bitslice()` constructors
- [ ] Benchmark improvements vs. Phase 1 baseline
- [ ] Add tests for `BitSlice` API

### Expected Improvements
- 10-20% speedup on large buffers (>64 KiB) from unrolling
- Better cache utilization with prefetching

---

## Phase 3: SIMD Backends and Runtime Dispatch

**Status**: Planned  
**Target**: Q2 2026

### Goals
Add vectorized implementations for x86-64 and AArch64, with runtime feature detection and safe dispatch.

### Tasks
- [ ] **AVX2 backend (x86-64)**:
  - 256-bit AND/OR/XOR/NOT via `_mm256_*` intrinsics
  - Vectorized population count with `_mm256_sad_epu8` + `_mm_popcnt_u64`
  - Implement shifts using shuffles or blend instructions
  - Mark functions `#[target_feature(enable = "avx2")]` and gate with `unsafe`
- [ ] **AVX-512 backend (x86-64)**:
  - 512-bit operations via `_mm512_*` intrinsics
  - Mask-based operations for partial words
  - Optional fallback to AVX2 if not available
- [ ] **NEON backend (AArch64)**:
  - 128-bit operations via `vld1q_u64` / `vst1q_u64` / `veorq_u64` etc.
  - Population count with `vcntq_u8` + horizontal sum
  - Shifts with `vshlq_n_u64` or `vextq_u8` for byte-level shifts
- [ ] Runtime dispatch in `select_kernel()`:
  - Detect AVX2/AVX-512 on x86-64 via `is_x86_feature_detected!`
  - NEON is mandatory on AArch64, detect crypto extensions
  - Return best available kernel from a static registry
- [ ] Integration with `BitVec` methods:
  - Route large-buffer ops through selected kernel
  - Keep inline paths for small sizes (<1 word)
- [ ] Benchmarks comparing scalar vs. SIMD:
  - Measure speedups on 1 KiB, 64 KiB, 1 MiB
  - Track performance across different CPUs in CI
- [ ] Safety audit:
  - Document all `unsafe` blocks
  - Verify alignment and length requirements
  - Add miri tests if feasible

### Expected Improvements
- **AVX2**: 2-4x speedup on large buffers
- **AVX-512**: 3-6x speedup where available
- **NEON**: 2-3x speedup on AArch64

---

## Phase 4: Rank/Select and Advanced Bit Scanning

**Status**: Planned  
**Target**: Q3 2026

### Goals
Add efficient rank (population count up to index) and select (find k-th set bit) operations with auxiliary data structures.

### Tasks
- [ ] Implement rank with superblock/block indexes:
  - Superblocks (4096 bits): cumulative popcount
  - Blocks (512 bits): relative popcount within superblock
  - O(1) rank queries with small overhead (~6.25% space)
- [ ] Implement select with broadword techniques:
  - Scan blocks to find target rank range
  - Use PDEP/PEXT (BMI2) or broadword magic within word
  - O(1) select for dense bit vectors
- [ ] Add API methods:
  - `rank(idx) -> u64` - count of set bits in `[0, idx)`
  - `select(k) -> Option<usize>` - index of k-th set bit (0-indexed)
  - Lazy index building on first query
- [ ] Optimize for different densities:
  - Dense (>10% set): full indexing
  - Sparse (<1% set): simple scan or alternate representation
- [ ] Benchmarks for rank/select vs. naive scan
- [ ] Tests with property-based verification

### Expected Improvements
- O(1) rank and select vs. O(n) scan
- Essential for coding theory algorithms (syndrome decoding, etc.)

---

## Phase 5: GF(2) Polynomial Arithmetic

**Status**: Planned  
**Target**: Q4 2026

### Goals
Add carry-less multiplication and polynomial operations over GF(2), critical for coding theory applications (BCH, Reed-Solomon, etc.).

### Tasks
- [ ] **Scalar baseline**:
  - Schoolbook carry-less multiplication (XOR instead of addition)
  - O(n²) for n-bit polynomials
- [ ] **CLMUL acceleration (x86-64)**:
  - Use PCLMULQDQ (`_mm_clmulepi64_si128`) for 64-bit × 64-bit
  - Chain operations for larger polynomials
  - Integrate with AVX2/AVX-512 for wide operations (VPCLMULQDQ)
- [ ] **VMULL.P64 on AArch64**:
  - Use crypto extension `vmull_p64` for carry-less multiply
  - Fallback to scalar on older ARMv8 without crypto
- [ ] **Karatsuba multiplication**:
  - O(n^1.585) asymptotic complexity
  - Tune threshold for crossover from schoolbook (~128-256 bits)
- [ ] **Toom-Cook multiplication**:
  - O(n^1.465) for very large polynomials
  - Evaluate vs. Karatsuba for 1K+ bit sizes
- [ ] **Polynomial division and modular reduction**:
  - Long division for GF(2)[x]
  - Barrett reduction for repeated mod operations
- [ ] **GCD and extended Euclidean algorithm**:
  - Polynomial GCD over GF(2)
  - Useful for key generation and algebraic decoding
- [ ] API design:
  - `GF2Poly` type wrapping `BitVec` with polynomial semantics
  - Methods: `mul()`, `div()`, `mod()`, `gcd()`, `eval()`
- [ ] Benchmarks for multiplication at various sizes
- [ ] Property tests: commutativity, distributivity, modular identities

### Expected Improvements
- **CLMUL**: 10-50x speedup over scalar for 64-512 bit polynomials
- **Karatsuba**: Enables efficient operations on 1K+ bit polynomials

---

## Phase 6: Coding Theory Algorithms

**Status**: Research  
**Target**: 2027

### Goals
Build higher-level coding theory primitives on top of GF(2) operations, potentially as a separate crate or module.

### Potential Features
- **Linear codes**:
  - Generator matrix (G) and parity-check matrix (H) representations
  - Encoding: `c = m * G`
  - Syndrome computation: `s = r * H^T`
  - Syndrome decoding for simple codes
- **Cyclic codes**:
  - Polynomial-based encoding/decoding
  - BCH codes with configurable error correction
  - Reed-Solomon over GF(2^m) (requires extension field arithmetic)
- **LDPC codes**:
  - Sparse matrix representation
  - Belief propagation decoding
  - Efficient rank/select for sparse graphs
- **Turbo and polar codes**:
  - Investigate if GF(2) operations provide meaningful acceleration
- **API design considerations**:
  - Should this be a separate `gf2-coding` crate?
  - What abstractions make sense for diverse code families?
  - Performance vs. generality trade-offs

### Open Questions
- Best data structures for sparse vs. dense matrices?
- Integration with existing coding libraries in Rust ecosystem?
- Support for soft-decision decoding (floating-point log-likelihoods)?

---

## Long-Term Vision

**Ultimate Goal**: Provide a comprehensive, high-performance toolkit for GF(2)-based computations in Rust, enabling:
- Research and prototyping of new error-correcting codes
- Production use in storage, networking, and communication systems
- Educational tools for learning coding theory
- Foundation for cryptographic primitives (e.g., GF(2^128) for AES-GCM)

**Guiding Principles**:
- **Safety first**: Minimize `unsafe`, audit rigorously when needed
- **Performance matters**: SIMD where beneficial, but don't sacrifice correctness
- **Test extensively**: Property-based testing, fuzzing, and CI coverage
- **Document well**: Make the library accessible to both experts and learners
- **Composable abstractions**: Enable building blocks for diverse applications

---

## Contributing

Interested in helping with any phase? See areas for contribution:
- **Phase 2-3**: SIMD expertise, benchmarking on diverse hardware
- **Phase 4**: Broadword algorithms, succinct data structures
- **Phase 5**: Algebraic algorithms, polynomial math optimization
- **Phase 6**: Coding theory domain knowledge, algorithm implementation

Refer to the main README for development setup and testing guidelines.

---

*This roadmap is subject to change based on community feedback, performance measurements, and evolving priorities.*
