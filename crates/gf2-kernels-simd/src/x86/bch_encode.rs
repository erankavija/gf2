//! Accelerated BCH batch-encoding kernels: two kernels of `crate::bch_encode`'s
//! bundle, each under the processor features it alone needs; the bundle's
//! predicate is their union.

use core::arch::x86_64::*;

/// Register coefficients one 256-bit iteration advances.
const LANE_BLOCK: usize = 4;

/// AVX2 lane: advances a bit-sliced shift register over one block of message
/// degrees.
///
/// The arguments are the `crate::bch_encode::BchBitsliceReduceFn` contract:
/// `register` holds `masks.len() + 1` words with a leading pad,
/// `masks[j]` is all ones exactly when the generator's coefficient of $x^j$
/// is one, and `slices` holds one bit-sliced message coefficient per degree,
/// consumed from the last index to the first.
///
/// # Safety
///
/// The caller must ensure the `avx2` processor feature is available at
/// runtime; that is this function's whole safety condition, and the complete
/// feature set its instructions need.
/// `crate::bch_encode::detect` publishes a pointer to this kernel only when
/// `is_x86_feature_detected!("avx2")` returns true, so a caller reaching it
/// through that bundle upholds the condition by construction. The buffer
/// geometry is not a safety condition: the kernel decides it below and
/// panics rather than reading out of bounds.
///
/// # Panics
///
/// Panics when `register` does not hold exactly one word more than `masks`.
///
/// # Complexity
///
/// $\lceil r/4 \rceil$ 256-bit operations per entry of `slices`, for
/// `crate::bch_encode::BITSLICE_LANES` frames at once, where $r$ is
/// `masks.len()`.
#[target_feature(enable = "avx2")]
pub(crate) unsafe fn bitslice_reduce_avx2(register: &mut [u64], masks: &[u64], slices: &[u64]) {
    let redundancy = masks.len();
    assert_eq!(
        register.len(),
        redundancy + 1,
        "a bit-sliced register holds one pad word below its {redundancy} coefficient slices"
    );
    if redundancy == 0 {
        return;
    }

    let register_ptr = register.as_mut_ptr();
    let masks_ptr = masks.as_ptr();
    for &entering in slices.iter().rev() {
        // The feedback bit of every lane, in that lane's bit of one word.
        let feedback = *register_ptr.add(redundancy) ^ entering;
        let broadcast = _mm256_set1_epi64x(feedback as i64);

        // Coefficients `degree .. degree + 4` per iteration, from the top of
        // the register down, so the loads of one iteration read words the
        // stores have not reached.
        let mut degree = redundancy;
        while degree >= LANE_BLOCK {
            degree -= LANE_BLOCK;
            let below = _mm256_loadu_si256(register_ptr.add(degree).cast::<__m256i>());
            let mask = _mm256_loadu_si256(masks_ptr.add(degree).cast::<__m256i>());
            let advanced = _mm256_xor_si256(below, _mm256_and_si256(mask, broadcast));
            _mm256_storeu_si256(register_ptr.add(degree + 1).cast::<__m256i>(), advanced);
        }
        while degree > 0 {
            degree -= 1;
            *register_ptr.add(degree + 1) =
                *register_ptr.add(degree) ^ (*masks_ptr.add(degree) & feedback);
        }
    }
}

/// PCLMULQDQ lane: folds a run of 64-degree message blocks into one frame's
/// packed remainder.
///
/// The arguments are the `crate::bch_encode::BchFoldBlocksFn` contract:
/// `register` holds the running remainder in `redundancy.div_ceil(64)` packed
/// words, `low` the generator's low coefficients in the same packing,
/// `barrett` the constant `crate::bch_encode::fold_barrett_constant` derives,
/// and `blocks` the 64-coefficient blocks entering at $x^r$, highest degrees
/// first.
///
/// # Safety
///
/// The caller must ensure the `pclmulqdq` and `sse4.1` processor features are
/// available at runtime; that pair is this function's whole safety condition,
/// and the complete feature set its instructions need. It is the same pair
/// `crate::x86::clmul::clmul_u64` demands, which is the only intrinsic this
/// kernel reaches. `crate::bch_encode::detect` publishes a pointer to this
/// kernel only when `is_x86_feature_detected!("pclmulqdq")` and
/// `is_x86_feature_detected!("sse4.1")` both return true, so a caller
/// reaching it through that bundle upholds the condition by construction. The
/// buffer geometry is not a safety condition: the shared step decides it and
/// panics rather than reading out of bounds.
///
/// # Panics
///
/// Panics when `register` and `low` are not both `redundancy.div_ceil(64)`
/// words, or when `redundancy` is zero.
///
/// # Complexity
///
/// $1 + \lceil r/64 \rceil$ `pclmulqdq` instructions and
/// $O(\lceil r/64 \rceil)$ word operations per block, where $r$ is
/// `redundancy`.
#[target_feature(enable = "pclmulqdq", enable = "sse4.1")]
pub(crate) unsafe fn fold_blocks_pclmul(
    register: &mut [u64],
    low: &[u64],
    redundancy: usize,
    barrett: u64,
    blocks: &[u64],
) {
    crate::bch_encode::fold_blocks_over(
        // SAFETY: this closure is reached only from this function's body, and
        // this function's safety condition is exactly what
        // `crate::x86::clmul::clmul_u64` requires.
        |a, b| unsafe { crate::x86::clmul::clmul_u64(a, b) },
        register,
        low,
        redundancy,
        barrett,
        blocks,
    );
}

/// PCLMULQDQ lane of `crate::bch_encode::BchFoldChunksFn`: folds whole chunks
/// of message blocks into an unreduced accumulator.
///
/// The accumulator stays in vector registers as word pairs, and each product
/// selects its accumulator word and constant word by the instruction's
/// immediate, so no word moves between vector and general registers inside a
/// chunk. A product with an even constant word lands on a word pair; one with
/// an odd word lands one word up and is realigned once per chunk.
///
/// # Safety
///
/// The caller must ensure the `pclmulqdq`, `sse4.1` and `ssse3` processor
/// features are available at runtime; that set is this function's whole
/// safety condition, and the complete feature set its instructions need.
/// `crate::bch_encode::detect` publishes a pointer to this kernel only when
/// all three, with `avx2`, are detected. The buffer geometry is not a safety
/// condition: the kernel decides it below and panics rather than reading out
/// of bounds.
///
/// # Panics
///
/// Panics as `crate::bch_encode::fold_chunks_scalar` does.
///
/// # Complexity
///
/// $\lceil r/64 \rceil$ `pclmulqdq` instructions per block, independent
/// within a chunk.
#[target_feature(enable = "pclmulqdq", enable = "sse4.1", enable = "ssse3")]
pub(crate) unsafe fn fold_chunks_pclmul(state: &mut [u64], constants: &[u64], blocks: &[u64]) {
    let accumulators = state.len();
    assert!(
        accumulators > 0 && constants.len().is_multiple_of(accumulators),
        "the constants are one run per accumulator"
    );
    // Accumulator counts up to eight, which covers every default count,
    // compile with the count constant so the chunk loop unrolls into
    // registers; larger overrides run the same body with a runtime count.
    macro_rules! counts {
        ($w:literal) => {
            match accumulators {
                2 => fold_chunks_pclmul_words::<$w>(2, state, constants, blocks),
                3 => fold_chunks_pclmul_words::<$w>(3, state, constants, blocks),
                4 => fold_chunks_pclmul_words::<$w>(4, state, constants, blocks),
                5 => fold_chunks_pclmul_words::<$w>(5, state, constants, blocks),
                6 => fold_chunks_pclmul_words::<$w>(6, state, constants, blocks),
                7 => fold_chunks_pclmul_words::<$w>(7, state, constants, blocks),
                8 => fold_chunks_pclmul_words::<$w>(8, state, constants, blocks),
                other => fold_chunks_pclmul_words::<$w>(other, state, constants, blocks),
            }
        };
    }
    match constants.len() / accumulators {
        1 => counts!(1),
        2 => counts!(2),
        3 => counts!(3),
        4 => counts!(4),
        words => panic!(
            "a chunk fold runs over 1 to {} words, not {words}",
            crate::bch_encode::FOLD_UNROLLED_WORDS
        ),
    }
}

/// Word pairs of the largest accumulator.
const FOLD_PAIRS: usize = crate::bch_encode::FOLD_MAX_ACCUMULATORS / 2;

/// The chunk fold for constants of `W` words and `accumulators` accumulators,
/// inlined into each call site so that a constant count unrolls.
#[inline(always)]
unsafe fn fold_chunks_pclmul_words<const W: usize>(
    accumulators: usize,
    state: &mut [u64],
    constants: &[u64],
    blocks: &[u64],
) {
    assert_eq!(state.len(), accumulators);
    assert!(
        accumulators > W && accumulators <= crate::bch_encode::FOLD_MAX_ACCUMULATORS,
        "{accumulators} accumulators cannot hold a product of {W} words"
    );
    assert_eq!(constants.len(), accumulators * W);
    assert!(
        blocks.len().is_multiple_of(accumulators),
        "{} blocks are not whole chunks of {accumulators}",
        blocks.len()
    );
    let pairs = accumulators.div_ceil(2);
    // Constant word pairs of each accumulator, the odd tail word paired with
    // zero.
    let constant_pairs = W.div_ceil(2);
    let mut k = [[_mm_setzero_si128(); 2]; crate::bch_encode::FOLD_MAX_ACCUMULATORS];
    for (index, run) in constants.chunks_exact(W).enumerate() {
        for pair in 0..constant_pairs {
            let high = if 2 * pair + 1 < W {
                run[2 * pair + 1]
            } else {
                0
            };
            k[index][pair] = _mm_set_epi64x(high as i64, run[2 * pair] as i64);
        }
    }
    let mut x = [_mm_setzero_si128(); FOLD_PAIRS];
    for (pair, slot) in x.iter_mut().enumerate().take(pairs) {
        let high = state.get(2 * pair + 1).copied().unwrap_or(0);
        *slot = _mm_set_epi64x(high as i64, state[2 * pair] as i64);
    }

    for chunk in blocks.chunks_exact(accumulators) {
        // Word w of the chunk is chunk[accumulators - 1 - w].
        let mut even = [_mm_setzero_si128(); FOLD_PAIRS];
        let mut odd = [_mm_setzero_si128(); FOLD_PAIRS + 1];
        for (pair, slot) in even.iter_mut().enumerate().take(pairs) {
            let top = accumulators - 1 - 2 * pair;
            *slot = if top == 0 {
                _mm_cvtsi64_si128(chunk[0] as i64)
            } else {
                // chunk[top - 1], chunk[top] are words 2p + 1, 2p.
                let loaded = _mm_loadu_si128(chunk.as_ptr().add(top - 1).cast::<__m128i>());
                _mm_shuffle_epi32::<0x4E>(loaded)
            };
        }
        for index in 0..accumulators {
            let source = x[index / 2];
            for word in 0..W {
                let constant = k[index][word / 2];
                let product = match (index % 2, word % 2) {
                    (0, 0) => _mm_clmulepi64_si128::<0x00>(source, constant),
                    (0, _) => _mm_clmulepi64_si128::<0x10>(source, constant),
                    (_, 0) => _mm_clmulepi64_si128::<0x01>(source, constant),
                    _ => _mm_clmulepi64_si128::<0x11>(source, constant),
                };
                if word % 2 == 0 {
                    even[word / 2] = _mm_xor_si128(even[word / 2], product);
                } else {
                    odd[word / 2] = _mm_xor_si128(odd[word / 2], product);
                }
            }
        }
        // odd[p] covers words 2p + 1 and 2p + 2; realign onto pairs.
        let mut below = _mm_setzero_si128();
        for pair in 0..pairs {
            let aligned = _mm_alignr_epi8::<8>(odd[pair], below);
            below = odd[pair];
            x[pair] = _mm_xor_si128(even[pair], aligned);
        }
    }
    for pair in 0..pairs {
        let mut words = [0u64; 2];
        _mm_storeu_si128(words.as_mut_ptr().cast::<__m128i>(), x[pair]);
        state[2 * pair] = words[0];
        if 2 * pair + 1 < accumulators {
            state[2 * pair + 1] = words[1];
        }
    }
}
