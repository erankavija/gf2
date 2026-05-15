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
//!     -E 'test(test_permanent_bipedal5_gpu_bit_identity)'
//! ```

#![cfg(feature = "hip")]

#[path = "common/mod.rs"]
mod common;

use common::{run_with_device_buffers, xorshift64};
use gf2_algebra::packed::packed5::Packed5Matrix;
use gf2_algebra::permanent::bipedal5::permanent_bipedal5_singleword;
use gf2_core::gfp::Fp;
use gf2_kernels_hip::permanent::compute_permanent_gf5_batch;
use std::os::raw::c_int;

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

    // Generate random matrices on the host.
    let mut rng = seed;
    let mut host_matrices: Vec<u8> = Vec::with_capacity(m_count * mat_bytes);
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

    // Run the GPU batch kernel via the shared alloc/H2D/launch/D2H/free helper.
    // SAFETY: requires a live HIP device context; host_matrices has m_count*n*n bytes.
    let gpu_results = run_with_device_buffers(&host_matrices, n, m_count, |d_in, d_out| {
        // SAFETY: d_in/d_out are valid device allocations; n,m_count validated above.
        let rc = unsafe { compute_permanent_gf5_batch(d_in, n as c_int, m_count as c_int, d_out) };
        assert_eq!(rc, 0, "permanent_bipedal5_hip_batch failed: code {rc}");
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
// Tests — one per n ∈ {8, 12} (success criterion 2 of b43cdf33).
// All gated on `#[cfg(feature = "hip")]` (inherited from the module-level
// cfg) and `#[ignore = "external: gfx1030 device required"]`.
// ---------------------------------------------------------------------------

/// n=8: 100 matrices, 2^8 = 256 ops each — completes in < 1 s on gfx1030.
#[test]
#[ignore = "external: gfx1030 device required"]
fn test_permanent_bipedal5_gpu_bit_identity_n8() {
    // SAFETY: requires a live HIP device context with gfx1030 support.
    unsafe { run_bit_identity_check(8, 100, 0xF5CA_FEDE_ADBE_EF08u64) }
}

/// n=12: 100 matrices, 2^12 = 4096 ops each — completes in < 1 s on gfx1030.
#[test]
#[ignore = "external: gfx1030 device required"]
fn test_permanent_bipedal5_gpu_bit_identity_n12() {
    // SAFETY: requires a live HIP device context with gfx1030 support.
    unsafe { run_bit_identity_check(12, 100, 0xF5CA_FEDE_ADBE_EF12u64) }
}
