//! AVX2 batch entry points for the F_5 bit-sliced 3-plane encoding.
//!
//! These are the actual SIMD-emitting functions for the F_5 kernel. They
//! operate on three parallel `&[u64]` planes per operand (one plane per bit
//! in the 3-bit canonical encoding) and step by 4 u64 words (one AVX2 lane)
//! per plane per iteration.
//!
//! The decode/cross-product/encode circuit from `gf2_algebra::packed::packed5`
//! (R1 Candidate D) is reproduced here using AVX2 256-bit bitwise operations
//! (`vpand`, `vpor`, `vpxor`). One AVX2 lane processes 4 u64 word-units
//! simultaneously, each representing 64 independent F_5 lanes, giving 256
//! F_5 elements per register group.
//!
//! Each public `run_*5_batch` function is `#[target_feature(enable = "avx2")]`.
//! Private helper functions are `#[inline(always)]` `unsafe fn` without
//! `#[target_feature]` — they are only called from within
//! `#[target_feature(enable = "avx2")]` bodies and inherit that feature
//! guarantee from the call chain.
//!
//! ## Slice contract
//!
//! All nine input/output slices for binary ops must have the same length `n`
//! where `n % 4 == 0`. Empty slices (`n = 0`) are allowed (no-op). The three
//! output slices for unary neg have the same shape: 3 parallel planes of
//! length `n`.
//!
//! ## AVX-512 deferral
//!
//! AVX-512 variants (using `vpternlogd` for 3-input ternary logic to reduce
//! op counts) are deferred. The aspirational criterion in the issue allows
//! this omission.

#[cfg(target_arch = "x86")]
use core::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::*;

// ---------------------------------------------------------------------------
// AVX2 decode / encode / circuit helpers
// (no #[target_feature] — called from target_feature-enabled entry points)
// ---------------------------------------------------------------------------

/// Decode a triple of AVX2 lanes `(b0, b1, b2)` into five selector lanes.
///
/// Mirrors `crate::bipedal::packed5::decode5_word` over 4 u64 words at once.
///
/// # Safety
///
/// Caller must be executing within an AVX2 `#[target_feature]` function.
#[inline(always)]
unsafe fn decode5_avx2(b0: __m256i, b1: __m256i, b2: __m256i) -> [__m256i; 5] {
    // SAFETY: called only from #[target_feature(enable = "avx2")] paths.
    let ones = _mm256_set1_epi8(-1i8);
    let n0 = _mm256_xor_si256(b0, ones);
    let n1 = _mm256_xor_si256(b1, ones);
    let n2 = _mm256_xor_si256(b2, ones);
    let n2n1 = _mm256_and_si256(n2, n1);
    let n2_1 = _mm256_and_si256(n2, b1);
    let n1n0 = _mm256_and_si256(n1, n0);
    let e0 = _mm256_and_si256(n2n1, n0);
    let e1 = _mm256_and_si256(n2n1, b0);
    let e2 = _mm256_and_si256(n2_1, n0);
    let e3 = _mm256_and_si256(n2_1, b0);
    let e4 = _mm256_and_si256(b2, n1n0);
    [e0, e1, e2, e3, e4]
}

/// Encode per-result selectors into output bit-planes `(c0, c1, c2)`.
///
/// - `c0 = r[1] | r[3]`, `c1 = r[2] | r[3]`, `c2 = r[4]`
///
/// # Safety
///
/// Caller must be executing within an AVX2 `#[target_feature]` function.
#[inline(always)]
unsafe fn encode5_avx2(r: [__m256i; 5]) -> (__m256i, __m256i, __m256i) {
    // SAFETY: called only from #[target_feature(enable = "avx2")] paths.
    let c0 = _mm256_or_si256(r[1], r[3]);
    let c1 = _mm256_or_si256(r[2], r[3]);
    let c2 = r[4];
    (c0, c1, c2)
}

/// F_5 add circuit on AVX2 lanes — cross-product `(i + j) mod 5 == k`.
///
/// # Safety
///
/// Caller must be executing within an AVX2 `#[target_feature]` function.
#[inline(always)]
unsafe fn add5_avx2(
    b0a: __m256i,
    b1a: __m256i,
    b2a: __m256i,
    b0b: __m256i,
    b1b: __m256i,
    b2b: __m256i,
) -> (__m256i, __m256i, __m256i) {
    // SAFETY: called only from #[target_feature(enable = "avx2")] paths.
    let ea = decode5_avx2(b0a, b1a, b2a);
    let eb = decode5_avx2(b0b, b1b, b2b);
    let r1 = _mm256_or_si256(
        _mm256_or_si256(
            _mm256_or_si256(
                _mm256_and_si256(ea[0], eb[1]),
                _mm256_and_si256(ea[1], eb[0]),
            ),
            _mm256_or_si256(
                _mm256_and_si256(ea[2], eb[4]),
                _mm256_and_si256(ea[3], eb[3]),
            ),
        ),
        _mm256_and_si256(ea[4], eb[2]),
    );
    let r2 = _mm256_or_si256(
        _mm256_or_si256(
            _mm256_or_si256(
                _mm256_and_si256(ea[0], eb[2]),
                _mm256_and_si256(ea[1], eb[1]),
            ),
            _mm256_or_si256(
                _mm256_and_si256(ea[2], eb[0]),
                _mm256_and_si256(ea[3], eb[4]),
            ),
        ),
        _mm256_and_si256(ea[4], eb[3]),
    );
    let r3 = _mm256_or_si256(
        _mm256_or_si256(
            _mm256_or_si256(
                _mm256_and_si256(ea[0], eb[3]),
                _mm256_and_si256(ea[1], eb[2]),
            ),
            _mm256_or_si256(
                _mm256_and_si256(ea[2], eb[1]),
                _mm256_and_si256(ea[3], eb[0]),
            ),
        ),
        _mm256_and_si256(ea[4], eb[4]),
    );
    let r4 = _mm256_or_si256(
        _mm256_or_si256(
            _mm256_or_si256(
                _mm256_and_si256(ea[0], eb[4]),
                _mm256_and_si256(ea[1], eb[3]),
            ),
            _mm256_or_si256(
                _mm256_and_si256(ea[2], eb[2]),
                _mm256_and_si256(ea[3], eb[1]),
            ),
        ),
        _mm256_and_si256(ea[4], eb[0]),
    );
    let zero = _mm256_setzero_si256();
    encode5_avx2([zero, r1, r2, r3, r4])
}

/// F_5 sub circuit on AVX2 lanes — cross-product `(i - j + 5) mod 5 == k`.
///
/// # Safety
///
/// Caller must be executing within an AVX2 `#[target_feature]` function.
#[inline(always)]
unsafe fn sub5_avx2(
    b0a: __m256i,
    b1a: __m256i,
    b2a: __m256i,
    b0b: __m256i,
    b1b: __m256i,
    b2b: __m256i,
) -> (__m256i, __m256i, __m256i) {
    // SAFETY: called only from #[target_feature(enable = "avx2")] paths.
    let ea = decode5_avx2(b0a, b1a, b2a);
    let eb = decode5_avx2(b0b, b1b, b2b);
    let r1 = _mm256_or_si256(
        _mm256_or_si256(
            _mm256_or_si256(
                _mm256_and_si256(ea[0], eb[4]),
                _mm256_and_si256(ea[1], eb[0]),
            ),
            _mm256_or_si256(
                _mm256_and_si256(ea[2], eb[1]),
                _mm256_and_si256(ea[3], eb[2]),
            ),
        ),
        _mm256_and_si256(ea[4], eb[3]),
    );
    let r2 = _mm256_or_si256(
        _mm256_or_si256(
            _mm256_or_si256(
                _mm256_and_si256(ea[0], eb[3]),
                _mm256_and_si256(ea[1], eb[4]),
            ),
            _mm256_or_si256(
                _mm256_and_si256(ea[2], eb[0]),
                _mm256_and_si256(ea[3], eb[1]),
            ),
        ),
        _mm256_and_si256(ea[4], eb[2]),
    );
    let r3 = _mm256_or_si256(
        _mm256_or_si256(
            _mm256_or_si256(
                _mm256_and_si256(ea[0], eb[2]),
                _mm256_and_si256(ea[1], eb[3]),
            ),
            _mm256_or_si256(
                _mm256_and_si256(ea[2], eb[4]),
                _mm256_and_si256(ea[3], eb[0]),
            ),
        ),
        _mm256_and_si256(ea[4], eb[1]),
    );
    let r4 = _mm256_or_si256(
        _mm256_or_si256(
            _mm256_or_si256(
                _mm256_and_si256(ea[0], eb[1]),
                _mm256_and_si256(ea[1], eb[2]),
            ),
            _mm256_or_si256(
                _mm256_and_si256(ea[2], eb[3]),
                _mm256_and_si256(ea[3], eb[4]),
            ),
        ),
        _mm256_and_si256(ea[4], eb[0]),
    );
    let zero = _mm256_setzero_si256();
    encode5_avx2([zero, r1, r2, r3, r4])
}

/// F_5 mul circuit on AVX2 lanes — cross-product `(i * j) mod 5 == k`.
///
/// # Safety
///
/// Caller must be executing within an AVX2 `#[target_feature]` function.
#[inline(always)]
unsafe fn mul5_avx2(
    b0a: __m256i,
    b1a: __m256i,
    b2a: __m256i,
    b0b: __m256i,
    b1b: __m256i,
    b2b: __m256i,
) -> (__m256i, __m256i, __m256i) {
    // SAFETY: called only from #[target_feature(enable = "avx2")] paths.
    let ea = decode5_avx2(b0a, b1a, b2a);
    let eb = decode5_avx2(b0b, b1b, b2b);
    let r1 = _mm256_or_si256(
        _mm256_or_si256(
            _mm256_and_si256(ea[1], eb[1]),
            _mm256_and_si256(ea[2], eb[3]),
        ),
        _mm256_or_si256(
            _mm256_and_si256(ea[3], eb[2]),
            _mm256_and_si256(ea[4], eb[4]),
        ),
    );
    let r2 = _mm256_or_si256(
        _mm256_or_si256(
            _mm256_and_si256(ea[1], eb[2]),
            _mm256_and_si256(ea[2], eb[1]),
        ),
        _mm256_or_si256(
            _mm256_and_si256(ea[3], eb[4]),
            _mm256_and_si256(ea[4], eb[3]),
        ),
    );
    let r3 = _mm256_or_si256(
        _mm256_or_si256(
            _mm256_and_si256(ea[1], eb[3]),
            _mm256_and_si256(ea[2], eb[4]),
        ),
        _mm256_or_si256(
            _mm256_and_si256(ea[3], eb[1]),
            _mm256_and_si256(ea[4], eb[2]),
        ),
    );
    let r4 = _mm256_or_si256(
        _mm256_or_si256(
            _mm256_and_si256(ea[1], eb[4]),
            _mm256_and_si256(ea[2], eb[2]),
        ),
        _mm256_or_si256(
            _mm256_and_si256(ea[3], eb[3]),
            _mm256_and_si256(ea[4], eb[1]),
        ),
    );
    let zero = _mm256_setzero_si256();
    encode5_avx2([zero, r1, r2, r3, r4])
}

/// F_5 neg on AVX2 lanes — remap `(e0,e1,e2,e3,e4) -> (e0,e4,e3,e2,e1)`.
///
/// # Safety
///
/// Caller must be executing within an AVX2 `#[target_feature]` function.
#[inline(always)]
unsafe fn neg5_avx2(b0: __m256i, b1: __m256i, b2: __m256i) -> (__m256i, __m256i, __m256i) {
    // SAFETY: called only from #[target_feature(enable = "avx2")] paths.
    let e = decode5_avx2(b0, b1, b2);
    encode5_avx2([e[0], e[4], e[3], e[2], e[1]])
}

// ---------------------------------------------------------------------------
// Load / store helpers
// ---------------------------------------------------------------------------

#[inline(always)]
unsafe fn load256(src: &[u64], offset: usize) -> __m256i {
    // SAFETY: caller ensures offset + 4 <= src.len() and AVX2 available.
    _mm256_loadu_si256(src.as_ptr().add(offset) as *const __m256i)
}

#[inline(always)]
unsafe fn store256(dst: &mut [u64], offset: usize, v: __m256i) {
    // SAFETY: caller ensures offset + 4 <= dst.len() and AVX2 available.
    _mm256_storeu_si256(dst.as_mut_ptr().add(offset) as *mut __m256i, v);
}

// ---------------------------------------------------------------------------
// Public batch entry points
// ---------------------------------------------------------------------------

/// Apply F_5 add over 3-plane `(b0a,b1a,b2a) + (b0b,b1b,b2b)` streams via AVX2.
///
/// Each AVX2 lane covers 4 u64 words (= 256 F_5 lanes). All nine slices must
/// have the same length `n` where `n % 4 == 0`. Empty input is allowed (no-op).
///
/// # Arguments
///
/// * `b0a, b1a, b2a` — first operand plane slices.
/// * `b0b, b1b, b2b` — second operand plane slices.
/// * `out_b0, out_b1, out_b2` — output plane slices.
///
/// # Safety
///
/// AVX2 must be available at runtime. All nine slices share the same length,
/// which must be divisible by 4. Behaviour is undefined otherwise.
///
/// # Complexity
///
/// `O(n / 4)` AVX2 ops.
#[inline]
#[allow(clippy::too_many_arguments)]
#[target_feature(enable = "avx2")]
pub unsafe fn run_add5_batch(
    b0a: &[u64],
    b1a: &[u64],
    b2a: &[u64],
    b0b: &[u64],
    b1b: &[u64],
    b2b: &[u64],
    out_b0: &mut [u64],
    out_b1: &mut [u64],
    out_b2: &mut [u64],
) {
    // SAFETY: AVX2 + bounds + multiple-of-4 are the caller's preconditions.
    debug_assert_eq!(b0a.len() % 4, 0);
    debug_assert_eq!(b0a.len(), b1a.len());
    debug_assert_eq!(b0a.len(), b2a.len());
    debug_assert_eq!(b0a.len(), b0b.len());
    debug_assert_eq!(b0a.len(), b1b.len());
    debug_assert_eq!(b0a.len(), b2b.len());
    debug_assert_eq!(b0a.len(), out_b0.len());
    debug_assert_eq!(b0a.len(), out_b1.len());
    debug_assert_eq!(b0a.len(), out_b2.len());
    let n = b0a.len();
    let mut i = 0usize;
    while i < n {
        let v_b0a = load256(b0a, i);
        let v_b1a = load256(b1a, i);
        let v_b2a = load256(b2a, i);
        let v_b0b = load256(b0b, i);
        let v_b1b = load256(b1b, i);
        let v_b2b = load256(b2b, i);
        let (c0, c1, c2) = add5_avx2(v_b0a, v_b1a, v_b2a, v_b0b, v_b1b, v_b2b);
        store256(out_b0, i, c0);
        store256(out_b1, i, c1);
        store256(out_b2, i, c2);
        i += 4;
    }
}

/// Apply F_5 sub over 3-plane streams via AVX2.
///
/// See [`run_add5_batch`] for the slice-shape contract.
///
/// # Safety
///
/// AVX2 must be available at runtime. All nine slices share the same length
/// divisible by 4.
///
/// # Complexity
///
/// `O(n / 4)` AVX2 ops.
#[inline]
#[allow(clippy::too_many_arguments)]
#[target_feature(enable = "avx2")]
pub unsafe fn run_sub5_batch(
    b0a: &[u64],
    b1a: &[u64],
    b2a: &[u64],
    b0b: &[u64],
    b1b: &[u64],
    b2b: &[u64],
    out_b0: &mut [u64],
    out_b1: &mut [u64],
    out_b2: &mut [u64],
) {
    // SAFETY: AVX2 + bounds + multiple-of-4 are the caller's preconditions.
    debug_assert_eq!(b0a.len() % 4, 0);
    debug_assert_eq!(b0a.len(), b1a.len());
    debug_assert_eq!(b0a.len(), b2a.len());
    debug_assert_eq!(b0a.len(), b0b.len());
    debug_assert_eq!(b0a.len(), b1b.len());
    debug_assert_eq!(b0a.len(), b2b.len());
    debug_assert_eq!(b0a.len(), out_b0.len());
    debug_assert_eq!(b0a.len(), out_b1.len());
    debug_assert_eq!(b0a.len(), out_b2.len());
    let n = b0a.len();
    let mut i = 0usize;
    while i < n {
        let v_b0a = load256(b0a, i);
        let v_b1a = load256(b1a, i);
        let v_b2a = load256(b2a, i);
        let v_b0b = load256(b0b, i);
        let v_b1b = load256(b1b, i);
        let v_b2b = load256(b2b, i);
        let (c0, c1, c2) = sub5_avx2(v_b0a, v_b1a, v_b2a, v_b0b, v_b1b, v_b2b);
        store256(out_b0, i, c0);
        store256(out_b1, i, c1);
        store256(out_b2, i, c2);
        i += 4;
    }
}

/// Apply F_5 mul over 3-plane streams via AVX2.
///
/// See [`run_add5_batch`] for the slice-shape contract.
///
/// # Safety
///
/// AVX2 must be available at runtime. All nine slices share the same length
/// divisible by 4.
///
/// # Complexity
///
/// `O(n / 4)` AVX2 ops.
#[inline]
#[allow(clippy::too_many_arguments)]
#[target_feature(enable = "avx2")]
pub unsafe fn run_mul5_batch(
    b0a: &[u64],
    b1a: &[u64],
    b2a: &[u64],
    b0b: &[u64],
    b1b: &[u64],
    b2b: &[u64],
    out_b0: &mut [u64],
    out_b1: &mut [u64],
    out_b2: &mut [u64],
) {
    // SAFETY: AVX2 + bounds + multiple-of-4 are the caller's preconditions.
    debug_assert_eq!(b0a.len() % 4, 0);
    debug_assert_eq!(b0a.len(), b1a.len());
    debug_assert_eq!(b0a.len(), b2a.len());
    debug_assert_eq!(b0a.len(), b0b.len());
    debug_assert_eq!(b0a.len(), b1b.len());
    debug_assert_eq!(b0a.len(), b2b.len());
    debug_assert_eq!(b0a.len(), out_b0.len());
    debug_assert_eq!(b0a.len(), out_b1.len());
    debug_assert_eq!(b0a.len(), out_b2.len());
    let n = b0a.len();
    let mut i = 0usize;
    while i < n {
        let v_b0a = load256(b0a, i);
        let v_b1a = load256(b1a, i);
        let v_b2a = load256(b2a, i);
        let v_b0b = load256(b0b, i);
        let v_b1b = load256(b1b, i);
        let v_b2b = load256(b2b, i);
        let (c0, c1, c2) = mul5_avx2(v_b0a, v_b1a, v_b2a, v_b0b, v_b1b, v_b2b);
        store256(out_b0, i, c0);
        store256(out_b1, i, c1);
        store256(out_b2, i, c2);
        i += 4;
    }
}

/// Apply F_5 neg over 3-plane streams via AVX2.
///
/// All six slices (`b0, b1, b2, out_b0, out_b1, out_b2`) must have the same
/// length `n` where `n % 4 == 0`.
///
/// # Safety
///
/// AVX2 must be available at runtime. All six slices share the same length
/// divisible by 4.
///
/// # Complexity
///
/// `O(n / 4)` AVX2 ops.
#[inline]
#[target_feature(enable = "avx2")]
pub unsafe fn run_neg5_batch(
    b0: &[u64],
    b1: &[u64],
    b2: &[u64],
    out_b0: &mut [u64],
    out_b1: &mut [u64],
    out_b2: &mut [u64],
) {
    // SAFETY: AVX2 + bounds + multiple-of-4 are the caller's preconditions.
    debug_assert_eq!(b0.len() % 4, 0);
    debug_assert_eq!(b0.len(), b1.len());
    debug_assert_eq!(b0.len(), b2.len());
    debug_assert_eq!(b0.len(), out_b0.len());
    debug_assert_eq!(b0.len(), out_b1.len());
    debug_assert_eq!(b0.len(), out_b2.len());
    let n = b0.len();
    let mut i = 0usize;
    while i < n {
        let v_b0 = load256(b0, i);
        let v_b1 = load256(b1, i);
        let v_b2 = load256(b2, i);
        let (c0, c1, c2) = neg5_avx2(v_b0, v_b1, v_b2);
        store256(out_b0, i, c0);
        store256(out_b1, i, c1);
        store256(out_b2, i, c2);
        i += 4;
    }
}

// AVX-512 paths are deferred (aspirational criterion). A
// `#[cfg(target_feature = "avx512f")]` block would replace decode + cross-product
// ORs with `_mm512_ternarylogic_epi64` (vpternlogd) to reduce instruction count.
// Deferred per issue 1f769232 aspirational note.
