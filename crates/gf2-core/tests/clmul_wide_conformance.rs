//! Shared conformance suite for the wide carry-less product: the public
//! [`clmul_wide`] and [`clmul_wide_slice`] API, the capability dispatch and the
//! forced portable fallback are compared, over the complete double-width
//! output, against the independent shift-and-XOR oracle [`bitwise_product`].

#![cfg(feature = "test-support")]

use gf2_core::gf2m::wide::{
    clmul_wide, clmul_wide_slice, force_scalar_clmul_wide, last_clmul_wide_lane, PORTABLE_LANE,
};
use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
use std::sync::Mutex;

/// Serialises `force_scalar_clmul_wide` toggle-and-observe sections: the
/// override is process-wide, so a test asserting which lane
/// [`last_clmul_wide_lane`] reports could otherwise observe another thread's
/// toggle.
static DISPATCH_LANE_MUTEX: Mutex<()> = Mutex::new(());

const RANDOM_PAIRS: usize = 48;

/// SplitMix64 (`@/citation/Steele2014`), the seeded generator the operand
/// fixtures draw from.
struct SplitMix64(u64);

impl SplitMix64 {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

/// Independent oracle: the carry-less product of `a` and `b` as `2 * N` words,
/// by shift-and-XOR over the set bits of `b` (bit `i` lives at
/// `words[i >> 6] >> (i & 63) & 1`).
fn bitwise_product<const N: usize>(a: &[u64; N], b: &[u64; N]) -> Vec<u64> {
    let mut out = vec![0u64; 2 * N];
    for bit in 0..(64 * N) {
        if (b[bit >> 6] >> (bit & 63)) & 1 == 0 {
            continue;
        }
        let word_shift = bit >> 6;
        let bit_shift = bit & 63;
        for (index, operand) in a.iter().enumerate() {
            out[index + word_shift] ^= operand << bit_shift;
            if bit_shift != 0 {
                out[index + word_shift + 1] ^= operand >> (64 - bit_shift);
            }
        }
    }
    out
}

/// Operand pairs that stress carry propagation, lane tails and boundaries.
fn adversarial_pairs<const N: usize>() -> Vec<([u64; N], [u64; N])> {
    let zero = [0u64; N];
    let one = {
        let mut words = [0u64; N];
        words[0] = 1;
        words
    };
    let ones = [u64::MAX; N];
    let alternating = [0x5555_5555_5555_5555u64; N];
    let inverted = [0xAAAA_AAAA_AAAA_AAAAu64; N];
    let low_bits = {
        // The word-boundary bits 0, 1, 63, 64 and 65.
        let mut words = [0u64; N];
        words[0] = 0b11 | (1 << 63);
        if N > 1 {
            words[1] = 0b11;
        }
        words
    };
    let top_bit = {
        let mut words = [0u64; N];
        words[N - 1] = 1 << 63;
        words
    };
    let sparse = {
        let mut words = [0u64; N];
        for (index, word) in words.iter_mut().enumerate() {
            *word = 1u64 << ((index * 17) % 64);
        }
        words
    };
    vec![
        (zero, ones),
        (ones, zero),
        (one, ones),
        (ones, one),
        (ones, ones),
        (alternating, inverted),
        (inverted, alternating),
        (low_bits, low_bits),
        (top_bit, top_bit),
        (top_bit, ones),
        (sparse, ones),
        (sparse, sparse),
        (alternating, ones),
    ]
}

fn random_pairs<const N: usize>(seed: u64) -> Vec<([u64; N], [u64; N])> {
    let mut mixer = SplitMix64(seed);
    (0..RANDOM_PAIRS)
        .map(|_| {
            let mut a = [0u64; N];
            let mut b = [0u64; N];
            for word in a.iter_mut().chain(b.iter_mut()) {
                *word = mixer.next_u64();
            }
            (a, b)
        })
        .collect()
}

fn pairs<const N: usize>(seed: u64) -> Vec<([u64; N], [u64; N])> {
    let mut all = adversarial_pairs::<N>();
    all.extend(random_pairs::<N>(seed));
    all
}

/// Asserts that both public entry points agree with the oracle at width `N`,
/// over the complete `M == 2 * N`-word output, and that the slice form's
/// XOR-accumulating contract holds on a dirty destination.
fn check_public_product<const N: usize, const M: usize>(seed: u64) {
    for (a, b) in pairs::<N>(seed) {
        let expected = bitwise_product::<N>(&a, &b);

        let owned = clmul_wide::<N, M>(&a, &b);
        assert_eq!(
            owned.as_slice(),
            expected.as_slice(),
            "clmul_wide::<{N}, {M}> disagrees with the oracle for a={a:?}, b={b:?}"
        );

        let mut fresh = vec![0u64; M];
        clmul_wide_slice::<N>(&a, &b, &mut fresh);
        assert_eq!(
            fresh.as_slice(),
            expected.as_slice(),
            "clmul_wide_slice::<{N}> disagrees with the oracle for a={a:?}, b={b:?}"
        );

        // The slice form XOR-accumulates: a destination holding `prior` must
        // come back holding `prior ^ product`.
        let prior: Vec<u64> = (0..M)
            .map(|i| 0x0F0F_0F0F_0F0F_0F0Fu64 ^ i as u64)
            .collect();
        let mut dirty = prior.clone();
        clmul_wide_slice::<N>(&a, &b, &mut dirty);
        let accumulated: Vec<u64> = prior
            .iter()
            .zip(expected.iter())
            .map(|(p, e)| p ^ e)
            .collect();
        assert_eq!(
            dirty, accumulated,
            "clmul_wide_slice::<{N}> did not XOR-accumulate for a={a:?}, b={b:?}"
        );
    }
}

/// The lane a width is required to run on when the host has the capability
/// and the build enables it.
#[cfg(feature = "simd")]
fn expected_lane<const N: usize>() -> &'static str {
    match N {
        4 => gf2_kernels_simd::gf2m_wide::detect()
            .map(|fns| fns.name)
            .unwrap_or(PORTABLE_LANE),
        9 => gf2_kernels_simd::gf2m_wide::detect_571()
            .map(|fns| fns.name)
            .unwrap_or(PORTABLE_LANE),
        _ => PORTABLE_LANE,
    }
}

/// Without the `simd` feature no width dispatches, whatever the host detects.
#[cfg(not(feature = "simd"))]
fn expected_lane<const N: usize>() -> &'static str {
    PORTABLE_LANE
}

/// Asserts that both public entry points run on the dispatched lane of width
/// `N`, and that forcing the portable fallback moves them onto it without
/// changing a single output word.
fn check_public_dispatch<const N: usize, const M: usize>(seed: u64) {
    let _guard = DISPATCH_LANE_MUTEX
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let lane = expected_lane::<N>();
    eprintln!("dispatch-lane-witness N={N} lane={lane}");
    let (a, b) = pairs::<N>(seed)[0];

    let dispatched_owned = clmul_wide::<N, M>(&a, &b);
    assert_eq!(
        last_clmul_wide_lane(),
        lane,
        "clmul_wide::<{N}, {M}> did not reach the capability dispatch"
    );
    let mut dispatched_slice = vec![0u64; M];
    clmul_wide_slice::<N>(&a, &b, &mut dispatched_slice);
    assert_eq!(
        last_clmul_wide_lane(),
        lane,
        "clmul_wide_slice::<{N}> did not reach the capability dispatch"
    );

    let restore = force_scalar_clmul_wide(true);
    let portable_owned = clmul_wide::<N, M>(&a, &b);
    assert_eq!(
        last_clmul_wide_lane(),
        PORTABLE_LANE,
        "clmul_wide::<{N}, {M}> ignored the forced portable fallback"
    );
    let mut portable_slice = vec![0u64; M];
    clmul_wide_slice::<N>(&a, &b, &mut portable_slice);
    assert_eq!(
        last_clmul_wide_lane(),
        PORTABLE_LANE,
        "clmul_wide_slice::<{N}> ignored the forced portable fallback"
    );
    force_scalar_clmul_wide(restore);

    assert_eq!(
        dispatched_owned.as_slice(),
        portable_owned.as_slice(),
        "clmul_wide::<{N}, {M}> lanes disagree"
    );
    assert_eq!(
        dispatched_slice, portable_slice,
        "clmul_wide_slice::<{N}> lanes disagree"
    );
}

/// Asserts the forced portable fallback produces the oracle's words at width
/// `N` for every operand pair, which is the coverage a host outside
/// `clmul_wide_dispatch`'s dispatch predicate gets from its own dispatch.
fn check_portable_fallback<const N: usize, const M: usize>(seed: u64) {
    let _guard = DISPATCH_LANE_MUTEX
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let restore = force_scalar_clmul_wide(true);
    for (a, b) in pairs::<N>(seed) {
        let expected = bitwise_product::<N>(&a, &b);
        let owned = clmul_wide::<N, M>(&a, &b);
        assert_eq!(
            owned.as_slice(),
            expected.as_slice(),
            "the portable fallback of clmul_wide::<{N}, {M}> disagrees with the oracle"
        );
        assert_eq!(last_clmul_wide_lane(), PORTABLE_LANE);
        let mut slice = vec![0u64; M];
        clmul_wide_slice::<N>(&a, &b, &mut slice);
        assert_eq!(
            slice.as_slice(),
            expected.as_slice(),
            "the portable fallback of clmul_wide_slice::<{N}> disagrees with the oracle"
        );
    }
    force_scalar_clmul_wide(restore);
}

/// GF(2^256) with the `@/citation/Seroussi1998` Table 1 pentanomial for m = 256.
struct Gf2m256Config;

impl Gf2mWideConfig<4> for Gf2m256Config {
    const M: usize = 256;
    const MODULUS: [u64; 4] = [0x425, 0, 0, 0];
    const NAME: &'static str = "Gf2m256Config";
}

#[test]
fn public_product_matches_the_oracle_at_every_width() {
    check_public_product::<1, 2>(0x1C60_2857_0001);
    check_public_product::<2, 4>(0x1C60_2857_0002);
    check_public_product::<3, 6>(0x1C60_2857_0003);
    check_public_product::<4, 8>(0x1C60_2857_0004);
    check_public_product::<5, 10>(0x1C60_2857_0005);
    check_public_product::<8, 16>(0x1C60_2857_0008);
    check_public_product::<9, 18>(0x1C60_2857_0009);
    check_public_product::<16, 32>(0x1C60_2857_0010);
}

#[test]
fn portable_fallback_matches_the_oracle_at_every_width() {
    check_portable_fallback::<1, 2>(0x1C60_2857_0101);
    check_portable_fallback::<2, 4>(0x1C60_2857_0102);
    check_portable_fallback::<4, 8>(0x1C60_2857_0104);
    check_portable_fallback::<9, 18>(0x1C60_2857_0109);
    check_portable_fallback::<16, 32>(0x1C60_2857_0110);
}

#[test]
fn public_product_reaches_capability_dispatch() {
    check_public_dispatch::<1, 2>(0x1C60_2857_0201);
    check_public_dispatch::<2, 4>(0x1C60_2857_0202);
    check_public_dispatch::<4, 8>(0x1C60_2857_0204);
    check_public_dispatch::<9, 18>(0x1C60_2857_0209);
    check_public_dispatch::<16, 32>(0x1C60_2857_0210);
}

/// One switch covers every caller of the canonical dispatch, so `Gf2mWide`
/// multiplication — which reaches it for its product and twice more inside
/// Barrett reduction — moves onto the portable lane with the public API and
/// returns the same field element.
#[test]
fn field_multiplication_shares_the_canonical_dispatch() {
    let _guard = DISPATCH_LANE_MUTEX
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let a = Gf2mWide::<4, Gf2m256Config>::new([
        0xDEAD_BEEF_CAFE_BABE,
        0x0123_4567_89AB_CDEF,
        0xFEDC_BA98_7654_3210,
        0xAAAA_5555_AAAA_5555,
    ]);
    let b = Gf2mWide::<4, Gf2m256Config>::new([
        0x5555_AAAA_5555_AAAA,
        0x1122_3344_5566_7788,
        0xFFFF_FFFF_0000_0000,
        0x0F0F_F0F0_0F0F_F0F0,
    ]);

    let dispatched = a * b;
    assert_eq!(last_clmul_wide_lane(), expected_lane::<4>());

    let restore = force_scalar_clmul_wide(true);
    let portable = a * b;
    assert_eq!(last_clmul_wide_lane(), PORTABLE_LANE);
    force_scalar_clmul_wide(restore);

    assert_eq!(dispatched, portable);
}
