//! Executable evidence for the arbitrary-offset bit-shift mapping result.
//!
//! The survey records gf2's `BitVec::shift_left`/`shift_right` as unmatched:
//! no surveyed coding source exposes an operation-equivalent primitive, and
//! the circular shifts they do expose (srsRAN's `srsvec::circ_shift_backward`,
//! used for 5G NR LDPC lifting) are a different operation. That claim rests on
//! a semantic difference, so this binary measures the difference rather than
//! asserting it in prose:
//!
//! 1. gf2's shifts are exactly zero-fill: output bit `i` is input bit `i - k`
//!    (left) or `i + k` (right), and every vacated position is zero. Checked
//!    against an independent reference over the boundary lengths and offsets
//!    the engineering contract names, including `k = 0`, `k = len` and
//!    `k > len`.
//! 2. A wrap-around rotation of the same vector by the same offset differs
//!    from the zero-fill result whenever a set bit crosses the boundary, so a
//!    circular-shift API cannot stand in for these calls.
//! 3. Canonical little-endian indexing and zero tail padding survive both.

use gf2_core::BitVec;
use tuning_campaign_support::abtest::SplitMix64;

const LENGTHS: [usize; 9] = [0, 1, 63, 64, 65, 127, 128, 4096, 64_800];

/// Seeded packed words of exactly `bits` bits, with the canonical zero tail.
fn seeded_words(seed: u64, bits: usize) -> Vec<u64> {
    let count = bits.div_ceil(64);
    let mut mixer = SplitMix64::new(seed);
    let mut words: Vec<u64> = (0..count).map(|_| mixer.next_u64()).collect();
    let tail_bits = bits % 64;
    if tail_bits != 0 {
        words[count - 1] &= (1_u64 << tail_bits) - 1;
    }
    words
}

/// Independent reference: zero-fill shift on a plain bool vector.
fn reference_zero_fill(bits: &[bool], k: usize, left: bool) -> Vec<bool> {
    let n = bits.len();
    (0..n)
        .map(|index| {
            if left {
                index.checked_sub(k).map(|source| bits[source])
            } else {
                index
                    .checked_add(k)
                    .filter(|source| *source < n)
                    .map(|source| bits[source])
            }
            .unwrap_or(false)
        })
        .collect()
}

/// Independent reference: wrap-around rotation on a plain bool vector.
fn reference_wrap(bits: &[bool], k: usize, left: bool) -> Vec<bool> {
    let n = bits.len();
    if n == 0 {
        return Vec::new();
    }
    let k = k % n;
    (0..n)
        .map(|index| {
            let source = if left {
                (index + n - k) % n
            } else {
                (index + k) % n
            };
            bits[source]
        })
        .collect()
}

fn to_bools(vector: &BitVec) -> Vec<bool> {
    (0..vector.len()).map(|index| vector.get(index)).collect()
}

fn check_tail_padding(vector: &BitVec, context: &str) {
    let tail_bits = vector.len() % 64;
    if tail_bits == 0 {
        return;
    }
    let last = *vector.words().last().expect("a nonempty vector has a word");
    assert_eq!(
        last >> tail_bits,
        0,
        "{context}: tail padding is nonzero after the shift"
    );
}

fn offsets_for(length: usize) -> Vec<usize> {
    let mut offsets = vec![0_usize, 1, 63, 64, 65, 127, 128];
    if length > 0 {
        offsets.push(length - 1);
    }
    offsets.push(length);
    offsets.push(length + 1);
    offsets.sort_unstable();
    offsets.dedup();
    offsets
}

fn main() {
    let mut zero_fill_checks = 0_usize;
    let mut wrap_differences = 0_usize;
    let mut wrap_comparisons = 0_usize;
    for length in LENGTHS {
        let source = BitVec::from_words(seeded_words(0x5c1f_7a11 ^ length as u64, length), length);
        let bits = to_bools(&source);
        for k in offsets_for(length) {
            for left in [true, false] {
                let mut shifted = source.clone();
                if left {
                    shifted.shift_left(k);
                } else {
                    shifted.shift_right(k);
                }
                assert_eq!(
                    shifted.len(),
                    length,
                    "length {length} k {k}: the shift changed the vector length"
                );
                let context = format!("length {length} k {k} left {left}");
                assert_eq!(
                    to_bools(&shifted),
                    reference_zero_fill(&bits, k, left),
                    "{context}: gf2 does not match the zero-fill reference"
                );
                check_tail_padding(&shifted, &context);
                zero_fill_checks += 1;

                // A wrap-around rotation is a different function of the same
                // input whenever a set bit crosses the boundary. An all-ones
                // vector makes that unconditional for every offset that is not
                // a whole number of turns: the rotation returns all ones and
                // the zero-fill shift cannot.
                if length > 0 && k % length != 0 {
                    let saturated = BitVec::ones(length);
                    let saturated_bits = to_bools(&saturated);
                    let mut saturated_shifted = saturated;
                    if left {
                        saturated_shifted.shift_left(k);
                    } else {
                        saturated_shifted.shift_right(k);
                    }
                    wrap_comparisons += 1;
                    if reference_wrap(&saturated_bits, k, left) != to_bools(&saturated_shifted) {
                        wrap_differences += 1;
                    }
                }
            }
        }
    }
    println!("PASS zero-fill semantics: {zero_fill_checks} shifts over lengths {LENGTHS:?} match an independent zero-fill reference with canonical indexing and zero tail padding");
    assert_eq!(
        wrap_differences, wrap_comparisons,
        "a wrap-around rotation coincided with the zero-fill shift"
    );
    println!("PASS wrap is a different operation: {wrap_differences}/{wrap_comparisons} nonzero offsets give a different result under wrap-around rotation than under gf2's zero-fill shift");
}
