# RREF SIMD Optimization Notes

## Current Status (Phase 12.2)

**Achievement**: Core RREF implementation with 32-40% optimization from eliminating allocations.

**Performance**:
- Small matrices (256×256): **3.8× slower than M4RI** ✅ (within target)
- DVB-T2 Short (9K×16K): **5.1 seconds** ✅ (target: <10s, achieved!)
- Gap widens for larger matrices: 6-21× slower than M4RI

## Identified Bottleneck: Row XOR Operations

The hot path is `BitMatrix::row_xor()`:

```rust
pub fn row_xor(&mut self, dst: usize, src: usize) {
    let start_dst = dst * self.stride_words;
    let start_src = src * self.stride_words;

    // Current: Scalar XOR, one word at a time
    for i in 0..self.stride_words {
        self.data[start_dst + i] ^= self.data[start_src + i];
    }
}
```

**Profiling shows**: This loop is called O(m² × n) times during RREF.

## SIMD Opportunity in gf2-kernels-simd

The `gf2-kernels-simd` crate **allows unsafe code**, making it ideal for SIMD optimization:

### Proposed Optimization

1. **Move optimized row_xor to kernels**:
   - Create `gf2-kernels-simd/src/rref.rs`
   - Implement SIMD row XOR using AVX2/AVX-512
   - Process 256-512 bits per instruction instead of 64

2. **Expected speedup**:
   - AVX2 (256-bit): **~4× faster** row operations
   - AVX-512 (512-bit): **~8× faster** row operations
   - Overall RREF: **2-3× improvement** (not all time in XOR)

3. **Implementation approach**:

```rust
// In gf2-kernels-simd/src/rref.rs (unsafe allowed here!)
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

pub unsafe fn row_xor_avx2(
    dst: &mut [u64],
    src: &[u64],
) {
    let len = dst.len().min(src.len());
    let chunks = len / 4; // 4 u64s = 256 bits
    
    for i in 0..chunks {
        let offset = i * 4;
        
        // Load 256 bits from src
        let src_vec = _mm256_loadu_si256(
            src.as_ptr().add(offset) as *const __m256i
        );
        
        // Load 256 bits from dst
        let dst_vec = _mm256_loadu_si256(
            dst.as_ptr().add(offset) as *const __m256i
        );
        
        // XOR and store back to dst
        let result = _mm256_xor_si256(dst_vec, src_vec);
        _mm256_storeu_si256(
            dst.as_mut_ptr().add(offset) as *mut __m256i,
            result
        );
    }
    
    // Handle remainder with scalar ops
    for i in (chunks * 4)..len {
        dst[i] ^= src[i];
    }
}
```

4. **Integration with BitMatrix**:

```rust
// In gf2-core/src/matrix.rs
pub fn row_xor(&mut self, dst: usize, src: usize) {
    let start_dst = dst * self.stride_words;
    let start_src = src * self.stride_words;
    
    #[cfg(all(feature = "simd", target_arch = "x86_64"))]
    unsafe {
        use gf2_kernels_simd::rref::row_xor_avx2;
        row_xor_avx2(
            &mut self.data[start_dst..start_dst + self.stride_words],
            &self.data[start_src..start_src + self.stride_words],
        );
        return;
    }
    
    // Fallback: scalar implementation
    for i in 0..self.stride_words {
        self.data[start_dst + i] ^= self.data[start_src + i];
    }
}
```

### Benefits of This Approach

1. **Keeps gf2-core safe**: No unsafe code in core library
2. **Optional feature**: Users can opt-in with `--features simd`
3. **Platform-specific**: SIMD only on supported platforms
4. **Maintainable**: Clear separation between safe and unsafe code
5. **Testable**: Can benchmark SIMD vs scalar

### Expected Performance After SIMD

| Size | Current | With AVX2 (est.) | M4RI Target | Status |
|------|---------|------------------|-------------|--------|
| 256×256 | 343 µs | ~170 µs | 90 µs (1.9× M4RI) | ✅ Excellent |
| 1024×1024 | 8.37 ms | ~4.2 ms | 1.22 ms (3.4× M4RI) | ✅ Within target |
| 9K×16K (DVB) | 5.1 s | ~2.5 s | 241 ms (10× M4RI) | ✅ Good enough |

## Implementation Priority

**Current Status**: Phase 12.2 complete, **primary goals achieved**
- ✅ <10 seconds for DVB-T2 Short (5.1s achieved)
- ✅ Competitive on small matrices (3.8× vs M4RI)

**SIMD Priority**: **MEDIUM**
- Primary use case (LDPC) already practical
- Would improve competitive position
- Requires careful unsafe implementation
- Good learning opportunity for SIMD optimization

## References

- Existing SIMD kernels: `gf2-kernels-simd/src/bitwise.rs`
- AVX2 reference: Intel Intrinsics Guide
- Similar optimization: `kernels::ops::xor_inplace` (already SIMD-aware)
- M4RI source: Uses SIMD heavily in elimination loops

## Action Items

- [x] Phase 12.2: Eliminate allocation bottleneck
- [ ] Profile to confirm row_xor is dominant hotspot
- [ ] Implement AVX2 row_xor in gf2-kernels-simd
- [ ] Benchmark SIMD vs scalar
- [ ] Consider AVX-512 for even better performance
- [ ] Document SIMD feature in README

---

**Note**: The `gf2-kernels-simd` crate is specifically designed for unsafe SIMD code, keeping the main `gf2-core` crate safe. This is the right architecture for SIMD optimizations.
