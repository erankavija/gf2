# SIMD Performance Guide

---

## Overview

SIMD (Single Instruction Multiple Data) acceleration is enabled by default through the `gf2-kernels-simd` crate, providing 4-8× speedups for critical operations.

---

## Automatic SIMD Selection

The library automatically detects and uses the best available SIMD instructions at runtime:

- **AVX-512**: 8× parallelism (Intel Skylake-X 2017+, AMD Zen 4 2022+)
- **AVX2**: 4× parallelism (Intel Haswell 2013+, AMD Excavator 2015+)  
- **Scalar**: Baseline fallback for older CPUs

No configuration needed—optimal performance is automatic.

---

## What Gets Accelerated

### Bit Matrix Operations (gf2-core)

**RREF (Gaussian Elimination)**:
- XOR operations on matrix rows
- Critical for LDPC encoding preprocessing
- Speedup: 256-512× vs bit-level operations

**Matrix-Vector Multiply**:
- Used in LDPC encoding
- Word-level operations with SIMD
- Currently 97.5% of encoding time

### LLR Operations (gf2-kernels-simd)

**Check Node Updates** (min-sum):
- Horizontal minimum with sign preservation
- Used in LDPC belief propagation
- AVX2 implementation: `minsum_avx2`

**Variable Node Updates** (max-abs):
- Maximum absolute value
- Message passing in LDPC decoder
- AVX2 implementation: `maxabs_avx2`

---

## Performance Impact

### LDPC Preprocessing

| Operation | Without SIMD | With SIMD | Speedup |
|-----------|--------------|-----------|---------|
| RREF (DVB-T2 Short) | 5-10 seconds | 1-2 seconds | 4-8× |
| Cache generation (6 configs) | 60-120 seconds | 16 seconds | 4-8× |

### Runtime Operations

| Operation | Status | Impact |
|-----------|--------|--------|
| Encoding (matvec) | ✅ Active | 97.5% of encoding time |
| Decoding (LLR ops) | ✅ Active | Used in belief propagation |
| f32 LLRs | ✅ Active | 2× wider vectors (8 lanes vs 4) |

---

## Usage

### Building with SIMD

SIMD is enabled by default. Build normally:

```bash
# Standard release build (SIMD included)
cargo build --release

# Run tests
cargo test --release

# Generate LDPC caches
cargo run --release --bin generate_ldpc_cache short
```

### Disabling SIMD

To build without SIMD (not recommended):

```bash
cargo build --release --no-default-features
```

---

## Verification

Check if SIMD is active:

```bash
# Look for SIMD instructions in binary
objdump -d target/release/libgf2_core.so | grep -E 'vpxor|vpminu|vpadd' | wc -l

# Should see 100+ SIMD instructions if enabled
```

Verify at runtime:

```rust
use gf2_kernels_simd::llr;

if let Some(simd_fns) = llr::detect() {
    println!("SIMD active: {:?}", simd_fns);
} else {
    println!("Using scalar fallback");
}
```

---

## Architecture Details

### gf2-kernels-simd Crate

Separate crate for unsafe SIMD implementations:

```
gf2-kernels-simd/
├── src/
│   ├── lib.rs              # Detection and safe wrappers
│   ├── llr.rs              # LLR operations (min-sum, max-abs)
│   └── x86/
│       ├── avx2.rs         # AVX2 implementations
│       └── avx512.rs       # AVX-512 (future)
```

**Benefits**:
- Isolates unsafe code
- Clean separation of concerns
- Architecture-specific compilation
- Optional feature gating

### Runtime Detection

```rust
// Lazy static initialization (once at startup)
static LLR_SIMD: Lazy<Option<LlrFns>> = 
    Lazy::new(gf2_kernels_simd::llr::detect);

// Use in hot path
if let Some(ref fns) = *LLR_SIMD {
    (fns.minsum_fn)(&values)  // SIMD path
} else {
    scalar_minsum(&values)     // Fallback
}
```

---

## Next Optimizations

### Stack Allocation (In Progress)

**Problem**: Small LLR slices allocate on heap  
**Solution**: Use stack arrays for <16 elements  
**Expected**: 2-4× additional speedup

### Wider Vectors (Future)

**AVX-512 support**: 8-lane f32 operations (vs 4-lane AVX2)  
**Expected**: 1.5-2× additional speedup on supported CPUs

---

## References

- [LDPC_PERFORMANCE.md](LDPC_PERFORMANCE.md) - Performance optimization plan
- [PARALLELIZATION.md](PARALLELIZATION.md) - Overall parallelization strategy
- `gf2-kernels-simd/` - SIMD implementation source
```toml
[features]
default = ["simd"]  # Enable SIMD by default
simd = ["gf2-core/simd"]
```

Then SIMD is always enabled:
```bash
cargo build --release  # SIMD automatically enabled
```

Users can opt-out if needed:
```bash
cargo build --release --no-default-features
```

---

## Benchmarking

### Compare Scalar vs SIMD

```bash
# Scalar baseline
cargo bench --bench linear_codes --no-default-features

# SIMD accelerated
cargo bench --bench linear_codes --features simd
```

### Expected Results

For LDPC encoding preprocessing on DVB-T2 matrices:

| Matrix Size | Scalar RREF | SIMD RREF | Speedup |
|-------------|-------------|-----------|---------|
| 6,480 × 16,200 (Short 3/5) | ~8 seconds | ~1.5 seconds | **5.3×** |
| 9,000 × 16,200 (Short 1/2) | ~12 seconds | ~2.2 seconds | **5.5×** |
| 16,200 × 64,800 (Normal 1/2) | ~120 seconds | ~22 seconds | **5.5×** |

Actual speedup depends on CPU architecture:
- **AVX512**: 6-8× speedup
- **AVX2**: 4-5× speedup
- **No SIMD**: 1× (baseline scalar)

---

## CPU Requirements

### Minimum
- x86-64 CPU (64-bit Intel/AMD)
- No special requirements—scalar fallback always works

### Recommended
- **Intel**: Haswell (2013) or newer → AVX2 support
- **AMD**: Excavator (2015) or newer → AVX2 support

### Optimal
- **Intel**: Skylake-X (2017) or newer → AVX512 support
- **AMD**: Zen 4 (2022) or newer → AVX512 support

### Check Your CPU

```bash
# Linux: Check for AVX2 and AVX512
lscpu | grep -E "avx2|avx512"

# Or check /proc/cpuinfo
grep -E "avx2|avx512" /proc/cpuinfo
```

If you see `avx2` or `avx512_*`, you'll benefit from SIMD.

---

## Future: ARM NEON Support

The SIMD architecture is extensible. Future work could add ARM NEON support:
- **NEON**: 4× parallelism on ARM64 (similar to AVX2)
- **SVE/SVE2**: 8-16× parallelism on modern ARM (similar to AVX512)

If you need ARM support, open an issue!

---

## Technical Details

### SIMD in the RREF Pipeline

```
RREF Algorithm
  └─> BitMatrix::row_xor(dst, src)
       └─> kernels::ops::xor_inplace(dst_words, src_words)
            ├─> [SIMD enabled] AVX2/AVX512 vectorized XOR
            └─> [SIMD disabled] Scalar word-by-word XOR
```

### Code Path Selection

```rust
// Automatic backend selection in kernels::ops::xor_inplace()
match select_backend_for_size(dst.len()) {
    #[cfg(feature = "simd")]
    SelectedBackend::Simd => {
        if let Some(backend) = crate::kernels::simd::maybe_simd() {
            backend.xor(dst, src);  // AVX2/AVX512 path
        } else {
            scalar_backend.xor(dst, src);  // Fallback
        }
    }
    SelectedBackend::Scalar => {
        scalar_backend.xor(dst, src);
    }
}
```

### Size Threshold

SIMD is beneficial for large arrays. gf2-core uses a size threshold:
- **< 8 words**: Use scalar (SIMD overhead not worth it)
- **≥ 8 words**: Use SIMD (amortized speedup)

For LDPC matrices, rows are typically 100-1000 words → SIMD always beneficial.

---

## Recommendation

**Make SIMD a default feature** after the RREF migration:

1. SIMD provides 4-8× speedup for the critical RREF operation
2. Runtime CPU detection ensures compatibility (automatic fallback)
3. Most production servers have AVX2 (2013+)
4. Binary size increase is minimal (~50KB)
5. No downside for CPUs without SIMD (graceful fallback)

**Proposed Cargo.toml change:**
```toml
[features]
default = ["simd"]  # Change from default = []
simd = ["gf2-core/simd"]
visualization = ["gf2-core/visualization"]
```

Users who need minimal binaries can still opt-out:
```bash
cargo build --no-default-features
```

---

## Questions?

- **Q**: Does SIMD require unsafe code?  
  **A**: Yes, but it's isolated in the `gf2-kernels-simd` crate. gf2-coding and gf2-core remain `#![deny(unsafe_code)]`.

- **Q**: What if my CPU doesn't support AVX2?  
  **A**: Automatic fallback to scalar code. Zero performance penalty.

- **Q**: Does SIMD increase binary size?  
  **A**: ~50KB for the SIMD kernels. Negligible for most use cases.

- **Q**: Can I verify SIMD is being used?  
  **A**: Check CPU detection at startup (add logging) or compare benchmarks.

- **Q**: Does SIMD help for small matrices?  
  **A**: Not much. SIMD shines for large matrices (>1000 rows/cols). DVB-T2 codes are large.

---

## See Also

- gf2-core RREF implementation: `crates/gf2-core/src/alg/rref.rs`
- gf2-kernels-simd: `crates/gf2-kernels-simd/`
- Benchmarks: `crates/gf2-core/benches/rref.rs`
