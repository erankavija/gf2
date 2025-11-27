# LDPC Throughput Performance Optimization Plan

## Current Status (2025-11-27)

### Baseline Performance (Release Build)
- **LDPC Encoding**: 0.63 Mbps (12.4s for 202 blocks × 38,880 bits)
- **LDPC Decoding**: 1.35 Mbps (5.8s for 202 blocks × 38,880 bits)
- **BCH Encoding**: >100 Mbps (reference: fast)

### Target Performance
- **Primary Goal**: 10-50 Mbps for real-time DVB-T2 processing
- **Stretch Goal**: 100+ Mbps for high-throughput applications
- **Required Speedup**: 15-80× improvement

### Performance Gap Analysis
Current performance is **2-3 orders of magnitude slower** than target. This suggests:
1. Algorithmic inefficiencies (not just micro-optimizations needed)
2. Memory access patterns (cache misses)
3. Lack of SIMD/parallelization
4. Unnecessary allocations/copies

---

## Phase 1: Profiling & Hotspot Identification (1 day)

### 1.1: CPU Profiling Setup
**Goal**: Identify where time is spent

**Tools**:
- `cargo flamegraph` - Visual call stack analysis
- `perf` - Linux performance counters
- `cargo bench` with criterion - Baseline measurements

**Actions**:
```bash
# Install profiling tools
cargo install flamegraph
sudo apt install linux-perf  # if needed

# Profile LDPC encoding
cargo flamegraph --release --test dvb_t2_ldpc_verification_suite \
  -- test_ldpc_encoding_tp05_to_tp06 --ignored --nocapture

# Profile LDPC decoding
cargo flamegraph --release --test dvb_t2_ldpc_verification_suite \
  -- test_ldpc_decoding_tp06_to_tp05_error_free --ignored --nocapture
```

**Expected Hotspots**:
- LDPC Encoding:
  - Cache loading (should be fast, 16ms)
  - Matrix-vector multiply (P^T × message_part)
  - BitVec operations
  - Memory allocations
  
- LDPC Decoding:
  - Check node update (min-sum approximation)
  - Variable node update (sum of LLRs)
  - Sparse matrix traversal (row/col iterators)
  - Syndrome computation

### 1.2: Memory Profiling
**Goal**: Identify allocation hotspots and cache misses

**Tools**:
- `cargo bench --bench ldpc_throughput` (to be created)
- `heaptrack` - Memory allocation profiling
- `perf stat -e cache-misses` - Cache miss analysis

**Metrics to Collect**:
- Allocations per encode/decode
- Peak memory usage
- Cache miss rate
- Branch mispredictions

### 1.3: Benchmark Suite Creation
**Goal**: Reproducible performance measurements

**Create**: `benches/ldpc_throughput.rs`

**Benchmarks**:
1. LDPC encoding (single block, batched)
2. LDPC decoding (single block, batched)
3. Cache loading time
4. Matrix-vector multiply (isolated)
5. Check node update (isolated)
6. Variable node update (isolated)
7. Syndrome computation

---

## Phase 2: Quick Wins (1-2 days)

### 2.1: Algorithmic Improvements

#### 2.1.1: Batch Processing
**Current**: Process blocks one at a time
**Improvement**: Batch multiple blocks for better cache utilization

```rust
// Before: 202 sequential encode operations
for block in blocks {
    encoder.encode(block);
}

// After: Batch encode with shared cache
encoder.encode_batch(&blocks);
```

**Expected Speedup**: 2-5× (better cache reuse, reduced setup overhead)

#### 2.1.2: Pre-allocate Decoder State
**Current**: Decoder may allocate on every decode
**Improvement**: Reuse decoder state across blocks

```rust
// Before: Decoder may reallocate internally
for block in blocks {
    decoder.decode_iterative(block, 50);
}

// After: Reset and reuse
for block in blocks {
    decoder.reset();  // Already exists!
    decoder.decode_iterative(block, 50);
}
```

**Expected Speedup**: 1.5-2× (eliminate allocations)

#### 2.1.3: Early Termination in Decoding
**Current**: Always iterate up to max_iterations
**Improvement**: Already implemented! (converges in 1 iteration for error-free)

**Status**: ✅ Already optimized

### 2.2: Memory Layout Optimizations

#### 2.2.1: Cache Line Alignment
**Improvement**: Align frequently accessed data structures to cache lines

```rust
#[repr(align(64))]
struct AlignedMessages {
    check_to_var: Vec<Vec<Llr>>,
    var_to_check: Vec<Vec<Llr>>,
}
```

**Expected Speedup**: 1.2-1.5× (reduce false sharing, better cache utilization)

#### 2.2.2: Structure of Arrays (SoA) Layout
**Current**: Array of structs (Vec<Vec<Llr>>)
**Improvement**: Flat arrays for better SIMD potential

```rust
// Before: Vec<Vec<Llr>> - pointer chasing
// After: Flat Vec<Llr> with index mapping
```

**Expected Speedup**: 1.5-2× (better cache locality, SIMD-friendly)

### 2.3: Sparse Matrix Optimizations

#### 2.3.1: CSR vs Dual Format Evaluation
**Current**: `SpBitMatrixDual` (both CSR and CSC)
**Question**: Is dual format optimal for LDPC operations?

**Profile**: Check if both formats are actually used efficiently

**Potential**: Switch to single format if one dominates

---

## Phase 3: SIMD Vectorization (2-3 days)

### 3.1: LLR Operations Vectorization
**Target**: Check node update (min-sum)

**Current**: Scalar operations on LLR values
**Improvement**: SIMD min/max operations

```rust
// Use std::simd or explicit SIMD intrinsics
use std::simd::{f32x8, SimdFloat};

fn check_node_update_simd(llrs: &[Llr]) -> Llr {
    // Process 8 LLRs at a time with SIMD
    // min-sum: min(|llr|) * product(sign(llr))
}
```

**Expected Speedup**: 4-8× for check node updates

### 3.2: Sparse Matrix-Vector Multiply Vectorization
**Target**: Encoding (P^T × message)

**Current**: Scalar bit operations
**Improvement**: Already using gf2-core's `matvec_transpose` with word-level operations

**Status**: Check if gf2-core's dense matvec uses SIMD
**Action**: Add SIMD to gf2-core if not present

### 3.3: Syndrome Computation Vectorization
**Target**: Fast syndrome check (H × c)

**Current**: Sparse matrix-vector multiply
**Improvement**: Vectorize XOR operations

**Expected Speedup**: 2-4× for syndrome computation

---

## Phase 4: Parallelization (2-3 days)

### 4.1: Block-Level Parallelism
**Target**: Process multiple blocks in parallel

```rust
use rayon::prelude::*;

// Encode 202 blocks in parallel
let codewords: Vec<_> = blocks.par_iter()
    .map(|block| encoder.encode(block))
    .collect();
```

**Expected Speedup**: 4-8× on 8-core CPU (limited by cache contention)

**Consideration**: Each thread needs its own encoder/decoder state

### 4.2: Intra-Block Parallelism (Advanced)
**Target**: Parallelize within a single block

**LDPC Decoding**: Variable node updates are independent
```rust
// Update all variable nodes in parallel
beliefs.par_iter_mut()
    .zip(channel_llrs.par_iter())
    .for_each(|(belief, &llr)| {
        *belief = llr + sum_of_check_messages;
    });
```

**Expected Speedup**: 2-4× on top of block-level parallelism

**Complexity**: High - requires careful message passing synchronization

---

## Phase 5: Algorithmic Alternatives (1-2 weeks, if needed)

### 5.1: Layered Decoding
**Current**: Flooding schedule (update all check nodes, then all variable nodes)
**Alternative**: Layered schedule (update by layers, better convergence)

**Benefits**:
- Faster convergence (fewer iterations)
- Better cache locality (process subset at a time)

**Expected Speedup**: 2-3× (fewer iterations needed)

### 5.2: Quantized LLRs
**Current**: f32 LLRs (4 bytes each)
**Alternative**: Fixed-point LLRs (1-2 bytes each)

**Benefits**:
- 2-4× less memory bandwidth
- Better cache utilization
- SIMD processes more values at once

**Trade-off**: Slight accuracy loss (acceptable for DVB-T2)

**Expected Speedup**: 2-3× (memory bandwidth bound)

### 5.3: GPU Offload (Optional)
**Target**: Massively parallel LDPC decoding

**Considerations**:
- Transfer overhead (PCIe bandwidth)
- Only worth it for batch processing
- High implementation complexity

**Expected Speedup**: 10-100× for large batches (offset by transfer overhead)

---

## Phase 6: Encoding-Specific Optimizations (1 day)

### 6.1: Generator Matrix Format
**Current**: Dense `BitMatrix` (40-50% density for DVB-T2)
**Evaluation**: Is dense optimal vs. sparse?

**Profile**: Time spent in matrix-vector multiply

**Alternatives**:
- Hybrid format (dense for high-density regions, sparse for others)
- Specialized DVB-T2 format (exploit structure)

### 6.2: Message Extraction Optimization
**Current**: Loop over k bits
**Improvement**: Bulk copy using BitVec slice operations

```rust
// Before: Bit-by-bit copy
for i in 0..k {
    message.push_bit(codeword.get(i));
}

// After: Bulk slice operation (if BitVec supports it)
let message = codeword.slice(0, k);
```

**Expected Speedup**: 1.5-2× for message extraction

---

## Implementation Priority

### Priority 1: Must Do (Target: 10 Mbps)
1. ✅ Profiling & benchmarking (Phase 1)
2. 🔧 Batch processing (Phase 2.1.1)
3. 🔧 Pre-allocate decoder state (Phase 2.1.2)
4. 🔧 Block-level parallelism (Phase 4.1)

**Expected Combined**: 10-20× speedup → **6-12 Mbps**

### Priority 2: Should Do (Target: 50 Mbps)
5. 🔧 LLR operations SIMD (Phase 3.1)
6. 🔧 Memory layout optimization (Phase 2.2)
7. 🔧 Quantized LLRs (Phase 5.2)

**Expected Combined**: 40-60× speedup → **25-38 Mbps**

### Priority 3: Nice to Have (Target: 100+ Mbps)
8. 🔧 Layered decoding (Phase 5.1)
9. 🔧 Intra-block parallelism (Phase 4.2)
10. 🔧 GPU offload (Phase 5.3, optional)

**Expected Combined**: 80-160× speedup → **50-100 Mbps**

---

## Success Metrics

### Tier 1: Functional (Current)
- ✅ Correctness: 100% match with test vectors
- ✅ Convergence: Proper belief propagation

### Tier 2: Acceptable Performance (Target: Week 1)
- 🎯 Encoding: 10+ Mbps
- 🎯 Decoding: 5+ Mbps
- 🎯 Acceptable for offline processing

### Tier 3: Real-Time Performance (Target: Week 2)
- 🎯 Encoding: 50+ Mbps
- 🎯 Decoding: 25+ Mbps
- 🎯 Suitable for software-defined radio (SDR)

### Tier 4: High-Throughput (Stretch Goal)
- 🎯 Encoding: 100+ Mbps
- 🎯 Decoding: 50+ Mbps
- 🎯 Competitive with hardware implementations

---

## Profiling Checklist

Before starting optimizations, collect baseline data:

- [ ] CPU flamegraph (encoding)
- [ ] CPU flamegraph (decoding)
- [ ] Memory allocation profile
- [ ] Cache miss rate
- [ ] Branch misprediction rate
- [ ] Criterion benchmarks (baseline)
- [ ] Throughput per block size
- [ ] Scalability with batch size

---

## Risk Assessment

### Low Risk (High Confidence)
- Batch processing: Standard optimization, proven effective
- Pre-allocation: Simple, no algorithmic changes
- Block parallelism: Embarrassingly parallel workload

### Medium Risk (Moderate Confidence)
- SIMD vectorization: Requires careful implementation, testing
- Memory layout changes: May interact poorly with cache
- Quantized LLRs: Accuracy trade-off needs validation

### High Risk (Experimental)
- Layered decoding: Significant algorithm change
- Intra-block parallelism: Synchronization complexity
- GPU offload: Transfer overhead may negate gains

---

## Tools & Infrastructure

### Required Tools
- `cargo flamegraph` - CPU profiling
- `criterion` - Benchmarking (already available)
- `perf` - Performance counters
- `heaptrack` - Memory profiling

### Optional Tools
- `cargo-asm` - Inspect generated assembly
- `cargo-llvm-lines` - Identify code bloat
- `valgrind --tool=cachegrind` - Cache simulation

### Benchmark Infrastructure
Create `benches/ldpc_throughput.rs` with:
- Single block encode/decode
- Batch encode/decode (10, 50, 100, 200 blocks)
- Isolated operation benchmarks
- Memory bandwidth tests

---

## Next Steps

1. **Immediate**: Run Phase 1 profiling to identify hotspots
2. **Day 1**: Implement batch processing (Phase 2.1.1)
3. **Day 2**: Add block parallelism (Phase 4.1)
4. **Day 3-4**: SIMD vectorization for hot paths (Phase 3)
5. **Day 5**: Re-benchmark and assess progress
6. **Week 2**: Advanced optimizations if needed

**Goal**: Achieve 10+ Mbps encoding / 5+ Mbps decoding within 1 week.
