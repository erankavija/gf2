//! GPU bit-identity tests for the F_7 HIP permanent kernel against the CPU
//! reference `permanent_bipedal7_singleword`, and a checksum test of the
//! device `d_MUL_LUT` against the host `MUL_LUT`.
//!
//! The CPU reference panics for n > 16 (`Packed7::LANES`), so the
//! bit-identity tests use n ∈ {8, 12}.

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

fn rand_fp7(state: &mut u64) -> u8 {
    // 2^64 mod 7 = 2, so the modulo bias is negligible.
    (xorshift64(state) % 7) as u8
}

/// Compares `m_count` random n×n GF(7) matrices, drawn from xorshift64 state
/// `seed`, between the GPU batch kernel and `permanent_bipedal7_singleword`.
///
/// # Safety
///
/// Must only be called on a host with a live ROCm/HIP context.
unsafe fn run_bit_identity_check(n: usize, m_count: usize, seed: u64) {
    assert!(
        (1..=16).contains(&n),
        "n must be in 1..=16 (CPU reference `permanent_bipedal7_singleword` limit)"
    );
    assert!(m_count >= 1, "m_count must be >= 1");

    let mat_bytes = n * n; // bytes per matrix (one u8 per GF(7) element)

    let mut rng = seed;
    let mut host_matrices: Vec<u8> = Vec::with_capacity(m_count * mat_bytes);
    for _ in 0..(m_count * n * n) {
        host_matrices.push(rand_fp7(&mut rng));
    }

    let cpu_results: Vec<u64> = (0..m_count)
        .map(|i| {
            let slice = &host_matrices[i * mat_bytes..(i + 1) * mat_bytes];
            let fp7_data: Vec<Fp<7>> = slice.iter().map(|&v| Fp::<7>::new(v as u64)).collect();
            let mat = Packed7Matrix::from_row_major(&fp7_data, n, n);
            permanent_bipedal7_singleword(&mat).value()
        })
        .collect();

    // The library takes the LUTs from the caller; the init is idempotent.
    use gf2_algebra::packed::packed7::{ADD_LUT, SUB_LUT};
    // SAFETY: ADD_LUT, SUB_LUT, MUL_LUT are 'static [u8; 65536].
    let rc = unsafe { init_permanent_gf7(ADD_LUT.as_ptr(), SUB_LUT.as_ptr(), MUL_LUT.as_ptr()) };
    assert_eq!(rc, 0, "init_permanent_gf7 failed: code {rc}");

    // SAFETY: requires a live HIP device context; host_matrices has m_count*n*n bytes.
    let gpu_results = run_with_device_buffers(&host_matrices, n, m_count, |d_in, d_out| {
        // SAFETY: d_in/d_out are valid device allocations; n,m_count validated above.
        let rc = unsafe { compute_permanent_gf7_batch(d_in, n as c_int, m_count as c_int, d_out) };
        assert_eq!(rc, 0, "permanent_bipedal7_hip_batch failed: code {rc}");
    });

    for i in 0..m_count {
        assert_eq!(
            gpu_results[i], cpu_results[i],
            "permanent mismatch at matrix {i}: GPU={} CPU={} (n={n})",
            gpu_results[i], cpu_results[i]
        );
    }
}

#[test]
#[ignore = "external: gfx1030 device required"]
fn test_permanent_bipedal7_gpu_bit_identity_n8() {
    // SAFETY: requires a live HIP device context with gfx1030 support.
    unsafe { run_bit_identity_check(8, 100, 0xF7CA_FEDE_ADBE_EF08u64) }
}

#[test]
#[ignore = "external: gfx1030 device required"]
fn test_permanent_bipedal7_gpu_bit_identity_n12() {
    // SAFETY: requires a live HIP device context with gfx1030 support.
    unsafe { run_bit_identity_check(12, 100, 0xF7CA_FEDE_ADBE_EF12u64) }
}

#[test]
#[ignore = "external: gfx1030 device required"]
fn test_permanent_bipedal7_constant_lut_checksum_matches_host() {
    use gf2_algebra::packed::packed7::{ADD_LUT, SUB_LUT};

    let host_sum: u64 = MUL_LUT.iter().map(|&b| b as u64).sum();

    let mut d_out: *mut c_void = std::ptr::null_mut();
    // SAFETY: hipMalloc writes a valid device pointer on success.
    let rc = unsafe { hipMalloc(&mut d_out, std::mem::size_of::<u64>()) };
    assert_eq!(rc, 0, "hipMalloc(d_out) failed: code {rc}");

    // SAFETY: ADD_LUT, SUB_LUT, MUL_LUT are 'static [u8; 65536] from gf2_algebra.
    // The HIP runtime is live (gfx1030 device required).
    let rc = unsafe { init_permanent_gf7(ADD_LUT.as_ptr(), SUB_LUT.as_ptr(), MUL_LUT.as_ptr()) };
    assert_eq!(rc, 0, "permanent_bipedal7_hip_init failed: code {rc}");

    // SAFETY: d_out is a valid device allocation of 8 bytes.
    // init_permanent_gf7 was called above so d_MUL_LUT is populated.
    let rc = unsafe { compute_lut_checksum_gpu(d_out as *mut u64) };
    assert_eq!(
        rc, 0,
        "permanent_bipedal7_hip_lut_checksum failed: code {rc}"
    );

    // SAFETY: hipDeviceSynchronize has no preconditions.
    let rc = unsafe { hipDeviceSynchronize() };
    assert_eq!(rc, 0, "hipDeviceSynchronize failed: code {rc}");

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

    // SAFETY: d_out was allocated by hipMalloc above.
    unsafe { hipFree(d_out) };

    assert_eq!(
        host_sum, gpu_sum,
        "GPU __constant__ MUL_LUT byte-checksum mismatch: host={host_sum} gpu={gpu_sum}"
    );
}
