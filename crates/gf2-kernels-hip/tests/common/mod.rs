//! Common test helpers shared across the F_3/F_5/F_7 GPU bit-identity tests.
//!
//! Extracts the HIP runtime FFI bindings, host-side memcpy direction constants,
//! a small xorshift64 PRNG, and the alloc/H2D/launch/D2H/compare skeleton used
//! by every per-prime bit-identity test. The per-prime test files in this crate
//! consume this module via `#[path = "common/mod.rs"] mod common;`
//! (the Cargo integration-test layout doesn't include `tests/*.rs` modules
//! automatically; each test file must declare the path-import explicitly).
#![cfg(feature = "hip")]
#![allow(dead_code)] // not every per-prime test calls every helper

use std::ffi::c_void;
use std::os::raw::c_int;

pub const HIP_MEMCPY_HOST_TO_DEVICE: c_int = 1;
pub const HIP_MEMCPY_DEVICE_TO_HOST: c_int = 2;

extern "C" {
    pub fn hipMalloc(ptr: *mut *mut c_void, size: usize) -> c_int;
    pub fn hipFree(ptr: *mut c_void) -> c_int;
    pub fn hipMemcpy(dst: *mut c_void, src: *const c_void, size: usize, kind: c_int) -> c_int;
    pub fn hipDeviceSynchronize() -> c_int;
}

/// Simple xorshift64 PRNG for reproducible random-matrix generation in tests.
/// Identical state machine across the F_3/F_5/F_7 tests so seed-pinning gives
/// the same matrices regardless of prime.
pub fn xorshift64(state: &mut u64) -> u64 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *state = x;
    x
}

/// Allocate device buffers for `m` matrices of `n*n` bytes each, plus an
/// output buffer for `m` u64s.  Copies `host_matrices` H2D, invokes the
/// supplied `launch` closure (which is responsible for the kernel launch),
/// copies outputs D2H, frees device memory, and returns the host output vector.
///
/// The closure receives raw device pointers `(d_matrices, d_out)` and must
/// ensure the GPU kernel has finished writing before returning (e.g. by calling
/// `hipDeviceSynchronize` or including that call in the kernel launch wrapper).
///
/// # Safety
///
/// Must only be called with a live ROCm/HIP context (gfx1030 device present).
/// `host_matrices` must have exactly `m * n * n` elements.
pub unsafe fn run_with_device_buffers<F>(
    host_matrices: &[u8],
    n: usize,
    m: usize,
    launch: F,
) -> Vec<u64>
where
    F: FnOnce(*const u8, *mut u64),
{
    let total_bytes = m * n * n;
    debug_assert_eq!(
        host_matrices.len(),
        total_bytes,
        "host_matrices length mismatch: expected {total_bytes}, got {}",
        host_matrices.len()
    );
    let out_bytes = m * std::mem::size_of::<u64>();

    // Allocate device memory.
    let mut d_matrices: *mut c_void = std::ptr::null_mut();
    let mut d_out: *mut c_void = std::ptr::null_mut();

    // SAFETY: hipMalloc writes a valid device pointer on success.
    let rc = hipMalloc(&mut d_matrices, total_bytes);
    assert_eq!(rc, 0, "hipMalloc(d_matrices) failed: code {rc}");

    let rc = hipMalloc(&mut d_out, out_bytes);
    assert_eq!(rc, 0, "hipMalloc(d_out) failed: code {rc}");

    // Copy matrices H2D.
    // SAFETY: d_matrices is a valid device allocation of `total_bytes`;
    // host_matrices is a valid slice of the same length.
    let rc = hipMemcpy(
        d_matrices,
        host_matrices.as_ptr() as *const c_void,
        total_bytes,
        HIP_MEMCPY_HOST_TO_DEVICE,
    );
    assert_eq!(rc, 0, "hipMemcpy H2D failed: code {rc}");

    // Invoke the prime-specific kernel launch closure.
    // SAFETY: d_matrices and d_out are valid device allocations; the caller
    // validates n and m before passing them into this helper.
    launch(d_matrices as *const u8, d_out as *mut u64);

    // Synchronize — ensure kernel has finished before D2H copy.
    // SAFETY: hipDeviceSynchronize has no preconditions.
    let rc = hipDeviceSynchronize();
    assert_eq!(rc, 0, "hipDeviceSynchronize failed: code {rc}");

    // Copy outputs D2H.
    let mut gpu_out = vec![0u64; m];
    // SAFETY: d_out is a valid device allocation of `out_bytes`; gpu_out
    // is a mutable slice of the same byte length.
    let rc = hipMemcpy(
        gpu_out.as_mut_ptr() as *mut c_void,
        d_out as *const c_void,
        out_bytes,
        HIP_MEMCPY_DEVICE_TO_HOST,
    );
    assert_eq!(rc, 0, "hipMemcpy D2H failed: code {rc}");

    // Free device memory.
    // SAFETY: d_matrices and d_out were allocated by hipMalloc above
    // and have not been freed yet.
    hipFree(d_matrices);
    hipFree(d_out);

    gpu_out
}
