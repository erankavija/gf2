//! Every form, including the fallback, against the independent bit-addressed
//! reference. Deterministic and untimed.

use residual_shift_feasibility::{reference, select, shift_left, shift_right, words_for, Form};

/// splitmix64, so the corpus is reproducible without a dependency.
fn fill(len_bits: usize, seed: u64) -> Vec<u64> {
    let mut state = seed;
    let mut data: Vec<u64> = (0..words_for(len_bits))
        .map(|_| {
            state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = state;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^ (z >> 31)
        })
        .collect();
    // The input obeys the same tail invariant the output must hold.
    let rem = len_bits % 64;
    if rem != 0 {
        if let Some(last) = data.last_mut() {
            *last &= (1u64 << rem) - 1;
        }
    }
    data
}

/// Lengths: the required set, plus the sizes that reach the 256-bit group
/// loop, cross its lanes and leave an incomplete final word.
const LENGTHS: &[usize] = &[
    0, 1, 7, 8, 63, 64, 65, 127, 128, 129, 191, 192, 255, 256, 257, 383, 448, 511, 512, 513, 575,
    576, 577, 1023, 1024, 1025, 4095, 4096, 4097,
];

/// Offsets: the required set, plus word-crossing and group-crossing values.
const OFFSETS: &[usize] = &[
    0, 1, 7, 8, 63, 64, 65, 66, 127, 128, 129, 191, 255, 256, 257, 511, 512, 513, 1000, 4096,
];

fn cases() -> impl Iterator<Item = (usize, usize)> {
    LENGTHS.iter().copied().flat_map(|len_bits| {
        // Offsets at and beyond the length are part of the contract.
        let edges = [
            len_bits.saturating_sub(1),
            len_bits,
            len_bits + 1,
            len_bits.saturating_mul(2),
            usize::MAX / 2,
        ];
        OFFSETS
            .iter()
            .copied()
            .chain(edges)
            .map(move |k| (len_bits, k))
    })
}

fn tail_is_zero(data: &[u64], len_bits: usize) -> bool {
    let rem = len_bits % 64;
    rem == 0 || data.last().is_none_or(|w| w >> rem == 0)
}

#[test]
fn every_form_matches_the_zero_fill_reference() {
    for (len_bits, k) in cases() {
        for form in Form::ALL {
            let seed = (len_bits as u64) << 32 | k as u64 & 0xFFFF_FFFF;

            let mut expected = fill(len_bits, seed);
            reference::shift_left(&mut expected, len_bits, k);
            let mut got = fill(len_bits, seed);
            let used = shift_left(form, &mut got, len_bits, k);
            assert_eq!(
                got, expected,
                "shift_left {form:?} as {used:?} len={len_bits} k={k}"
            );
            assert!(
                tail_is_zero(&got, len_bits),
                "left tail {form:?} len={len_bits} k={k}"
            );

            let mut expected = fill(len_bits, seed);
            reference::shift_right(&mut expected, len_bits, k);
            let mut got = fill(len_bits, seed);
            let used = shift_right(form, &mut got, len_bits, k);
            assert_eq!(
                got, expected,
                "shift_right {form:?} as {used:?} len={len_bits} k={k}"
            );
            assert!(
                tail_is_zero(&got, len_bits),
                "right tail {form:?} len={len_bits} k={k}"
            );
        }
    }
}

#[test]
fn an_unavailable_form_falls_back_to_the_scalar_funnel() {
    for form in Form::ALL {
        let available = match form {
            Form::Scalar => true,
            Form::Avx2 => cfg!(target_arch = "x86_64") && is_x86_feature_detected!("avx2"),
            Form::Bmi2Pair | Form::Bmi2Dp => {
                cfg!(target_arch = "x86_64") && is_x86_feature_detected!("bmi2")
            }
        };
        let expected = if available { form } else { Form::Scalar };
        assert_eq!(select(form), expected, "gate for {form:?}");

        // The reported form is the one the entry point ran, so a fallback is
        // visible to the caller and not merely internal.
        let mut data = fill(4096, 1);
        assert_eq!(shift_left(form, &mut data, 4096, 65), expected);
        assert_eq!(shift_right(form, &mut data, 4096, 65), expected);
    }
}

#[test]
fn the_corpus_reaches_the_vector_regime() {
    // The AVX2 group loop runs only where a full 256-bit group fits below the
    // write window, so a corpus that never reaches it would pass vacuously.
    let reached = cases()
        .filter(|&(len_bits, k)| {
            k != 0 && k < len_bits && k % 64 != 0 && words_for(len_bits) >= k / 64 + 8
        })
        .count();
    assert!(
        reached > 100,
        "only {reached} cases reach the 256-bit group loop"
    );
}
