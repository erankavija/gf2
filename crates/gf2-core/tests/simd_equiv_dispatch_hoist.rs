//! Equivalence coverage for `resolve_xor_inplace`, the XOR callable that M4RM
//! hot loops bind once and reuse, against a word loop local to this file. The
//! resolver answers on every host; [`resolvers`] names what it resolves to.

mod simd_equiv;

use gf2_core::kernels::backend::contract::{assert_each, backends, Implementation};
use gf2_core::kernels::ops::resolve_xor_inplace;
use proptest::prelude::any;
use proptest::strategy::Strategy;
use simd_equiv::{assert_simd_matches_scalar, unaligned_slice, WORD_BOUNDARY_LENGTHS};

type Resolver = fn(usize) -> fn(&mut [u64], &[u64]);

/// The resolver under test, labelled with the backends it selects between on
/// this host.
fn resolvers() -> Vec<Implementation<Resolver>> {
    let selected: Vec<String> = backends().into_iter().map(|entry| entry.label).collect();
    vec![Implementation::new(
        format!("resolve_xor_inplace over {}", selected.join(", ")),
        resolve_xor_inplace as Resolver,
    )]
}

/// The definition the resolved callable is compared with.
fn word_loop_xor(dst: &mut [u64], src: &[u64]) {
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
fn hoisted_xor_dispatch_matches_the_word_loop_proptest() {
    assert_each(&resolvers(), |resolve| {
        assert_simd_matches_scalar::<(Vec<u64>, Vec<u64>), (), _, _, _>(
            |(dst, src)| word_loop_xor(dst, src),
            |(dst, src)| {
                let xor = resolve(dst.len());
                xor(dst, src);
            },
            xor_pair_strategy(),
        );
    });
}

#[test]
fn hoisted_xor_dispatch_word_boundaries() {
    assert_each(&resolvers(), |resolve| {
        let word_lengths = WORD_BOUNDARY_LENGTHS
            .iter()
            .map(|bits| bits.div_ceil(64))
            .chain([7, 8, 9, 15, 16, 63, 64, 65]);

        for words in word_lengths {
            let mut expected: Vec<u64> = (0..words as u64)
                .map(|i| 0x1357_9bdf_2468_ace0 ^ i.rotate_left(7))
                .collect();
            let mut hoisted_dst = expected.clone();
            let src: Vec<u64> = (0..words as u64)
                .map(|i| 0xfedc_ba98_7654_3210 ^ i.rotate_left(11))
                .collect();

            word_loop_xor(&mut expected, &src);
            let xor = resolve(words);
            xor(&mut hoisted_dst, &src);

            assert_eq!(
                expected, hoisted_dst,
                "resolved XOR diverged at {words} words"
            );
        }
    });
}

#[test]
fn hoisted_xor_dispatch_unaligned_slices() {
    const LEN: usize = 16;
    assert_each(&resolvers(), |resolve| {
        let mut expected_back = vec![0u64; 8 + LEN];
        let mut hoisted_back = vec![0u64; 8 + LEN];
        let src_back: Vec<u64> = (0..(8 + LEN) as u64)
            .map(|i| 0x0123_4567_89ab_cdef ^ i.rotate_left(13))
            .collect();
        let xor = resolve(LEN);

        for offset in 0..8 {
            expected_back.fill(0xaaaa_5555_cccc_3333);
            hoisted_back.fill(0xaaaa_5555_cccc_3333);

            let src_view = &src_back[offset..offset + LEN];
            word_loop_xor(unaligned_slice(&mut expected_back, offset, LEN), src_view);
            xor(unaligned_slice(&mut hoisted_back, offset, LEN), src_view);

            assert_eq!(
                expected_back, hoisted_back,
                "resolved XOR diverged at unaligned offset {offset}"
            );
        }
    });
}
