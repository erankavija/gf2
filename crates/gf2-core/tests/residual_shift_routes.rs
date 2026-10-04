//! Behavioural suite shared by both routes of the residual branch of
//! [`BitVec::shift_left`] and [`BitVec::shift_right`], the branch a non-zero
//! `k % 64` reaches. One corpus and one bit-addressed zero-fill reference
//! drive the scalar funnel, held by the force switch of
//! [`gf2_core::residual_shift`], and the route the library selects once the
//! switch is released; the lane witness says which route each arm ran.

use gf2_core::residual_shift::{
    force_scalar_residual_shift, last_residual_shift_route, reset_last_residual_shift_route,
    residual_shift_route, ResidualShiftRoute,
};
use gf2_core::BitVec;
use std::sync::Mutex;

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

/// Deterministic SplitMix64 (`@/citation/Steele2014`) fill, so a failure
/// names a reproducible case.
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

/// `route` names the route in every failure message.
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

/// The caller holds [`ROUTE_MUTEX`]: the switch and the witness are
/// process-wide.
fn observed_route() -> ResidualShiftRoute {
    reset_last_residual_shift_route();
    let mut bv = BitVec::ones(200);
    bv.shift_left(65);
    last_residual_shift_route().expect("a shift by 65 bits reaches the residual branch")
}

/// Serialises the toggle-execute-observe-restore section of every test that
/// touches the process-wide force switch.
static ROUTE_MUTEX: Mutex<()> = Mutex::new(());

fn host_has_bmi2() -> bool {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        std::arch::is_x86_feature_detected!("bmi2")
    }
    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
    {
        false
    }
}

#[test]
fn every_route_answers_the_shift_corpus() {
    let guard = ROUTE_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let previously_forced = force_scalar_residual_shift(true);

    // Arm A: the fallback, driven as a reachable route rather than assumed.
    assert_eq!(residual_shift_route(), ResidualShiftRoute::ScalarFunnel);
    assert_eq!(observed_route(), ResidualShiftRoute::ScalarFunnel);
    assert_shift_corpus(ResidualShiftRoute::ScalarFunnel.name());

    // Arm B: whatever this build and host select once the switch is clear.
    force_scalar_residual_shift(false);
    let released = residual_shift_route();
    assert_eq!(
        observed_route(),
        released,
        "the executed route is the reported one"
    );
    assert_shift_corpus(released.name());

    force_scalar_residual_shift(previously_forced);
    drop(guard);
}

#[test]
fn the_kernel_route_is_selected_exactly_where_it_is_available() {
    let guard = ROUTE_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let previously_forced = force_scalar_residual_shift(false);

    // The gate's verdict against the host's own feature detection and the
    // build's own feature set, so neither route is assumed from either.
    let expected_kernel = cfg!(feature = "simd") && host_has_bmi2();
    assert_eq!(
        observed_route() == ResidualShiftRoute::Bmi2Funnel,
        expected_kernel,
        "simd feature {}, host bmi2 {}",
        cfg!(feature = "simd"),
        host_has_bmi2()
    );

    force_scalar_residual_shift(previously_forced);
    drop(guard);
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
