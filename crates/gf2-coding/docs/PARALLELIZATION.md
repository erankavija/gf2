# Parallelization Strategy

**Status**: Phase 1 Complete (CPU parallelism), Phase 2 in progress

---

## Architecture Overview

### Layered Parallelization Model

```
┌─────────────────────────────────────────────────────────┐
│ Application Layer                                       │
│ - Simple API: code.encode_batch(messages)              │
│ - Backend selection: CpuBackend, GpuBackend (future)   │
└─────────────────────────────────────────────────────────┘
                       ▼
┌─────────────────────────────────────────────────────────┐
│ Backend Abstraction (gf2-core/compute)                  │
│ - Trait: ComputeBackend                                 │
│ - Implementations: CpuBackend, VulkanBackend (planned)  │
│ - Batch operations: Always operate on batches (≥1)      │
└─────────────────────────────────────────────────────────┘
                       ▼
┌─────────────────────────────────────────────────────────┐
│ Algorithm Layer (gf2-coding)                            │
│ - LDPC: Belief propagation (data-parallel)             │
│ - BCH: Syndrome + BM (mixed parallelism)               │
└─────────────────────────────────────────────────────────┘
                       ▼
┌─────────────────────────────────────────────────────────┐
│ Primitive Layer (gf2-core + gf2-kernels-simd)          │
│ - Safe abstractions with runtime dispatch               │
│ - SIMD implementations (AVX2/AVX-512/NEON)              │
│ - Bit ops, LLR ops, GF(2^m) ops                        │
└─────────────────────────────────────────────────────────┘
```

---

## Current Implementation

### ✅ Phase 1: CPU Parallelism (Complete)

**LDPC Codes**:
- Rayon-based parallelism: 6.7× speedup on 24-core CPU
- Throughput: 1.23 Mbps (serial) → 8.29 Mbps (parallel, 202 blocks)
- Batch API: `decode_batch(&[Vec<Llr>])` for embarrassingly parallel workloads
- Thread-local state: Each thread creates own `LdpcDecoder` instance
- Arc-based code sharing: Cheap cloning of parity-check matrices

**SIMD Primitives** (gf2-kernels-simd):
- Architecture-specific SIMD code in separate crate
- Bit operations: AVX2 kernels for XOR, AND, popcount
- LLR operations: AVX2 horizontal min-sum, max-abs
- Runtime CPU detection with safe wrappers
- 178 SIMD instructions active in optimized builds

**Other Codes** (Serial - planned for Phase 2):
- BCH: Sequential encoding/decoding (no batch API yet)
- Viterbi: Single-threaded trellis decoding
- Block codes: No parallel primitives

### ✅ Phase 1b: ComputeBackend Infrastructure (Complete)

**Delivered**:
- `compute::ComputeBackend` trait in gf2-core
- `CpuBackend` with auto-selection of SIMD kernels
- Batch operations: `batch_matvec`, `batch_matvec_transpose`
- Thread-safe BitVec/BitMatrix with Mutex
- 19 comprehensive tests (all passing)

**Integration**:
- LDPC encoder uses `ComputeBackend::batch_matvec_transpose`
- BCH encoder batch API: `BchEncoder::encode_batch()`
- Richardson-Urbanke encoding uses backend for parallel operations
- Zero breaking changes (all 221 tests pass)

---

## SIMD Implementation Strategy

### gf2-kernels-simd Architecture

**Separate crate benefits**:
- Isolates unsafe code from safe abstractions
- Optional dependency (feature-gated)
- Architecture-specific compilation
- Clean separation of concerns

**Module Organization**:
```
gf2-kernels-simd/
├── src/
│   ├── lib.rs              # Detection, safe wrappers
│   ├── llr.rs              # LLR operations (min-sum, max-abs)
│   ├── gf2m.rs             # GF(2^m) field arithmetic (future)
│   └── x86/
│       ├── mod.rs          # x86-64 detection
│       ├── avx2.rs         # AVX2 implementations
│       └── avx512.rs       # AVX-512 implementations (future)
```

### LLR Operations

**Current** (✅ Exists):
- `minsum_avx2(inputs: &[f32]) -> f32`: Sign-preserving horizontal min
- `maxabs_avx2(inputs: &[f32]) -> f32`: Maximum absolute value

**Next**:
- Stack allocation for small slices (<16 elements)
- Saturate and hard-decision batch operations
- Extended SIMD widths (AVX-512)

### Integration Pattern

**Runtime Selection**:
```rust
// gf2-coding/src/llr.rs
impl Llr {
    pub fn boxplus_minsum_n(llrs: &[Llr]) -> Llr {
        #[cfg(feature = "simd")]
        {
            static LLR_SIMD: Lazy<Option<LlrFns>> =
                Lazy::new(gf2_kernels_simd::llr::detect);
            
            if let Some(ref fns) = *LLR_SIMD {
                let values: Vec<f32> = llrs.iter().map(|l| l.value()).collect();
                return Llr::new((fns.minsum_fn)(&values));
            }
        }
        // Scalar fallback
        scalar_minsum(llrs)
    }
}
```

**Automatic CPU Detection**:
- **AVX512**: 8× parallelism (Intel Skylake-X 2017+, AMD Zen 4 2022+)
- **AVX2**: 4× parallelism (Intel Haswell 2013+, AMD Excavator 2015+)
- **Scalar**: Baseline fallback

---

## Performance Tiers

### Tier 1: Offline Processing (Current)
- **Current**: 3.85 Mbps encoding, 8.29 Mbps decoding (parallel)
- **Use case**: Post-capture file processing
- **Real-time factor**: 0.3× (30% of real-time)

### Tier 2: Near Real-Time (Week 1 ✅ Achieved)
- **Target**: 10-20 Mbps encoding, 5-10 Mbps decoding
- **Use case**: Software recording with 2-5× delay
- **Real-time factor**: 0.2-0.6× (20-60% of real-time)
- **Status**: Decoder achieved with rayon parallelism

### Tier 3: Real-Time SDR (Week 2 🎯 Goal)
- **Target**: 50-100 Mbps encoding/decoding
- **Use case**: Live DVB-T2 reception on PC
- **Real-time factor**: 1-2× (real-time to 2× real-time)
- **Approach**: SIMD + memory optimizations + parallelism

### Tier 4: High-Performance (Stretch Goal)
- **Target**: 200+ Mbps encoding/decoding
- **Use case**: Multi-channel processing, 4K UHDTV
- **Real-time factor**: 4-6×
- **Approach**: GPU offload or aggressive optimizations

---

## Optimization Roadmap

### ⏭ Phase 2: SIMD Optimization (Week 2)

**LLR Operations**:
- Vectorize check node updates (min-sum)
- Stack allocation for small slices
- Batch operations to reduce overhead
- Expected: 2-4× speedup

**Sparse Matrix Operations**:
- Cache-friendly iteration
- Prefetch neighbors
- Coalesced memory access
- Expected: 1.5-2× speedup

### 🔬 Phase 3: Advanced Optimizations (Weeks 3-4)

**Layered Decoding**:
- Better convergence (fewer iterations)
- Improved cache locality
- Expected: 2-3× speedup

**Quantized LLRs**:
- Fixed-point (1-2 bytes vs 4 bytes)
- Reduced memory bandwidth
- Better cache utilization
- Expected: 2-3× speedup

### 🔬 Phase 4: GPU Prototype (Months 3-6)

**Research Questions**:
- Is LDPC belief propagation memory-bound or compute-bound on GPU?
- What batch size amortizes PCIe transfer overhead?
- GPU crossover point vs 24-core CPU?

**Technology Choices**:
- Vulkan Compute (recommended): Cross-platform
- CUDA: NVIDIA-only, mature ecosystem
- FPGA: Broadcast applications (1 Gbps+)

**Decision Criteria**: Only proceed if profiling shows >20% time in GPU-acceleratable operations with >5× speedup potential.

---

## Thread Configuration

### Rayon Thread Control

```bash
# Sequential baseline
RAYON_NUM_THREADS=1 cargo bench --features parallel

# Physical cores (recommended)
RAYON_NUM_THREADS=12 cargo bench --features parallel

# All cores (with hyperthreading)
cargo bench --features parallel
```

### Performance Metrics

**Speedup** = Time(1 thread) / Time(N threads)  
**Efficiency** = Speedup / N × 100%

- **Good efficiency**: >70%
- **Excellent efficiency**: >85%

**Current LDPC decoding**: 6.7× on 24 cores = 28% efficiency
- Indicates memory bandwidth saturation
- Room for improvement with SIMD to reduce memory pressure

---

## Benchmarking

### Quick Iteration

```bash
# Fast feedback (~2 minutes)
cargo bench --bench quick_parallel --features parallel

# Automated thread scaling (~5 minutes)
./benchmark_quick.sh
```

### Full Benchmarking

```bash
# Comprehensive throughput
cargo bench --bench ldpc_throughput --features parallel

# Backend comparison
cargo bench --bench batch_operations --features parallel

# View results
xdg-open target/criterion/report/index.html
```

---

## Future Work

### ⏸️ GF(2^m) Thread Safety (Prerequisite for BCH/RS)

**Status**: Blocked on gf2-core Phase 15  
**Problem**: `Gf2mField` uses `Rc<FieldParams>` (not `Send + Sync`)  
**Impact**: BCH/RS batch operations cannot use rayon  
**Solution**: Replace `Rc` with `Arc` in gf2-core

### GPU/FPGA Exploration

**GPU**: Vulkan compute shaders for massively parallel belief propagation  
**FPGA**: Custom bit widths, hardware pipelines for broadcast (1 Gbps+)  
**Timeline**: Research phase after achieving 50-100 Mbps on CPU

---

## References

- [LDPC_PERFORMANCE.md](LDPC_PERFORMANCE.md) - Detailed LDPC optimization plan
- [DVB_T2.md](DVB_T2.md) - DVB-T2 implementation and verification
- `gf2-core/docs/COMPUTE_BACKEND_DESIGN.md` - Backend architecture details
- `benches/` - Performance benchmarks
