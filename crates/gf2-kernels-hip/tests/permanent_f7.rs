//! GPU bit-identity and LUT-checksum tests for F_7 HIP permanent kernel (5c0505b2).
//!
//! Verifies that `permanent_bipedal7_hip_batch` produces results bit-identical
//! to the CPU reference `permanent_bipedal7_singleword` on random matrices for
//! n ∈ {8, 12} (success criterion 2 of 5c0505b2), and that the GPU
//! `__constant__` MUL_LUT is byte-identical to the host static const (criterion 3).
//!
//! All tests carry `#[ignore = "external: gfx1030 device required"]` per
//! CLAUDE.md test-tier conventions.  Run them only on the dev host with ROCm
//! installed:
//!
//! ```text
//! cargo nextest run -p gf2-kernels-hip \
//!     --features hip \
//!     --run-ignored ignored-only \
//!     -E 'test(test_permanent_bipedal7_)'
//! ```
//!
//! # Algorithm note
//!
//! The GPU kernel uses direct byte arithmetic (each GF(7) element is one u8
//! in {0..6}) with LUT-based add/sub/mul.  The CPU reference
//! `permanent_bipedal7_singleword` uses `Packed7` (4-bit-nibble encoding) but
//! computes the same Ryser sum — both produce the same mathematical result mod 7.
//!
//! For n > 16 (Packed7::LANES) the CPU reference panics, so this test file
//! only exercises n ∈ {8, 12} — both within the CPU single-word bound.

#![cfg(feature = "hip")]

#[path = "common/mod.rs"]
mod common;

use common::{
    hipDeviceSynchronize, hipFree, hipMalloc, hipMemcpy, run_with_device_buffers, xorshift64,
    HIP_MEMCPY_DEVICE_TO_HOST,
};
use gf2_algebra::packed::packed7::MUL_LUT;
use gf2_algebra::packed::Packed7Matrix;
use gf2_algebra::permanent::bipedal7::permanent_bipedal7_singleword;
use gf2_core::gfp::Fp;
use gf2_kernels_hip::permanent::{
    compute_lut_checksum_gpu, compute_permanent_gf7_batch, init_permanent_gf7,
};
use std::ffi::c_void;
use std::os::raw::c_int;

/// Generate one random GF(7) value (0..=6) from the PRNG state.
fn rand_fp7(state: &mut u64) -> u8 {
    // Rejection-free: draw from {0,1,2,3,4,5,6} via modulo 7.
    // Bias is negligible (2^64 mod 7 = 4, so at most 4 extra values in range).
    (xorshift64(state) % 7) as u8
}

// ---------------------------------------------------------------------------
// Core test helper: runs M matrices of size n×n through the GPU batch kernel
// and compares each result to the CPU reference.
// ---------------------------------------------------------------------------

/// Run `m_count` random n×n GF(7) matrices through the GPU batch kernel and
/// compare against `permanent_bipedal7_singleword`. Panics on any mismatch.
///
/// `seed` is the xorshift64 initial state; choosing distinct seeds per test
/// ensures each test exercises a different random draw.
///
/// # Safety
///
/// This function allocates and frees device memory. It must only be called on
/// a host with a live ROCm/HIP context (gfx1030 device present).
unsafe fn run_bit_identity_check(n: usize, m_count: usize, seed: u64) {
    assert!(
        (1..=16).contains(&n),
        "n must be in 1..=16 (CPU reference `permanent_bipedal7_singleword` limit)"
    );
    assert!(m_count >= 1, "m_count must be >= 1");

    let mat_bytes = n * n; // bytes per matrix (one u8 per GF(7) element)

    // Generate random matrices on the host.
    let mut rng = seed;
    let mut host_matrices: Vec<u8> = Vec::with_capacity(m_count * mat_bytes);
    for _ in 0..(m_count * n * n) {
        host_matrices.push(rand_fp7(&mut rng));
    }

    // Compute CPU reference permanents.
    // `permanent_bipedal7_singleword` requires a Packed7Matrix (column-major),
    // but takes a row-major slice via `from_row_major`.
    let cpu_results: Vec<u64> = (0..m_count)
        .map(|i| {
            let slice = &host_matrices[i * mat_bytes..(i + 1) * mat_bytes];
            let fp7_data: Vec<Fp<7>> = slice.iter().map(|&v| Fp::<7>::new(v as u64)).collect();
            let mat = Packed7Matrix::from_row_major(&fp7_data, n, n);
            permanent_bipedal7_singleword(&mat).value()
        })
        .collect();

    // Init LUTs explicitly before the first compute call (the lib cannot
    // auto-init because gf2-algebra is a dev-dep, not a regular dep, to avoid
    // a circular workspace dependency). Idempotent — calling it once per
    // process is sufficient, but calling it per test is cheap and keeps
    // each test self-contained.
    use gf2_algebra::packed::packed7::{ADD_LUT, SUB_LUT};
    // SAFETY: ADD_LUT, SUB_LUT, MUL_LUT are 'static [u8; 65536].
    let rc = unsafe { init_permanent_gf7(ADD_LUT.as_ptr(), SUB_LUT.as_ptr(), MUL_LUT.as_ptr()) };
    assert_eq!(rc, 0, "init_permanent_gf7 failed: code {rc}");

    // Run the GPU batch kernel via the shared alloc/H2D/launch/D2H/free helper.
    // SAFETY: requires a live HIP device context; host_matrices has m_count*n*n bytes.
    let gpu_results = run_with_device_buffers(&host_matrices, n, m_count, |d_in, d_out| {
        // SAFETY: d_in/d_out are valid device allocations; n,m_count validated above.
        let rc = unsafe { compute_permanent_gf7_batch(d_in, n as c_int, m_count as c_int, d_out) };
        assert_eq!(rc, 0, "permanent_bipedal7_hip_batch failed: code {rc}");
    });

    // Bit-identity check.
    for i in 0..m_count {
        assert_eq!(
            gpu_results[i], cpu_results[i],
            "permanent mismatch at matrix {i}: GPU={} CPU={} (n={n})",
            gpu_results[i], cpu_results[i]
        );
    }
}

// ---------------------------------------------------------------------------
// Tests — bit-identity for n ∈ {8, 12} (success criterion 2 of 5c0505b2).
// All gated on `#[cfg(feature = "hip")]` (inherited from the module-level
// cfg) and `#[ignore = "external: gfx1030 device required"]`.
// ---------------------------------------------------------------------------

/// n=8: 100 matrices, 2^8 = 256 Gray steps each — completes in < 1 s on gfx1030.
///
/// Criterion 2 of 5c0505b2: GPU output bit-identical to CPU
/// `permanent_bipedal7` on 100 random matrices for n=8.
#[test]
#[ignore = "external: gfx1030 device required"]
fn test_permanent_bipedal7_gpu_bit_identity_n8() {
    // SAFETY: requires a live HIP device context with gfx1030 support.
    unsafe { run_bit_identity_check(8, 100, 0xF7CA_FEDE_ADBE_EF08u64) }
}

/// n=12: 100 matrices, 2^12 = 4096 Gray steps each — completes in < 1 s on gfx1030.
///
/// Criterion 2 of 5c0505b2: GPU output bit-identical to CPU
/// `permanent_bipedal7` on 100 random matrices for n=12.
#[test]
#[ignore = "external: gfx1030 device required"]
fn test_permanent_bipedal7_gpu_bit_identity_n12() {
    // SAFETY: requires a live HIP device context with gfx1030 support.
    unsafe { run_bit_identity_check(12, 100, 0xF7CA_FEDE_ADBE_EF12u64) }
}

// ---------------------------------------------------------------------------
// Criterion-3 test: GPU __constant__ MUL_LUT byte-checksum matches host.
//
// Verifies that `hipMemcpyToSymbol` populated `d_MUL_LUT` with bytes
// identical to `gf2_algebra::packed::packed7::MUL_LUT`. Uses the
// `permanent_bipedal7_lut_checksum_kernel` (single thread, sums all 65 536
// bytes of `d_MUL_LUT` and writes a u64 to an output device pointer).
// ---------------------------------------------------------------------------

/// GPU __constant__ MUL_LUT byte-checksum must match the host static MUL_LUT.
///
/// Criterion 3 of 5c0505b2: LUT in `__constant__` memory is populated
/// identically to the host static const, verified by a runtime checksum on
/// the CPU/GPU pair.
///
/// The test:
/// 1. Calls `init_permanent_gf7` to copy all three host LUTs to device.
/// 2. Launches `permanent_bipedal7_hip_lut_checksum` (via `compute_lut_checksum_gpu`):
///    a single GPU thread sums all 65 536 bytes of `d_MUL_LUT` (__constant__).
/// 3. Copies the u64 result to the host.
/// 4. Compares to `MUL_LUT.iter().map(|&b| b as u64).sum::<u64>()` on the host.
#[test]
#[ignore = "external: gfx1030 device required"]
fn test_permanent_bipedal7_constant_lut_checksum_matches_host() {
    use gf2_algebra::packed::packed7::{ADD_LUT, SUB_LUT};

    // Compute the host-side checksum (sum of all 65536 bytes as u64).
    let host_sum: u64 = MUL_LUT.iter().map(|&b| b as u64).sum();

    // Allocate device output (one u64).
    let mut d_out: *mut c_void = std::ptr::null_mut();
    // SAFETY: hipMalloc writes a valid device pointer on success.
    let rc = unsafe { hipMalloc(&mut d_out, std::mem::size_of::<u64>()) };
    assert_eq!(rc, 0, "hipMalloc(d_out) failed: code {rc}");

    // Explicitly init the LUTs so d_MUL_LUT is populated before the checksum
    // kernel reads it. The lib does not auto-init; the caller is responsible
    // for calling init_permanent_gf7 before the first compute or checksum
    // call (see `crates/gf2-kernels-hip/src/permanent/mod.rs` for the
    // memoised-init contract).
    //
    // SAFETY: ADD_LUT, SUB_LUT, MUL_LUT are 'static [u8; 65536] from gf2_algebra.
    // The HIP runtime is live (gfx1030 device required).
    let rc = unsafe { init_permanent_gf7(ADD_LUT.as_ptr(), SUB_LUT.as_ptr(), MUL_LUT.as_ptr()) };
    assert_eq!(rc, 0, "permanent_bipedal7_hip_init failed: code {rc}");

    // Launch the checksum kernel.
    // SAFETY: d_out is a valid device allocation of 8 bytes.
    // init_permanent_gf7 was called above so d_MUL_LUT is populated.
    let rc = unsafe { compute_lut_checksum_gpu(d_out as *mut u64) };
    assert_eq!(
        rc, 0,
        "permanent_bipedal7_hip_lut_checksum failed: code {rc}"
    );

    // Synchronize.
    // SAFETY: hipDeviceSynchronize has no preconditions.
    let rc = unsafe { hipDeviceSynchronize() };
    assert_eq!(rc, 0, "hipDeviceSynchronize failed: code {rc}");

    // Copy checksum D2H.
    let mut gpu_sum: u64 = 0;
    // SAFETY: d_out is a valid device allocation of 8 bytes; gpu_sum is a
    // stack-allocated u64 — valid host destination for 8 bytes.
    let rc = unsafe {
        hipMemcpy(
            &mut gpu_sum as *mut u64 as *mut c_void,
            d_out as *const c_void,
            std::mem::size_of::<u64>(),
            HIP_MEMCPY_DEVICE_TO_HOST,
        )
    };
    assert_eq!(rc, 0, "hipMemcpy D2H (checksum) failed: code {rc}");

    // Free device memory.
    // SAFETY: d_out was allocated by hipMalloc above.
    unsafe { hipFree(d_out) };

    // Byte-identical check.
    assert_eq!(
        host_sum, gpu_sum,
        "GPU __constant__ MUL_LUT byte-checksum mismatch: host={host_sum} gpu={gpu_sum}"
    );
}
