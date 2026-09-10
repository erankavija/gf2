//! Demo + mutation-test driver for the `simd_equiv` helper module.
//!
//! The helper itself lives in `tests/simd_equiv/mod.rs` and is loaded
//! here via the standard `mod simd_equiv;` integration-test
//! convention. This file contains:
//!
//! 1. A working demo — `LogicalFns::xor_fn` (AVX2) versus a scalar
//!    in-place XOR — that exercises the helper's full API surface
//!    (`assert_simd_matches_scalar`, `WORD_BOUNDARY_LENGTHS`,
//!    `unaligned_slice`).
//! 2. A mutation test that wires a deliberately-broken scalar XOR
//!    into the helper and asserts the helper PANICS, proving the
//!    helper actually compares output rather than silently passing.
//!
//! Tier A–D kernels under the PPC-spiral epic should follow the same
//! shape.

mod simd_equiv;

use proptest::prelude::any;
use proptest::strategy::Strategy;

use simd_equiv::{assert_simd_matches_scalar, unaligned_slice, WORD_BOUNDARY_LENGTHS};

/// Reference scalar XOR — the canonical baseline used everywhere a
/// kernel claims SIMD-equivalent behaviour. Mirrors the in-place
/// signature of `LogicalFns::xor_fn`.
fn scalar_xor_inplace(dst: &mut [u64], src: &[u64]) {
    let n = dst.len().min(src.len());
    for i in 0..n {
        dst[i] ^= src[i];
    }
}

/// Build a proptest strategy that yields `(dst, src)` pairs of
/// `len` words each, drawn from `any::<u64>()`.
fn xor_pair_strategy(len: usize) -> impl Strategy<Value = (Vec<u64>, Vec<u64>)> {
    (
        proptest::collection::vec(any::<u64>(), len..=len),
        proptest::collection::vec(any::<u64>(), len..=len),
    )
}

/// AVX2-backed XOR via the safe-wrapper API in
/// `gf2_core::kernels::simd::maybe_simd()`. Returns `None` if the
/// host has no SIMD backend; tests skip in that case.
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

    // Run the helper at a representative length (256 bits == 4 words);
    // the boundary-list test below covers the rest.
    assert_simd_matches_scalar::<(Vec<u64>, Vec<u64>), (), _, _, _>(
        |pair| {
            let (dst, src) = pair;
            scalar_xor_inplace(dst, src);
        },
        |pair| {
            let (dst, src) = pair;
            // safe-wrapper around AVX2 xor_fn; the boolean is dropped
            // because both branches behave identically when the
            // backend is present (we early-returned above).
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

    // Exercise every canonical boundary length end-to-end. We use a
    // deterministic fill rather than proptest here to keep the test
    // fast and to make any failure trivially reproducible.
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

    // Over-allocate, then slice the same `len` window at offsets 0..7
    // to exercise every alignment class within an AVX-512 cache line.
    const LEN: usize = 16; // 128 bytes — covers AVX2 and exceeds AVX-512 alignment classes
    let mut scalar_back = vec![0u64; 8 + LEN];
    let mut simd_back = vec![0u64; 8 + LEN];
    let src_back: Vec<u64> = (0..(8 + LEN) as u64)
        .map(|i| 0x1234_5678_9abc_def0 ^ i)
        .collect();

    for offset in 0..8 {
        // Reset windows for this iteration.
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

/// Mutation test — the whole point of the helper is to *catch* a
/// broken scalar reference. Wire in a one-bit-flipped variant and
/// confirm `assert_simd_matches_scalar` panics.
#[test]
fn helper_rejects_mutated_scalar() {
    if gf2_core::kernels::simd::maybe_simd().is_none() {
        eprintln!("SIMD backend unavailable on this host — skipping mutation test.");
        return;
    }

    let result = std::panic::catch_unwind(|| {
        assert_simd_matches_scalar::<(Vec<u64>, Vec<u64>), (), _, _, _>(
            // Mutated scalar — flips bit 0 of every word, so it
            // disagrees with any honest XOR for the overwhelming
            // majority of inputs. (The two would only coincide when
            // every src word had bit 0 already set in dst, which a
            // 64-case proptest run will dismiss in case 1.)
            |pair| {
                let (dst, src) = pair;
                let n = dst.len().min(src.len());
                for i in 0..n {
                    dst[i] ^= src[i];
                    dst[i] ^= 1;
                }
            },
            // Honest SIMD reference.
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

/// Sanity test — passing the *same* function as both scalar and SIMD
/// must succeed. Without this, the mutation test could be a false
/// positive (helper always panics).
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
