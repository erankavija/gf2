//! Shared behavioural suite for every residual `BitVec` shift route
//! (jit:f8dd4dde).
//!
//! The residual branch of [`BitVec::shift_left`] and [`BitVec::shift_right`] is
//! the one a non-zero `k % 64` reaches. One corpus and one independent
//! bit-addressed zero-fill reference drive whichever route the library selects,
//! so both the scalar funnel and the capability-gated kernel answer the same
//! cases. The corpus shape is the one the planning-time feasibility record's
//! prototype uses
//! (`dev/active/c04dd4ac-zen3-shifts-and-permutations/shift-feasibility-record.md`):
//! the repository's word-boundary lengths and offsets, offsets at and beyond
//! the length, lengths leaving an incomplete final word, and lengths long
//! enough for an unrolled funnel loop to run.

use gf2_core::BitVec;

/// Which public method a case drives.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Direction {
    Left,
    Right,
}

impl Direction {
    const ALL: [Direction; 2] = [Direction::Left, Direction::Right];

    fn apply(self, bv: &mut BitVec, k: usize) {
        match self {
            Direction::Left => bv.shift_left(k),
            Direction::Right => bv.shift_right(k),
        }
    }
}

/// The reference: canonical little-endian bit index `i` takes the bit `k`
/// places below it on a left shift and `k` places above it on a right shift,
/// and zero where that index leaves the vector.
///
/// Bit-addressed and independent of the word layout the implementations use,
/// so it shares no code with either route.
fn reference(direction: Direction, bits: &[bool], k: usize) -> Vec<bool> {
    let len = bits.len();
    (0..len)
        .map(|i| match direction {
            Direction::Left => i.checked_sub(k).is_some_and(|j| bits[j]),
            Direction::Right => i.checked_add(k).is_some_and(|j| j < len && bits[j]),
        })
        .collect()
}

/// Deterministic case fill, so a failure names a reproducible case.
///
/// SplitMix64 [Steele2014], the mixer the repository's harnesses use.
fn splitmix_bits(len: usize, seed: u64) -> Vec<bool> {
    let mut state = seed;
    (0..len)
        .map(|_| {
            state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = state;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            (z ^ (z >> 31)) & 1 == 1
        })
        .collect()
}

/// The patterns every length is tested under, with the label a failure reports.
fn patterns(len: usize) -> Vec<(String, Vec<bool>)> {
    let mut cases = vec![
        ("splitmix".to_owned(), splitmix_bits(len, 0xf8dd_4dde)),
        ("all-ones".to_owned(), vec![true; len]),
        ("all-zeros".to_owned(), vec![false; len]),
    ];
    if len > 0 {
        let mut lowest = vec![false; len];
        lowest[0] = true;
        cases.push(("lowest-bit".to_owned(), lowest));
        let mut highest = vec![false; len];
        highest[len - 1] = true;
        cases.push(("highest-bit".to_owned(), highest));
    }
    cases
}

/// The word-boundary lengths the repository's bit-packed policy requires, the
/// lengths that leave an incomplete final word, and lengths long enough for a
/// funnel loop unrolled by two to run its unrolled body and its tail.
const LENGTHS: [usize; 14] = [0, 1, 7, 8, 63, 64, 65, 100, 127, 128, 129, 200, 513, 1000];

/// The offsets every length is tested at, before the per-length additions in
/// [`offsets`]. Each non-multiple of 64 reaches the residual branch.
const BASE_OFFSETS: [usize; 13] = [0, 1, 7, 8, 63, 64, 65, 66, 71, 127, 128, 129, 191];

/// Offsets for one length: the shared set plus the boundary cases that depend
/// on the length — one below it, at it, one past it, and twice it.
fn offsets(len: usize) -> Vec<usize> {
    let mut all: Vec<usize> = BASE_OFFSETS.to_vec();
    all.extend([len.saturating_sub(1), len, len + 1, 2 * len, 2 * len + 1]);
    all.sort_unstable();
    all.dedup();
    all
}

/// Builds a vector of `bits.len()` bits holding `bits`.
fn bitvec_from_bits(bits: &[bool]) -> BitVec {
    let mut bv = BitVec::zeros(bits.len());
    for (i, &bit) in bits.iter().enumerate() {
        bv.set(i, bit);
    }
    bv
}

/// Asserts the storage holds exactly the words the length needs and that the
/// padding above the length is zero.
fn assert_zero_tail_padding(bv: &BitVec, label: &str) {
    let words = bv.words();
    assert_eq!(
        words.len(),
        bv.len().div_ceil(64),
        "{label}: word count for {} bits",
        bv.len()
    );
    let rem = bv.len() % 64;
    if rem != 0 {
        let last = words[words.len() - 1];
        assert_eq!(last >> rem, 0, "{label}: padding above bit {}", bv.len());
    }
}

/// Runs one case: shift, then compare every bit against the reference and
/// assert the length and the zero tail padding survive.
fn assert_case(direction: Direction, bits: &[bool], k: usize, label: &str) {
    let mut bv = bitvec_from_bits(bits);
    direction.apply(&mut bv, k);

    assert_eq!(bv.len(), bits.len(), "{label}: length changed");
    let expected = reference(direction, bits, k);
    for (i, &want) in expected.iter().enumerate() {
        assert_eq!(bv.get(i), want, "{label}: bit {i}");
    }
    assert_zero_tail_padding(&bv, label);
}

/// The whole corpus, against whichever route the library currently selects.
///
/// `route` names the route in every failure message; a caller that has forced
/// a route passes its name so a failure says which one broke.
fn assert_shift_corpus(route: &str) {
    for len in LENGTHS {
        for k in offsets(len) {
            for (pattern, bits) in patterns(len) {
                for direction in Direction::ALL {
                    let label = format!("{route}: {direction:?} len={len} k={k} pattern={pattern}");
                    assert_case(direction, &bits, k, &label);
                }
            }
        }
    }
}

#[test]
fn the_selected_route_answers_the_shift_corpus() {
    assert_shift_corpus("selected route");
}

#[test]
fn the_reference_zero_fills_and_drops_what_leaves_the_vector() {
    // The oracle's own witness: a shift by the length empties the vector in
    // either direction, and a shift by zero is the identity. Without this a
    // reference that returned the input unchanged would pass every case above.
    let bits = splitmix_bits(70, 0x0123_4567);
    for direction in Direction::ALL {
        assert_eq!(reference(direction, &bits, 0), bits);
        assert_eq!(reference(direction, &bits, 70), vec![false; 70]);
        assert_eq!(reference(direction, &bits, 999), vec![false; 70]);
    }
    assert_eq!(
        reference(Direction::Left, &[true, false], 1),
        vec![false, true]
    );
    assert_eq!(
        reference(Direction::Right, &[false, true], 1),
        vec![true, false]
    );
}
