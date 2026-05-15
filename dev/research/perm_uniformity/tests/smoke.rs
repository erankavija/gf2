// Smoke tests for perm-uniformity harness (JIT 8e4e19a0).
//
// Each test uses small cells (q in {3,5,7}, n in {4,6}, N=1000) that complete
// well within the 5 s fast-tier budget.
//
// Tests verify:
//   1. Determinism: same seed -> bit-identical TVD.
//   2. TVD is in [0, 0.5] for random matrices.
//   3. The perm and det kernels are reachable (no link errors).

use gf2_algebra::packed::{Bipedal3Matrix, Packed5Matrix, Packed7Matrix};
use gf2_algebra::permanent::{permanent_bipedal3, permanent_bipedal5, permanent_bipedal7};
use gf2_core::field::inverse::det;
use gf2_core::field::matrix::FieldMatrix;
use gf2_core::gfp::Fp;
use gf2_core::rng::Lcg;

const SMOKE_SEED: u64 = 0xdead_beef_cafe_1234_u64;
const SMOKE_N: usize = 1000;

/// Compute TVD of empirical counts from uniform over q elements.
fn tvd(counts: &[u64], n_total: u64, q: usize) -> f64 {
    let uniform = 1.0 / q as f64;
    let mut sum = 0.0f64;
    for &c in counts {
        sum += (c as f64 / n_total as f64 - uniform).abs();
    }
    0.5 * sum
}

/// Run a small F_3 cell and return (tvd_perm, tvd_det).
fn cell_f3(n: usize, seed: u64) -> (f64, f64) {
    let mut rng = Lcg::new(seed);
    let mut perm_counts = [0u64; 3];
    let mut det_counts = [0u64; 3];
    for _ in 0..SMOKE_N {
        let flat: Vec<Fp<3>> = (0..n * n)
            .map(|_| Fp::<3>::new(rng.next_u64() % 3))
            .collect();
        let mat = Bipedal3Matrix::from_row_major(&flat, n, n);
        let p = permanent_bipedal3(&mat);
        perm_counts[p.value() as usize] += 1;
    }
    let mut rng2 = Lcg::new(seed.wrapping_add(1));
    for _ in 0..SMOKE_N {
        let mut mat = FieldMatrix::<Fp<3>>::zeros(n, n);
        for r in 0..n {
            for c in 0..n {
                mat.set(r, c, Fp::<3>::new(rng2.next_u64() % 3));
            }
        }
        let d = det(&mat);
        det_counts[d.value() as usize] += 1;
    }
    (
        tvd(&perm_counts, SMOKE_N as u64, 3),
        tvd(&det_counts, SMOKE_N as u64, 3),
    )
}

/// Run a small F_5 cell and return (tvd_perm, tvd_det).
fn cell_f5(n: usize, seed: u64) -> (f64, f64) {
    let mut rng = Lcg::new(seed);
    let mut perm_counts = [0u64; 5];
    let mut det_counts = [0u64; 5];
    for _ in 0..SMOKE_N {
        let flat: Vec<Fp<5>> = (0..n * n)
            .map(|_| Fp::<5>::new(rng.next_u64() % 5))
            .collect();
        let mat = Packed5Matrix::from_row_major(&flat, n, n);
        let p = permanent_bipedal5(&mat);
        perm_counts[p.value() as usize] += 1;
    }
    let mut rng2 = Lcg::new(seed.wrapping_add(1));
    for _ in 0..SMOKE_N {
        let mut mat = FieldMatrix::<Fp<5>>::zeros(n, n);
        for r in 0..n {
            for c in 0..n {
                mat.set(r, c, Fp::<5>::new(rng2.next_u64() % 5));
            }
        }
        let d = det(&mat);
        det_counts[d.value() as usize] += 1;
    }
    (
        tvd(&perm_counts, SMOKE_N as u64, 5),
        tvd(&det_counts, SMOKE_N as u64, 5),
    )
}

/// Run a small F_7 cell and return (tvd_perm, tvd_det).
fn cell_f7(n: usize, seed: u64) -> (f64, f64) {
    let mut rng = Lcg::new(seed);
    let mut perm_counts = [0u64; 7];
    let mut det_counts = [0u64; 7];
    for _ in 0..SMOKE_N {
        let flat: Vec<Fp<7>> = (0..n * n)
            .map(|_| Fp::<7>::new(rng.next_u64() % 7))
            .collect();
        let mat = Packed7Matrix::from_row_major(&flat, n, n);
        let p = permanent_bipedal7(&mat);
        perm_counts[p.value() as usize] += 1;
    }
    let mut rng2 = Lcg::new(seed.wrapping_add(1));
    for _ in 0..SMOKE_N {
        let mut mat = FieldMatrix::<Fp<7>>::zeros(n, n);
        for r in 0..n {
            for c in 0..n {
                mat.set(r, c, Fp::<7>::new(rng2.next_u64() % 7));
            }
        }
        let d = det(&mat);
        det_counts[d.value() as usize] += 1;
    }
    (
        tvd(&perm_counts, SMOKE_N as u64, 7),
        tvd(&det_counts, SMOKE_N as u64, 7),
    )
}

// ── Correctness range checks ─────────────────────────────────────────────────

#[test]
fn test_tvd_f3_n4_in_range() {
    let (tvd_perm, tvd_det) = cell_f3(4, SMOKE_SEED);
    assert!(
        (0.0..=0.5).contains(&tvd_perm),
        "TVD_perm out of [0,0.5]: {tvd_perm}"
    );
    assert!(
        (0.0..=0.5).contains(&tvd_det),
        "TVD_det out of [0,0.5]: {tvd_det}"
    );
}

#[test]
fn test_tvd_f3_n6_in_range() {
    let (tvd_perm, tvd_det) = cell_f3(6, SMOKE_SEED.wrapping_add(10));
    assert!((0.0..=0.5).contains(&tvd_perm));
    assert!((0.0..=0.5).contains(&tvd_det));
}

#[test]
fn test_tvd_f5_n4_in_range() {
    let (tvd_perm, tvd_det) = cell_f5(4, SMOKE_SEED.wrapping_add(20));
    assert!((0.0..=0.5).contains(&tvd_perm));
    assert!((0.0..=0.5).contains(&tvd_det));
}

#[test]
fn test_tvd_f5_n6_in_range() {
    let (tvd_perm, tvd_det) = cell_f5(6, SMOKE_SEED.wrapping_add(30));
    assert!((0.0..=0.5).contains(&tvd_perm));
    assert!((0.0..=0.5).contains(&tvd_det));
}

#[test]
fn test_tvd_f7_n4_in_range() {
    let (tvd_perm, tvd_det) = cell_f7(4, SMOKE_SEED.wrapping_add(40));
    assert!((0.0..=0.5).contains(&tvd_perm));
    assert!((0.0..=0.5).contains(&tvd_det));
}

#[test]
fn test_tvd_f7_n6_in_range() {
    let (tvd_perm, tvd_det) = cell_f7(6, SMOKE_SEED.wrapping_add(50));
    assert!((0.0..=0.5).contains(&tvd_perm));
    assert!((0.0..=0.5).contains(&tvd_det));
}

// ── Determinism checks ────────────────────────────────────────────────────────

/// Same seed must produce bit-identical TVD for F_3.
#[test]
fn test_determinism_f3() {
    let (a_perm, a_det) = cell_f3(6, SMOKE_SEED.wrapping_add(100));
    let (b_perm, b_det) = cell_f3(6, SMOKE_SEED.wrapping_add(100));
    assert_eq!(
        a_perm.to_bits(),
        b_perm.to_bits(),
        "F_3 perm TVD not deterministic"
    );
    assert_eq!(
        a_det.to_bits(),
        b_det.to_bits(),
        "F_3 det TVD not deterministic"
    );
}

/// Same seed must produce bit-identical TVD for F_5.
#[test]
fn test_determinism_f5() {
    let (a_perm, a_det) = cell_f5(6, SMOKE_SEED.wrapping_add(200));
    let (b_perm, b_det) = cell_f5(6, SMOKE_SEED.wrapping_add(200));
    assert_eq!(
        a_perm.to_bits(),
        b_perm.to_bits(),
        "F_5 perm TVD not deterministic"
    );
    assert_eq!(
        a_det.to_bits(),
        b_det.to_bits(),
        "F_5 det TVD not deterministic"
    );
}

/// Same seed must produce bit-identical TVD for F_7.
#[test]
fn test_determinism_f7() {
    let (a_perm, a_det) = cell_f7(6, SMOKE_SEED.wrapping_add(300));
    let (b_perm, b_det) = cell_f7(6, SMOKE_SEED.wrapping_add(300));
    assert_eq!(
        a_perm.to_bits(),
        b_perm.to_bits(),
        "F_7 perm TVD not deterministic"
    );
    assert_eq!(
        a_det.to_bits(),
        b_det.to_bits(),
        "F_7 det TVD not deterministic"
    );
}
