# Parallelization Strategy for gf2-coding

**Date**: 2025-11-30  
**Status**: Week 2 Complete (Feature Flag & Benchmarking), Phase 1 In Progress  
**Goal**: Unified parallelization framework across all coding methods targeting 10 Mbps (SDR) to 1 Gbps (broadcast)

---

## Executive Summary

This document establishes the parallelization strategy for `gf2-coding`, covering:

1. **CPU Parallelism** (✅ Implemented): Block-level parallelism with rayon (6.7× speedup)
2. **Unified Backend Abstraction** (⏭ Planned): `ComputeBackend` trait for CPU, GPU, and FPGA
3. **Algorithm Coverage**: LDPC (priority), BCH, Viterbi, Polar codes (future)
4. **Performance Tiers**: 10 Mbps (software recording) → 50-100 Mbps (CPU optimized) → 1 Gbps (GPU/FPGA)

---

## Architecture Philosophy

### Layered Parallelization Model

```
┌─────────────────────────────────────────────────────────────────┐
│ Application Layer (User Code)                                   │
│ - Simple API: code.encode_batch(messages)                       │
│ - Backend selection: CpuBackend, GpuBackend, FpgaBackend        │
│ - Fallback chain: Try GPU → fallback to CPU if unavailable     │
└─────────────────────────────────────────────────────────────────┘
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│ Backend Abstraction (gf2-core/compute)                          │
│ - Trait: ComputeBackend                                         │
│ - Implementations: CpuBackend, VulkanBackend, FpgaBackend       │
│ - Batch processing: Always operate on batches (size ≥ 1)       │
│ - Memory management: Pinned buffers, zero-copy where possible   │
└─────────────────────────────────────────────────────────────────┘
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│ Algorithm Layer (gf2-coding)                                    │
│ - LDPC: Belief propagation (data-parallel, GPU-friendly)       │
│ - BCH: Syndrome + BM (mixed: parallel syndrome, serial BM)     │
│ - Viterbi: Trellis operations (SIMD butterflies, serial TB)    │
│ - Polar: SCL decoder (parallel path evaluation)                │
└─────────────────────────────────────────────────────────────────┘
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│ Primitive Layer (gf2-core + gf2-kernels-simd)                  │
│ - gf2-core: Safe abstractions, runtime dispatch                │
│ - gf2-kernels-simd: Unsafe SIMD implementations (isolated)     │
│   * Bit ops: XOR, AND, popcount (AVX2/AVX-512/NEON)            │
│   * LLR ops: horizontal min-sum, max-abs for LDPC              │
│   * GF(2^m) ops: field arithmetic with SIMD (future)           │
│ - Memory layout: Structure-of-Arrays for GPU coalescing        │
└─────────────────────────────────────────────────────────────────┘
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│ Hardware Layer                                                  │
│ - CPU: Rayon thread pool, NUMA-aware scheduling                │
│ - GPU: Vulkan compute shaders, CUDA kernels                    │
│ - FPGA: PCIe DMA, hardware pipelines, custom bit widths        │
└─────────────────────────────────────────────────────────────────┘
```

---

## Current Implementation Status

### ✅ Phase 1: CPU Parallelism (Complete - 2025-11-28)

**LDPC Codes** (✅ Implemented):
- **Rayon parallelism**: 6.7× speedup on 24-core CPU
- **Throughput**: 1.23 Mbps (serial) → 8.29 Mbps (parallel, 202 blocks)
- **Batch API**: `decode_batch(&[Vec<Llr>])` for embarrassingly parallel workloads
- **Thread-local state**: Each thread creates own `LdpcDecoder` instance (avoids Sync complexity)
- **Result**: Achieved immediate performance goal for software recording (~10 Mbps)

**Implementation Details**:
- Each rayon thread creates its own `LdpcDecoder` instance via `into_par_iter()`
- Arc-based code sharing enables cheap cloning of parity matrices
- Scales well up to ~200 blocks (6.7× on 24 cores, likely memory bandwidth bound)
- State leakage bug fixed during TDD implementation (message reset on decode start)

**SIMD Primitives**:
- **gf2-kernels-simd**: Separate crate for unsafe, architecture-specific SIMD code
  - Bit operations: AVX2 kernels for XOR, AND, popcount (178 SIMD instructions active)
  - LLR operations: AVX2 horizontal min-sum, max-abs for belief propagation
  - Word-level operations: 64-bit words for cache efficiency
- **gf2-core integration**: Runtime CPU detection, safe wrappers
- **Performance**: BitMatrix RREF operations with SIMD (256-512× speedup vs bit-level)

**Other Codes** (❌ Serial - planned for Phase 2):
- BCH: Sequential encoding/decoding (no batch API yet)
- Viterbi: Single-threaded trellis decoding
- Block codes: No parallel primitives

### ⏭ Phase 2: Backend Abstraction (Planned)

**Goal**: Unified `ComputeBackend` trait for CPU, GPU, and FPGA

**Design Principles**:
1. **Batch-first**: All operations on batches, even if batch size = 1
2. **Stateless**: Pure functions `(input) -> output`, no hidden mutable state
3. **Zero-copy**: Minimize data marshaling between Rust and backends
4. **Fallback**: Automatic CPU fallback if GPU/FPGA unavailable
5. **Type-safe**: Leverage Rust's type system to prevent errors

### 🔬 Phase 3: GPU/FPGA (Research)

**Research Questions**:
- Is LDPC belief propagation memory-bound or compute-bound on GPU?
- What batch size amortizes PCIe transfer overhead? (Hypothesis: >100 blocks)
- GPU crossover point: When does GPU outperform 24-core CPU?
- FPGA feasibility: Power/area trade-offs for specific codes

**Technology Choices**:
- **Vulkan Compute** (recommended): Cross-platform, modern API
- **CUDA**: NVIDIA-only, mature ecosystem
- **FPGA**: Xilinx HLS for rapid prototyping

---

## SIMD Kernel Implementation Strategy

### Architecture: gf2-kernels-simd (Separate Crate)

**Why a separate crate?**
- Isolates **unsafe code** from safe abstractions
- Enables **optional dependency** (feature-gated in gf2-core/gf2-coding)
- Allows **architecture-specific compilation** without polluting main crates
- Provides **clean separation** of concerns: safe high-level API vs unsafe low-level kernels

### Module Organization

```
gf2-kernels-simd/
├── src/
│   ├── lib.rs              # Detection, safe wrappers
│   ├── llr.rs              # LLR operations (min-sum, max-abs, saturate)
│   ├── gf2m.rs             # GF(2^m) field arithmetic (future)
│   └── x86/
│       ├── mod.rs          # x86-64 detection
│       ├── avx2.rs         # AVX2 implementations
│       └── avx512.rs       # AVX-512 implementations (future)
```

### LLR Operations in gf2-kernels-simd/src/llr.rs

**Current Implementation** (✅ Exists):
- `minsum_avx2(inputs: &[f32]) -> f32`: Sign-preserving horizontal min
- `maxabs_avx2(inputs: &[f32]) -> f32`: Maximum absolute value

**Needed Extensions** (⏭ Next):
1. **f64 support**: Current uses f32, but gf2-coding::Llr uses f64
   - Options: Convert f64↔f32 at boundary, or implement f64 SIMD variants
   - Trade-off: f32 = 2× SIMD width (8 lanes AVX2), f64 = higher precision (4 lanes)
2. **saturate_batch()**: Clip LLRs to [-max, +max] range
3. **hard_decision_batch()**: Vec<Llr> → BitVec conversion

### Integration Pattern

**Step 1: Detection in gf2-kernels-simd**
```rust
// gf2-kernels-simd/src/llr.rs
pub struct LlrFns {
    pub minsum_fn: fn(&[f32]) -> f32,
    pub saturate_fn: fn(&mut [f32], f32),
    pub hard_decision_fn: fn(&[f32]) -> Vec<bool>,
}

pub fn detect() -> Option<LlrFns> {
    #[cfg(target_arch = "x86_64")]
    if is_x86_feature_detected!("avx2") {
        return Some(LlrFns {
            minsum_fn: minsum_avx2_safe,
            saturate_fn: saturate_avx2_safe,
            hard_decision_fn: hard_decision_avx2_safe,
        });
    }
    None  // No SIMD available, fallback to scalar
}
```

**Step 2: Runtime Selection in gf2-coding**
```rust
// gf2-coding/src/llr.rs
impl Llr {
    pub fn boxplus_minsum_n(llrs: &[Llr]) -> Llr {
        #[cfg(feature = "simd")]
        {
            // Lazy static: detect once at startup
            static LLR_SIMD: once_cell::sync::Lazy<Option<gf2_kernels_simd::llr::LlrFns>> =
                once_cell::sync::Lazy::new(gf2_kernels_simd::llr::detect);
            
            if let Some(ref fns) = *LLR_SIMD {
                let values: Vec<f32> = llrs.iter().map(|l| l.value() as f32).collect();
                return Llr::new((fns.minsum_fn)(&values) as f64);
            }
        }
        
        // Fallback: scalar implementation (always available)
        Self::boxplus_minsum_n_scalar(llrs)
    }
}
```

### Performance Expectations

| Operation | Scalar | AVX2 (f32) | AVX2 (f64) | Speedup |
|-----------|--------|------------|------------|---------|
| `boxplus_minsum_n` (8 LLRs) | 8 ops | 1 vector op | 2 vector ops | 4-8× |
| `saturate` (1024 LLRs) | 1024 clamps | 128 vector clamps | 256 vector clamps | 4-8× |
| `hard_decision` (1024 LLRs) | 1024 compares | 128 vector compares + pack | 256 vector compares + pack | 4-8× |

**Impact on LDPC Decoding**:
- Belief propagation spends **69.8%** of time in LLR operations
- Target speedup: **4-8×** on vectorized operations
- Overall LDPC decoding improvement: **2-4×** (Amdahl's law: 0.698 × 4 + 0.302 × 1 ≈ 3.1×)

### Design Principles

1. **Safety First**: All public APIs in gf2-kernels-simd are safe wrappers
2. **Runtime Detection**: Never assume CPU features, always check at runtime
3. **Scalar Fallback**: Every SIMD operation must have scalar equivalent
4. **Type Conversion**: Handle f32 ↔ f64 conversion at boundary, not in hot loop
5. **Testing**: Property-based tests verify SIMD matches scalar exactly

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

**LDPC Belief Propagation** (⏭ Next Priority):
```rust
// Current: Scalar LLR operations in gf2-coding/src/llr.rs
let min_val = inputs.iter().map(|llr| llr.abs()).min();

// Target: SIMD horizontal min in gf2-kernels-simd/src/llr.rs
// Implementation location: gf2-kernels-simd/src/llr.rs (extend existing module)
use gf2_kernels_simd::llr;
let llr_fns = llr::detect().unwrap_or_else(|| fallback_to_scalar());
let result = llr_fns.minsum_fn(&inputs);  // AVX2: 8 floats at once

// Integration in gf2-coding/src/llr.rs
impl Llr {
    pub fn boxplus_minsum_n(llrs: &[Llr]) -> Llr {
        #[cfg(feature = "simd")]
        if let Some(simd_fns) = gf2_kernels_simd::llr::detect() {
            return Llr::new((simd_fns.minsum_fn)(llrs_as_f32_slice(llrs)));
        }
        // Fallback to scalar
        scalar_boxplus_minsum_n(llrs)
    }
}
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

## Unified Backend Architecture

### gf2-core: Dual-Layer Backend Design ✅ IMPLEMENTED

**gf2-core now has TWO complementary backend abstractions:**

```
┌────────────────────────────────────────────────────────────┐
│ compute::ComputeBackend (Algorithm Operations)             │
│ - matmul(), rref(), encode_batch(), decode_batch()         │
│ - Implementations: CpuBackend, GpuBackend (future)         │
│ - Feature: parallel (opt-in)                               │
└────────────────────────────────────────────────────────────┘
                            ▼ uses
┌────────────────────────────────────────────────────────────┐
│ kernels::Backend (Primitive Operations)                    │
│ - xor(), and(), popcount(), parity()                       │
│ - Implementations: ScalarBackend, SimdBackend              │
│ - Feature: simd (opt-in)                                   │
└────────────────────────────────────────────────────────────┘
```

**Key Design Principle**: 
- **kernels::Backend**: Building blocks (XOR, AND, popcount)
- **compute::ComputeBackend**: Algorithms (matmul, RREF, batch ops)
- **Composable**: ComputeBackend uses kernels::Backend internally

### Implementation Status

**Phase 1: ComputeBackend in gf2-core** ✅ COMPLETE
- ✅ `compute::ComputeBackend` trait (17 tests, all passing)
- ✅ `compute::CpuBackend` with auto-selection (Scalar or SIMD kernels)
- ✅ Feature flags: `parallel` (rayon, opt-in)
- ✅ Backward compatible: All 441 gf2-core tests pass

**Usage in gf2-coding**:
```rust
use gf2_core::compute::{ComputeBackend, CpuBackend};

let backend = CpuBackend::new();  // Auto-selects best kernel backend
let result = backend.matmul(&a, &b);  // Uses SIMD if available
```

### Why This Architecture Prevents Divergence

✅ **Single pattern**: All performance code follows the same backend trait model
✅ **Composable**: Algorithm backends build on primitive backends  
✅ **Reusable**: gf2-coding, gf2-dsp, gf2-crypto all use same abstractions  
✅ **Pay-as-you-go**: Features are optional (no forced dependencies)  
✅ **Type-safe**: Static dispatch, zero runtime overhead

### Feature Flag Strategy

```toml
# gf2-core/Cargo.toml
[features]
default = ["rand", "io"]
simd = ["gf2-kernels-simd"]       # Kernel-level SIMD
parallel = ["rayon"]              # Algorithm-level parallelism (NEW)
gpu = ["vulkano"]                 # GPU backend (future)

# gf2-coding/Cargo.toml
[features]
default = ["rayon-backend"]
rayon-backend = ["gf2-core/parallel"]  # Enable rayon in gf2-core
gpu-backend = ["gf2-core/gpu"]         # Enable GPU in gf2-core (future)
```

---

## Core Abstraction: ComputeBackend Trait ✅ IMPLEMENTED

### Unified Backend Interface (Implemented in gf2-core)

**Location**: `gf2-core/src/compute/backend.rs`

```rust
/// Compute backend for algorithm-level operations.
///
/// This trait abstracts execution strategies for computationally intensive
/// operations. Implementations may use different hardware (CPU, GPU, FPGA)
/// or parallelization strategies (rayon, SIMD).
pub trait ComputeBackend: Send + Sync {
    /// Returns a human-readable name for this backend.
    fn name(&self) -> &str;

    /// Returns the underlying kernel backend for primitive operations.
    fn kernel_backend(&self) -> &dyn crate::kernels::Backend;

    /// Matrix multiplication over GF(2): C = A × B.
    fn matmul(&self, a: &BitMatrix, b: &BitMatrix) -> BitMatrix;

    /// Reduced Row Echelon Form (RREF) with configurable pivoting.
    fn rref(&self, matrix: &BitMatrix, pivot_from_right: bool) -> RrefResult;
    
    // Future Phase 2: Batch operations for coding
    // fn encode_batch(&self, encoder: &dyn Encodable, msgs: &[BitVec]) -> Vec<BitVec>;
    // fn decode_batch(&self, decoder: &dyn Decodable, llrs: &[Vec<Llr>]) -> Vec<Result>;
}
```

**Design**: Phase 1 focuses on core matrix operations. Batch encoding/decoding will be added in Phase 2 when needed by gf2-coding.

### Backend Implementations

#### 1. CpuBackend - ✅ IMPLEMENTED (gf2-core v0.1.0)
**Location**: `gf2-core/src/compute/cpu.rs`

```rust
pub struct CpuBackend {
    kernel: Box<dyn kernels::Backend>,  // Auto-selected (Scalar or SIMD)
    #[cfg(feature = "parallel")]
    thread_pool: rayon::ThreadPool,
}

impl CpuBackend {
    pub fn new() -> Self {
        // Auto-selects best kernel backend (SIMD if available)
        // Creates rayon thread pool if `parallel` feature enabled
    }
}

impl ComputeBackend for CpuBackend {
    fn name(&self) -> &str {
        // Returns: "CPU (rayon + SIMD)", "CPU (SIMD)", etc.
    }
    
    fn matmul(&self, a: &BitMatrix, b: &BitMatrix) -> BitMatrix {
        // Currently: delegates to BitMatrix::mul (will parallelize in Phase 2)
        a * b
    }
    
    fn rref(&self, matrix: &BitMatrix, pivot_right: bool) -> RrefResult {
        // Uses existing gf2-core::alg::rref (already SIMD-accelerated)
        crate::alg::rref::rref(matrix, pivot_right)
    }
}
```

**Status**: 
- ✅ 17 tests passing
- ✅ Auto-selects SIMD kernel when available
- ✅ Ready for rayon parallelism (Phase 2)
- ✅ Zero breaking changes to existing code

#### 2. GpuBackend (Vulkan/CUDA) - ⏭ Phase 3 Planned
```rust
pub struct VulkanBackend {
    device: vulkano::Device,
    queue: Arc<Queue>,
    compute_pipeline: Arc<ComputePipeline>,
}

impl ComputeBackend for VulkanBackend {
    fn name(&self) -> &str { "Vulkan GPU" }
    fn optimal_batch_size(&self) -> usize { 
        256  // Large batches to amortize PCIe overhead
    }
    // ... Vulkan compute shader implementation
}
```

#### 3. FpgaBackend - 🔬 Phase 5 Research
```rust
pub struct FpgaBackend {
    pcie_handle: PcieDevice,
    dma_buffers: Vec<PinnedBuffer>,
}

impl ComputeBackend for FpgaBackend {
    fn name(&self) -> &str { "FPGA PCIe" }
    fn optimal_batch_size(&self) -> usize { 
        16  // Continuous streaming mode
    }
    // ... PCIe DMA transfers to hardware pipeline
}
```

### Current Usage Example

```rust
// gf2-core usage (matrix operations)
use gf2_core::{BitMatrix, compute::{ComputeBackend, CpuBackend}};

let backend = CpuBackend::new();  // Auto-selects SIMD if available
let a = BitMatrix::random(100, 100, &mut rng);
let b = BitMatrix::random(100, 100, &mut rng);
let c = backend.matmul(&a, &b);  // Uses optimized kernel backend

// Future Phase 2: gf2-coding usage (batch encoding/decoding)
// use gf2_core::compute::CpuBackend;
// use gf2_coding::LdpcCode;
//
// let backend = CpuBackend::new();
// let code = LdpcCode::dvb_t2_normal(CodeRate::Rate3_5);
// let results = backend.decode_batch(&code, &llrs, 50);
```

---

## Parallelization Strategy by Code Type

### 1. LDPC Codes (High Priority) - ✅ Phase 1 Complete

**Current Status**: Block-level parallelism with rayon (6.7× speedup)

**Characteristics**:
- Embarrassingly parallel: Independent block encoding/decoding
- Belief propagation: Data-parallel (good GPU candidate)
- Sparse matrices: Irregular memory access (GPU challenge)

**Performance**:
- CPU (rayon): 8.29 Mbps (202 blocks, 24 cores)
- Target (GPU): 50-100 Mbps (research needed)

**Next Steps**:
- ⏭ **SIMD LLR operations** (gf2-kernels-simd): Extend existing `llr.rs` module
  - Add f64 support (current: f32 only)
  - Add saturate, batch hard decisions
  - Integrate into gf2-coding/src/llr.rs with runtime detection
- 🔬 GPU prototype (Vulkan compute shaders)
- 🔬 Measure memory-bound vs compute-bound on GPU

### 2. BCH Codes (Medium Priority)

**Status**: ⏸️ **BLOCKED** - Requires gf2-core Phase 15 (GF(2^m) Thread Safety)

**Current Blocker**: `Gf2mField` uses `Rc<FieldParams>` which is not `Send + Sync`
- See: [gf2-core Phase 15 Roadmap](../../gf2-core/ROADMAP.md#phase-15-gf2m-thread-safety)
- See: [GF2M Thread Safety Requirements](../../gf2-core/docs/GF2M_THREAD_SAFETY_REQUIREMENTS.md)
- **Timeline**: 5 days (Week 1 implementation)
- **After fix**: 8× speedup for BCH batch decoding on 12-core CPU

**Encoding**: Embarrassingly parallel (independent polynomial division)
```rust
impl BchEncoder {
    pub fn encode_batch(&self, messages: &[BitVec]) -> Vec<BitVec> {
        // ❌ Currently blocked: Gf2mField not Send+Sync
        // ✅ After Phase 15: Will use rayon parallelism
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

### 3. Convolutional Codes (Viterbi) - Medium Priority

**Decoding**: Highly parallel within each trace
- Butterfly operations: SIMD vectorizable
- Path metrics: Independent updates
- Traceback: **Serial** (requires backtracking through trellis)

**Strategy**: 
- Block-level: Parallel decoding of independent traces
- Within-block: SIMD for butterfly operations
- Traceback: Keep serial (< 5% of total time)

---

### 4. Polar Codes (Low Priority - Future)

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

## Performance Targets and Projections

### Performance Tiers

| Tier | Target Throughput | Backend | Use Case | Status |
|------|-------------------|---------|----------|--------|
| **Software Recording** | 10-20 Mbps | CPU (rayon) | SDR captures, offline processing | ✅ **ACHIEVED** (8.29 Mbps) |
| **CPU Optimized** | 50-100 Mbps | CPU (rayon + SIMD) | Real-time SDR reception | ⏭ Planned (SIMD LLR ops) |
| **GPU Accelerated** | 100-500 Mbps | Vulkan/CUDA | Multi-channel processing | 🔬 Research needed |
| **Broadcast** | 500 Mbps - 1 Gbps | FPGA/ASIC | Real-time DVB-T2 transmitters | 🔬 Long-term research |

### Current LDPC Performance (DVB-T2 Rate 3/5, 50 iterations)

| Configuration | Throughput (single) | Method | Speedup | Status |
|--------------|---------------------|--------|---------|--------|
| Serial baseline (f64) | 1.00 Mbps (30.0 ms) | Single-threaded | 1.0× | Baseline |
| Serial (f32) | 1.05 Mbps (28.5 ms) | f32 precision | 1.05× | ✅ Complete |
| + Neighbor caching | 1.43 Mbps (20.9 ms) | Check + var neighbors cached | 1.43× | ✅ Complete |
| Rayon (24 cores) | 8.29 Mbps (batch) | Block-level parallelism | 8× (batch) | ✅ Complete |
| Target (algorithmic) | ~2-3 Mbps | Min-sum variants, early term | ~2-3× | ⏭ Possible |
| Target (+ GPU) | ~100 Mbps | + Vulkan compute | ~100× | 🔬 Research |

### Roadmap by Phase

**Phase 1: CPU Parallelism** ✅ COMPLETE
- Encoding: 3.85 Mbps (sequential, Sync bounds TODO)
- Decoding: **8.29 Mbps** (batch of 202, **6.7× speedup**, 24-core CPU)
- **Achieved**: Software recording tier (~10 Mbps) ✓

**Phase 2: LLR f32 Migration** ✅ COMPLETE (2025-12-01)
- Changed Llr from f64 to f32 (breaking change)
- Scalar: 5% improvement (30.0 ms → 28.5 ms)
- Eliminates conversion overhead for SIMD
- Enables 2× wider SIMD vectors (8 lanes vs 4 in AVX2)

**Phase 2b: Allocation Elimination** ✅ COMPLETE (2025-12-01)

**Goal**: Remove allocation overhead blocking SIMD visibility

**Profiling Analysis**:
- Initial profiling found 23.1% overhead in allocations
  - 17.9% in `row_iter().collect()` (check node neighbors)
  - 5.2% in malloc/free
- Evidence-driven TDD approach with comprehensive profiling

**Implementation (3 iterations)**:
1. **Check neighbor caching**: Pre-compute at construction (9% gain)
2. **Temp buffer reuse**: Eliminate repeated allocations (3% gain)
3. **Variable neighbor caching**: Discovered via re-profiling (26% gain)

**Results**:
- Combined: **30% faster** (30.0 ms → 20.9 ms per block, 228 KiB/s)
- Allocation: Reduced 78% (23.1% → 5.1%)
- SIMD: Now visible at 12.9% (was hidden by allocation noise)
- Memory cost: +4.5 MB per decoder (negligible)

**Key Learnings**:
- Original "SIMD stack allocation" hypothesis was incorrect
- Real bottleneck: Multiple row_iter()/col_iter() calls per iteration
- Re-profiling after each step revealed additional opportunities
- Simple caching > complex stack allocation schemes

**Post-Optimization Profile** (20.9 ms per block):
- 70.6% in belief propagation computation (expected/optimal)
- 12.9% in SIMD min-sum (AVX2, now visible)
- 5.1% remaining allocation (acceptable)
- 2.7% syndrome check
- **Verdict**: Now compute-bound, which is ideal

**Phase 3: Next Optimization Options** ⏭ DECISION POINT

**Current State**: Compute-bound at 70.6% belief propagation
- Single-thread: 1.43 Mbps (20.9 ms per block)
- Multi-thread: 8.29 Mbps (24-core rayon)
- Allocation eliminated, SIMD working

**Option A: Algorithmic Improvements** (2-3 weeks)
- Normalized/offset min-sum (better convergence)
- Early termination optimization
- Layered decoding schedules
- Expected: 1.2-1.5× gain (algorithmic efficiency)

**Option B: GPU Prototype** 🔬 RESEARCH (3-6 months)
- Encoding: 200-500 Mbps (large batches >100 blocks)
- Decoding: 500-1000 Mbps (Vulkan compute shaders)
- **Target**: Professional broadcast equipment performance
- **Risk**: Memory-bound limitations, PCIe overhead

**Phase 4: FPGA Exploration** 🔬 LONG-TERM (1-2 years)
- Encoding: 1-10 Gbps (full hardware pipeline)
- Decoding: 1-10 Gbps (custom bit widths, unrolled BP)
- Latency: < 10 μs (vs. 1 ms CPU)
- **Target**: Real-time 4K/8K video broadcast, satellite links

---

## Implementation Roadmap

### Phase 1: ComputeBackend Infrastructure (Week 1) ✅ COMPLETE

**gf2-core ComputeBackend** ✅ COMPLETE (2025-11-29):
1. ✅ Created `compute::ComputeBackend` trait in gf2-core
2. ✅ Implemented `compute::CpuBackend` with kernel backend composition
3. ✅ Added 17 comprehensive tests (all passing)
4. ✅ Feature flag: `parallel` for rayon (opt-in)
5. ✅ All 441 gf2-core tests pass (backward compatible)

**gf2-coding LDPC Parallelism** ✅ Week 1 Complete (2025-11-28):
1. ✅ Add rayon dependency to `gf2-coding`
2. ⚠️ Implement parallel `encode_batch()` - sequential (Sync bounds TODO)
3. ✅ Implement parallel `decode_batch()` with rayon - **6.7× speedup**
4. ✅ Benchmark 1, 10, 50, 100, 202 block batches
5. ✅ Validate throughput: 1.23 Mbps → 8.29 Mbps (batch of 202)

**Week 2: Feature Flag & Benchmarking** ✅ COMPLETE (2025-12-01):
6. ✅ Added `parallel` feature flag (opt-in, matches gf2-core)
7. ✅ Made rayon optional dependency with conditional compilation
8. ✅ Fixed `CpuBackend` to use global rayon pool (respects `RAYON_NUM_THREADS`)
9. ✅ Created `benches/quick_parallel.rs` with fast iteration (~2 min)
10. ✅ Created `benchmark_quick.sh` automation script (~5 min)
11. ✅ Verified parallel scaling: 3.9× speedup on 8 threads
12. ✅ Comprehensive user guide: `PARALLEL_BENCHMARKING.md`
13. ✅ All 226 tests pass with/without parallel feature

**Short-Term** 🔧 IN PROGRESS (Weeks 3-4):
14. ✅ Thread scaling benchmarks complete (1, 2, 4, 8, 12 threads)
15. ✅ Parallel scaling documented: 3.9× on 8 threads (good efficiency)
16. ⏭ **NEXT**: Vectorize LDPC LLR operations in **gf2-kernels-simd/src/llr.rs**
    - Extend existing AVX2 min-sum implementation (currently f32, need f64)
    - Add saturate() SIMD implementation
    - Add batch hard_decision() for Vec<Llr> → BitVec conversion
    - Integrate into gf2-coding/src/llr.rs with runtime detection
    - **Target**: 69.8% of decode time spent in LLR operations
17. ⏭ Optimize sparse iteration patterns (17.7% of decode time)
18. ⏭ Optimize memory layout for cache efficiency (Structure-of-Arrays)
19. ✅ Parallel BCH batch APIs implemented (`BchEncoder::encode_batch()`, `BchDecoder::decode_batch()`)
20. ✅ Thread safety: All public APIs use Arc/global pool, documented in code

### Phase 2: Integrate with gf2-coding (Weeks 2-3) ⏭ NEXT

**Goal**: Use gf2-core's ComputeBackend in gf2-coding algorithms

11. ⏭ Add batch operations to ComputeBackend trait (encode_batch, decode_batch)
12. ⏭ Refactor `LdpcDecoder::decode_batch()` to use `CpuBackend`
13. ⏭ Add `BchEncoder::encode_batch()` using ComputeBackend
14. ⏭ Implement parallel matmul in `CpuBackend` (when `parallel` feature enabled)
15. ⏭ Document backend usage in gf2-coding README

### Phase 3: GPU Prototype (Month 3-6) 🔬 RESEARCH

**Goal**: Validate GPU acceleration feasibility

15. 🔬 Implement Vulkan compute shader prototype (LDPC decoder only)
16. 🔬 Measure memory-bound vs compute-bound characteristics
17. 🔬 Benchmark CPU vs GPU crossover point (batch size, block size)
18. 🔬 Validate PCIe transfer overhead amortization (batch >100 blocks)

**Research Questions**:
- Is BP memory-bound or compute-bound on GPU?
- What batch size justifies GPU offload?
- Sparse matrix performance on GPU (irregular access patterns)?

### Phase 4: GPU Production (Month 7-12) ⏭ CONDITIONAL

**Condition**: Phase 3 shows positive results (>5× speedup vs CPU)

19. ⏭ Production `VulkanBackend` with error handling
20. ⏭ Automatic fallback chain (GPU → CPU)
21. ⏭ Optimize memory transfers (pinned buffers, zero-copy)
22. ⏭ Support multiple algorithms (LDPC, Viterbi)

### Phase 5: FPGA Exploration (Year 1-2) 🔬 LONG-TERM

**Goal**: Feasibility study for broadcast applications

23. 🔬 FPGA feasibility study (Viterbi decoder on Xilinx)
24. 🔬 HLS prototype from Rust algorithm
25. 🔬 Power/area/throughput trade-off analysis
26. 🔬 Integration with GNU Radio for real-time SDR

## Success Metrics

### Phase 1 (CPU) - ✅ Achieved
- **Throughput**: 8.29 Mbps (target: ≥8 Mbps) ✓
- **Speedup**: 6.7× (target: ≥5×) ✓
- **Code quality**: Zero unsafe, clean API ✓

### Phase 2 (Abstraction) - Target: Month 2
- **API design**: `ComputeBackend` trait implemented
- **Backwards compatibility**: Existing code unaffected
- **Documentation**: User guide for backend selection

### Phase 3 (GPU Prototype) - Target: Month 6
- **Prototype working**: LDPC decoder on Vulkan
- **Performance data**: CPU vs GPU benchmarks
- **Decision**: Go/no-go for production GPU backend

### Phase 4 (Production GPU) - Target: Month 12 (if Phase 3 positive)
- **Throughput**: ≥100 Mbps LDPC decoding
- **Reliability**: 99.9% uptime with CPU fallback
- **Multi-algorithm**: LDPC + Viterbi support

### Phase 5 (FPGA) - Target: Year 2
- **Feasibility report**: Power, cost, throughput analysis
- **Prototype**: Single-algorithm FPGA implementation
- **Research publication**: Academic validation

---

## Research Questions and Open Problems

### GPU Acceleration
1. **Memory-bound vs compute-bound**: Profile LDPC BP on GPU to identify bottleneck
2. **Batch size crossover**: At what batch size does GPU outperform 24-core CPU?
3. **Sparse matrix performance**: How do irregular memory patterns affect GPU utilization?
4. **Transfer overhead**: Can we amortize PCIe with large batches (>100 blocks)?

### FPGA Acceleration
1. **Power efficiency**: Watts per Gbps compared to GPU/CPU
2. **Algorithm portability**: Which codes benefit most from custom hardware?
3. **Development cost**: HLS vs hand-written HDL trade-offs
4. **Flexibility**: Can FPGA adapt to multiple code rates/frame sizes?

### Algorithm-Specific
1. **LDPC**: Parallelizable belief propagation iterations vs inherent dependencies
2. **BCH**: Serial Berlekamp-Massey limits—can we use alternative algorithms?
3. **Viterbi**: Traceback serialization—what percentage of runtime?
4. **Polar**: SCL parallelism scalability—does list size L affect GPU performance?

## References

### Internal Documents
- **Profiling Results**: `docs/LDPC_PROFILING_RESULTS.md`
- **Performance Plan**: `docs/LDPC_PERFORMANCE_PLAN.md`
- **SDR Integration**: `docs/SDR_INTEGRATION.md`
- **Verification Status**: `docs/DVB_T2_VERIFICATION_STATUS.md`
- **SIMD Guide**: `docs/SIMD_PERFORMANCE_GUIDE.md`

### External Resources
- **Rayon**: Rust data-parallelism library (rayon-rs/rayon)
- **Vulkano**: Safe Vulkan bindings for Rust
- **CUDA**: NVIDIA GPU programming (consider for comparison)
- **Xilinx HLS**: High-Level Synthesis for FPGA prototyping
- **GNU Radio**: SDR framework integration target

**Last Updated**: 2025-11-29
