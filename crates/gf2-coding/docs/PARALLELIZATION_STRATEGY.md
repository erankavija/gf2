# Parallelization Strategy for gf2

**Date**: 2025-11-28  
**Status**: Planning & Sequential Batch Complete  
**Goal**: Achieve 50-100 Mbps throughput with future extensibility to GPU/FPGA

---

## Architecture Philosophy

### Layered Parallelization

```
┌────────────────────────────────────────────────────────────┐
│ Application Layer (gf2-coding)                             │
│ - Block-level parallelism (rayon)                          │
│ - Batch processing APIs                                    │
│ - Thread pools for independent operations                  │
└────────────────────────────────────────────────────────────┘
                            ▼
┌────────────────────────────────────────────────────────────┐
│ Algorithm Layer (gf2-coding)                               │
│ - Data parallelism within algorithms                       │
│ - Vectorized belief propagation (LDPC)                     │
│ - Parallel Gaussian elimination (sparse matrices)          │
└────────────────────────────────────────────────────────────┘
                            ▼
┌────────────────────────────────────────────────────────────┐
│ Primitive Layer (gf2-core)                                 │
│ - SIMD kernels (AVX2/AVX-512/NEON)                        │
│ - Word-level bit operations                                │
│ - Cache-friendly memory layouts                            │
└────────────────────────────────────────────────────────────┘
                            ▼
┌────────────────────────────────────────────────────────────┐
│ Hardware Layer (Future)                                    │
│ - GPU offload (CUDA/OpenCL/Vulkan Compute)               │
│ - FPGA acceleration (Verilog/VHDL generation)             │
│ - Custom ASICs for specific codes                         │
└────────────────────────────────────────────────────────────┘
```

---

## Current Implementation Status

### ✅ Completed (Phase 1)

1. **SIMD Primitives** (gf2-core)
   - AVX2 kernels in `gf2-kernels-simd`
   - Word-level bit operations (64-bit words)
   - BitMatrix operations with SIMD

2. **Sequential Batch Processing** (gf2-coding)
   - `LdpcEncoder::encode_batch()` - sequential iteration
   - `LdpcDecoder::decode_batch()` - sequential iteration
   - Clean API ready for parallelization

### 🔧 In Progress (Phase 2)

3. **Block-Level Parallelism** (Current Task)
   - Use rayon for embarrassingly parallel workloads
   - Independent block encoding/decoding
   - Thread-local decoder pools

---

## Parallelization Levels

### Level 1: Block-Level Parallelism (Embarrassingly Parallel)

**Target**: LDPC encoding/decoding of independent blocks

**Characteristics**:
- No data dependencies between blocks
- Ideal for rayon's `par_iter()`
- Linear speedup up to core count
- Minimal synchronization overhead

**Implementation**:
```rust
impl LdpcEncoder {
    pub fn encode_batch(&self, messages: &[BitVec]) -> Vec<BitVec> {
        use rayon::prelude::*;
        messages.par_iter()
            .map(|msg| self.encode(msg))
            .collect()
    }
}

impl LdpcDecoder {
    pub fn decode_batch(code: &LdpcCode, llr_blocks: &[Vec<Llr>], max_iter: usize) 
        -> Vec<DecoderResult> {
        use rayon::prelude::*;
        llr_blocks.par_iter()
            .map(|llrs| {
                let mut decoder = Self::new(code.clone());
                decoder.decode_iterative(llrs, max_iter)
            })
            .collect()
    }
}
```

**Expected Speedup**: 4-8× on 8-core CPU (accounting for memory bandwidth)

**Applicability**:
- ✅ LDPC encoding/decoding
- ✅ BCH encoding/decoding
- ✅ Viterbi decoding (independent traces)
- ✅ Reed-Solomon (future)
- ✅ Polar codes (future)

---

### Level 2: Data Parallelism Within Algorithms

**Target**: Vectorize hot loops within single block operations

**LDPC Belief Propagation**:
```rust
// Current: Scalar LLR operations
let min_val = inputs.iter().map(|llr| llr.abs()).min();

// Target: SIMD horizontal min across 8 LLRs at once
use std::arch::x86_64::*;
let mins = simd_horizontal_min_f32x8(&inputs);
```

**Sparse Matrix Operations**:
```rust
// Parallel row reduction (Gaussian elimination)
rows.par_chunks_mut(64)
    .for_each(|chunk| {
        // Process 64 rows in parallel
        for row in chunk {
            row.reduce();
        }
    });
```

**Expected Speedup**: 2-4× on top of block-level parallelism

**Challenges**:
- Requires careful synchronization (belief propagation has message dependencies)
- Memory bandwidth becomes bottleneck
- Not all algorithms are data-parallel (e.g., Berlekamp-Massey is sequential)

---

### Level 3: Pipeline Parallelism (Future)

**Target**: DVB-T2 FEC chain (BCH → LDPC → Interleaver → QAM)

**Stages**:
1. BCH outer decoding
2. LDPC inner decoding
3. Bit deinterleaving
4. QAM demapping

**Implementation**: 
- Use crossbeam channels for stage-to-stage communication
- Each stage runs in separate thread pool
- Overlap computation across pipeline stages

**Expected Speedup**: 1.5-2× on top of data parallelism (Amdahl's law applies)

**When to use**: SDR real-time processing (GNU Radio integration)

---

## gf2-core Parallelization Needs

### Current State

**gf2-core is intentionally serial** - it provides low-level primitives that higher layers can parallelize:

1. **BitVec operations**: Word-level, SIMD-optimized, but single-threaded
2. **BitMatrix operations**: Dense matvec uses SIMD, but no multi-threading
3. **Sparse matrices**: CSR/CSC iteration is serial

### Why gf2-core stays serial:

✅ **Clean API boundary**: Parallelism strategy belongs at algorithm level  
✅ **Composability**: Caller decides granularity (block vs. word vs. bit)  
✅ **No overhead**: Zero-cost abstraction for single-threaded use cases  
✅ **Predictable performance**: No hidden thread pools or synchronization

### Future gf2-core Enhancements (Optional):

```rust
// Optional parallel iterators for large operations
impl BitMatrix {
    #[cfg(feature = "parallel")]
    pub fn par_rows(&self) -> impl ParallelIterator<Item = &[u64]> {
        self.words.par_chunks(self.row_words())
    }
}
```

**Decision**: Keep gf2-core serial for now. Parallelism lives in gf2-coding.

---

## Interaction with Other Coding Methods

### BCH Codes

**Encoding**: Embarrassingly parallel (independent polynomial division)
```rust
impl BchEncoder {
    pub fn encode_batch(&self, messages: &[BitVec]) -> Vec<BitVec> {
        messages.par_iter().map(|m| self.encode(m)).collect()
    }
}
```

**Decoding**: Partially parallel
- Syndrome computation: Parallel (independent for each codeword)
- Berlekamp-Massey: **Serial** (dynamic programming, not parallelizable)
- Chien search: Parallel (independent root evaluation)

**Strategy**: Parallelize at block level, accept serial bottleneck in BM algorithm.

---

### Convolutional Codes (Viterbi)

**Decoding**: Highly parallel within each trace
- Butterfly operations: SIMD vectorizable
- Path metrics: Independent updates
- Traceback: **Serial** (requires backtracking through trellis)

**Strategy**: 
- Block-level: Parallel decoding of independent traces
- Within-block: SIMD for butterfly operations
- Traceback: Keep serial (< 5% of total time)

---

### Polar Codes (Future)

**Encoding**: Fast Hadamard transform - naturally parallel
**Decoding**: 
- Successive Cancellation (SC): **Serial** by definition
- SC-List (SCL): Parallel across candidate paths
- Factor graph: Message passing parallelism similar to LDPC

**Strategy**: 
- Prioritize SCL decoder for parallelism
- Block-level parallelism for multiple codewords
- Consider GPU for large list sizes (L > 32)

---

## Preparing for GPU/FPGA Acceleration

### Design Principles for Offload

1. **Data-Oriented Design**
   - Flat memory layouts (no pointer chasing)
   - Structure-of-Arrays (SoA) instead of Array-of-Structures (AoS)
   - Coalesced memory access patterns

2. **Batch-First APIs**
   - Always design for batches (even if batch size = 1)
   - Amortize transfer overhead (PCIe, DDR)
   - Example: `decode_batch(&[1000 blocks])` not `decode_batch(&[1 block])` × 1000

3. **Stateless Kernels**
   - Pure functions: `(input) -> output`
   - No hidden mutable state
   - Easier to compile to GPU/FPGA

4. **Trait Abstraction for Backends**

```rust
pub trait ComputeBackend {
    fn ldpc_decode_batch(
        &self,
        code: &LdpcCode,
        llrs: &[Vec<Llr>],
        max_iter: usize,
    ) -> Vec<DecoderResult>;
}

struct CpuBackend;
struct GpuBackend { device: Device };
struct FpgaBackend { device: FpgaDevice };

impl ComputeBackend for CpuBackend { /* rayon impl */ }
impl ComputeBackend for GpuBackend { /* CUDA/Vulkan impl */ }
impl ComputeBackend for FpgaBackend { /* PCIe transfer + kernel exec */ }
```

**Usage**:
```rust
let backend = if has_gpu() { 
    GpuBackend::new() 
} else { 
    CpuBackend 
};

let results = backend.ldpc_decode_batch(&code, &llrs, 50);
```

---

### GPU Acceleration Strategy

#### When to offload to GPU:

✅ **Large batches** (> 100 blocks): Amortize PCIe transfer  
✅ **Highly parallel** (LDPC BP, polar SCL): 1000s of threads  
✅ **Regular memory access** (dense matrices, structured codes)  
❌ **Small batches** (< 10 blocks): CPU faster (no transfer overhead)  
❌ **Irregular algorithms** (sparse Gaussian elimination): Poor GPU utilization  

#### Technology Choices:

1. **Vulkan Compute** (Recommended)
   - Cross-platform (NVIDIA, AMD, Intel, Apple Silicon)
   - Rust bindings: `vulkano`, `wgpu`
   - Modern API, explicit memory management

2. **CUDA** (NVIDIA-only)
   - Mature, well-documented
   - Best performance on NVIDIA GPUs
   - Rust bindings: `cuda-sys`, `cudarc`

3. **OpenCL** (Legacy)
   - Cross-platform but outdated
   - Use Vulkan instead for new projects

#### Implementation Plan:

**Phase 1**: Profile CPU implementation (Done)  
**Phase 2**: CPU parallelization with rayon (In Progress)  
**Phase 3**: Optimize memory layout for GPU (SoA format)  
**Phase 4**: Vulkan compute shader prototype (LDPC decoder)  
**Phase 5**: Benchmark CPU vs GPU crossover point  
**Phase 6**: Production GPU backend with fallback

**Timeline**: 6-12 months after CPU optimization complete

---

### FPGA Acceleration Strategy

#### When to use FPGA:

✅ **Ultra-low latency** (< 1 μs): Hardware pipelines  
✅ **Custom bit widths**: 5-bit LLRs instead of 32-bit float  
✅ **High throughput** (> 1 Gbps): Dedicated datapaths  
✅ **Power efficiency**: 10-100× better than GPU for specific algorithms  
❌ **Prototyping**: Long dev cycle (weeks to implement vs. hours for CPU)  
❌ **General-purpose**: FPGA excels at specific, fixed algorithms

#### FPGA-Friendly Algorithms:

1. **Convolutional codes (Viterbi)**
   - Deeply pipelined trellis
   - Fixed-point arithmetic
   - Systolic arrays for butterfly operations

2. **LDPC (Min-sum decoder)**
   - Fully unrolled message passing
   - Custom precision (5-6 bit LLRs)
   - Tanner graph mapped to hardware

3. **BCH/Reed-Solomon**
   - Polynomial arithmetic in GF(2^m)
   - Chien search parallelized across roots
   - Syndrome computation pipelined

#### High-Level Synthesis (HLS) Approach:

**Tool**: Xilinx Vitis HLS, Intel HLS Compiler

**Strategy**:
1. Write reference C++ implementation from Rust design
2. Add HLS pragmas for pipelining/unrolling
3. Synthesize to Verilog/VHDL
4. Integrate into FPGA fabric with DMA

**Example** (Viterbi butterfly in HLS):
```cpp
// Generated from gf2-coding Rust implementation
void viterbi_butterfly(
    const llr_t llrs[K],     // 6-bit fixed-point LLRs
    metric_t metrics[N],     // Path metrics
    decision_t decisions[N]  // Traceback decisions
) {
#pragma HLS PIPELINE II=1
#pragma HLS ARRAY_PARTITION variable=metrics complete
    
    // Compute butterfly operations in parallel
    for (int i = 0; i < N/2; i++) {
#pragma HLS UNROLL
        update_metric(llrs, metrics, i);
    }
}
```

**Rust Integration**:
- Use PCIe DMA for host ↔ FPGA data transfer
- Rust driver: `libpci`, `sysfs` access
- Batch transfers to amortize latency

**Timeline**: 12-24 months after CPU/GPU implementations stabilize

---

## Performance Projections

### Current (Sequential Baseline)
- Encoding: 3.85 Mbps (9.87 ms/block)
- Decoding: 1.23 Mbps (30.8 ms/block)

### After Rayon Parallelization ✅ ACHIEVED (Week 1)
- Encoding: 3.85 Mbps (sequential, Sync bounds TODO)
- Decoding: **8.29 Mbps** (batch of 202, **6.7× speedup**, 24-core CPU)
- **Achieved**: Partial Week 1 goal (decoder only)

### After SIMD LLR Ops (Week 2)
- Encoding: 50-100 Mbps (additional 2-4× from vectorization)
- Decoding: 20-100 Mbps (4-8× speedup on BP loop)
- **Achieves**: Real-time DVB-T2 reception on PC

### GPU Offload (Month 3-6)
- Encoding: 200-500 Mbps (10-20× CPU, batch size > 100)
- Decoding: 500-1000 Mbps (10-30× CPU)
- **Achieves**: Professional broadcast equipment performance

### FPGA (Year 1-2)
- Encoding: 1-10 Gbps (full hardware pipeline)
- Decoding: 1-10 Gbps (custom bit widths, unrolled BP)
- Latency: < 10 μs (vs. 1 ms CPU)
- **Achieves**: Real-time 4K/8K video broadcast, satellite links

---

## Recommendations

### Immediate (This Week) ✅ COMPLETE
1. ✅ Add rayon dependency to `gf2-coding`
2. ⚠️ Implement parallel `encode_batch()` - sequential (Sync bounds TODO)
3. ✅ Implement parallel `decode_batch()` with rayon - **6.7× speedup**
4. ✅ Benchmark 1, 10, 50, 100, 202 block batches
5. ✅ Validate throughput: 1.23 Mbps → 8.29 Mbps (batch of 202)

### Short-Term (2-4 Weeks)
6. ⏭ Vectorize LDPC LLR operations (SIMD horizontal min/sum)
7. ⏭ Optimize memory layout for cache efficiency
8. ⏭ Add `par_encode_batch()` for explicit parallel API
9. ⏭ Document thread safety guarantees in public API
10. ⏭ Profile scaling on NUMA systems (multi-socket)

### Medium-Term (2-3 Months)
11. ⏭ Design `ComputeBackend` trait abstraction
12. ⏭ Implement Vulkan compute shader prototype (LDPC decoder only)
13. ⏭ Benchmark CPU vs GPU crossover point
14. ⏭ Add feature flag: `default = ["rayon"], gpu = ["vulkano"]`

### Long-Term (6-12 Months)
15. ⏭ Production GPU backend with fallback
16. ⏭ FPGA feasibility study (Viterbi decoder on Xilinx)
17. ⏭ HLS code generation from Rust traits
18. ⏭ Integration with GNU Radio for real-time SDR

---

## References

- **Profiling Results**: `docs/LDPC_PROFILING_RESULTS.md`
- **Action Plan**: `docs/OPTIMIZATION_ACTION_PLAN.md`
- **SDR Integration**: `docs/SDR_INTEGRATION.md`
- **DVB-T2 Design**: `docs/DVB_T2_DESIGN.md`

**Last Updated**: 2025-11-28
