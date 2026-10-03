//! GPU bit-identity tests for the F_5 HIP permanent kernel against the CPU
//! reference `permanent_bipedal5_singleword`, for n ∈ {8, 12}.

#![cfg(feature = "hip")]

#[path = "common/mod.rs"]
mod common;

use common::{run_with_device_buffers, xorshift64};
use gf2_algebra::packed::packed5::Packed5Matrix;
use gf2_algebra::permanent::bipedal5::permanent_bipedal5_singleword;
use gf2_core::gfp::Fp;
use gf2_kernels_hip::permanent::compute_permanent_gf5_batch;
use std::os::raw::c_int;

fn rand_fp5(state: &mut u64) -> u8 {
    // 2^64 mod 5 = 1, so the modulo bias is negligible.
    (xorshift64(state) % 5) as u8
}

/// Compares `m_count` random n×n GF(5) matrices, drawn from xorshift64 state
/// `seed`, between the GPU batch kernel and `permanent_bipedal5_singleword`.
///
/// # Safety
///
/// Must only be called on a host with a live ROCm/HIP context.
unsafe fn run_bit_identity_check(n: usize, m_count: usize, seed: u64) {
    assert!((1..=63).contains(&n), "n must be in 1..=63");
    assert!(m_count >= 1, "m_count must be >= 1");

    let mat_bytes = n * n; // bytes per matrix (one u8 per GF(5) element)

    let mut rng = seed;
    let mut host_matrices: Vec<u8> = Vec::with_capacity(m_count * mat_bytes);
    for _ in 0..(m_count * n * n) {
        host_matrices.push(rand_fp5(&mut rng));
    }

    let cpu_results: Vec<u64> = (0..m_count)
        .map(|i| {
            let slice = &host_matrices[i * mat_bytes..(i + 1) * mat_bytes];
            let fp5_data: Vec<Fp<5>> = slice.iter().map(|&v| Fp::<5>::new(v as u64)).collect();
            let mat = Packed5Matrix::from_row_major(&fp5_data, n, n);
            permanent_bipedal5_singleword(&mat).value()
        })
        .collect();

    // SAFETY: requires a live HIP device context; host_matrices has m_count*n*n bytes.
    let gpu_results = run_with_device_buffers(&host_matrices, n, m_count, |d_in, d_out| {
        // SAFETY: d_in/d_out are valid device allocations; n,m_count validated above.
        let rc = unsafe { compute_permanent_gf5_batch(d_in, n as c_int, m_count as c_int, d_out) };
        assert_eq!(rc, 0, "permanent_bipedal5_hip_batch failed: code {rc}");
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
fn test_permanent_bipedal5_gpu_bit_identity_n8() {
    // SAFETY: requires a live HIP device context with gfx1030 support.
    unsafe { run_bit_identity_check(8, 100, 0xF5CA_FEDE_ADBE_EF08u64) }
}

#[test]
#[ignore = "external: gfx1030 device required"]
fn test_permanent_bipedal5_gpu_bit_identity_n12() {
    // SAFETY: requires a live HIP device context with gfx1030 support.
    unsafe { run_bit_identity_check(12, 100, 0xF5CA_FEDE_ADBE_EF12u64) }
}
