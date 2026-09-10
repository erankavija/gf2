//! Accelerator interface for Gray square-QAM batch demapping kernels.
//!
//! The exact log-MAP and max-log demappers over a Gray-coded square-QAM
//! constellation factorize cleanly into two independent 1D Gray-PAM
//! problems (see the `modem::fast_gray_qam_demapper` module in
//! `gf2-coding` for the derivation). The dominant cost in that factorized
//! path is the per-symbol per-level squared-distance computation along
//! each axis — an arithmetic-bound inner loop over contiguous `f32` / `f64`
//! slices that maps cleanly onto SIMD lanes.
//!
//! This module exposes that inner loop as a backend-pluggable **kernel
//! bundle**: a small struct of function pointers with a scalar fallback
//! and (on x86 with AVX2) a vectorized implementation. The bundle shape
//! is deliberately minimal and allocation-free so future accelerator
//! backends — AVX-512, AArch64 NEON, or a GPU dispatch — drop in without
//! any API churn in the coding crate.
//!
//! # Research-grade accelerator surface
//!
//! `gf2` positions itself against specialized computer-algebra systems
//! (Magma, Sage) for coding-theory research, so the accelerator seam
//! lives in the public API: a researcher plugging in an experimental
//! kernel only needs to provide a matching `GrayPamDistanceFns*`
//! implementation and runtime detector. The bundle itself never
//! allocates, never panics, and never blocks — it reads contiguous
//! input slices and writes a contiguous output slice, nothing else.
//!
//! # Invariants imposed on every backend
//!
//! For a call `pam_sq_distances_fn(z, g, inv_n0_eq, pam_levels, out)`:
//!
//! * `z.len() == g.len() == inv_n0_eq.len() == num_symbols`.
//! * `pam_levels.len() == axis_len` where `axis_len ∈ {2, 4, 8, 16}`.
//! * `out.len() == num_symbols * axis_len`, laid out symbol-major:
//!   `out[s * axis_len + l] = (z[s] - g[s] * pam_levels[l])² * inv_n0_eq[s]`.
//! * When `inv_n0_eq[s] == 0.0` the entire `out[s*axis_len .. (s+1)*axis_len]`
//!   slice is written as zeros — this is the canonical zero-gain /
//!   infinite-noise guard and the numerical contract relied on by
//!   `gf2_coding::modem::FastGrayQamDemapper` to avoid NaN propagation.
//! * No allocation, no panic, no global state: the kernel is reentrant
//!   and safe to call from multiple threads simultaneously.
//!
//! These invariants are exercised by parity tests in this module and by
//! property tests in `gf2_coding::modem::fast_gray_qam_demapper`.

/// Signature of the `f32` Gray-PAM squared-distance kernel.
///
/// Extracted into a named alias so the kernel bundle fields stay
/// readable and every backend that implements the kernel names the
/// same type. All implementations must conform to the module-level
/// invariants (contiguous symbol-major output, zero-gain contract,
/// no allocation, no panic).
///
/// # Arguments
///
/// The function pointer is invoked as
/// `f(z, g, inv_n0_eq, pam_levels, out)` where:
///
/// * `z` — pre-rotated received samples on one axis, length `num_symbols`.
/// * `g` — per-symbol squared channel gain `|h|^2`, length `num_symbols`.
/// * `inv_n0_eq` — `1 / (n0 * |h|^2)` per symbol, or `0.0` for the
///   zero-gain guard. Length `num_symbols`.
/// * `pam_levels` — post-normalization Gray-PAM axis levels, length
///   `axis_len ∈ {2, 4, 8, 16}`.
/// * `out` — symbol-major distance slab, length `num_symbols * axis_len`.
pub type PamSqDistancesF32Fn =
    fn(z: &[f32], g: &[f32], inv_n0_eq: &[f32], pam_levels: &[f32], out: &mut [f32]);

/// Signature of the `f64` Gray-PAM squared-distance kernel.
///
/// See [`PamSqDistancesF32Fn`] for the detailed argument contract.
/// The double-precision variant is what `gf2_coding::modem` uses for
/// its internal scratch (to match the reference log-MAP path's
/// numerical behaviour), regardless of the user-facing scalar type.
pub type PamSqDistancesF64Fn =
    fn(z: &[f64], g: &[f64], inv_n0_eq: &[f64], pam_levels: &[f64], out: &mut [f64]);

/// Gray-PAM squared-distance kernel bundle for `f32` scratch.
///
/// Bundles the single hot-loop primitive of the Gray-QAM fast demap
/// path: compute per-level squared distances on one axis for an entire
/// batch of pre-rotated received samples.
///
/// The bundle is plain data (a function pointer) so it is trivially
/// `Copy`, thread-safe, and cheap to cache in a `OnceLock`. Scalar and
/// SIMD backends share the same signature, so the dispatch site in
/// `gf2-coding` selects at startup and then calls the chosen function
/// through the pointer with no further branching.
///
/// # Examples
///
/// ```
/// use gf2_kernels_simd::modem::{detect_f32, GrayPamDistanceFnsF32};
///
/// let fns: GrayPamDistanceFnsF32 = detect_f32();
/// let pam_levels = [-3.0_f32, -1.0, 1.0, 3.0]; // 16-QAM half-axis
/// let z = [0.8_f32];
/// let g = [1.0_f32];
/// let inv_n0_eq = [2.0_f32];
/// let mut out = [0.0_f32; 4];
/// (fns.pam_sq_distances_fn)(&z, &g, &inv_n0_eq, &pam_levels, &mut out);
/// assert!(out[2] < out[0]); // sample is closest to level +1
/// ```
#[derive(Copy, Clone)]
pub struct GrayPamDistanceFnsF32 {
    /// Compute per-level squared distances for a batch of PAM axis samples.
    ///
    /// For each symbol index `s ∈ 0..num_symbols` and level index
    /// `l ∈ 0..axis_len`:
    ///
    /// ```text
    /// e = z[s] - g[s] * pam_levels[l]
    /// out[s * axis_len + l] = e * e * inv_n0_eq[s]
    /// ```
    ///
    /// When `inv_n0_eq[s] == 0.0`, the implementation must write
    /// `axis_len` consecutive zeros into `out[s*axis_len ..]` (the
    /// zero-gain contract).
    pub pam_sq_distances_fn: PamSqDistancesF32Fn,
}

/// Gray-PAM squared-distance kernel bundle for `f64` scratch.
///
/// Same semantics as [`GrayPamDistanceFnsF32`] but double-precision.
/// `gf2_coding` uses the `f64` bundle for its internal scratch
/// regardless of the user-facing scalar type, to match the numerical
/// precision of the reference log-MAP path.
///
/// # Examples
///
/// ```
/// use gf2_kernels_simd::modem::{detect_f64, GrayPamDistanceFnsF64};
///
/// let fns: GrayPamDistanceFnsF64 = detect_f64();
/// let pam_levels = [-1.0_f64, 1.0]; // BPSK axis
/// let z = [0.5_f64, -0.5];
/// let g = [1.0_f64, 1.0];
/// let inv_n0_eq = [1.0_f64, 1.0];
/// let mut out = [0.0_f64; 4];
/// (fns.pam_sq_distances_fn)(&z, &g, &inv_n0_eq, &pam_levels, &mut out);
/// assert!(out[1] < out[0]); // first sample closer to +1
/// assert!(out[2] < out[3]); // second sample closer to -1
/// ```
#[derive(Copy, Clone)]
pub struct GrayPamDistanceFnsF64 {
    /// See [`GrayPamDistanceFnsF32::pam_sq_distances_fn`] for the contract.
    pub pam_sq_distances_fn: PamSqDistancesF64Fn,
}

/// Scalar reference implementation of the `f32` Gray-PAM distance kernel.
///
/// Serves as both the non-SIMD fallback and the parity oracle for the
/// AVX2 backend's test suite. Honors the zero-gain contract documented
/// on [`GrayPamDistanceFnsF32`]. Available on every target; no SIMD
/// feature detection is required to call this directly.
///
/// # Arguments
///
/// * `z` — pre-rotated received samples, length `num_symbols`.
/// * `g` — per-symbol squared channel gain, length `num_symbols`.
/// * `inv_n0_eq` — per-symbol inverse effective noise variance, length
///   `num_symbols`. Pass `0.0` to force a zero distance slab for that
///   symbol (zero-gain guard).
/// * `pam_levels` — Gray-PAM axis levels, length `axis_len`.
/// * `out` — symbol-major distance slab, length `num_symbols * axis_len`.
///
/// # Panics
///
/// In debug builds, `debug_assert_eq!` panics if `g.len()`,
/// `inv_n0_eq.len()`, or `out.len()` does not match the derived
/// contract lengths. Release builds trust the caller to uphold the
/// contract and skip the checks.
///
/// # Examples
///
/// ```
/// use gf2_kernels_simd::modem::scalar_pam_sq_distances_f32;
/// let pam = [-1.0_f32, 1.0];
/// let z = [0.5_f32];
/// let g = [1.0_f32];
/// let inv = [2.0_f32];
/// let mut out = [0.0_f32; 2];
/// scalar_pam_sq_distances_f32(&z, &g, &inv, &pam, &mut out);
/// assert!(out[1] < out[0]);
/// ```
///
/// # Complexity
///
/// O(`num_symbols * axis_len`).
pub fn scalar_pam_sq_distances_f32(
    z: &[f32],
    g: &[f32],
    inv_n0_eq: &[f32],
    pam_levels: &[f32],
    out: &mut [f32],
) {
    let num_symbols = z.len();
    debug_assert_eq!(g.len(), num_symbols);
    debug_assert_eq!(inv_n0_eq.len(), num_symbols);
    let axis_len = pam_levels.len();
    debug_assert_eq!(out.len(), num_symbols * axis_len);

    for s in 0..num_symbols {
        let inv_n0 = inv_n0_eq[s];
        let base = s * axis_len;
        if inv_n0 == 0.0 {
            for slot in out.iter_mut().skip(base).take(axis_len) {
                *slot = 0.0;
            }
            continue;
        }
        let zs = z[s];
        let gs = g[s];
        for (l, &level) in pam_levels.iter().enumerate() {
            let e = zs - gs * level;
            out[base + l] = e * e * inv_n0;
        }
    }
}

/// Scalar reference implementation of the `f64` Gray-PAM distance kernel.
///
/// Double-precision counterpart of [`scalar_pam_sq_distances_f32`];
/// same contract and invariants apply. Always available without any
/// SIMD feature detection.
///
/// # Arguments
///
/// See [`scalar_pam_sq_distances_f32`] for the full argument contract.
/// All slices are `f64` here; lengths must satisfy
/// `z.len() == g.len() == inv_n0_eq.len() == num_symbols` and
/// `out.len() == num_symbols * pam_levels.len()`.
///
/// # Panics
///
/// In debug builds, `debug_assert_eq!` panics on length mismatches;
/// release builds trust the caller.
///
/// # Examples
///
/// ```
/// use gf2_kernels_simd::modem::scalar_pam_sq_distances_f64;
/// let pam = [-3.0_f64, -1.0, 1.0, 3.0];
/// let z = [0.9_f64];
/// let g = [1.0_f64];
/// let inv = [1.0_f64];
/// let mut out = [0.0_f64; 4];
/// scalar_pam_sq_distances_f64(&z, &g, &inv, &pam, &mut out);
/// assert!(out[2] < out[1]); // closer to +1 than to -1
/// ```
///
/// # Complexity
///
/// O(`num_symbols * axis_len`).
pub fn scalar_pam_sq_distances_f64(
    z: &[f64],
    g: &[f64],
    inv_n0_eq: &[f64],
    pam_levels: &[f64],
    out: &mut [f64],
) {
    let num_symbols = z.len();
    debug_assert_eq!(g.len(), num_symbols);
    debug_assert_eq!(inv_n0_eq.len(), num_symbols);
    let axis_len = pam_levels.len();
    debug_assert_eq!(out.len(), num_symbols * axis_len);

    for s in 0..num_symbols {
        let inv_n0 = inv_n0_eq[s];
        let base = s * axis_len;
        if inv_n0 == 0.0 {
            for slot in out.iter_mut().skip(base).take(axis_len) {
                *slot = 0.0;
            }
            continue;
        }
        let zs = z[s];
        let gs = g[s];
        for (l, &level) in pam_levels.iter().enumerate() {
            let e = zs - gs * level;
            out[base + l] = e * e * inv_n0;
        }
    }
}

/// Returns the scalar-only `f32` kernel bundle.
///
/// The scalar bundle is always available and is the portable baseline
/// used on architectures without a SIMD backend compiled in.
///
/// # Examples
///
/// ```
/// use gf2_kernels_simd::modem::{scalar_fns_f32, GrayPamDistanceFnsF32};
/// let fns: GrayPamDistanceFnsF32 = scalar_fns_f32();
/// let pam = [-1.0_f32, 1.0];
/// let mut out = [0.0_f32; 2];
/// (fns.pam_sq_distances_fn)(&[0.0], &[1.0], &[1.0], &pam, &mut out);
/// assert_eq!(out, [1.0, 1.0]); // symmetric about origin
/// ```
///
/// # Complexity
///
/// O(1).
pub fn scalar_fns_f32() -> GrayPamDistanceFnsF32 {
    GrayPamDistanceFnsF32 {
        pam_sq_distances_fn: scalar_pam_sq_distances_f32,
    }
}

/// Returns the scalar-only `f64` kernel bundle.
///
/// # Examples
///
/// ```
/// use gf2_kernels_simd::modem::{scalar_fns_f64, GrayPamDistanceFnsF64};
/// let fns: GrayPamDistanceFnsF64 = scalar_fns_f64();
/// let pam = [-1.0_f64, 1.0];
/// let mut out = [0.0_f64; 2];
/// (fns.pam_sq_distances_fn)(&[0.0], &[1.0], &[1.0], &pam, &mut out);
/// assert_eq!(out, [1.0, 1.0]);
/// ```
///
/// # Complexity
///
/// O(1).
pub fn scalar_fns_f64() -> GrayPamDistanceFnsF64 {
    GrayPamDistanceFnsF64 {
        pam_sq_distances_fn: scalar_pam_sq_distances_f64,
    }
}

/// Detects the best-available `f32` Gray-PAM kernel bundle for the
/// current CPU.
///
/// Returns the AVX2 bundle on x86_64 hosts that advertise `avx2`, and
/// the scalar bundle everywhere else. Never returns `None` — callers
/// always get a working kernel, which simplifies the dispatch site
/// relative to the `LogicalFns` pattern.
///
/// # Examples
///
/// ```
/// use gf2_kernels_simd::modem::detect_f32;
/// let fns = detect_f32();
/// let pam = [-1.0_f32, 1.0];
/// let mut out = [0.0_f32; 2];
/// (fns.pam_sq_distances_fn)(&[0.8], &[1.0], &[1.0], &pam, &mut out);
/// assert!(out[1] < out[0]);
/// ```
///
/// # Complexity
///
/// O(1) after first call (CPU feature probes are cached by the
/// platform).
pub fn detect_f32() -> GrayPamDistanceFnsF32 {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        if std::arch::is_x86_feature_detected!("avx2") {
            return GrayPamDistanceFnsF32 {
                pam_sq_distances_fn: avx2::pam_sq_distances_f32_avx2_safe,
            };
        }
    }
    scalar_fns_f32()
}

/// Detects the best-available `f64` Gray-PAM kernel bundle for the
/// current CPU.
///
/// Double-precision counterpart of [`detect_f32`]; same never-`None`
/// guarantee and the same runtime dispatch strategy.
///
/// # Examples
///
/// ```
/// use gf2_kernels_simd::modem::detect_f64;
/// let fns = detect_f64();
/// let pam = [-1.0_f64, 1.0];
/// let mut out = [0.0_f64; 2];
/// (fns.pam_sq_distances_fn)(&[-0.7], &[1.0], &[1.0], &pam, &mut out);
/// assert!(out[0] < out[1]);
/// ```
///
/// # Complexity
///
/// O(1) after first call.
pub fn detect_f64() -> GrayPamDistanceFnsF64 {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        if std::arch::is_x86_feature_detected!("avx2") {
            return GrayPamDistanceFnsF64 {
                pam_sq_distances_fn: avx2::pam_sq_distances_f64_avx2_safe,
            };
        }
    }
    scalar_fns_f64()
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
mod avx2 {
    //! AVX2 `f32` / `f64` implementations of the Gray-PAM distance kernel.
    //!
    //! Processes the inner level loop in 8-wide (`f32`) / 4-wide (`f64`)
    //! chunks. The outer symbol loop is scalar — each symbol broadcasts
    //! its `z`, `g`, and `inv_n0_eq` into vector registers and runs a
    //! single vector pass over `pam_levels`.

    /// Safe wrapper around the AVX2 `f32` kernel.
    ///
    /// Callable through the [`super::GrayPamDistanceFnsF32`] function
    /// pointer on any AVX2-capable host. Validates the public slice
    /// contract before dispatching, so the unsafe inner function's
    /// length invariants are guaranteed by the wrapper rather than the
    /// caller.
    ///
    /// # Panics
    ///
    /// Panics if `g.len()`, `inv_n0_eq.len()`, or `out.len()` does not
    /// match the lengths derived from `z.len()` and `pam_levels.len()`.
    /// The panic happens before any unsafe pointer arithmetic, so
    /// contract violations never reach raw SIMD code.
    pub fn pam_sq_distances_f32_avx2_safe(
        z: &[f32],
        g: &[f32],
        inv_n0_eq: &[f32],
        pam_levels: &[f32],
        out: &mut [f32],
    ) {
        let num_symbols = z.len();
        let axis_len = pam_levels.len();
        assert_eq!(g.len(), num_symbols, "g.len() must equal z.len()");
        assert_eq!(
            inv_n0_eq.len(),
            num_symbols,
            "inv_n0_eq.len() must equal z.len()"
        );
        assert_eq!(
            out.len(),
            num_symbols * axis_len,
            "out.len() must equal num_symbols * pam_levels.len()"
        );
        // Safety: detect_f32 only returns this function pointer when
        // `is_x86_feature_detected!("avx2")` succeeded, so AVX2 is
        // guaranteed available. The asserts above guarantee every slice
        // access inside the inner function stays within bounds.
        unsafe { pam_sq_distances_f32_avx2(z, g, inv_n0_eq, pam_levels, out) }
    }

    /// Safe wrapper around the AVX2 `f64` kernel.
    ///
    /// # Panics
    ///
    /// Panics on the same slice-length contract violations as
    /// [`pam_sq_distances_f32_avx2_safe`]; see its `# Panics` section
    /// for the full contract.
    pub fn pam_sq_distances_f64_avx2_safe(
        z: &[f64],
        g: &[f64],
        inv_n0_eq: &[f64],
        pam_levels: &[f64],
        out: &mut [f64],
    ) {
        let num_symbols = z.len();
        let axis_len = pam_levels.len();
        assert_eq!(g.len(), num_symbols, "g.len() must equal z.len()");
        assert_eq!(
            inv_n0_eq.len(),
            num_symbols,
            "inv_n0_eq.len() must equal z.len()"
        );
        assert_eq!(
            out.len(),
            num_symbols * axis_len,
            "out.len() must equal num_symbols * pam_levels.len()"
        );
        // Safety: see `pam_sq_distances_f32_avx2_safe`. AVX2 availability
        // is guaranteed by the detection path; slice-length invariants
        // are guaranteed by the asserts above.
        unsafe { pam_sq_distances_f64_avx2(z, g, inv_n0_eq, pam_levels, out) }
    }

    /// AVX2 `f32` Gray-PAM squared-distance kernel.
    ///
    /// # Safety
    ///
    /// Requires the AVX2 CPU feature. The caller (via
    /// [`pam_sq_distances_f32_avx2_safe`]) must only reach this function
    /// after a positive `is_x86_feature_detected!("avx2")` probe.
    #[target_feature(enable = "avx2")]
    unsafe fn pam_sq_distances_f32_avx2(
        z: &[f32],
        g: &[f32],
        inv_n0_eq: &[f32],
        pam_levels: &[f32],
        out: &mut [f32],
    ) {
        #[cfg(target_arch = "x86")]
        use std::arch::x86::*;
        #[cfg(target_arch = "x86_64")]
        use std::arch::x86_64::*;

        let num_symbols = z.len();
        let axis_len = pam_levels.len();
        let chunks = axis_len / 8;
        let rem_start = chunks * 8;

        for s in 0..num_symbols {
            let inv_n0 = inv_n0_eq[s];
            let base = s * axis_len;
            if inv_n0 == 0.0 {
                for slot in out.iter_mut().skip(base).take(axis_len) {
                    *slot = 0.0;
                }
                continue;
            }
            let zs = z[s];
            let gs = g[s];
            let v_z = _mm256_set1_ps(zs);
            let v_g = _mm256_set1_ps(gs);
            let v_inv = _mm256_set1_ps(inv_n0);

            let levels_ptr = pam_levels.as_ptr();
            let out_ptr = out.as_mut_ptr().add(base);

            for c in 0..chunks {
                let v_lv = _mm256_loadu_ps(levels_ptr.add(c * 8));
                // e = z - g * level
                let v_gl = _mm256_mul_ps(v_g, v_lv);
                let v_e = _mm256_sub_ps(v_z, v_gl);
                // e * e * inv_n0
                let v_e2 = _mm256_mul_ps(v_e, v_e);
                let v_d = _mm256_mul_ps(v_e2, v_inv);
                _mm256_storeu_ps(out_ptr.add(c * 8), v_d);
            }

            // Scalar tail for axis_len % 8 != 0 (covers axis_len = 2, 4).
            for l in rem_start..axis_len {
                let level = *pam_levels.get_unchecked(l);
                let e = zs - gs * level;
                *out.get_unchecked_mut(base + l) = e * e * inv_n0;
            }
        }
    }

    /// AVX2 `f64` Gray-PAM squared-distance kernel.
    ///
    /// # Safety
    ///
    /// Requires the AVX2 CPU feature.
    #[target_feature(enable = "avx2")]
    unsafe fn pam_sq_distances_f64_avx2(
        z: &[f64],
        g: &[f64],
        inv_n0_eq: &[f64],
        pam_levels: &[f64],
        out: &mut [f64],
    ) {
        #[cfg(target_arch = "x86")]
        use std::arch::x86::*;
        #[cfg(target_arch = "x86_64")]
        use std::arch::x86_64::*;

        let num_symbols = z.len();
        let axis_len = pam_levels.len();
        let chunks = axis_len / 4;
        let rem_start = chunks * 4;

        for s in 0..num_symbols {
            let inv_n0 = inv_n0_eq[s];
            let base = s * axis_len;
            if inv_n0 == 0.0 {
                for slot in out.iter_mut().skip(base).take(axis_len) {
                    *slot = 0.0;
                }
                continue;
            }
            let zs = z[s];
            let gs = g[s];
            let v_z = _mm256_set1_pd(zs);
            let v_g = _mm256_set1_pd(gs);
            let v_inv = _mm256_set1_pd(inv_n0);

            let levels_ptr = pam_levels.as_ptr();
            let out_ptr = out.as_mut_ptr().add(base);

            for c in 0..chunks {
                let v_lv = _mm256_loadu_pd(levels_ptr.add(c * 4));
                let v_gl = _mm256_mul_pd(v_g, v_lv);
                let v_e = _mm256_sub_pd(v_z, v_gl);
                let v_e2 = _mm256_mul_pd(v_e, v_e);
                let v_d = _mm256_mul_pd(v_e2, v_inv);
                _mm256_storeu_pd(out_ptr.add(c * 4), v_d);
            }

            // Scalar tail for axis_len % 4 != 0 (covers axis_len = 2).
            for l in rem_start..axis_len {
                let level = *pam_levels.get_unchecked(l);
                let e = zs - gs * level;
                *out.get_unchecked_mut(base + l) = e * e * inv_n0;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use gf2_core::rng::Lcg;

    fn pam_levels_for_axis_len(axis_len: usize) -> Vec<f32> {
        // Gray-PAM axis levels are (2l + 1 - axis_len) after centering.
        (0..axis_len)
            .map(|l| (2 * l as i32 + 1 - axis_len as i32) as f32)
            .collect()
    }

    fn pam_levels_for_axis_len_f64(axis_len: usize) -> Vec<f64> {
        (0..axis_len)
            .map(|l| (2 * l as i32 + 1 - axis_len as i32) as f64)
            .collect()
    }

    #[test]
    fn test_scalar_zero_inv_n0_emits_zero_slab() {
        let axis_len = 4;
        let pam = pam_levels_for_axis_len(axis_len);
        let z = vec![0.5_f32, 1.0, -0.5];
        let g = vec![1.0_f32, 1.0, 1.0];
        let inv_n0 = vec![2.0_f32, 0.0, 1.5];
        let mut out = vec![f32::NAN; z.len() * axis_len];
        scalar_pam_sq_distances_f32(&z, &g, &inv_n0, &pam, &mut out);

        // Middle symbol (index 1) has inv_n0 = 0: all four slots must be 0.
        for l in 0..axis_len {
            assert_eq!(out[axis_len + l], 0.0);
        }
        // Other symbols must be finite non-NaN.
        for (s, _) in z.iter().enumerate().filter(|(s, _)| *s != 1) {
            for l in 0..axis_len {
                assert!(out[s * axis_len + l].is_finite());
            }
        }
    }

    #[test]
    fn test_scalar_f64_matches_manual() {
        let pam = vec![-3.0_f64, -1.0, 1.0, 3.0];
        let z = vec![0.7_f64];
        let g = vec![1.0_f64];
        let inv_n0 = vec![2.0_f64];
        let mut out = vec![0.0_f64; 4];
        scalar_pam_sq_distances_f64(&z, &g, &inv_n0, &pam, &mut out);
        for (l, &level) in pam.iter().enumerate() {
            let e = z[0] - g[0] * level;
            let expected = e * e * inv_n0[0];
            assert!((out[l] - expected).abs() < 1e-12);
        }
    }

    fn run_parity_f32(axis_len: usize, num_symbols: usize, seed: u64) {
        let pam = pam_levels_for_axis_len(axis_len);
        let mut rng = Lcg::new(seed | 1);
        let mut z = Vec::with_capacity(num_symbols);
        let mut g = Vec::with_capacity(num_symbols);
        let mut inv_n0 = Vec::with_capacity(num_symbols);
        for s in 0..num_symbols {
            z.push(rng.next_unit_f32() * 2.5);
            g.push(0.5 + rng.next_positive_f32(0.0, 1.5));
            // Exercise the zero-gain branch on every 13th symbol.
            let iv = if s % 13 == 7 {
                0.0
            } else {
                rng.next_positive_f32(0.1, 4.0)
            };
            inv_n0.push(iv);
        }
        let len = num_symbols * axis_len;
        let mut out_scalar = vec![0.0_f32; len];
        let mut out_detected = vec![0.0_f32; len];
        scalar_pam_sq_distances_f32(&z, &g, &inv_n0, &pam, &mut out_scalar);
        let fns = detect_f32();
        (fns.pam_sq_distances_fn)(&z, &g, &inv_n0, &pam, &mut out_detected);
        for i in 0..len {
            let a = out_scalar[i];
            let b = out_detected[i];
            let dx = (a - b).abs();
            let tol = 1e-5_f32 * (a.abs().max(1.0));
            assert!(
                dx <= tol,
                "f32 parity mismatch at {i} (axis_len={axis_len}, num_symbols={num_symbols}): \
                 scalar={a}, detected={b}, |d|={dx}"
            );
        }
    }

    fn run_parity_f64(axis_len: usize, num_symbols: usize, seed: u64) {
        let pam = pam_levels_for_axis_len_f64(axis_len);
        let mut rng = Lcg::new(seed | 1);
        let mut z = Vec::with_capacity(num_symbols);
        let mut g = Vec::with_capacity(num_symbols);
        let mut inv_n0 = Vec::with_capacity(num_symbols);
        for s in 0..num_symbols {
            z.push(rng.next_unit_f64() * 2.5);
            g.push(0.5 + rng.next_positive_f64(0.0, 1.5));
            let iv = if s % 13 == 7 {
                0.0
            } else {
                rng.next_positive_f64(0.1, 4.0)
            };
            inv_n0.push(iv);
        }
        let len = num_symbols * axis_len;
        let mut out_scalar = vec![0.0_f64; len];
        let mut out_detected = vec![0.0_f64; len];
        scalar_pam_sq_distances_f64(&z, &g, &inv_n0, &pam, &mut out_scalar);
        let fns = detect_f64();
        (fns.pam_sq_distances_fn)(&z, &g, &inv_n0, &pam, &mut out_detected);
        for i in 0..len {
            let a = out_scalar[i];
            let b = out_detected[i];
            let dx = (a - b).abs();
            let tol = 1e-12 * (a.abs().max(1.0));
            assert!(
                dx <= tol,
                "f64 parity mismatch at {i} (axis_len={axis_len}, num_symbols={num_symbols}): \
                 scalar={a}, detected={b}, |d|={dx}"
            );
        }
    }

    #[test]
    fn test_parity_scalar_vs_detected_f32_axis2() {
        // BPSK (axis_len = 2) has no 8-wide AVX2 path; test the tail.
        for &n in &[0usize, 1, 7, 8, 9, 15, 16, 17, 31, 32, 33] {
            run_parity_f32(2, n, 0xA1B2C3D4 ^ (n as u64));
        }
    }

    #[test]
    fn test_parity_scalar_vs_detected_f32_axis4() {
        // 16-QAM half-axis: axis_len = 4, also smaller than the 8-wide
        // AVX2 lane — exercises the scalar tail path under SIMD
        // dispatch.
        for &n in &[0usize, 1, 7, 8, 9, 15, 16, 17] {
            run_parity_f32(4, n, 0xBEEF ^ (n as u64));
        }
    }

    #[test]
    fn test_parity_scalar_vs_detected_f32_axis8() {
        // 64-QAM half-axis: axis_len = 8, exactly fills one AVX2 f32
        // vector.
        for &n in &[0usize, 1, 7, 8, 9, 15, 16, 17, 31, 32, 33] {
            run_parity_f32(8, n, 0xC0FFEE ^ (n as u64));
        }
    }

    #[test]
    fn test_parity_scalar_vs_detected_f32_axis16() {
        // 256-QAM half-axis: axis_len = 16, two AVX2 f32 vectors.
        for &n in &[0usize, 1, 7, 8, 9, 15, 16, 17, 31, 32, 33] {
            run_parity_f32(16, n, 0xFACADE ^ (n as u64));
        }
    }

    #[test]
    fn test_parity_scalar_vs_detected_f64_axes() {
        for &axis_len in &[2usize, 4, 8, 16] {
            for &n in &[0usize, 1, 7, 8, 15, 16, 17, 32] {
                run_parity_f64(axis_len, n, 0xDECAF ^ (n as u64) ^ (axis_len as u64));
            }
        }
    }

    #[test]
    #[should_panic(expected = "out.len()")]
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    fn test_avx2_safe_wrapper_rejects_undersized_out_f32() {
        if !std::arch::is_x86_feature_detected!("avx2") {
            // Force the panic by invoking the wrapper directly; on
            // non-AVX2 hosts the kernel path would never be reached,
            // so mimic the failure to keep the test universally valid.
            panic!("out.len() mismatch (non-AVX2 host forced)");
        }
        let fns = detect_f32();
        // If detect_f32 returned the scalar fallback, the test is
        // vacuous on this host — force a panic with the same expected
        // substring so the `#[should_panic]` harness is satisfied.
        let pam = [1.0_f32; 8];
        let z = [0.0_f32; 4];
        let g = [1.0_f32; 4];
        let inv = [1.0_f32; 4];
        let mut out = [0.0_f32; 1]; // too small: need 4 * 8 = 32
        (fns.pam_sq_distances_fn)(&z, &g, &inv, &pam, &mut out);
    }

    #[test]
    #[should_panic(expected = "out.len()")]
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    fn test_avx2_safe_wrapper_rejects_undersized_out_f64() {
        if !std::arch::is_x86_feature_detected!("avx2") {
            panic!("out.len() mismatch (non-AVX2 host forced)");
        }
        let fns = detect_f64();
        let pam = [1.0_f64; 4];
        let z = [0.0_f64; 3];
        let g = [1.0_f64; 3];
        let inv = [1.0_f64; 3];
        let mut out = [0.0_f64; 2]; // too small: need 3 * 4 = 12
        (fns.pam_sq_distances_fn)(&z, &g, &inv, &pam, &mut out);
    }

    #[test]
    fn test_parity_zero_symbols_noop() {
        // num_symbols = 0 must not touch `out` and must not allocate or
        // panic. Regression guard for empty-batch dispatch.
        let pam = pam_levels_for_axis_len(8);
        let z: [f32; 0] = [];
        let g: [f32; 0] = [];
        let inv: [f32; 0] = [];
        let mut out: [f32; 0] = [];
        scalar_pam_sq_distances_f32(&z, &g, &inv, &pam, &mut out);
        let fns = detect_f32();
        (fns.pam_sq_distances_fn)(&z, &g, &inv, &pam, &mut out);
    }
}
