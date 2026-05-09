//! Intrinsic feasibility stub for JIT issue `4c534d31` (D4).
//!
//! This stand-alone crate exists to prove that the AVX2 and AVX-512 intrinsics
//! the bipedal SIMD kernels need (lane-wise AND, XOR, OR over `__m256i` /
//! `__m512i`, plus unaligned load/store) are stable on MSRV 1.95.0.
//!
//! Verification command (from this crate's directory):
//! ```text
//! rustup run 1.95.0 cargo check --release
//! RUSTFLAGS="-C target-feature=+avx2,+avx512f" rustup run 1.95.0 cargo check --release
//! ```
//!
//! See `dev/plans/d4_intrinsic_feasibility.md` for the design rationale and
//! the prior `afac2262` lesson cited from `CLAUDE.md`.

#![allow(clippy::missing_safety_doc, unused_unsafe)]

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub mod avx2_stub {
    //! AVX2 lane-wise logical primitives over 256-bit lanes (4 x u64).
    //!
    //! These mirror the `Bipedal3` add/sub/mul/div formulas (paper §2.2):
    //! everything is bitwise AND, XOR, OR over wide registers.

    #[cfg(target_arch = "x86")]
    use core::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
    use core::arch::x86_64::*;

    /// Unaligned load of a 256-bit lane from `src[offset..]`.
    #[inline]
    #[target_feature(enable = "avx2")]
    pub unsafe fn loadu(src: &[u64], offset: usize) -> __m256i {
        debug_assert!(offset + 4 <= src.len());
        // SAFETY: caller-checked bounds; AVX2 is a runtime precondition of the
        // `target_feature` attribute.
        unsafe { _mm256_loadu_si256(src.as_ptr().add(offset) as *const __m256i) }
    }

    /// Unaligned store of a 256-bit lane to `dst[offset..]`.
    #[inline]
    #[target_feature(enable = "avx2")]
    pub unsafe fn storeu(dst: &mut [u64], offset: usize, v: __m256i) {
        // SAFETY: caller must ensure offset + 4 <= dst.len() (debug-asserted)
        // and that AVX2 is available (precondition of `target_feature`).
        debug_assert!(offset + 4 <= dst.len());
        unsafe { _mm256_storeu_si256(dst.as_mut_ptr().add(offset) as *mut __m256i, v) }
    }

    /// Bipedal multiplication mag: `m_x = m1 & m2` (paper §2.2, mul, 1 of 2 ops).
    #[inline]
    #[target_feature(enable = "avx2")]
    pub unsafe fn bipedal_and(a: __m256i, b: __m256i) -> __m256i {
        // SAFETY: caller must ensure AVX2 is available (precondition of `target_feature`).
        unsafe { _mm256_and_si256(a, b) }
    }

    /// Bipedal multiplication sgn: `s_x = s1 ^ s2` (paper §2.2, mul, 2 of 2 ops).
    /// Same primitive used in add/sub for `t = m1 ^ s1 ^ s2` etc.
    #[inline]
    #[target_feature(enable = "avx2")]
    pub unsafe fn bipedal_xor(a: __m256i, b: __m256i) -> __m256i {
        // SAFETY: caller must ensure AVX2 is available (precondition of `target_feature`).
        unsafe { _mm256_xor_si256(a, b) }
    }

    /// Bipedal addition mag finishing OR: `m_+ = u | (m1 ^ m2)` (paper §2.2).
    #[inline]
    #[target_feature(enable = "avx2")]
    pub unsafe fn bipedal_or(a: __m256i, b: __m256i) -> __m256i {
        // SAFETY: caller must ensure AVX2 is available (precondition of `target_feature`).
        unsafe { _mm256_or_si256(a, b) }
    }

    /// Full bipedal F_3 add over a 256-bit lane (paper §2.2, 6 ops with CSE).
    ///
    /// Inputs `(m1, s1)` and `(m2, s2)` each occupy a `__m256i`. Returns
    /// `(m_plus, s_plus)`.
    #[inline]
    #[target_feature(enable = "avx2")]
    pub unsafe fn bipedal_add(
        m1: __m256i,
        s1: __m256i,
        m2: __m256i,
        s2: __m256i,
    ) -> (__m256i, __m256i) {
        // SAFETY: caller must ensure AVX2 is available (precondition of `target_feature`).
        unsafe {
            let t = _mm256_xor_si256(_mm256_xor_si256(m1, s1), s2);
            let u = _mm256_and_si256(m2, t);
            let m_plus = _mm256_or_si256(u, _mm256_xor_si256(m1, m2));
            let s_plus = _mm256_xor_si256(u, s1);
            (m_plus, s_plus)
        }
    }

    /// Drive every required AVX2 intrinsic with realistic inputs and write the
    /// result to caller-owned slices, defeating dead-code elimination.
    ///
    /// Returns `false` if `mag1`, `sgn1`, `mag2`, `sgn2`, `out_mag`, `out_sgn`
    /// do not all share a common length that is a multiple of 4 u64 lanes.
    pub fn drive_avx2(
        mag1: &[u64],
        sgn1: &[u64],
        mag2: &[u64],
        sgn2: &[u64],
        out_mag: &mut [u64],
        out_sgn: &mut [u64],
    ) -> bool {
        let n = mag1.len();
        if sgn1.len() != n
            || mag2.len() != n
            || sgn2.len() != n
            || out_mag.len() != n
            || out_sgn.len() != n
            || n % 4 != 0
        {
            return false;
        }
        if !is_x86_feature_detected!("avx2") {
            return false;
        }
        // SAFETY: AVX2 just verified at runtime; bounds + multiple-of-4 verified above.
        unsafe {
            let mut i = 0;
            while i < n {
                let v_m1 = loadu(mag1, i);
                let v_s1 = loadu(sgn1, i);
                let v_m2 = loadu(mag2, i);
                let v_s2 = loadu(sgn2, i);
                let (m_plus, s_plus) = bipedal_add(v_m1, v_s1, v_m2, v_s2);
                // Touch every individually-named primitive too, so the
                // feasibility coverage is not hidden behind composite functions.
                let _and = bipedal_and(v_m1, v_m2);
                let _xor = bipedal_xor(v_s1, v_s2);
                let _or = bipedal_or(_and, _xor);
                storeu(out_mag, i, m_plus);
                storeu(out_sgn, i, s_plus);
                i += 4;
            }
        }
        true
    }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub mod avx512_stub {
    //! AVX-512F lane-wise logical primitives over 512-bit lanes (8 x u64).
    //!
    //! Coded for portability and verified to compile on MSRV 1.95.0; runtime
    //! execution is gated on `is_x86_feature_detected!("avx512f")`. The dev
    //! host (Ryzen 9 5900X / Zen 3) lacks AVX-512, so this code path is
    //! `[aspirational]` per the project plan §4 hardware envelope.

    #[cfg(target_arch = "x86")]
    use core::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
    use core::arch::x86_64::*;

    /// Unaligned load of a 512-bit lane from `src[offset..]`.
    #[inline]
    #[target_feature(enable = "avx512f")]
    pub unsafe fn loadu(src: &[u64], offset: usize) -> __m512i {
        // SAFETY: caller must ensure offset + 8 <= src.len() (debug-asserted)
        // and that AVX-512F is available (precondition of `target_feature`).
        debug_assert!(offset + 8 <= src.len());
        unsafe { _mm512_loadu_si512(src.as_ptr().add(offset) as *const __m512i) }
    }

    /// Unaligned store of a 512-bit lane to `dst[offset..]`.
    #[inline]
    #[target_feature(enable = "avx512f")]
    pub unsafe fn storeu(dst: &mut [u64], offset: usize, v: __m512i) {
        // SAFETY: caller must ensure offset + 8 <= dst.len() (debug-asserted)
        // and that AVX-512F is available (precondition of `target_feature`).
        debug_assert!(offset + 8 <= dst.len());
        unsafe { _mm512_storeu_si512(dst.as_mut_ptr().add(offset) as *mut __m512i, v) }
    }

    /// Bipedal multiplication mag (8 x u64 lane): `m_x = m1 & m2`.
    #[inline]
    #[target_feature(enable = "avx512f")]
    pub unsafe fn bipedal_and(a: __m512i, b: __m512i) -> __m512i {
        // SAFETY: caller must ensure AVX-512F is available (precondition of `target_feature`).
        unsafe { _mm512_and_si512(a, b) }
    }

    /// Bipedal XOR (8 x u64 lane).
    #[inline]
    #[target_feature(enable = "avx512f")]
    pub unsafe fn bipedal_xor(a: __m512i, b: __m512i) -> __m512i {
        // SAFETY: caller must ensure AVX-512F is available (precondition of `target_feature`).
        unsafe { _mm512_xor_si512(a, b) }
    }

    /// Bipedal OR (8 x u64 lane).
    #[inline]
    #[target_feature(enable = "avx512f")]
    pub unsafe fn bipedal_or(a: __m512i, b: __m512i) -> __m512i {
        // SAFETY: caller must ensure AVX-512F is available (precondition of `target_feature`).
        unsafe { _mm512_or_si512(a, b) }
    }

    /// Full bipedal F_3 add over a 512-bit lane.
    #[inline]
    #[target_feature(enable = "avx512f")]
    pub unsafe fn bipedal_add(
        m1: __m512i,
        s1: __m512i,
        m2: __m512i,
        s2: __m512i,
    ) -> (__m512i, __m512i) {
        // SAFETY: caller must ensure AVX-512F is available (precondition of `target_feature`).
        unsafe {
            let t = _mm512_xor_si512(_mm512_xor_si512(m1, s1), s2);
            let u = _mm512_and_si512(m2, t);
            let m_plus = _mm512_or_si512(u, _mm512_xor_si512(m1, m2));
            let s_plus = _mm512_xor_si512(u, s1);
            (m_plus, s_plus)
        }
    }

    /// AVX-512 driver, mirroring `avx2_stub::drive_avx2`. Returns `false` if
    /// AVX-512F is unavailable at runtime; this is the [aspirational] path.
    pub fn drive_avx512(
        mag1: &[u64],
        sgn1: &[u64],
        mag2: &[u64],
        sgn2: &[u64],
        out_mag: &mut [u64],
        out_sgn: &mut [u64],
    ) -> bool {
        let n = mag1.len();
        if sgn1.len() != n
            || mag2.len() != n
            || sgn2.len() != n
            || out_mag.len() != n
            || out_sgn.len() != n
            || n % 8 != 0
        {
            return false;
        }
        if !is_x86_feature_detected!("avx512f") {
            return false;
        }
        // SAFETY: AVX512F just verified at runtime; bounds + multiple-of-8 verified above.
        unsafe {
            let mut i = 0;
            while i < n {
                let v_m1 = loadu(mag1, i);
                let v_s1 = loadu(sgn1, i);
                let v_m2 = loadu(mag2, i);
                let v_s2 = loadu(sgn2, i);
                let (m_plus, s_plus) = bipedal_add(v_m1, v_s1, v_m2, v_s2);
                let _and = bipedal_and(v_m1, v_m2);
                let _xor = bipedal_xor(v_s1, v_s2);
                let _or = bipedal_or(_and, _xor);
                storeu(out_mag, i, m_plus);
                storeu(out_sgn, i, s_plus);
                i += 8;
            }
        }
        true
    }
}

/// Public driver. On non-x86 targets this stub is a no-op: it compiles, but
/// reports back `false` so callers can detect the unsupported architecture.
#[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
pub fn run_all() -> (bool, bool) {
    (false, false)
}

/// Public driver. Calls each stub once with deterministic inputs and reports
/// `(avx2_ran, avx512_ran)`.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub fn run_all() -> (bool, bool) {
    let mag1: Vec<u64> = (0..64).map(|i| 0x5555_5555_5555_5555 ^ i).collect();
    let sgn1: Vec<u64> = (0..64).map(|i| 0xAAAA_AAAA_AAAA_AAAA ^ i).collect();
    let mag2: Vec<u64> = (0..64).map(|i| 0xF0F0_F0F0_F0F0_F0F0 ^ i).collect();
    let sgn2: Vec<u64> = (0..64).map(|i| 0x0F0F_0F0F_0F0F_0F0F ^ i).collect();
    let mut out_mag = vec![0u64; 64];
    let mut out_sgn = vec![0u64; 64];

    let avx2_ok = avx2_stub::drive_avx2(&mag1, &sgn1, &mag2, &sgn2, &mut out_mag, &mut out_sgn);
    // Mix the AVX2 outputs back in so they cannot be DCE'd before the AVX-512 call.
    let mag1b = out_mag.clone();
    let sgn1b = out_sgn.clone();
    let avx512_ok =
        avx512_stub::drive_avx512(&mag1b, &sgn1b, &mag2, &sgn2, &mut out_mag, &mut out_sgn);

    // Force a final use so nothing collapses.
    let sink: u64 = out_mag.iter().chain(out_sgn.iter()).fold(0u64, |a, x| a ^ x);
    std::hint::black_box(sink);
    (avx2_ok, avx512_ok)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_all_compiles_and_executes() {
        let (avx2_ok, avx512_ok) = run_all();
        // We do not assert these are true: this stub merely needs to compile
        // on MSRV 1.95.0. On the dev host (5900X) avx2_ok=true, avx512_ok=false.
        let _ = (avx2_ok, avx512_ok);
    }
}
