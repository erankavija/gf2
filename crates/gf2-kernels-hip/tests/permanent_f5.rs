//! GPU bit-identity tests for F_5 HIP permanent kernel (b43cdf33).
//!
//! Verifies that `permanent_bipedal5_hip_batch` produces results
//! bit-identical to the CPU reference `permanent_bipedal5_singleword` on
//! random matrices for n ∈ {8, 12} (success criterion 2 of b43cdf33).
//!
//! All tests carry `#[ignore = "external: gfx1030 device required"]` per
//! CLAUDE.md test-tier conventions.  Run them only on the dev host with ROCm
//! installed:
//!
//! ```text
//! cargo nextest run -p gf2-kernels-hip \
//!     --features hip \
//!     --run-ignored ignored-only \
//!     -E 'test(gpu_f5_bit_identity)'
//! ```

#![cfg(feature = "hip")]

use gf2_algebra::packed::packed5::Packed5Matrix;
use gf2_algebra::permanent::bipedal5::permanent_bipedal5_singleword;
use gf2_core::gfp::Fp;
use gf2_kernels_hip::permanent::compute_permanent_gf5_batch;
use std::ffi::c_void;
use std::os::raw::c_int;

// ---------------------------------------------------------------------------
// HIP runtime bindings needed for device memory management in tests.
//
// We bind directly to the amdhip64 symbols — the same library already
// linked by build.rs — rather than adding a separate wrapper crate.
// ---------------------------------------------------------------------------

extern "C" {
    fn hipMalloc(ptr: *mut *mut c_void, size: usize) -> c_int;
    fn hipFree(ptr: *mut c_void) -> c_int;
    fn hipMemcpy(dst: *mut c_void, src: *const c_void, size: usize, kind: c_int) -> c_int;
    fn hipDeviceSynchronize() -> c_int;
}

/// HIP memcpy direction: host → device.
const HIP_MEMCPY_HOST_TO_DEVICE: c_int = 1;
/// HIP memcpy direction: device → host.
const HIP_MEMCPY_DEVICE_TO_HOST: c_int = 2;

// ---------------------------------------------------------------------------
// PRNG — xorshift64 seeded deterministically so tests are reproducible.
// ---------------------------------------------------------------------------

fn xorshift64(state: &mut u64) -> u64 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *state = x;
    x
}

/// Generate one random GF(5) value (0, 1, 2, 3, or 4) from the PRNG state.
fn rand_fp5(state: &mut u64) -> u8 {
    // Rejection-free: draw from {0,1,2,3,4} via modulo 5.
    // Bias is negligible (2^64 mod 5 = 1, so the rejection region is at most 1 value).
    (xorshift64(state) % 5) as u8
}

// ---------------------------------------------------------------------------
// Core test helper: runs M matrices of size n×n through the GPU batch kernel
// and compares each result to the CPU reference.
// ---------------------------------------------------------------------------

/// Run `m_count` random n×n GF(5) matrices through the GPU batch kernel and
/// compare against `permanent_bipedal5_singleword`. Panics on any mismatch.
///
/// `seed` is the xorshift64 initial state; choosing distinct seeds per test
/// ensures each test exercises a different random draw.
///
/// # Safety
///
/// This function allocates and frees device memory. It must only be called on
/// a host with a live ROCm/HIP context (gfx1030 device present).
unsafe fn run_bit_identity_check(n: usize, m_count: usize, seed: u64) {
    assert!((1..=63).contains(&n), "n must be in 1..=63");
    assert!(m_count >= 1, "m_count must be >= 1");

    let mat_bytes = n * n; // bytes per matrix (one u8 per GF(5) element)
    let total_bytes = m_count * mat_bytes;

    // Generate random matrices on the host.
    let mut rng = seed;
    let mut host_matrices: Vec<u8> = Vec::with_capacity(total_bytes);
    for _ in 0..(m_count * n * n) {
        host_matrices.push(rand_fp5(&mut rng));
    }

    // Compute CPU reference permanents.
    let cpu_results: Vec<u64> = (0..m_count)
        .map(|i| {
            let slice = &host_matrices[i * mat_bytes..(i + 1) * mat_bytes];
            let fp5_data: Vec<Fp<5>> = slice.iter().map(|&v| Fp::<5>::new(v as u64)).collect();
            let mat = Packed5Matrix::from_row_major(&fp5_data, n, n);
            permanent_bipedal5_singleword(&mat).value()
        })
        .collect();

    // Allocate device memory for matrices and outputs.
    let mut d_matrices: *mut c_void = std::ptr::null_mut();
    let mut d_out: *mut c_void = std::ptr::null_mut();
    let out_bytes = m_count * std::mem::size_of::<u64>();

    // SAFETY: hipMalloc writes a valid device pointer on success.
    let rc = hipMalloc(&mut d_matrices, total_bytes);
    assert_eq!(rc, 0, "hipMalloc(d_matrices) failed: code {rc}");

    let rc = hipMalloc(&mut d_out, out_bytes);
    assert_eq!(rc, 0, "hipMalloc(d_out) failed: code {rc}");

    // Copy matrices H→D.
    // SAFETY: d_matrices is a valid device allocation of `total_bytes`.
    // host_matrices is a valid slice of the same length.
    let rc = hipMemcpy(
        d_matrices,
        host_matrices.as_ptr() as *const c_void,
        total_bytes,
        HIP_MEMCPY_HOST_TO_DEVICE,
    );
    assert_eq!(rc, 0, "hipMemcpy H2D failed: code {rc}");

    // Launch GPU kernel.
    // SAFETY: d_matrices and d_out are valid device allocations with the sizes
    // documented above. n and m_count are validated at function entry.
    let rc = compute_permanent_gf5_batch(
        d_matrices as *const u8,
        n as c_int,
        m_count as c_int,
        d_out as *mut u64,
    );
    assert_eq!(rc, 0, "permanent_bipedal5_hip_batch failed: code {rc}");

    // Synchronize to ensure the kernel has finished before D→H copy.
    // SAFETY: hipDeviceSynchronize has no preconditions.
    let rc = hipDeviceSynchronize();
    assert_eq!(rc, 0, "hipDeviceSynchronize failed: code {rc}");

    // Copy outputs D→H.
    let mut gpu_results = vec![0u64; m_count];
    // SAFETY: d_out is a valid device allocation of `out_bytes`; gpu_results
    // is a mutable slice of the same byte length.
    let rc = hipMemcpy(
        gpu_results.as_mut_ptr() as *mut c_void,
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
// Tests — one per n ∈ {8, 12} (success criterion 2 of b43cdf33).
// All gated on `#[cfg(feature = "hip")]` (inherited from the module-level
// cfg) and `#[ignore = "external: gfx1030 device required"]`.
// ---------------------------------------------------------------------------

/// n=8: 100 matrices, 2^8 = 256 ops each — completes in < 1 s on gfx1030.
#[test]
#[ignore = "external: gfx1030 device required"]
fn gpu_f5_bit_identity_n8() {
    // SAFETY: requires a live HIP device context with gfx1030 support.
    unsafe { run_bit_identity_check(8, 100, 0xF5CA_FEDE_ADBE_EF08u64) }
}

/// n=12: 100 matrices, 2^12 = 4096 ops each — completes in < 1 s on gfx1030.
#[test]
#[ignore = "external: gfx1030 device required"]
fn gpu_f5_bit_identity_n12() {
    // SAFETY: requires a live HIP device context with gfx1030 support.
    unsafe { run_bit_identity_check(12, 100, 0xF5CA_FEDE_ADBE_EF12u64) }
}
