//! Driver for the `simd_equiv` helper module: an XOR equivalence demo over
//! its whole API, and a mutation test showing that the helper rejects a
//! deliberately broken implementation. Each test runs every [`backends`]
//! entry against a word loop local to this file; the scalar backend is in the
//! list on every host.

mod simd_equiv;

use gf2_core::kernels::backend::contract::{assert_each, backends};
use proptest::prelude::any;
use proptest::strategy::Strategy;

use simd_equiv::{assert_simd_matches_scalar, unaligned_slice, WORD_BOUNDARY_LENGTHS};

/// The definition the backends are compared with.
fn word_loop_xor(dst: &mut [u64], src: &[u64]) {
    let n = dst.len().min(src.len());
    for i in 0..n {
        dst[i] ^= src[i];
    }
}

fn xor_pair_strategy(len: usize) -> impl Strategy<Value = (Vec<u64>, Vec<u64>)> {
    (
        proptest::collection::vec(any::<u64>(), len..=len),
        proptest::collection::vec(any::<u64>(), len..=len),
    )
}

/// Also guards the mutation test against a helper that always panics.
#[test]
fn demo_xor_matches_the_word_loop_proptest() {
    assert_each(&backends(), |backend| {
        assert_simd_matches_scalar::<(Vec<u64>, Vec<u64>), (), _, _, _>(
            |(dst, src)| word_loop_xor(dst, src),
            |(dst, src)| backend.xor(dst, src),
            xor_pair_strategy(4),
        );
    });
}

#[test]
fn demo_xor_word_boundary_lengths() {
    assert_each(&backends(), |backend| {
        // Deterministic fill, so a failure is reproducible.
        for &bits in WORD_BOUNDARY_LENGTHS {
            let words = bits.div_ceil(64);
            let mut expected: Vec<u64> = (0..words as u64)
                .map(|i| 0xa5a5_a5a5_a5a5_a5a5 ^ i)
                .collect();
            let mut got = expected.clone();
            let src: Vec<u64> = (0..words as u64)
                .map(|i| 0x5a5a_5a5a_5a5a_5a5a ^ i)
                .collect();

            word_loop_xor(&mut expected, &src);
            backend.xor(&mut got, &src);

            assert_eq!(
                expected,
                got,
                "{} diverged at boundary length {bits} bits ({words} words)",
                backend.name()
            );
        }
    });
}

#[test]
fn demo_xor_unaligned_slice_offsets() {
    // Offsets 0..8 place the window at every word alignment of a 64-byte line.
    const LEN: usize = 16;
    assert_each(&backends(), |backend| {
        let mut expected_back = vec![0u64; 8 + LEN];
        let mut got_back = vec![0u64; 8 + LEN];
        let src_back: Vec<u64> = (0..(8 + LEN) as u64)
            .map(|i| 0x1234_5678_9abc_def0 ^ i)
            .collect();

        for offset in 0..8 {
            expected_back.fill(0xdead_beef_cafe_babe);
            got_back.fill(0xdead_beef_cafe_babe);

            let src_view = &src_back[offset..offset + LEN];
            word_loop_xor(unaligned_slice(&mut expected_back, offset, LEN), src_view);
            backend.xor(unaligned_slice(&mut got_back, offset, LEN), src_view);

            assert_eq!(
                expected_back,
                got_back,
                "{} diverged at offset {offset} (LEN={LEN})",
                backend.name()
            );
        }
    });
}

#[test]
fn helper_rejects_a_mutated_reference() {
    assert_each(&backends(), |backend| {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            assert_simd_matches_scalar::<(Vec<u64>, Vec<u64>), (), _, _, _>(
                // Flips bit 0 of every word.
                |(dst, src)| {
                    word_loop_xor(dst, src);
                    for word in dst.iter_mut() {
                        *word ^= 1;
                    }
                },
                |(dst, src)| backend.xor(dst, src),
                xor_pair_strategy(4),
            );
        }));

        assert!(
            result.is_err(),
            "the helper accepted a reference with one bit flipped against {}",
            backend.name()
        );
    });
}
