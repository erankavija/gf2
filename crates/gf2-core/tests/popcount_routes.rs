//! Behavioural suite shared by every population-count and fused
//! AND-population-count route: the scalar backend, the routes
//! `kernels::ops::resolve_popcount` and `resolve_and_popcount` resolve, and
//! each function pointer of the detected kernel bundle. The lengths bracket
//! the empty buffer, the sub-vector widths, the 0/1/63/64/65 word boundaries
//! and the carry-save block boundary, on random, all-zero and all-one data;
//! the fused routes also run on slices at each word offset of their buffers.

mod simd_equiv;

use gf2_core::kernels::ops::{
    and_popcount, and_popcount_route, popcount, popcount_route, resolve_and_popcount,
    resolve_popcount, AndPopcountFn, PopcountFn, PopcountRoute,
};
use gf2_core::kernels::Backend;
use gf2_core::BitVec;

use simd_equiv::unaligned_slice;

/// Words one carry-save block folds on hosts that have the kernel; the suite
/// uses the same figure on every host so the lengths do not vary.
const CSA_BLOCK_WORDS: usize = 64;

fn lengths() -> Vec<usize> {
    vec![
        0,
        1,
        2,
        3,
        4,
        5,
        7,
        8,
        9,
        63,
        64,
        65,
        CSA_BLOCK_WORDS - 1,
        CSA_BLOCK_WORDS,
        CSA_BLOCK_WORDS + 1,
        2 * CSA_BLOCK_WORDS,
        2 * CSA_BLOCK_WORDS + 3,
        1000,
    ]
}

/// Deterministic words, independent of the crate's own generators.
fn seeded(len: usize, seed: u64) -> Vec<u64> {
    let mut state = seed;
    (0..len)
        .map(|_| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            state ^ (state >> 31)
        })
        .collect()
}

fn reference(words: &[u64]) -> u64 {
    words.iter().map(|word| u64::from(word.count_ones())).sum()
}

fn patterns(len: usize, seed: u64) -> Vec<(&'static str, Vec<u64>)> {
    vec![
        ("random", seeded(len, seed)),
        ("all-zero", vec![0; len]),
        ("all-one", vec![u64::MAX; len]),
    ]
}

fn popcount_routes(len: usize) -> Vec<(String, PopcountFn)> {
    let mut routes: Vec<(String, PopcountFn)> = vec![
        ("ops::popcount".to_owned(), popcount as PopcountFn),
        ("resolve_popcount".to_owned(), resolve_popcount(len)),
    ];
    if let Some(fns) = gf2_kernels_simd::detect() {
        routes.push(("bundle popcnt_fn".to_owned(), fns.popcnt_fn));
        routes.push(("bundle popcnt_scalar_fn".to_owned(), fns.popcnt_scalar_fn));
        routes.push(("bundle popcnt_csa_fn".to_owned(), fns.popcnt_csa_fn));
    }
    routes
}

fn and_routes(len: usize) -> Vec<(String, AndPopcountFn)> {
    let mut routes: Vec<(String, AndPopcountFn)> = vec![
        (
            "ops::and_popcount".to_owned(),
            and_popcount as AndPopcountFn,
        ),
        ("resolve_and_popcount".to_owned(), resolve_and_popcount(len)),
    ];
    if let Some(fns) = gf2_kernels_simd::detect() {
        routes.push(("bundle and_popcnt_fn".to_owned(), fns.and_popcnt_fn));
        routes.push(("bundle and_popcnt_csa_fn".to_owned(), fns.and_popcnt_csa_fn));
    }
    routes
}

#[test]
fn every_population_count_route_reproduces_the_reference() {
    for len in lengths() {
        for (label, words) in patterns(len, 0x5cbb_6545 ^ len as u64) {
            let expected = reference(&words);
            assert_eq!(
                gf2_core::kernels::scalar::SCALAR_BACKEND.popcount(&words),
                expected,
                "scalar backend, {label} length {len}"
            );
            for (route, count) in popcount_routes(len) {
                assert_eq!(count(&words), expected, "{route}, {label} length {len}");
            }
        }
    }
}

#[test]
fn every_fused_route_counts_the_intersection() {
    for len in lengths() {
        let lhs = seeded(len, 0xa1 ^ len as u64);
        let rhs = seeded(len, 0xb2 ^ len as u64);
        let expected: u64 = lhs
            .iter()
            .zip(&rhs)
            .map(|(left, right)| u64::from((left & right).count_ones()))
            .sum();
        for (route, and_count) in and_routes(len) {
            assert_eq!(and_count(&lhs, &rhs), expected, "{route}, length {len}");
        }
    }
}

#[test]
fn every_fused_route_counts_slices_at_each_word_offset() {
    for len in lengths() {
        let mut lhs_buf = seeded(len + 3, 0xe5 ^ len as u64);
        let mut rhs_buf = seeded(len + 3, 0xf6 ^ len as u64);
        for lhs_offset in 0..4 {
            for rhs_offset in 0..4 {
                let lhs = &*unaligned_slice(&mut lhs_buf, lhs_offset, len);
                let rhs = &*unaligned_slice(&mut rhs_buf, rhs_offset, len);
                let expected: u64 = lhs
                    .iter()
                    .zip(rhs)
                    .map(|(left, right)| u64::from((left & right).count_ones()))
                    .sum();
                for (route, and_count) in and_routes(len) {
                    assert_eq!(
                        and_count(lhs, rhs),
                        expected,
                        "{route}, length {len}, offsets {lhs_offset} and {rhs_offset}"
                    );
                }
            }
        }
    }
}

#[test]
fn every_fused_route_covers_the_shorter_operand() {
    let long = vec![u64::MAX; 2 * CSA_BLOCK_WORDS + 3];
    for short_len in [0, 1, 5, CSA_BLOCK_WORDS + 1] {
        let short = vec![u64::MAX; short_len];
        let expected = (short_len * 64) as u64;
        for (route, and_count) in and_routes(short_len) {
            assert_eq!(
                and_count(&long, &short),
                expected,
                "{route}, long-first, shorter length {short_len}"
            );
            assert_eq!(
                and_count(&short, &long),
                expected,
                "{route}, short-first, shorter length {short_len}"
            );
        }
    }
}

#[test]
fn automatic_routes_retain_the_established_implementations() {
    for words in [0, 1, 7] {
        assert_eq!(popcount_route(words), PopcountRoute::Scalar);
        assert_eq!(and_popcount_route(words), PopcountRoute::Scalar);
    }

    // A build without the `simd` feature resolves every width to scalar.
    let expected = if cfg!(feature = "simd") && gf2_kernels_simd::detect().is_some() {
        PopcountRoute::SimdNibbleLut
    } else {
        PopcountRoute::Scalar
    };
    for words in [8, 255, 256, 257, 1024, 16_384] {
        assert_eq!(popcount_route(words), expected, "popcount at {words} words");
        assert_eq!(
            and_popcount_route(words),
            expected,
            "fused count at {words} words"
        );
    }
}

#[test]
fn bit_vector_counts_honour_canonical_indexing_and_zero_tail_padding() {
    for len in [0, 1, 63, 64, 65, 127, 128, 129, 4096, 4097] {
        let mut all_set = BitVec::zeros(len);
        for index in 0..len {
            all_set.set(index, true);
        }
        assert_eq!(all_set.count_ones(), len, "every bit set, length {len}");

        // Only the canonical index is counted: the padding beyond `len` bits
        // stays zero, so a full trailing word never adds to the count.
        let mut single = BitVec::zeros(len);
        if len > 0 {
            single.set(len - 1, true);
            assert_eq!(single.count_ones(), 1, "last bit only, length {len}");
            assert!(single.get(len - 1));
        } else {
            assert_eq!(single.count_ones(), 0);
        }
    }
}

#[test]
fn matrix_vector_parity_agrees_across_strides_that_change_the_route() {
    use gf2_core::BitMatrix;

    for cols in [64, 512, CSA_BLOCK_WORDS * 64, CSA_BLOCK_WORDS * 64 + 193] {
        let rows = 9;
        let mut matrix = BitMatrix::zeros(rows, cols);
        let mut x = BitVec::zeros(cols);
        let entries = seeded(rows * cols.div_ceil(64), 0xc3 ^ cols as u64);
        for row in 0..rows {
            for col in 0..cols {
                let word = entries[(row * cols.div_ceil(64)) + col / 64];
                matrix.set(row, col, word >> (col % 64) & 1 == 1);
            }
        }
        let x_words = seeded(cols.div_ceil(64), 0xd4 ^ cols as u64);
        for col in 0..cols {
            x.set(col, x_words[col / 64] >> (col % 64) & 1 == 1);
        }

        let y = matrix.matvec(&x);
        for row in 0..rows {
            let parity = (0..cols)
                .filter(|&col| matrix.get(row, col) && x.get(col))
                .count()
                % 2
                == 1;
            assert_eq!(y.get(row), parity, "row {row} of a {cols}-column product");
        }
    }
}
