# SIMD Performance Guide for gf2-coding

## TL;DR

**Enable SIMD for 4-8× faster LDPC encoding preprocessing:**

```bash
cargo build --release --features simd
```

Or make it default in `Cargo.toml`:
```toml
[features]
default = ["simd"]
```

---

## Why SIMD Matters for LDPC Codes

LDPC encoding preprocessing involves Gaussian elimination on large parity-check matrices. This is the bottleneck in the encoding pipeline.

### Performance Impact

| Operation | Method | Time (DVB-T2 Short) |
|-----------|--------|---------------------|
| Gauss Elimination (manual bit-level) | Current | ~5-10 minutes |
| Gauss Elimination (scalar RREF) | After migration | ~5-10 seconds |
| Gauss Elimination (SIMD RREF) | After migration + SIMD | **~1-2 seconds** |

**Speedup Summary**:
- Manual → Scalar RREF: **64× faster**
- Manual → SIMD RREF: **256-512× faster**
- Scalar RREF → SIMD RREF: **4-8× faster**

---

## How SIMD Acceleration Works

### The RREF Algorithm

Gaussian elimination (RREF) performs row operations on matrices:
1. Find pivot row
2. **XOR pivot row into other rows** ← This is the hot loop
3. Repeat for all columns

Step 2 dominates runtime for large matrices.

### SIMD Optimization

Without SIMD (scalar):
```
for each 64-bit word:
    dst[i] ^= src[i]     // 1 word per cycle
```

With AVX2 SIMD:
```
for each 256-bit chunk (4 words):
    dst[i:i+3] ^= src[i:i+3]  // 4 words per cycle
```

With AVX512 SIMD:
```
for each 512-bit chunk (8 words):
    dst[i:i+7] ^= src[i:i+7]  // 8 words per cycle
```

### Automatic CPU Detection

The SIMD backend automatically detects CPU capabilities at runtime:
- **AVX512**: 8× parallelism (Intel Skylake-X 2017+, AMD Zen 4 2022+)
- **AVX2**: 4× parallelism (Intel Haswell 2013+, AMD Excavator 2015+)
- **Scalar**: 1× baseline (fallback for older CPUs)

No code changes needed—just enable the `simd` feature.

---

## Enabling SIMD

### Option 1: Feature Flag (Recommended for now)

```bash
# Build with SIMD
cargo build --release --features simd

# Test with SIMD
cargo test --features simd

# Benchmark with SIMD
cargo bench --features simd

# Generate LDPC caches with SIMD
cargo run --bin generate_cache --release --features simd
```

### Option 2: Default Feature (Recommended after migration)

Edit `Cargo.toml`:
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

- [GAUSS_ELIMINATION_MIGRATION_PLAN.md](GAUSS_ELIMINATION_MIGRATION_PLAN.md) - Full migration plan
- gf2-core RREF implementation: `crates/gf2-core/src/alg/rref.rs`
- gf2-kernels-simd: `crates/gf2-kernels-simd/`
- Benchmarks: `crates/gf2-core/benches/rref.rs`
