//! Equivalence coverage for the hoisted XOR dispatch helper.
//!
//! `resolve_xor_inplace` is the callable that M4RM hot loops bind once and
//! reuse. These tests exercise that resolved path against a scalar reference.

mod simd_equiv;

use gf2_core::kernels::ops::resolve_xor_inplace;
use proptest::prelude::any;
use proptest::strategy::Strategy;
use simd_equiv::{assert_simd_matches_scalar, unaligned_slice, WORD_BOUNDARY_LENGTHS};

fn scalar_xor_inplace(dst: &mut [u64], src: &[u64]) {
    for i in 0..dst.len().min(src.len()) {
        dst[i] ^= src[i];
    }
}

fn xor_pair_strategy() -> impl Strategy<Value = (Vec<u64>, Vec<u64>)> {
    (0usize..=64).prop_flat_map(|len| {
        (
            proptest::collection::vec(any::<u64>(), len..=len),
            proptest::collection::vec(any::<u64>(), len..=len),
        )
    })
}

#[test]
fn hoisted_xor_dispatch_matches_scalar_proptest() {
    if gf2_core::kernels::simd::maybe_simd().is_none() {
        eprintln!("SIMD backend unavailable on this host — skipping hoist proptest.");
        return;
    }

    assert_simd_matches_scalar::<(Vec<u64>, Vec<u64>), (), _, _, _>(
        |pair| {
            let (dst, src) = pair;
            scalar_xor_inplace(dst, src);
        },
        |pair| {
            let (dst, src) = pair;
            let xor = resolve_xor_inplace(dst.len());
            xor(dst, src);
        },
        xor_pair_strategy(),
    );
}

#[test]
fn hoisted_xor_dispatch_word_boundaries() {
    if gf2_core::kernels::simd::maybe_simd().is_none() {
        eprintln!("SIMD backend unavailable on this host — skipping hoist boundary test.");
        return;
    }

    let word_lengths = WORD_BOUNDARY_LENGTHS
        .iter()
        .map(|bits| bits.div_ceil(64))
        .chain([7, 8, 9, 15, 16, 63, 64, 65]);

    for words in word_lengths {
        let mut scalar_dst: Vec<u64> = (0..words as u64)
            .map(|i| 0x1357_9bdf_2468_ace0 ^ i.rotate_left(7))
            .collect();
        let mut hoisted_dst = scalar_dst.clone();
        let src: Vec<u64> = (0..words as u64)
            .map(|i| 0xfedc_ba98_7654_3210 ^ i.rotate_left(11))
            .collect();

        scalar_xor_inplace(&mut scalar_dst, &src);
        let xor = resolve_xor_inplace(words);
        xor(&mut hoisted_dst, &src);

        assert_eq!(
            scalar_dst, hoisted_dst,
            "resolved XOR diverged at {} words",
            words
        );
    }
}

#[test]
fn hoisted_xor_dispatch_unaligned_slices() {
    if gf2_core::kernels::simd::maybe_simd().is_none() {
        eprintln!("SIMD backend unavailable on this host — skipping hoist unaligned test.");
        return;
    }

    const LEN: usize = 16;
    let mut scalar_back = vec![0u64; 8 + LEN];
    let mut hoisted_back = vec![0u64; 8 + LEN];
    let src_back: Vec<u64> = (0..(8 + LEN) as u64)
        .map(|i| 0x0123_4567_89ab_cdef ^ i.rotate_left(13))
        .collect();
    let xor = resolve_xor_inplace(LEN);

    for offset in 0..8 {
        scalar_back.fill(0xaaaa_5555_cccc_3333);
        hoisted_back.fill(0xaaaa_5555_cccc_3333);

        let scalar_view = unaligned_slice(&mut scalar_back, offset, LEN);
        let hoisted_view = unaligned_slice(&mut hoisted_back, offset, LEN);
        let src_view = &src_back[offset..offset + LEN];

        scalar_xor_inplace(scalar_view, src_view);
        xor(hoisted_view, src_view);

        assert_eq!(
            scalar_back, hoisted_back,
            "resolved XOR diverged at unaligned offset {}",
            offset
        );
    }
}
