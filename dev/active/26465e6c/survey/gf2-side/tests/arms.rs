//! Every arm of both families returns the same count as an independent
//! bit-by-bit reference on the word-boundary sizes, every supported word
//! offset and every bit pattern.

use gf2_core::BitVec;
use popcount_survey::{AndArm, Fixture, Pattern, PopcountArm, VECTOR_BYTES};

const SIZES: &[usize] = &[0, 1, 3, 4, 7, 8, 12, 15, 16, 60, 63, 64, 65, 128, 256, 1025];
const PATTERNS: &[Pattern] = &[Pattern::Random, Pattern::AllZero, Pattern::AllOne];

fn bits(words: &[u64]) -> u64 {
    words
        .iter()
        .map(|word| (0..64).filter(|bit| word >> bit & 1 == 1).count() as u64)
        .sum()
}

#[test]
fn fixtures_sit_at_their_declared_offset_from_a_vector_boundary() {
    for offset in 0..4 {
        for &len in SIZES {
            let fixture = Fixture::new(len, 7, Pattern::Random, offset).unwrap();
            assert_eq!(fixture.words().len(), len);
            assert_eq!(fixture.misalignment_bytes(), offset * 8 % VECTOR_BYTES);
        }
    }
    assert!(Fixture::new(8, 7, Pattern::Random, 4).is_err());
}

#[test]
fn offset_windows_read_the_same_seeded_stream() {
    let aligned = Fixture::new(64, 11, Pattern::Random, 0).unwrap();
    let shifted = Fixture::new(64, 11, Pattern::Random, 3).unwrap();
    assert_eq!(&aligned.words()[3..], &shifted.words()[..61]);
}

#[test]
fn popcount_arms_match_the_reference_wherever_they_accept_the_window() {
    for &len in SIZES {
        for &pattern in PATTERNS {
            for offset in 0..4 {
                let fixture = Fixture::new(len, 0x5eed ^ len as u64, pattern, offset).unwrap();
                let expected = bits(fixture.words());
                for arm in PopcountArm::ALL {
                    let op = arm.resolve().unwrap();
                    if !arm.accepts_misalignment(fixture.misalignment_bytes()) {
                        assert_eq!(arm, PopcountArm::MulaAvx2HarleySeal);
                        assert_ne!(offset, 0);
                        continue;
                    }
                    // SAFETY: the arm accepted this window's alignment.
                    let got = unsafe { op(fixture.words()) };
                    assert_eq!(
                        got,
                        expected,
                        "{} len={len} {pattern:?} offset={offset}",
                        arm.name()
                    );
                }
            }
        }
    }
}

#[test]
fn and_arms_match_the_reference() {
    for &len in SIZES {
        for &pattern in PATTERNS {
            for offset in 0..4 {
                let lhs = Fixture::new(len, 1, pattern, offset).unwrap();
                let rhs = Fixture::new(len, 1001, pattern, offset).unwrap();
                let masked: Vec<u64> = lhs
                    .words()
                    .iter()
                    .zip(rhs.words())
                    .map(|(left, right)| left & right)
                    .collect();
                let expected = bits(&masked);
                for arm in AndArm::ALL {
                    let got = arm.resolve().unwrap()(lhs.words(), rhs.words());
                    assert_eq!(
                        got,
                        expected,
                        "{} len={len} {pattern:?} offset={offset}",
                        arm.name()
                    );
                }
            }
        }
    }
}

#[test]
fn bit_length_tails_count_only_bits_below_the_length() {
    for &len in &[0_usize, 1, 63, 64, 65, 127, 128, 129, 4095, 4096, 4097] {
        let mut vector = BitVec::zeros(len);
        for index in (0..len).step_by(3) {
            vector.set(index, true);
        }
        let expected = len.div_ceil(3) as u64;
        assert_eq!(vector.count_ones() as u64, expected, "BitVec len={len}");
        let aligned = Fixture::from_slice(vector.words(), 0).unwrap();
        for arm in PopcountArm::ALL {
            // SAFETY: offset-0 fixtures satisfy every arm's alignment.
            let got = unsafe { arm.resolve().unwrap()(aligned.words()) };
            assert_eq!(got, expected, "{} len={len}", arm.name());
        }
    }
}

#[test]
fn the_production_route_follows_gf2s_size_threshold() {
    assert_eq!(
        PopcountArm::ProductionDispatch.selected_path(7),
        "gf2-ops-popcount:scalar"
    );
    assert!(PopcountArm::ProductionDispatch
        .selected_path(8)
        .starts_with("gf2-ops-popcount:simd"));
    assert!(AndArm::TwoPass.selected_path(4).ends_with(":scalar"));
}

#[test]
fn only_the_two_pass_route_selects_a_backend_per_call() {
    for words in [4, 7, 8, 4096] {
        let simd = usize::from(popcount_survey::arms::dispatch_route(words) != "scalar");
        assert_eq!(AndArm::TwoPass.dispatch(words), 2 * simd, "words={words}");
        assert_eq!(AndArm::Fused.dispatch(words), 0);
        assert_eq!(AndArm::ScalarControl.dispatch(words), 0);
    }
}

#[test]
fn arm_names_round_trip() {
    for arm in PopcountArm::ALL {
        assert_eq!(PopcountArm::parse(arm.name()), Some(arm));
    }
    for arm in AndArm::ALL {
        assert_eq!(AndArm::parse(arm.name()), Some(arm));
    }
    assert_eq!(PopcountArm::parse("and-fused"), None);
}
