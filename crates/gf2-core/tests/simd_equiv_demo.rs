//! Driver for the `simd_equiv` helper module: an XOR equivalence demo over
//! its whole API, and a mutation test showing that the helper rejects a
//! deliberately broken scalar reference.

mod simd_equiv;

use proptest::prelude::any;
use proptest::strategy::Strategy;

use simd_equiv::{assert_simd_matches_scalar, unaligned_slice, WORD_BOUNDARY_LENGTHS};

fn scalar_xor_inplace(dst: &mut [u64], src: &[u64]) {
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

/// XORs through the detected SIMD backend; returns `false` when the host
/// has none.
fn simd_xor_inplace(dst: &mut [u64], src: &[u64]) -> bool {
    use gf2_core::kernels::simd::maybe_simd;
    use gf2_core::kernels::Backend;
    if let Some(backend) = maybe_simd() {
        backend.xor(dst, src);
        true
    } else {
        false
    }
}

#[test]
fn demo_xor_avx2_matches_scalar_proptest() {
    if gf2_core::kernels::simd::maybe_simd().is_none() {
        eprintln!("SIMD backend unavailable on this host — skipping demo proptest.");
        return;
    }

    assert_simd_matches_scalar::<(Vec<u64>, Vec<u64>), (), _, _, _>(
        |pair| {
            let (dst, src) = pair;
            scalar_xor_inplace(dst, src);
        },
        |pair| {
            let (dst, src) = pair;
            // The backend is present: the caller returned early otherwise.
            let _ = simd_xor_inplace(dst, src);
        },
        xor_pair_strategy(4),
    );
}

#[test]
fn demo_xor_word_boundary_lengths() {
    if gf2_core::kernels::simd::maybe_simd().is_none() {
        eprintln!("SIMD backend unavailable on this host — skipping boundary test.");
        return;
    }

    // Deterministic fill, so a failure is reproducible.
    for &bits in WORD_BOUNDARY_LENGTHS {
        let words = bits.div_ceil(64);
        let mut a_dst: Vec<u64> = (0..words as u64)
            .map(|i| 0xa5a5_a5a5_a5a5_a5a5 ^ i)
            .collect();
        let mut b_dst = a_dst.clone();
        let src: Vec<u64> = (0..words as u64)
            .map(|i| 0x5a5a_5a5a_5a5a_5a5a ^ i)
            .collect();

        scalar_xor_inplace(&mut a_dst, &src);
        let _ = simd_xor_inplace(&mut b_dst, &src);

        assert_eq!(
            a_dst, b_dst,
            "scalar/simd diverged at boundary length {} bits ({} words)",
            bits, words
        );
    }
}

#[test]
fn demo_xor_unaligned_slice_offsets() {
    if gf2_core::kernels::simd::maybe_simd().is_none() {
        eprintln!("SIMD backend unavailable on this host — skipping unaligned-slice test.");
        return;
    }

    // Offsets 0..8 place the window at every word alignment of a 64-byte line.
    const LEN: usize = 16;
    let mut scalar_back = vec![0u64; 8 + LEN];
    let mut simd_back = vec![0u64; 8 + LEN];
    let src_back: Vec<u64> = (0..(8 + LEN) as u64)
        .map(|i| 0x1234_5678_9abc_def0 ^ i)
        .collect();

    for offset in 0..8 {
        for w in scalar_back.iter_mut() {
            *w = 0xdead_beef_cafe_babe;
        }
        for w in simd_back.iter_mut() {
            *w = 0xdead_beef_cafe_babe;
        }

        let s_view = unaligned_slice(&mut scalar_back, offset, LEN);
        let m_view = unaligned_slice(&mut simd_back, offset, LEN);
        let src_view = &src_back[offset..offset + LEN];

        scalar_xor_inplace(s_view, src_view);
        let _ = simd_xor_inplace(m_view, src_view);

        assert_eq!(
            scalar_back, simd_back,
            "scalar/simd diverged at offset {} (LEN={})",
            offset, LEN
        );
    }
}

#[test]
fn helper_rejects_mutated_scalar() {
    if gf2_core::kernels::simd::maybe_simd().is_none() {
        eprintln!("SIMD backend unavailable on this host — skipping mutation test.");
        return;
    }

    let result = std::panic::catch_unwind(|| {
        assert_simd_matches_scalar::<(Vec<u64>, Vec<u64>), (), _, _, _>(
            // Flips bit 0 of every word.
            |pair| {
                let (dst, src) = pair;
                let n = dst.len().min(src.len());
                for i in 0..n {
                    dst[i] ^= src[i];
                    dst[i] ^= 1;
                }
            },
            |pair| {
                let (dst, src) = pair;
                let _ = simd_xor_inplace(dst, src);
            },
            xor_pair_strategy(4),
        );
    });

    assert!(
        result.is_err(),
        "helper failed to detect a one-bit-flipped scalar mutant — \
         the equivalence helper is not actually checking outputs"
    );
}

/// Guards the mutation test against a helper that always panics.
#[test]
fn helper_accepts_matching_implementations() {
    assert_simd_matches_scalar::<(Vec<u64>, Vec<u64>), (), _, _, _>(
        |pair| {
            let (dst, src) = pair;
            scalar_xor_inplace(dst, src);
        },
        |pair| {
            let (dst, src) = pair;
            scalar_xor_inplace(dst, src);
        },
        xor_pair_strategy(4),
    );
}
