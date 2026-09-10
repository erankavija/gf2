//! SIMD kernels for fixed-size multi-word carry-less multiplication.
//!
//! This module exposes dispatch for the 4×4 schoolbook multiply powering
//! `Gf2mWide<4>` (GF(2^256)) and the 9×9 multiply powering
//! `Gf2mWide<9>` at m=571.
//!
//! The kernels produce only the **unreduced** carry-less product. Barrett
//! reduction is performed by the caller in
//! `gf2-core::gf2m::wide::Gf2mWide::mul_ref`, so the dispatched functions keep
//! simple fixed-array signatures.
//!
//! Unsafe intrinsics are isolated in `x86/gf2m_wide.rs`; this module only
//! exposes safe function-pointer wrappers through the [`ClmulWide256Fns`]
//! table returned by [`detect`]. Callers without PCLMULQDQ receive `None`
//! and must fall back to the scalar schoolbook in `gf2-core`.
//!
//! # Lane preference
//!
//! [`detect_wide`] returns the fastest available lane:
//!
//! 1. **AVX2 + VPCLMULQDQ** (YMM, 256-bit) — 2 clmuls per instruction,
//!    16-product 4×4 schoolbook in 8 instructions and 81-product 9×9
//!    schoolbook in 41 instructions. Primary path on Zen 3.
//! 2. **PCLMULQDQ + SSE4.1** (XMM, 128-bit) — 1 clmul per instruction,
//!    one instruction per scalar word product. Universal x86_64 fallback.
//! 3. `None` — callers fall back to pure-Rust `clmul_wide` in `gf2-core`.
//!
//! A ZMM (AVX-512VL + VPCLMULQDQ) lane is out of scope while the test host
//! is AVX2-only (Zen 3); the required `_mm512_*` carry-less-multiply and
//! 128-bit-lane extraction intrinsics are stable since Rust 1.89, available
//! under the current MSRV (1.95). Add the lane when AVX-512 hardware is in
//! scope.

/// Kernel signature: computes the 8-limb carry-less product of two 4-limb
/// GF(2)-polynomial operands.
///
/// The function pointer is safe (`fn`, not `unsafe fn`): the safe wrappers
/// in this module guard the `#[target_feature]` intrinsics. [`detect`] only
/// publishes a function pointer when the CPU supports the corresponding
/// feature, so calling a pointer obtained from a populated
/// [`ClmulWide256Fns`] always upholds the feature precondition.
pub type ClmulWide256Fn = fn(&[u64; 4], &[u64; 4], &mut [u64; 8]);

/// Kernel signature: computes the 18-limb carry-less product of two 9-limb
/// GF(2)-polynomial operands.
pub type ClmulWide571Fn = fn(&[u64; 9], &[u64; 9], &mut [u64; 18]);

/// Bundle of dispatched carry-less multiply kernels for 4-limb operands.
#[derive(Copy, Clone)]
pub struct ClmulWide256Fns {
    /// 4×4 schoolbook carry-less multiply, writing the full 8-limb product.
    pub clmul: ClmulWide256Fn,
    /// Human-readable tag of the chosen lane: one of
    /// `"avx2+vpclmulqdq-ymm"`, `"pclmulqdq-scalar-xmm"`.
    pub name: &'static str,
}

/// Bundle of dispatched carry-less multiply kernels for 9-limb GF(2^571)
/// operands.
#[derive(Copy, Clone)]
pub struct ClmulWide571Fns {
    /// 9×9 schoolbook carry-less multiply, writing the full 18-limb product.
    pub clmul: ClmulWide571Fn,
    /// Human-readable tag of the chosen lane: one of
    /// `"avx2+vpclmulqdq-ymm"`, `"pclmulqdq-scalar-xmm"`.
    pub name: &'static str,
}

/// Bundle of all dispatched fixed-size wide GF(2^m) kernels.
#[derive(Copy, Clone)]
pub struct Gf2mWideFns {
    /// GF(2^256) / 4-limb clmul kernel.
    pub wide256: ClmulWide256Fns,
    /// GF(2^571) / 9-limb clmul kernel.
    pub wide571: ClmulWide571Fns,
    /// Human-readable tag of the chosen lane.
    pub name: &'static str,
}

/// Detect and return all available fixed-size wide carry-less multiply kernels.
///
/// Returns `None` on non-x86 targets, or when the runtime CPU lacks
/// PCLMULQDQ/SSE4.1 entirely. The preference order matches the module-level
/// documentation.
pub fn detect_wide() -> Option<Gf2mWideFns> {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        return detect_x86_wide();
    }
    #[allow(unreachable_code)]
    None
}

/// Detect and return the best available 4-limb carry-less multiply kernel.
///
/// Kept for compatibility with existing GF(2^256) callers. New wide-field
/// callers should use [`detect_wide`] so m=571 dispatch is available too.
pub fn detect() -> Option<ClmulWide256Fns> {
    detect_wide().map(|fns| fns.wide256)
}

/// Detect and return the best available 9-limb GF(2^571) carry-less multiply
/// kernel.
pub fn detect_571() -> Option<ClmulWide571Fns> {
    detect_wide().map(|fns| fns.wide571)
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn detect_x86_wide() -> Option<Gf2mWideFns> {
    use std::arch::is_x86_feature_detected;

    // 1. YMM (AVX2 + VPCLMULQDQ) — primary path on Zen 3.
    if is_x86_feature_detected!("avx2")
        && is_x86_feature_detected!("vpclmulqdq")
        && is_x86_feature_detected!("sse4.1")
    {
        return Some(Gf2mWideFns {
            wide256: ClmulWide256Fns {
                clmul: clmul_wide4_ymm_safe,
                name: "avx2+vpclmulqdq-ymm",
            },
            wide571: ClmulWide571Fns {
                clmul: clmul_wide9_ymm_safe,
                name: "avx2+vpclmulqdq-ymm",
            },
            name: "avx2+vpclmulqdq-ymm",
        });
    }

    // 2. XMM (PCLMULQDQ scalar-lane) — universal x86_64 fallback.
    if is_x86_feature_detected!("pclmulqdq") && is_x86_feature_detected!("sse4.1") {
        return Some(Gf2mWideFns {
            wide256: ClmulWide256Fns {
                clmul: clmul_wide4_xmm_safe,
                name: "pclmulqdq-scalar-xmm",
            },
            wide571: ClmulWide571Fns {
                clmul: clmul_wide9_xmm_safe,
                name: "pclmulqdq-scalar-xmm",
            },
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
    // SAFETY: `detect_x86` only returns this pointer when PCLMULQDQ and
    // SSE4.1 are available. Callers who bypass `detect` must uphold that
    // precondition.
    unsafe { crate::x86::gf2m_wide::clmul_wide4_xmm(a, b, out) }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn clmul_wide4_ymm_safe(a: &[u64; 4], b: &[u64; 4], out: &mut [u64; 8]) {
    // SAFETY: `detect_x86_wide` only returns this pointer when AVX2,
    // VPCLMULQDQ, and SSE4.1 are available.
    unsafe { crate::x86::gf2m_wide::clmul_wide4_ymm(a, b, out) }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn clmul_wide9_xmm_safe(a: &[u64; 9], b: &[u64; 9], out: &mut [u64; 18]) {
    // SAFETY: `detect_x86_wide` only returns this pointer when PCLMULQDQ and
    // SSE4.1 are available.
    unsafe { crate::x86::gf2m_wide::clmul_wide9_xmm(a, b, out) }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn clmul_wide9_ymm_safe(a: &[u64; 9], b: &[u64; 9], out: &mut [u64; 18]) {
    // SAFETY: `detect_x86_wide` only returns this pointer when AVX2,
    // VPCLMULQDQ, and SSE4.1 are available.
    unsafe { crate::x86::gf2m_wide::clmul_wide9_ymm(a, b, out) }
}

/// Test-only helpers shared between this module's tests and the inner-x86
/// kernel tests. Keeping `scalar_ref` here enforces SSOT: there is exactly
/// one bit-by-bit reference implementation of wide carry-less schoolbook.
#[cfg(test)]
pub(crate) mod test_helpers {
    /// Scalar reference matching `gf2_core::gf2m::wide::clmul_wide_slice::<N>`:
    /// the schoolbook built on the workspace-wide bit-by-bit carry-less
    /// 64×64 multiply SSOT (`crate::clmul_u64_scalar`).
    pub(crate) fn scalar_ref<const N: usize>(a: &[u64; N], b: &[u64; N]) -> Vec<u64> {
        let mut out = vec![0u64; 2 * N];
        for i in 0..N {
            for j in 0..N {
                let p = crate::clmul_u64_scalar(a[i], b[j]);
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
            if is_x86_feature_detected!("pclmulqdq") && is_x86_feature_detected!("sse4.1") {
                let fns = fns.expect("expected PCLMULQDQ-backed kernel on this host");
                // Name must reflect the lane.
                assert!(
                    fns.name == "pclmulqdq-scalar-xmm" || fns.name == "avx2+vpclmulqdq-ymm",
                    "unexpected kernel name: {}",
                    fns.name
                );
                let wide = detect_wide().expect("detect() implies detect_wide()");
                assert_eq!(wide.name, fns.name);
                let f571 = detect_571().expect("expected GF(2^571) kernel on this host");
                assert_eq!(f571.name, fns.name);
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
            assert_eq!(
                got.as_slice(),
                expected.as_slice(),
                "{} mismatch for a={a:?}, b={b:?}",
                fns.name
            );
        }
    }

    #[test]
    fn dispatched_wide571_kernel_matches_scalar() {
        let Some(fns) = detect_571() else {
            return; // no SIMD on this host
        };
        let cases: [([u64; 9], [u64; 9]); 3] = [
            ([0; 9], [1, 2, 3, 4, 5, 6, 7, 8, 9]),
            ([1; 9], [1; 9]),
            (
                [
                    0xDEAD_BEEF_CAFE_BABE,
                    0x0123_4567_89AB_CDEF,
                    0xFEDC_BA98_7654_3210,
                    0xAAAA_5555_AAAA_5555,
                    0x1357_9BDF_2468_ACE0,
                    0x0F0F_F0F0_3333_CCCC,
                    0xFFFF_0000_FFFF_0000,
                    0x1111_2222_3333_4444,
                    0x07FF_FFFF_FFFF_FFFF,
                ],
                [
                    0x5555_AAAA_5555_AAAA,
                    0x1122_3344_5566_7788,
                    0xFFFF_FFFF_0000_0000,
                    0x0F0F_F0F0_0F0F_F0F0,
                    0x2468_ACE0_1357_9BDF,
                    0x3333_CCCC_0F0F_F0F0,
                    0x0000_FFFF_0000_FFFF,
                    0x4444_3333_2222_1111,
                    0x03FF_FFFF_FFFF_FFFF,
                ],
            ),
        ];
        for (a, b) in cases {
            let mut got = [0u64; 18];
            (fns.clmul)(&a, &b, &mut got);
            let expected = scalar_ref(&a, &b);
            assert_eq!(
                got.as_slice(),
                expected.as_slice(),
                "{} mismatch for a={a:?}, b={b:?}",
                fns.name
            );
        }
    }
}
