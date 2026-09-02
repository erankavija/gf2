//! AVX2 bit-sliced BCH batch-encoding kernels.
//!
//! The algorithm is the one `crate::bch_encode` states: a shift register held
//! one word per coefficient, bit `l` of a word carrying lane `l`, advanced one
//! message degree per step across all lanes at once.
//!
//! The step writes coefficient slice `j` from coefficient slice `j - 1`, so
//! four consecutive slices are one unaligned 256-bit load and one unaligned
//! 256-bit store shifted by one word. The feedback word is the same in every
//! slice, so it is broadcast once per step and reused across the whole
//! register; the generator masks are a second unaligned load from a buffer the
//! caller prepared. Both loads fold into memory operands, so the committed
//! artefact `asm/bch_encode.asm.txt` shows the inner loop as `vpand` /
//! `vpxor` / `vmovdqu` over four register coefficients, under one
//! `vmovq` / `vpbroadcastq` pair per step, against four `mov`/`and`/`xor`/`mov`
//! quadruples in the portable kernel.
//!
//! The store range of one iteration overlaps the load range of the same
//! iteration and the load range of the previous one, so the loop runs from
//! the top of the register downwards and loads before it stores. The word
//! below coefficient zero is the register's leading pad, which the store
//! never reaches.

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
