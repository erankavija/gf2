//! FFI shims for per-prime permanent computation kernels.
//!
//! These declarations correspond to the placeholder device kernels in
//! `hip/permanent/`. Downstream issues (ad55b777, b43cdf33, 5c0505b2)
//! will replace the `.hip` stubs with real Ryser/BPAS implementations
//! without changing this FFI surface.
//!
//! # Safety
//!
//! All functions in this module are `unsafe`. Callers must satisfy the
//! preconditions documented on each function.
//!
//! # Feature gate
//!
//! This module is only compiled when the `hip` Cargo feature is enabled.
//! The corresponding `.hip` source files are compiled by `build.rs` under
//! the same condition.

use std::os::raw::c_int;

extern "C" {
    /// Compute the permanent of an n×n matrix over GF(3) on the GPU.
    ///
    /// Entry point implemented in `hip/permanent/permanent_bipedal3.hip`.
    /// The placeholder writes 0 to `*out_ptr`; the real implementation
    /// (ad55b777) will replace the body with a Ryser/BPAS kernel.
    ///
    /// # Arguments
    ///
    /// - `matrix_ptr` — device pointer to an n×n row-major array of `u8`
    ///   elements in GF(3) (values 0, 1, 2).
    /// - `n` — matrix dimension (n×n).
    /// - `out_ptr` — device pointer to a single `u64` output that receives
    ///   the permanent value modulo 3.
    ///
    /// # Returns
    ///
    /// 0 on success (`hipSuccess`), a non-zero HIP error code otherwise.
    fn permanent_bipedal3_hip(matrix_ptr: *const u8, n: c_int, out_ptr: *mut u64) -> c_int;

    /// Compute the permanent of an n×n matrix over GF(5) on the GPU.
    ///
    /// Entry point implemented in `hip/permanent/permanent_bipedal5.hip`.
    /// The placeholder writes 0 to `*out_ptr`; the real implementation
    /// (b43cdf33) will replace the body with a Ryser/BPAS kernel.
    ///
    /// # Arguments
    ///
    /// - `matrix_ptr` — device pointer to an n×n row-major array of `u8`
    ///   elements in GF(5) (values 0..4).
    /// - `n` — matrix dimension (n×n).
    /// - `out_ptr` — device pointer to a single `u64` output that receives
    ///   the permanent value modulo 5.
    ///
    /// # Returns
    ///
    /// 0 on success (`hipSuccess`), a non-zero HIP error code otherwise.
    fn permanent_bipedal5_hip(matrix_ptr: *const u8, n: c_int, out_ptr: *mut u64) -> c_int;

    /// Compute the permanent of an n×n matrix over GF(7) on the GPU.
    ///
    /// Entry point implemented in `hip/permanent/permanent_bipedal7.hip`.
    /// The placeholder writes 0 to `*out_ptr`; the real implementation
    /// (5c0505b2) will replace the body with a Ryser/BPAS kernel.
    ///
    /// # Arguments
    ///
    /// - `matrix_ptr` — device pointer to an n×n row-major array of `u8`
    ///   elements in GF(7) (values 0..6).
    /// - `n` — matrix dimension (n×n).
    /// - `out_ptr` — device pointer to a single `u64` output that receives
    ///   the permanent value modulo 7.
    ///
    /// # Returns
    ///
    /// 0 on success (`hipSuccess`), a non-zero HIP error code otherwise.
    fn permanent_bipedal7_hip(matrix_ptr: *const u8, n: c_int, out_ptr: *mut u64) -> c_int;
}

/// Call the GF(3) permanent kernel.
///
/// # Safety
///
/// - `matrix_ptr` must be a valid device allocation of at least `n * n` bytes,
///   containing GF(3) element values (0, 1, 2).
/// - `out_ptr` must be a valid device allocation of at least 8 bytes.
/// - `n` must be non-negative.
/// - The HIP runtime must be initialised and a device context must be active.
pub unsafe fn compute_permanent_gf3(matrix_ptr: *const u8, n: c_int, out_ptr: *mut u64) -> c_int {
    // SAFETY: preconditions forwarded verbatim from the caller (see doc comment).
    permanent_bipedal3_hip(matrix_ptr, n, out_ptr)
}

/// Call the GF(5) permanent kernel.
///
/// # Safety
///
/// - `matrix_ptr` must be a valid device allocation of at least `n * n` bytes,
///   containing GF(5) element values (0..4).
/// - `out_ptr` must be a valid device allocation of at least 8 bytes.
/// - `n` must be non-negative.
/// - The HIP runtime must be initialised and a device context must be active.
pub unsafe fn compute_permanent_gf5(matrix_ptr: *const u8, n: c_int, out_ptr: *mut u64) -> c_int {
    // SAFETY: preconditions forwarded verbatim from the caller (see doc comment).
    permanent_bipedal5_hip(matrix_ptr, n, out_ptr)
}

/// Call the GF(7) permanent kernel.
///
/// # Safety
///
/// - `matrix_ptr` must be a valid device allocation of at least `n * n` bytes,
///   containing GF(7) element values (0..6).
/// - `out_ptr` must be a valid device allocation of at least 8 bytes.
/// - `n` must be non-negative.
/// - The HIP runtime must be initialised and a device context must be active.
pub unsafe fn compute_permanent_gf7(matrix_ptr: *const u8, n: c_int, out_ptr: *mut u64) -> c_int {
    // SAFETY: preconditions forwarded verbatim from the caller (see doc comment).
    permanent_bipedal7_hip(matrix_ptr, n, out_ptr)
}
