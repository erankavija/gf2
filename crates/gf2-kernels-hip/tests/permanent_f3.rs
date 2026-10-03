//! GPU bit-identity tests for the F_3 HIP permanent kernel against the CPU
//! reference `permanent_bipedal3_singleword`, for n ∈ {16, 24, 32, 40, 63}.

#![cfg(feature = "hip")]

#[path = "common/mod.rs"]
mod common;

use common::{run_with_device_buffers, xorshift64};
use gf2_algebra::packed::Bipedal3Matrix;
use gf2_algebra::permanent::permanent_bipedal3_singleword;
use gf2_core::gfp::Fp;
use gf2_kernels_hip::permanent::compute_permanent_gf3_batch;
use std::os::raw::c_int;

fn rand_fp3(state: &mut u64) -> u8 {
    // 2^64 mod 3 = 1, so the modulo bias is negligible.
    (xorshift64(state) % 3) as u8
}

/// Compares `m_count` random n×n GF(3) matrices, drawn from xorshift64 state
/// `seed`, between the GPU batch kernel and `permanent_bipedal3_singleword`.
///
/// # Safety
///
/// Must only be called on a host with a live ROCm/HIP context.
unsafe fn run_bit_identity_check(n: usize, m_count: usize, seed: u64) {
    assert!((1..=63).contains(&n), "n must be in 1..=63");
    assert!(m_count >= 1, "m_count must be >= 1");

    let mat_bytes = n * n; // bytes per matrix (one u8 per GF(3) element)

    let mut rng = seed;
    let mut host_matrices: Vec<u8> = Vec::with_capacity(m_count * mat_bytes);
    for _ in 0..(m_count * n * n) {
        host_matrices.push(rand_fp3(&mut rng));
    }

    let cpu_results: Vec<u64> = (0..m_count)
        .map(|i| {
            let slice = &host_matrices[i * mat_bytes..(i + 1) * mat_bytes];
            let fp3_data: Vec<Fp<3>> = slice.iter().map(|&v| Fp::<3>::new(v as u64)).collect();
            let mat = Bipedal3Matrix::from_row_major(&fp3_data, n, n);
            permanent_bipedal3_singleword(&mat).value()
        })
        .collect();

    // SAFETY: requires a live HIP device context; host_matrices has m_count*n*n bytes.
    let gpu_results = run_with_device_buffers(&host_matrices, n, m_count, |d_in, d_out| {
        // SAFETY: d_in/d_out are valid device allocations; n,m_count validated above.
        let rc = unsafe { compute_permanent_gf3_batch(d_in, n as c_int, m_count as c_int, d_out) };
        assert_eq!(rc, 0, "permanent_bipedal3_hip_batch failed: code {rc}");
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
fn test_permanent_bipedal3_gpu_bit_identity_n16() {
    // SAFETY: requires a live HIP device context with gfx1030 support.
    unsafe { run_bit_identity_check(16, 100, 0xDEAD_BEEF_CAFE_1600u64) }
}

#[test]
#[ignore = "external: gfx1030 device required"]
fn test_permanent_bipedal3_gpu_bit_identity_n24() {
    // SAFETY: requires a live HIP device context with gfx1030 support.
    unsafe { run_bit_identity_check(24, 100, 0xDEAD_BEEF_CAFE_2400u64) }
}

#[test]
#[ignore = "external: gfx1030 device required"]
fn test_permanent_bipedal3_gpu_bit_identity_n32() {
    // SAFETY: requires a live HIP device context with gfx1030 support.
    unsafe { run_bit_identity_check(32, 10, 0xDEAD_BEEF_CAFE_3200u64) }
}

#[test]
#[ignore = "external: gfx1030 device required"]
fn test_permanent_bipedal3_gpu_bit_identity_n40() {
    // SAFETY: requires a live HIP device context with gfx1030 support.
    unsafe { run_bit_identity_check(40, 1, 0xDEAD_BEEF_CAFE_4000u64) }
}

/// n=63 is the maximum dimension the GPU kernel supports.
#[test]
#[ignore = "external: gfx1030 device required"]
fn test_permanent_bipedal3_gpu_bit_identity_n63() {
    // SAFETY: requires a live HIP device context with gfx1030 support.
    unsafe { run_bit_identity_check(63, 1, 0xDEAD_BEEF_CAFE_6300u64) }
}
