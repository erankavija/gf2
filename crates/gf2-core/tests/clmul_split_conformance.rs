//! Conformance of the split long carry-less product: [`clmul_slice`] and
//! [`clmul_slice_with`] at every operand length around each split point, and
//! the const-width API at the long widths, against the portable schoolbook
//! [`clmul_wide_slice_portable`], on the dispatched and the forced portable
//! lanes.

#![cfg(feature = "test-support")]

use gf2_core::gf2m::wide::{
    clmul_slice, clmul_slice_overwrite, clmul_slice_overwrite_with, clmul_slice_with, clmul_wide,
    clmul_wide_slice, clmul_wide_slice_portable, force_scalar_clmul_wide, ClmulSplit,
};
use std::num::NonZeroUsize;
use std::sync::Mutex;

/// Serialises the process-wide portable override against the other tests.
static LANE_MUTEX: Mutex<()> = Mutex::new(());

fn words(n: usize, seed: u64) -> Vec<u64> {
    let mut state = seed;
    (0..n)
        .map(|_| {
            state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = state;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^ (z >> 31)
        })
        .collect()
}

/// The portable schoolbook over slices, one word product at a time.
fn schoolbook(a: &[u64], b: &[u64]) -> Vec<u64> {
    let mut out = vec![0u64; a.len() + b.len()];
    for (i, &x) in a.iter().enumerate() {
        for (j, &y) in b.iter().enumerate() {
            let product = gf2_core::gf2m::barrett::clmul(x, y);
            out[i + j] ^= product as u64;
            out[i + j + 1] ^= (product >> 64) as u64;
        }
    }
    out
}

/// Lengths on both sides of each split: around the kernel widths, their
/// doubles and the powers of two up to 257.
fn lengths() -> Vec<usize> {
    let mut lengths: Vec<usize> = (0..=40).collect();
    for centre in [63, 64, 72, 128, 144, 256] {
        lengths.extend([centre - 1, centre, centre + 1]);
    }
    lengths
}

fn operand_pairs(n: usize) -> Vec<(Vec<u64>, Vec<u64>)> {
    vec![
        (words(n, 3 * n as u64 + 1), words(n, 5 * n as u64 + 7)),
        (vec![u64::MAX; n], vec![u64::MAX; n]),
        (
            (0..n).map(|i| 1u64 << (i % 64)).collect(),
            (0..n).map(|i| 1u64 << 63 >> (i % 64)).collect(),
        ),
    ]
}

/// Every split, structural or overridden, accumulates the schoolbook product
/// into a dirty destination, or replaces it with the product, on both lanes.
#[test]
fn every_split_accumulates_or_writes_the_schoolbook_product_on_both_lanes() {
    let _guard = LANE_MUTEX
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let split = |base: usize, leaf: usize| {
        ClmulSplit::new(
            NonZeroUsize::new(base).unwrap(),
            NonZeroUsize::new(leaf).unwrap(),
        )
    };
    // Seven basecase widths, each aligned to one-word, four-word and
    // eight-word leaves.
    let overrides: Vec<ClmulSplit> = [1, 2, 3, 4, 5, 9, 16]
        .into_iter()
        .flat_map(|base| [1, 4, 8].map(|leaf| split(base, leaf)))
        .collect();
    for forced in [false, true] {
        force_scalar_clmul_wide(forced);
        for n in lengths() {
            for (a, b) in operand_pairs(n) {
                let want = schoolbook(&a, &b);
                let dirty: Vec<u64> = words(2 * n, 11);
                let expected: Vec<u64> = want.iter().zip(&dirty).map(|(w, d)| w ^ d).collect();
                let mut got = dirty.clone();
                clmul_slice(&a, &b, &mut got);
                assert_eq!(got, expected, "structural, n {n}, forced {forced}");
                let mut got = dirty.clone();
                clmul_slice_overwrite(&a, &b, &mut got);
                assert_eq!(got, want, "overwrite, n {n}, forced {forced}");
                for &split in &overrides {
                    let mut got = dirty.clone();
                    clmul_slice_with(&a, &b, &mut got, split);
                    assert_eq!(got, expected, "{split:?}, n {n}, forced {forced}");
                    let mut got = dirty.clone();
                    clmul_slice_overwrite_with(&a, &b, &mut got, split);
                    assert_eq!(got, want, "overwrite {split:?}, n {n}, forced {forced}");
                }
            }
        }
    }
    force_scalar_clmul_wide(false);
}

fn assert_width<const N: usize, const M: usize>() {
    let a: [u64; N] = words(N, N as u64).try_into().unwrap();
    let b: [u64; N] = words(N, 2 * N as u64 + 1).try_into().unwrap();
    let mut want = vec![0u64; M];
    clmul_wide_slice_portable::<N>(&a, &b, &mut want);
    assert_eq!(
        clmul_wide::<N, M>(&a, &b).to_vec(),
        want,
        "clmul_wide::<{N}>"
    );
    let mut got = vec![0u64; M];
    clmul_wide_slice::<N>(&a, &b, &mut got);
    assert_eq!(got, want, "clmul_wide_slice::<{N}>");
}

/// The const-width API returns the portable schoolbook's words at the long
/// widths, on both lanes.
#[test]
fn the_long_widths_match_the_portable_schoolbook_on_both_lanes() {
    let _guard = LANE_MUTEX
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    for forced in [false, true] {
        force_scalar_clmul_wide(forced);
        assert_width::<16, 32>();
        assert_width::<64, 128>();
        assert_width::<256, 512>();
        assert_width::<2048, 4096>();
    }
    force_scalar_clmul_wide(false);
}

/// The structural basecase is the widest dispatched kernel's width, or one
/// word on the portable lane.
#[test]
fn the_structural_basecase_follows_the_dispatched_kernels() {
    let _guard = LANE_MUTEX
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    force_scalar_clmul_wide(true);
    assert_eq!(ClmulSplit::structural().basecase_words(), 1);
    force_scalar_clmul_wide(false);
    let structural = ClmulSplit::structural();
    let basecase = structural.basecase_words();
    let leaf = structural.leaf_words();
    assert_eq!(leaf, if basecase == 9 { 8 } else { basecase });
    if cfg!(feature = "simd") && gf2_kernels_simd::gf2m_wide::detect_571().is_some() {
        assert_eq!(basecase, 9);
    } else if cfg!(feature = "simd") && gf2_kernels_simd::gf2m_wide::detect().is_some() {
        assert_eq!(basecase, 4);
    } else {
        assert_eq!(basecase, 1);
    }
}

#[test]
#[should_panic(expected = "operand lengths")]
fn unequal_operands_panic() {
    clmul_slice(&[1, 2], &[3], &mut [0; 3]);
}

#[test]
#[should_panic(expected = "product length")]
fn a_short_destination_panics() {
    clmul_slice(&[1, 2], &[3, 4], &mut [0; 3]);
}

/// Lengths whose halving does not land on whole leaves take the cheaper of
/// halving and the aligned split, and still return the schoolbook product.
#[test]
fn lengths_off_the_leaf_grid_match_the_schoolbook() {
    let _guard = LANE_MUTEX
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    force_scalar_clmul_wide(false);
    for n in [1000, 1025, 1152, 1536, 1728] {
        let (a, b) = (words(n, n as u64), words(n, 3 * n as u64 + 1));
        let mut got = vec![u64::MAX; 2 * n];
        clmul_slice_overwrite(&a, &b, &mut got);
        assert_eq!(got, schoolbook(&a, &b), "n {n}");
    }
}
