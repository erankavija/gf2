//! SIMD batch kernel for the 4×4 schoolbook carry-less multiplication that
//! powers `Gf2mWide<4>` (GF(2^256)).
//!
//! The kernel produces only the **unreduced** 8-limb (512-bit) carry-less
//! product. Barrett reduction back to 4 limbs is performed by the caller in
//! `gf2-core::gf2m::wide::Gf2mWide::mul_ref`, so the dispatched function has
//! a simple (`&[u64; 4]`, `&[u64; 4]`, `&mut [u64; 8]`) signature.
//!
//! Unsafe intrinsics are isolated in `x86/gf2m_wide.rs`; this module only
//! exposes safe function-pointer wrappers through the [`ClmulWide256Fns`]
//! table returned by [`detect`]. Callers without PCLMULQDQ receive `None`
//! and must fall back to the scalar schoolbook in `gf2-core`.
//!
//! # Lane preference
//!
//! [`detect`] returns the fastest available lane:
//!
//! 1. **AVX2 + VPCLMULQDQ** (YMM, 256-bit) — 2 clmuls per instruction,
//!    16-product schoolbook in 8 instructions. Primary path on Zen 3.
//! 2. **PCLMULQDQ** (XMM, 128-bit) — 1 clmul per instruction, 16-product
//!    schoolbook in 16 instructions. Universal x86_64 fallback.
//! 3. `None` — callers fall back to pure-Rust `clmul_wide` in `gf2-core`.
//!
//! A ZMM (AVX-512VL + VPCLMULQDQ) lane is out of scope until the project's
//! MSRV moves to Rust ≥ 1.89, when the required 512-bit VPCLMULQDQ and
//! 128-bit-lane extraction intrinsics become stable.

/// Kernel signature: computes the 8-limb carry-less product of two 4-limb
/// GF(2)-polynomial operands.
///
/// The function pointer is safe (`fn`, not `unsafe fn`): the safe wrappers
/// in this module guard the `#[target_feature]` intrinsics. [`detect`] only
/// publishes a function pointer when the CPU supports the corresponding
/// feature, so calling a pointer obtained from a populated
/// [`ClmulWide256Fns`] always upholds the feature precondition.
pub type ClmulWide256Fn = fn(&[u64; 4], &[u64; 4], &mut [u64; 8]);

/// Bundle of dispatched carry-less multiply kernels for 4-limb operands.
#[derive(Copy, Clone)]
pub struct ClmulWide256Fns {
    /// 4×4 schoolbook carry-less multiply, writing the full 8-limb product.
    pub clmul: ClmulWide256Fn,
    /// Human-readable tag of the chosen lane: one of
    /// `"avx2+vpclmulqdq-ymm"`, `"pclmulqdq-scalar-xmm"`.
    pub name: &'static str,
}

/// Detect and return the best available 4-limb carry-less multiply kernel.
///
/// Returns `None` on non-x86 targets, or when the runtime CPU lacks
/// PCLMULQDQ entirely. The preference order matches the module-level
/// documentation.
pub fn detect() -> Option<ClmulWide256Fns> {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        return detect_x86();
    }
    #[allow(unreachable_code)]
    None
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn detect_x86() -> Option<ClmulWide256Fns> {
    use std::arch::is_x86_feature_detected;

    // 1. YMM (AVX2 + VPCLMULQDQ) — primary path on Zen 3.
    if is_x86_feature_detected!("avx2") && is_x86_feature_detected!("vpclmulqdq") {
        return Some(ClmulWide256Fns {
            clmul: clmul_wide4_ymm_safe,
            name: "avx2+vpclmulqdq-ymm",
        });
    }

    // 2. XMM (PCLMULQDQ scalar-lane) — universal x86_64 fallback.
    if is_x86_feature_detected!("pclmulqdq") {
        return Some(ClmulWide256Fns {
            clmul: clmul_wide4_xmm_safe,
            name: "pclmulqdq-scalar-xmm",
        });
    }

    None
}

// ---------------------------------------------------------------------------
// Safe function-pointer wrappers (unsafe isolated in `crate::x86::gf2m_wide`)
// ---------------------------------------------------------------------------
//
// `detect_x86` only publishes these function pointers when the corresponding
// feature is detected at runtime. Callers that bypass `detect` must uphold
// the feature precondition themselves.

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn clmul_wide4_xmm_safe(a: &[u64; 4], b: &[u64; 4], out: &mut [u64; 8]) {
    // SAFETY: `detect_x86` only returns this pointer when PCLMULQDQ is
    // available. Callers who bypass `detect` must uphold that precondition.
    unsafe { crate::x86::gf2m_wide::clmul_wide4_xmm(a, b, out) }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn clmul_wide4_ymm_safe(a: &[u64; 4], b: &[u64; 4], out: &mut [u64; 8]) {
    // SAFETY: `detect_x86` only returns this pointer when AVX2 and
    // VPCLMULQDQ are available.
    unsafe { crate::x86::gf2m_wide::clmul_wide4_ymm(a, b, out) }
}

/// Test-only helpers shared between this module's tests and the inner-x86
/// kernel tests. Keeping `scalar_ref` here enforces SSOT: there is exactly
/// one bit-by-bit reference implementation of the 4×4 carry-less schoolbook.
#[cfg(test)]
pub(crate) mod test_helpers {
    /// Scalar reference matching `gf2_core::gf2m::wide::clmul_wide_slice::<4>`:
    /// the 4×4 schoolbook built on a bit-by-bit carry-less 64×64 multiply.
    pub(crate) fn scalar_ref(a: &[u64; 4], b: &[u64; 4]) -> [u64; 8] {
        fn clmul_u64(a: u64, b: u64) -> u128 {
            let a = a as u128;
            let mut r: u128 = 0;
            let mut br = b;
            while br != 0 {
                let bit = br.trailing_zeros();
                r ^= a << bit;
                br &= br - 1;
            }
            r
        }
        let mut out = [0u64; 8];
        for i in 0..4 {
            for j in 0..4 {
                let p = clmul_u64(a[i], b[j]);
                out[i + j] ^= p as u64;
                out[i + j + 1] ^= (p >> 64) as u64;
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::test_helpers::scalar_ref;
    use super::*;

    #[test]
    fn detect_returns_some_when_pclmulqdq_available() {
        let fns = detect();
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        {
            use std::arch::is_x86_feature_detected;
            if is_x86_feature_detected!("pclmulqdq") {
                let fns = fns.expect("expected PCLMULQDQ-backed kernel on this host");
                // Name must reflect the lane.
                assert!(
                    fns.name == "pclmulqdq-scalar-xmm" || fns.name == "avx2+vpclmulqdq-ymm",
                    "unexpected kernel name: {}",
                    fns.name
                );
            }
        }
    }

    #[test]
    fn dispatched_kernel_matches_scalar() {
        let Some(fns) = detect() else {
            return; // no SIMD on this host
        };
        let cases: [([u64; 4], [u64; 4]); 4] = [
            ([0, 0, 0, 0], [1, 2, 3, 4]),
            ([1, 1, 1, 1], [1, 1, 1, 1]),
            (
                [
                    0xDEAD_BEEF_CAFE_BABE,
                    0x0123_4567_89AB_CDEF,
                    0xFEDC_BA98_7654_3210,
                    0xAAAA_5555_AAAA_5555,
                ],
                [
                    0x5555_AAAA_5555_AAAA,
                    0x1122_3344_5566_7788,
                    0xFFFF_FFFF_0000_0000,
                    0x0F0F_F0F0_0F0F_F0F0,
                ],
            ),
            (
                [u64::MAX, u64::MAX, u64::MAX, u64::MAX],
                [u64::MAX, u64::MAX, u64::MAX, u64::MAX],
            ),
        ];
        for (a, b) in cases {
            let mut got = [0u64; 8];
            (fns.clmul)(&a, &b, &mut got);
            let expected = scalar_ref(&a, &b);
            assert_eq!(got, expected, "{} mismatch for a={a:?}, b={b:?}", fns.name);
        }
    }
}
