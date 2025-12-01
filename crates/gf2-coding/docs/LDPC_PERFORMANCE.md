# LDPC Performance Optimization

**Status**: Week 1 Complete (parallel decoding), Week 2 in progress

---

## Current Performance

### Baseline (Release Build)

| Operation | Throughput | Time per Block | Notes |
|-----------|------------|----------------|-------|
| **LDPC Encoding** | 3.85 Mbps | 9.87 ms | Sequential only |
| **LDPC Decoding** | 1.35 Mbps | - | Sequential baseline |
| **LDPC Decoding (Parallel)** | 8.29 Mbps | 4.57 ms | 202 blocks, 6.7× speedup |
| **BCH Encoding** | >100 Mbps | <0.1 ms | Already fast |

**Configuration**: DVB-T2 Normal Rate 3/5 (n=64,800, k=38,880)

### Performance Targets

| Tier | Encoding | Decoding | Use Case | Status |
|------|----------|----------|----------|--------|
| **Tier 1: Functional** | 3.85 Mbps | 1.35 Mbps | Offline processing | ✅ Current |
| **Tier 2: Near Real-Time** | 10-20 Mbps | 5-10 Mbps | Software recording | ✅ Achieved (decoder) |
| **Tier 3: Real-Time SDR** | 50-100 Mbps | 25-50 Mbps | Live DVB-T2 reception | 🎯 Target |
| **Tier 4: Professional** | 200+ Mbps | 100+ Mbps | Multi-channel, 4K UHDTV | 🔬 Research |

---

## Optimization Progress

### ✅ Week 1: Parallel Decoding (Complete)

**Achievement**: 6.7× speedup with rayon parallelism (1.23 Mbps → 8.29 Mbps)

**Implementation**:
- Batch API: `decode_batch(&[Vec<Llr>])` processes multiple blocks in parallel
- Thread-local decoders: Each rayon thread creates its own `LdpcDecoder` instance
- Arc-based code sharing: Cheap cloning of parity-check matrices
- State management: Fixed message reset bug during TDD implementation

**Key Learning**: TDD approach caught state leakage bug (check_to_var messages not reset between calls) that would have caused subtle production errors.

### ✅ Phase: LLR f64 → f32 Migration (Complete)

**Achievement**: 5% baseline performance improvement + SIMD readiness

**Changes**:
- Switched `Llr` from f64 to f32 precision
- Eliminated f64↔f32 conversion overhead in SIMD operations
- f32 enables 2× wider SIMD vectors (8 lanes AVX2 vs 4 lanes)
- Reduces memory bandwidth by 50%

**Impact**:
- Encoding: 28.5 ms per block (5% faster)
- No FER degradation (f32 has 24-bit mantissa, far exceeds LDPC requirements)
- Test tolerance adjustments: Relaxed from 1e-8 to 1e-6 for f32 precision

### ✅ Phase: Decoder Allocation Optimization (Complete)

**Achievement**: 9.0% performance improvement via allocation elimination

**Changes**:
- Pre-cached check node neighbors in decoder struct
- Pre-allocated temp buffers for check node updates
- Eliminated repeated `row_iter().collect()` calls

**Results**:
- Decoding: 31.1 ms → 28.3 ms per block (9% faster)
- Throughput: 152 KiB/s → 167 KiB/s
- Memory cost: +2.3 MB per decoder (negligible)

**Key Learning**: Evidence-driven optimization (profiling showed 23.1% time in allocations) beats speculation.

---

## Profiling Results

### Encoding Hotspot (perf analysis)

| Function | % Time | Component | Priority |
|----------|--------|-----------|----------|
| `BitMatrix::matvec_transpose` | **97.5%** | gf2-core | CRITICAL |

**Analysis**: Single dominant bottleneck in dense matrix-vector multiplication. Optimization must target this function in gf2-core.

### Decoding Hotspots (perf analysis)

| Function | % Time | Component | Description |
|----------|--------|-----------|-------------|
| `LdpcDecoder::decode_iterative` | **69.8%** | gf2-coding | Main BP loop |
| `SpBitMatrix::row_iter` | **17.7%** | gf2-core | Sparse iteration |
| `malloc` / `free` | **4.9%** | libc | ✅ Fixed via pre-allocation |
| `SpBitMatrix::matvec` | **2.0%** | gf2-core | Syndrome check |

---

## Optimization Strategy

### Priority 1: SIMD Operations (Week 2)

**Target**: 2-4× speedup from vectorized LLR operations

**Approach**:
1. SIMD min-sum for check node updates (horizontal min + sign)
2. SIMD max-abs for variable node updates
3. Stack allocation for small slices (<16 elements)
4. Batch SIMD operations where possible

**Expected**: 20-30 Mbps decoding throughput

### Priority 2: Sparse Matrix Optimization

**Target**: 1.5-2× speedup from improved memory access

**Approach**:
1. Cache-friendly iteration patterns
2. Prefetch check node neighbors
3. Consider CSR format alternatives
4. Batch message passing operations

**Expected**: 30-50 Mbps decoding throughput

### Priority 3: Encoder Optimization

**Target**: 5-10× speedup with batch processing + parallelism

**Approach**:
1. Batch processing API (shared generator matrix)
2. Block-level parallelism with rayon (4-8× on multi-core)
3. Verify gf2-core SIMD usage in matvec_transpose

**Expected**: 20-40 Mbps encoding throughput

### Advanced: Algorithmic Improvements

**Layered Decoding**:
- Faster convergence (fewer iterations needed)
- Better cache locality (process subset at a time)
- Expected: 2-3× speedup

**Quantized LLRs**:
- Fixed-point LLRs (1-2 bytes vs 4 bytes for f32)
- 2-4× less memory bandwidth
- Better cache utilization
- Trade-off: Slight accuracy loss (acceptable for DVB-T2)
- Expected: 2-3× speedup

---

## Benchmarking

### Quick Benchmark

```bash
# Test with different thread counts
RAYON_NUM_THREADS=1 cargo bench --bench quick_parallel --features parallel
RAYON_NUM_THREADS=8 cargo bench --bench quick_parallel --features parallel

# Automated scaling test
./benchmark_quick.sh
```

### Full Throughput Measurement

```bash
# Comprehensive benchmarks
cargo bench --bench ldpc_throughput --features parallel

# View HTML reports
xdg-open target/criterion/report/index.html
```

---

## Known Issues

### LDPC Parity Check Bug (Historical - RESOLVED)

**Issue**: Initial implementation showed 80% encoding accuracy with parity check failures

**Root Cause**: RREF right-pivoting bug in gf2-core (incorrect pivot column identification)

**Resolution**: 
- Fixed row reordering in RREF algorithm (gf2-core commit 7963634)
- Added 22 property tests validating H × G^T = 0
- Result: All 446 tests passing, 100% encoding accuracy achieved

---

## References

- [DVB_T2.md](DVB_T2.md) - DVB-T2 implementation and verification
- [PARALLELIZATION.md](PARALLELIZATION.md) - Overall parallelization strategy
- `benches/` - Criterion benchmarks for performance tracking
