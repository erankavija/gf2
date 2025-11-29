# ComputeBackend Design and Implementation

**Date**: 2025-11-29  
**Status**: Phase 1 Complete  
**Location**: `gf2-core/src/compute/`

## Overview

The ComputeBackend module provides a unified abstraction for algorithm-level operations across different execution backends (CPU, GPU, FPGA). This complements the existing `kernels::Backend` abstraction for primitive operations.

## Architecture

### Two-Layer Backend Design

```
┌────────────────────────────────────────────────────────────┐
│ compute::ComputeBackend (Algorithm-Level)                  │
│ - matmul(), rref(), batch encode/decode                    │
│ - Implementations: CpuBackend, GpuBackend (future)         │
└────────────────────────────────────────────────────────────┘
                            ▼ uses
┌────────────────────────────────────────────────────────────┐
│ kernels::Backend (Primitive-Level)                         │
│ - xor(), and(), popcount(), parity()                       │
│ - Implementations: ScalarBackend, SimdBackend              │
└────────────────────────────────────────────────────────────┘
```

### Key Design Principles

1. **Composability**: ComputeBackend uses kernels::Backend for primitives
2. **Opt-in Performance**: Features are optional (parallel, gpu)
3. **Zero Divergence**: Single backend pattern across workspace
4. **Type Safety**: Static dispatch, compile-time backend selection
5. **Testability**: All backends implement same trait

## Implementation Status

### Phase 1: Core Infrastructure ✅ COMPLETE

**Files Created**:
- `src/compute/mod.rs` - Module exports and documentation
- `src/compute/backend.rs` - ComputeBackend trait (161 lines, 8 tests)
- `src/compute/cpu.rs` - CpuBackend implementation (219 lines, 9 tests)

**Test Coverage**: 17 tests, all passing
- Backend name and properties (3 tests)
- Matrix multiplication (4 tests)
- RREF operations (4 tests)
- Kernel backend integration (3 tests)
- Property tests (3 tests with rand feature)

**Integration**: All 441 gf2-core tests pass (zero breaking changes)

## API Reference

### ComputeBackend Trait

```rust
pub trait ComputeBackend: Send + Sync {
    fn name(&self) -> &str;
    fn kernel_backend(&self) -> &dyn crate::kernels::Backend;
    fn matmul(&self, a: &BitMatrix, b: &BitMatrix) -> BitMatrix;
    fn rref(&self, matrix: &BitMatrix, pivot_from_right: bool) -> RrefResult;
}
```

### CpuBackend

```rust
pub struct CpuBackend {
    kernel: Box<dyn Backend>,
    #[cfg(feature = "parallel")]
    thread_pool: rayon::ThreadPool,
}

impl CpuBackend {
    pub fn new() -> Self { /* Auto-selects best kernel backend */ }
}
```

**Features**:
- Auto-selects SIMD kernel if available
- Creates rayon thread pool when `parallel` feature enabled
- Returns backend name: "CPU (rayon + SIMD)", "CPU (SIMD)", "CPU (scalar)"

## Usage Examples

### Basic Matrix Operations

```rust
use gf2_core::{BitMatrix, compute::{ComputeBackend, CpuBackend}};

let backend = CpuBackend::new();
let a = BitMatrix::random(100, 100, &mut rng);
let b = BitMatrix::random(100, 100, &mut rng);
let c = backend.matmul(&a, &b);  // Uses SIMD if available
```

### RREF with Backend

```rust
use gf2_core::compute::CpuBackend;

let backend = CpuBackend::new();
let matrix = BitMatrix::random(50, 100, &mut rng);
let result = backend.rref(&matrix, false);
println!("Rank: {}", result.rank);
```

## Feature Flags

```toml
[features]
default = ["rand", "io"]
parallel = ["rayon"]  # Enable rayon thread pool in CpuBackend
gpu = ["vulkano"]     # Future: GPU backend
```

## Testing Strategy

All tests follow TDD methodology:
1. Tests written first
2. Minimal implementation to pass
3. Refactor while keeping tests green

Test categories:
- Unit tests: Individual backend methods
- Integration tests: Backend composition with kernels
- Property tests: Algebraic properties (with rand feature)

## Future Phases

### Phase 2: Batch Operations (Weeks 2-3)

Add to `ComputeBackend` trait:
```rust
fn encode_batch(&self, encoder: &dyn Encodable, msgs: &[BitVec]) -> Vec<BitVec>;
fn decode_batch(&self, decoder: &dyn Decodable, llrs: &[Vec<Llr>]) -> Vec<Result>;
```

### Phase 3: GPU Backend (Months 3-6)

```rust
#[cfg(feature = "gpu")]
pub struct GpuBackend {
    device: vulkano::Device,
    cpu_fallback: CpuBackend,
}

impl ComputeBackend for GpuBackend {
    // GPU implementations with CPU fallback
}
```

## Design Decisions

### Why Two Backend Layers?

**Primitives** (kernels::Backend):
- XOR, AND, popcount - building blocks
- SIMD vs Scalar choice
- Always available

**Algorithms** (compute::ComputeBackend):
- matmul, RREF, batch ops - composed operations
- CPU vs GPU vs FPGA choice
- Optional features

### Why in gf2-core?

1. **Prevents divergence**: Single backend pattern
2. **Reusability**: All workspace crates benefit
3. **Composability**: Algorithms build on primitives
4. **Optional**: Feature flags keep core lightweight

## References

- Implementation: `gf2-core/src/compute/`
- Tests: `gf2-core/src/compute/backend.rs`, `cpu.rs`
- Usage: `gf2-coding` (Phase 2)
- Related: `gf2-core/src/kernels/` (primitive backend)
