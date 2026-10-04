//! Population-count and fused AND-population-count kernels for x86-64.
//!
//! [`popcnt_words`] and [`count_ones_words`] count word by word;
//! [`avx2_popcnt_csa`] and [`avx2_and_popcnt_csa`] fold sixteen vectors
//! through Harley-Seal carry-save adders before one nibble lookup
//! (`@/citation/Mula2018`). A buffer shorter than one 512-byte block reaches
//! only their per-vector remainder.

use core::arch::x86_64::*;

/// Words per 256-bit vector.
const WORDS_PER_VECTOR: usize = 4;

/// Vectors folded by one Harley-Seal block: sixteen vectors, 512 bytes.
///
/// One block produces partial sums of weight 1, 2, 4, 8 and 16; only the
/// weight-16 register is looked up, so the block costs one lookup and fifteen
/// carry-save adders instead of sixteen lookups (`@/citation/Mula2018`).
pub const CSA_BLOCK_VECTORS: usize = 16;

/// Words folded by one Harley-Seal block.
pub const CSA_BLOCK_WORDS: usize = CSA_BLOCK_VECTORS * WORDS_PER_VECTOR;

#[inline(always)]
unsafe fn loadu(ptr: *const u8) -> __m256i {
    _mm256_loadu_si256(ptr as *const __m256i)
}

/// The 4-bit population-count lookup table, duplicated across both lanes.
#[inline(always)]
unsafe fn nibble_lut() -> __m256i {
    _mm256_setr_epi8(
        0, 1, 1, 2, 1, 2, 2, 3, 1, 2, 2, 3, 2, 3, 3, 4, 0, 1, 1, 2, 1, 2, 2, 3, 1, 2, 2, 3, 2, 3,
        3, 4,
    )
}

/// Byte-wise population count of one vector, widened into four 64-bit lanes.
///
/// Two `VPSHUFB` lookups and one `VPADDB` give a per-byte count, at most eight
/// per byte; `VPSADBW` against zero sums each 8-byte group into its lane.
#[inline(always)]
unsafe fn lane_sums(v: __m256i, lut: __m256i, mask0f: __m256i, zero: __m256i) -> __m256i {
    let lo = _mm256_and_si256(v, mask0f);
    let hi = _mm256_and_si256(_mm256_srli_epi16(v, 4), mask0f);
    let counted = _mm256_add_epi8(_mm256_shuffle_epi8(lut, lo), _mm256_shuffle_epi8(lut, hi));
    _mm256_sad_epu8(counted, zero)
}

/// Horizontally adds the four 64-bit lanes of `acc`.
///
/// `_mm_extract_epi64` needs SSE4.1, so the high half is read by shifting the
/// 128-bit register instead.
#[inline(always)]
unsafe fn horizontal_sum(acc: __m256i) -> u64 {
    let acc128 = _mm_add_epi64(
        _mm256_castsi256_si128(acc),
        _mm256_extracti128_si256(acc, 1),
    );
    let low = _mm_cvtsi128_si64(acc128) as u64;
    low + (_mm_cvtsi128_si64(_mm_srli_si128(acc128, 8)) as u64)
}

/// One carry-save (full) adder over three bit-planes.
///
/// Returns `(high, low)` where `low` holds the parity of the three inputs and
/// `high` their majority, so `2 * high + low` equals `a + b + c` bit column by
/// bit column (`@/citation/Mula2018`). Five Boolean operations over three
/// live inputs.
#[inline(always)]
unsafe fn csa(a: __m256i, b: __m256i, c: __m256i) -> (__m256i, __m256i) {
    let u = _mm256_xor_si256(a, b);
    let low = _mm256_xor_si256(u, c);
    let high = _mm256_or_si256(_mm256_and_si256(a, b), _mm256_and_si256(u, c));
    (high, low)
}

/// Folds eight bit-planes into the running weight-1, 2 and 4 accumulators and
/// returns the weight-8 carry they produce.
///
/// Seven carry-save adders: two pairs of planes make two weight-2 carries,
/// those make a weight-4 carry, the same again for the second four planes, and
/// the two weight-4 carries make the returned weight-8 carry. Five vector
/// accumulators stay live across the call, which leaves the remaining
/// architectural YMM registers for the eight loaded planes and the lookup
/// constants.
#[inline(always)]
unsafe fn fold_eight(
    ones: &mut __m256i,
    twos: &mut __m256i,
    fours: &mut __m256i,
    planes: [__m256i; 8],
) -> __m256i {
    let (twos_a, low) = csa(*ones, planes[0], planes[1]);
    *ones = low;
    let (twos_b, low) = csa(*ones, planes[2], planes[3]);
    *ones = low;
    let (fours_a, low) = csa(*twos, twos_a, twos_b);
    *twos = low;
    let (twos_a, low) = csa(*ones, planes[4], planes[5]);
    *ones = low;
    let (twos_b, low) = csa(*ones, planes[6], planes[7]);
    *ones = low;
    let (fours_b, low) = csa(*twos, twos_a, twos_b);
    *twos = low;
    let (eights_carry, low) = csa(*fours, fours_a, fours_b);
    *fours = low;
    eights_carry
}

/// Adds the weight-1, 2, 4 and 8 accumulators to a running count.
#[inline(always)]
unsafe fn fold_carries(
    ones: __m256i,
    twos: __m256i,
    fours: __m256i,
    eights: __m256i,
    lut: __m256i,
    mask0f: __m256i,
    zero: __m256i,
) -> u64 {
    horizontal_sum(lane_sums(eights, lut, mask0f, zero)) * 8
        + horizontal_sum(lane_sums(fours, lut, mask0f, zero)) * 4
        + horizontal_sum(lane_sums(twos, lut, mask0f, zero)) * 2
        + horizontal_sum(lane_sums(ones, lut, mask0f, zero))
}

/// Counts the set bits of `buf` one word at a time through the portable
/// `u64::count_ones` lowering.
///
/// This is the bundle's short-buffer route on a host whose capability report
/// lacks `POPCNT`; the compiler lowers it to the SWAR bit-twiddle sequence
/// there.
pub(crate) fn count_ones_words(buf: &[u64]) -> u64 {
    buf.iter().map(|word| u64::from(word.count_ones())).sum()
}

/// Counts the set bits of `buf` with the scalar `POPCNT` instruction.
///
/// # Safety
///
/// The host supports `POPCNT`; the caller establishes that by runtime
/// detection before taking this function's address.
#[target_feature(enable = "popcnt")]
pub(crate) unsafe fn popcnt_words(buf: &[u64]) -> u64 {
    let mut total = 0u64;
    for word in buf {
        total += u64::from(word.count_ones());
    }
    total
}

/// Counts the set bits of the words `buf` holds beyond `whole_vectors`
/// vectors, one word at a time.
///
/// The remainder is at most three words, so the lowering of `count_ones` never
/// dominates a call that reached the vector loop.
#[inline(always)]
unsafe fn word_tail(buf: &[u64], whole_vectors: usize) -> u64 {
    let mut total = 0u64;
    for index in (whole_vectors * WORDS_PER_VECTOR)..buf.len() {
        total += u64::from(buf.get_unchecked(index).count_ones());
    }
    total
}

/// Counts the set bits of `buf` with a Harley-Seal carry-save loop.
///
/// Sixteen vectors per block are folded through fifteen carry-save adders into
/// one weight-16 register, which is the only register looked up inside the
/// loop; the weight-1, 2, 4 and 8 carries live across the whole loop and are
/// counted once at the end. Buffers shorter than one block, and every block
/// remainder, take the per-vector nibble lookup, so the carry-save loop is
/// never entered on a short buffer.
///
/// # Safety
///
/// The host supports AVX2; `super::detect_x86` establishes that before taking
/// this function's address. Every load is unaligned, so `buf` needs no
/// alignment beyond `u64`.
#[target_feature(enable = "avx2")]
pub(crate) unsafe fn avx2_popcnt_csa(buf: &[u64]) -> u64 {
    if buf.is_empty() {
        return 0;
    }
    let ptr = buf.as_ptr() as *const u8;
    let vectors = buf.len() / WORDS_PER_VECTOR;

    let lut = nibble_lut();
    let mask0f = _mm256_set1_epi8(0x0f);
    let zero = _mm256_setzero_si256();

    let mut total = zero;
    let mut ones = zero;
    let mut twos = zero;
    let mut fours = zero;
    let mut eights = zero;

    let blocks = vectors / CSA_BLOCK_VECTORS;
    let mut block = 0usize;
    while block < blocks {
        let base = (block * CSA_BLOCK_VECTORS * 32) as isize;
        let load8 = |first: isize| -> [__m256i; 8] {
            [
                loadu(ptr.offset(first)),
                loadu(ptr.offset(first + 32)),
                loadu(ptr.offset(first + 64)),
                loadu(ptr.offset(first + 96)),
                loadu(ptr.offset(first + 128)),
                loadu(ptr.offset(first + 160)),
                loadu(ptr.offset(first + 192)),
                loadu(ptr.offset(first + 224)),
            ]
        };
        let eights_a = fold_eight(&mut ones, &mut twos, &mut fours, load8(base));
        let eights_b = fold_eight(&mut ones, &mut twos, &mut fours, load8(base + 256));
        let (sixteens, low) = csa(eights, eights_a, eights_b);
        eights = low;
        total = _mm256_add_epi64(total, lane_sums(sixteens, lut, mask0f, zero));
        block += 1;
    }

    let mut count = horizontal_sum(total) * 16;
    count += fold_carries(ones, twos, fours, eights, lut, mask0f, zero);

    let mut remainder = zero;
    let mut vector = blocks * CSA_BLOCK_VECTORS;
    while vector < vectors {
        let off = (vector * 32) as isize;
        remainder = _mm256_add_epi64(
            remainder,
            lane_sums(loadu(ptr.offset(off)), lut, mask0f, zero),
        );
        vector += 1;
    }
    count += horizontal_sum(remainder);
    count + word_tail(buf, vectors)
}

/// Counts the set bits of `lhs & rhs` with a Harley-Seal carry-save loop.
///
/// Identical in structure to [`avx2_popcnt_csa`], with each loaded bit-plane
/// replaced by the `VPAND` of the two operands' vectors, so the AND is fused
/// into the reduction and no temporary buffer exists. The count covers the
/// shorter operand.
///
/// # Safety
///
/// The host supports AVX2; `super::detect_x86` establishes that before taking
/// this function's address. Loads are unaligned and bounded by the shorter
/// operand.
#[target_feature(enable = "avx2")]
pub(crate) unsafe fn avx2_and_popcnt_csa(lhs: &[u64], rhs: &[u64]) -> u64 {
    let len = lhs.len().min(rhs.len());
    if len == 0 {
        return 0;
    }
    let lhs_ptr = lhs.as_ptr() as *const u8;
    let rhs_ptr = rhs.as_ptr() as *const u8;
    let vectors = len / WORDS_PER_VECTOR;

    let lut = nibble_lut();
    let mask0f = _mm256_set1_epi8(0x0f);
    let zero = _mm256_setzero_si256();
    let and_at = |off: isize| -> __m256i {
        _mm256_and_si256(loadu(lhs_ptr.offset(off)), loadu(rhs_ptr.offset(off)))
    };

    let mut total = zero;
    let mut ones = zero;
    let mut twos = zero;
    let mut fours = zero;
    let mut eights = zero;

    let blocks = vectors / CSA_BLOCK_VECTORS;
    let mut block = 0usize;
    while block < blocks {
        let base = (block * CSA_BLOCK_VECTORS * 32) as isize;
        let load8 = |first: isize| -> [__m256i; 8] {
            [
                and_at(first),
                and_at(first + 32),
                and_at(first + 64),
                and_at(first + 96),
                and_at(first + 128),
                and_at(first + 160),
                and_at(first + 192),
                and_at(first + 224),
            ]
        };
        let eights_a = fold_eight(&mut ones, &mut twos, &mut fours, load8(base));
        let eights_b = fold_eight(&mut ones, &mut twos, &mut fours, load8(base + 256));
        let (sixteens, low) = csa(eights, eights_a, eights_b);
        eights = low;
        total = _mm256_add_epi64(total, lane_sums(sixteens, lut, mask0f, zero));
        block += 1;
    }

    let mut count = horizontal_sum(total) * 16;
    count += fold_carries(ones, twos, fours, eights, lut, mask0f, zero);

    let mut remainder = zero;
    let mut vector = blocks * CSA_BLOCK_VECTORS;
    while vector < vectors {
        let off = (vector * 32) as isize;
        remainder = _mm256_add_epi64(remainder, lane_sums(and_at(off), lut, mask0f, zero));
        vector += 1;
    }
    count += horizontal_sum(remainder);
    for index in (vectors * WORDS_PER_VECTOR)..len {
        count += u64::from((lhs.get_unchecked(index) & rhs.get_unchecked(index)).count_ones());
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Word counts that bracket every boundary these kernels have: the empty
    /// buffer, sub-vector and sub-block lengths, the word boundaries the
    /// repository's bit-packed contract names, and lengths just under, at and
    /// over one and several carry-save blocks.
    const LENGTHS: [usize; 15] = [
        0,
        1,
        3,
        4,
        5,
        63,
        64,
        65,
        CSA_BLOCK_WORDS - 1,
        CSA_BLOCK_WORDS,
        CSA_BLOCK_WORDS + 1,
        2 * CSA_BLOCK_WORDS,
        3 * CSA_BLOCK_WORDS + 7,
        1000,
        4096,
    ];

    fn reference(words: &[u64]) -> u64 {
        words.iter().map(|word| u64::from(word.count_ones())).sum()
    }

    fn pattern(len: usize, seed: u64) -> Vec<u64> {
        let mut state = seed;
        (0..len)
            .map(|_| {
                state = state
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                state ^ (state >> 31)
            })
            .collect()
    }

    #[test]
    fn csa_matches_the_reference_across_block_boundaries() {
        if !std::arch::is_x86_feature_detected!("avx2") {
            return;
        }
        for len in LENGTHS {
            for (label, words) in [
                ("random", pattern(len, 0x5cbb_6545 ^ len as u64)),
                ("all-one", vec![u64::MAX; len]),
                ("all-zero", vec![0u64; len]),
            ] {
                // SAFETY: AVX2 was detected above.
                let counted = unsafe { avx2_popcnt_csa(&words) };
                assert_eq!(counted, reference(&words), "{label} length {len}");
            }
        }
    }

    #[test]
    fn fused_csa_matches_the_reference_across_block_boundaries() {
        if !std::arch::is_x86_feature_detected!("avx2") {
            return;
        }
        for len in LENGTHS {
            let lhs = pattern(len, 0xa1 ^ len as u64);
            let rhs = pattern(len, 0xb2 ^ len as u64);
            let expected: u64 = lhs
                .iter()
                .zip(&rhs)
                .map(|(left, right)| u64::from((left & right).count_ones()))
                .sum();
            // SAFETY: AVX2 was detected above.
            let counted = unsafe { avx2_and_popcnt_csa(&lhs, &rhs) };
            assert_eq!(counted, expected, "length {len}");
        }
    }

    #[test]
    fn fused_csa_counts_the_shorter_operand() {
        if !std::arch::is_x86_feature_detected!("avx2") {
            return;
        }
        let lhs = vec![u64::MAX; CSA_BLOCK_WORDS + 3];
        let rhs = vec![u64::MAX; 5];
        // SAFETY: AVX2 was detected above.
        assert_eq!(unsafe { avx2_and_popcnt_csa(&lhs, &rhs) }, 5 * 64);
        // SAFETY: AVX2 was detected above.
        assert_eq!(unsafe { avx2_and_popcnt_csa(&rhs, &lhs) }, 5 * 64);
    }

    #[test]
    fn scalar_popcnt_matches_the_reference() {
        if !std::arch::is_x86_feature_detected!("popcnt") {
            return;
        }
        for len in [0, 1, 2, 3, 4, 7, 8, 63, 64, 65] {
            let words = pattern(len, 0xc3 ^ len as u64);
            // SAFETY: POPCNT was detected above.
            let counted = unsafe { popcnt_words(&words) };
            assert_eq!(counted, reference(&words), "length {len}");
        }
    }
}
