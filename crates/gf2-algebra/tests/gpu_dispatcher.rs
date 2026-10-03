//! Batch GPU dispatcher tests for `gf2_algebra::gpu`: each
//! `permanent_batch_bipedal{3,5,7}` result is compared with a CPU permanent on
//! seeded random matrices. Every test requires a gfx1030 device and is ignored.

#![cfg(feature = "hip")]

use gf2_algebra::packed::Bipedal3Matrix;
use gf2_algebra::permanent::permanent_bipedal3;
use gf2_algebra::testutil::random_matrix;
use gf2_core::gfp::Fp;

#[cfg(feature = "f5")]
use gf2_algebra::packed::Packed5Matrix;
#[cfg(feature = "f5")]
use gf2_algebra::permanent::permanent_bipedal5;

#[cfg(feature = "f7")]
use gf2_algebra::packed::Packed7Matrix;
#[cfg(feature = "f7")]
use gf2_algebra::permanent::ryser::permanent_ryser;

const N: usize = 24;
const M: usize = 1_000;
const SEED: u64 = 0xDEAD_BEEF_u64;

const N_SMOKE: usize = 16;
const M_SMOKE: usize = 100;
const SEED_SMOKE: u64 = 0xC0DE_CAFE_BEEF_5555_u64;

#[test]
#[ignore = "external: gfx1030 device required"]
fn test_permanent_batch_bipedal3_matches_cpu_n24() {
    let mut gpu_inputs: Vec<Bipedal3Matrix> = Vec::with_capacity(M);
    let mut cpu_results: Vec<Fp<3>> = Vec::with_capacity(M);

    for trial in 0..M {
        let seed = SEED.wrapping_add((trial as u64).wrapping_mul(1_000_003));
        let row_major = random_matrix::<3>(N, seed);
        let mat = Bipedal3Matrix::from_row_major(&row_major, N, N);
        cpu_results.push(permanent_bipedal3(&mat));
        gpu_inputs.push(Bipedal3Matrix::from_row_major(&row_major, N, N));
    }

    let gpu_results = gf2_algebra::gpu::permanent_batch_bipedal3(&gpu_inputs);
    assert_eq!(
        gpu_results.len(),
        M,
        "GPU returned {} results, expected {M}",
        gpu_results.len()
    );

    for i in 0..M {
        assert_eq!(
            gpu_results[i],
            cpu_results[i],
            "permanent mismatch at matrix {i}: GPU={} CPU={} (n={N})",
            gpu_results[i].value(),
            cpu_results[i].value()
        );
    }
}

#[cfg(feature = "f5")]
#[test]
#[ignore = "external: gfx1030 device required"]
fn test_permanent_batch_bipedal5_matches_cpu_n24() {
    let mut gpu_inputs: Vec<Packed5Matrix> = Vec::with_capacity(M);
    let mut cpu_results: Vec<Fp<5>> = Vec::with_capacity(M);

    for trial in 0..M {
        let seed = SEED.wrapping_add((trial as u64).wrapping_mul(1_000_003));
        let row_major = random_matrix::<5>(N, seed);
        let mat = Packed5Matrix::from_row_major(&row_major, N, N);
        cpu_results.push(permanent_bipedal5(&mat));
        gpu_inputs.push(Packed5Matrix::from_row_major(&row_major, N, N));
    }

    let gpu_results = gf2_algebra::gpu::permanent_batch_bipedal5(&gpu_inputs);
    assert_eq!(
        gpu_results.len(),
        M,
        "GPU returned {} results, expected {M}",
        gpu_results.len()
    );

    for i in 0..M {
        assert_eq!(
            gpu_results[i],
            cpu_results[i],
            "permanent mismatch at matrix {i}: GPU={} CPU={} (n={N})",
            gpu_results[i].value(),
            cpu_results[i].value()
        );
    }
}

/// The CPU reference is `permanent_ryser::<Fp<7>>` because
/// `permanent_bipedal7` is limited to n <= 16 = Packed7::LANES.
#[cfg(feature = "f7")]
#[test]
#[ignore = "external: gfx1030 device required"]
fn test_permanent_batch_bipedal7_matches_cpu_n24() {
    let mut gpu_inputs: Vec<Packed7Matrix> = Vec::with_capacity(M);
    let mut cpu_results: Vec<Fp<7>> = Vec::with_capacity(M);

    for trial in 0..M {
        let seed = SEED.wrapping_add((trial as u64).wrapping_mul(1_000_003));
        let row_major = random_matrix::<7>(N, seed);
        cpu_results.push(permanent_ryser::<Fp<7>>(&row_major, N));
        let mat = Packed7Matrix::from_row_major(&row_major, N, N);
        gpu_inputs.push(mat);
    }

    let gpu_results = gf2_algebra::gpu::permanent_batch_bipedal7(&gpu_inputs);
    assert_eq!(
        gpu_results.len(),
        M,
        "GPU returned {} results, expected {M}",
        gpu_results.len()
    );

    for i in 0..M {
        assert_eq!(
            gpu_results[i],
            cpu_results[i],
            "permanent mismatch at matrix {i}: GPU={} CPU={} (n={N})",
            gpu_results[i].value(),
            cpu_results[i].value()
        );
    }
}

#[test]
#[ignore = "external: gfx1030 device required"]
fn test_permanent_batch_bipedal3_smoke_n16() {
    let mut gpu_inputs: Vec<Bipedal3Matrix> = Vec::with_capacity(M_SMOKE);
    let mut cpu_results: Vec<Fp<3>> = Vec::with_capacity(M_SMOKE);

    for trial in 0..M_SMOKE {
        let seed = SEED_SMOKE.wrapping_add((trial as u64).wrapping_mul(1_000_003));
        let row_major = random_matrix::<3>(N_SMOKE, seed);
        let mat = Bipedal3Matrix::from_row_major(&row_major, N_SMOKE, N_SMOKE);
        cpu_results.push(permanent_bipedal3(&mat));
        gpu_inputs.push(Bipedal3Matrix::from_row_major(&row_major, N_SMOKE, N_SMOKE));
    }

    let gpu_results = gf2_algebra::gpu::permanent_batch_bipedal3(&gpu_inputs);
    assert_eq!(gpu_results.len(), M_SMOKE);
    for i in 0..M_SMOKE {
        assert_eq!(
            gpu_results[i],
            cpu_results[i],
            "smoke mismatch at matrix {i}: GPU={} CPU={} (n={N_SMOKE})",
            gpu_results[i].value(),
            cpu_results[i].value()
        );
    }
}

#[cfg(feature = "f5")]
#[test]
#[ignore = "external: gfx1030 device required"]
fn test_permanent_batch_bipedal5_smoke_n16() {
    let mut gpu_inputs: Vec<Packed5Matrix> = Vec::with_capacity(M_SMOKE);
    let mut cpu_results: Vec<Fp<5>> = Vec::with_capacity(M_SMOKE);

    for trial in 0..M_SMOKE {
        let seed = SEED_SMOKE.wrapping_add((trial as u64).wrapping_mul(1_000_003));
        let row_major = random_matrix::<5>(N_SMOKE, seed);
        let mat = Packed5Matrix::from_row_major(&row_major, N_SMOKE, N_SMOKE);
        cpu_results.push(permanent_bipedal5(&mat));
        gpu_inputs.push(Packed5Matrix::from_row_major(&row_major, N_SMOKE, N_SMOKE));
    }

    let gpu_results = gf2_algebra::gpu::permanent_batch_bipedal5(&gpu_inputs);
    assert_eq!(gpu_results.len(), M_SMOKE);
    for i in 0..M_SMOKE {
        assert_eq!(
            gpu_results[i],
            cpu_results[i],
            "smoke mismatch at matrix {i}: GPU={} CPU={} (n={N_SMOKE})",
            gpu_results[i].value(),
            cpu_results[i].value()
        );
    }
}

/// Uses `permanent_bipedal7` (CPU single-word, limited to n ≤ 16) as the
/// reference because n=16 = Packed7::LANES is exactly the CPU fast-path limit.
#[cfg(feature = "f7")]
#[test]
#[ignore = "external: gfx1030 device required"]
fn test_permanent_batch_bipedal7_smoke_n16() {
    use gf2_algebra::permanent::permanent_bipedal7;

    let mut gpu_inputs: Vec<Packed7Matrix> = Vec::with_capacity(M_SMOKE);
    let mut cpu_results: Vec<Fp<7>> = Vec::with_capacity(M_SMOKE);

    for trial in 0..M_SMOKE {
        let seed = SEED_SMOKE.wrapping_add((trial as u64).wrapping_mul(1_000_003));
        let row_major = random_matrix::<7>(N_SMOKE, seed);
        let mat = Packed7Matrix::from_row_major(&row_major, N_SMOKE, N_SMOKE);
        cpu_results.push(permanent_bipedal7(&mat));
        gpu_inputs.push(Packed7Matrix::from_row_major(&row_major, N_SMOKE, N_SMOKE));
    }

    let gpu_results = gf2_algebra::gpu::permanent_batch_bipedal7(&gpu_inputs);
    assert_eq!(gpu_results.len(), M_SMOKE);
    for i in 0..M_SMOKE {
        assert_eq!(
            gpu_results[i],
            cpu_results[i],
            "smoke mismatch at matrix {i}: GPU={} CPU={} (n={N_SMOKE})",
            gpu_results[i].value(),
            cpu_results[i].value()
        );
    }
}

/// Device probe reports a usable accelerator on a gating-satisfying host.
///
/// The probe backs the campaign's refusal of a frozen accelerator selection on
/// a host that cannot serve it, so the branch that matters for a dataset is the
/// one asserted here: where the batch dispatchers above run, the probe agrees
/// that they can. Its `false` branch is only reachable on a host without a
/// usable device, which by this file's gating is never the host running it.
#[test]
#[ignore = "external: gfx1030 device required"]
fn test_has_usable_device_reports_true_on_a_device_host() {
    assert!(
        gf2_algebra::gpu::has_usable_device(),
        "probe reported no usable device on a host where the gated batch \
         dispatcher tests in this file run"
    );
}
