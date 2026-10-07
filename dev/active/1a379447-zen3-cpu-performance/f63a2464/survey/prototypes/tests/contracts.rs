//! Behavioural contracts of the three candidate prototypes.
//!
//! Every claim the numerical-contract review makes about a candidate is
//! asserted here against the canonical decoder or against the candidate's own
//! scalar reference, including the claims that a candidate does *not* satisfy.

use gf2_coding::ldpc::{
    min_sum_check_row, DecoderAlgorithm, DecoderConfig, LdpcCode, LdpcDecoder, LdpcEncoder,
    MinSumRule, QuasiCyclicLdpc,
};
use gf2_coding::llr::Llr;
use gf2_coding::traits::BlockEncoder;
use gf2_core::BitVec;
use ldpc_candidate_prototypes::{
    quantized_check_row, Alphabet, LayeredDecoder, QcDecoder, QuantizedDecoder, QuantizedRule,
};

/// A seeded generator, so every case below is reproducible without a
/// dependency: SplitMix64, the mixer the protocol's own generator seeds with.
struct Seeded(u64);

impl Seeded {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// A value in `[0, 1)`.
    fn next_unit(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u32 << 24) as f32
    }

    /// An approximately Gaussian deviate, from twelve uniforms.
    fn next_normal(&mut self) -> f32 {
        (0..12).map(|_| self.next_unit()).sum::<f32>() - 6.0
    }
}

/// A small quasi-cyclic code with a lower-triangular identity parity part, so
/// the lifted matrix has full rank and the code encodes at every lifting size.
fn small_qc_code(z: usize) -> QuasiCyclicLdpc {
    let base: Vec<Vec<i32>> = [
        [0, 1, 2, 0, -1, -1],
        [1, 0, 3, 0, 0, -1],
        [2, 3, 0, -1, 0, 0],
    ]
    .iter()
    .map(|row| {
        row.iter()
            .map(|&entry| if entry < 0 { -1 } else { entry % z as i32 })
            .collect()
    })
    .collect();
    QuasiCyclicLdpc::new(base, z)
}

fn noisy_llrs(code: &LdpcCode, seed: u64, sigma: f32, all_zero: bool) -> (BitVec, Vec<Llr>) {
    let encoder = LdpcEncoder::new(code.clone());
    let mut generator = Seeded(seed);
    let mut message = BitVec::zeros(encoder.k());
    if !all_zero {
        for index in 0..encoder.k() {
            if generator.next_u64() & 1 == 1 {
                message.set(index, true);
            }
        }
    }
    let codeword = encoder.encode(&message);
    let llrs = (0..code.n())
        .map(|index| {
            let transmitted = if codeword.get(index) { -1.0 } else { 1.0 };
            let received = transmitted + sigma * generator.next_normal();
            Llr::new(2.0 * received / (sigma * sigma))
        })
        .collect();
    (codeword, llrs)
}

// --- Family QC -------------------------------------------------------------

/// The claim the review makes: unchanged numerical contract, tested rather
/// than asserted. Beliefs must agree bit for bit with the canonical decoder.
#[test]
fn qc_posteriors_match_the_canonical_decoder_bit_for_bit() {
    for z in [8usize, 12, 96] {
        let qc = small_qc_code(z);
        let code = LdpcCode::from_quasi_cyclic(&qc);
        for rule in [
            MinSumRule::Plain,
            MinSumRule::Normalized(0.75),
            MinSumRule::Offset(0.5),
        ] {
            let algorithm = match rule {
                MinSumRule::Plain => DecoderAlgorithm::MinSum,
                MinSumRule::Normalized(alpha) => DecoderAlgorithm::NormalizedMinSum(alpha),
                MinSumRule::Offset(beta) => DecoderAlgorithm::OffsetMinSum(beta),
            };
            let mut canonical =
                LdpcDecoder::with_config(code.clone(), DecoderConfig::new(algorithm, false));
            let mut candidate = QcDecoder::new(&qc, rule);

            for (seed, all_zero) in [(1u64, true), (2, false), (3, false)] {
                let (_, llrs) = noisy_llrs(&code, seed, 0.9, all_zero);
                let mut codeword = BitVec::with_capacity(code.n());
                canonical.decode_codeword_into(&llrs, 8, &mut codeword);
                candidate.decode(&llrs, 8, false);
                for (index, (&got, want)) in candidate
                    .posterior()
                    .iter()
                    .zip(canonical.posterior_llrs().iter())
                    .enumerate()
                {
                    assert_eq!(
                        got.to_bits(),
                        want.value().to_bits(),
                        "z {z} rule {rule:?} seed {seed} position {index}"
                    );
                }
            }
        }
    }
}

/// Punctured positions enter at zero and filler positions at a finite
/// magnitude; both are ordinary channel LLRs and neither changes the identity.
#[test]
fn qc_matches_the_canonical_decoder_on_punctured_and_filler_inputs() {
    let z = 24;
    let qc = small_qc_code(z);
    let code = LdpcCode::from_quasi_cyclic(&qc);
    let (_, mut llrs) = noisy_llrs(&code, 11, 0.8, false);
    for slot in llrs.iter_mut().take(2 * z) {
        *slot = Llr::zero();
    }
    for slot in llrs.iter_mut().skip(code.n() - z) {
        *slot = Llr::new(15.0);
    }

    let mut canonical = LdpcDecoder::with_config(
        code.clone(),
        DecoderConfig::new(DecoderAlgorithm::NormalizedMinSum(0.75), false),
    );
    let mut candidate = QcDecoder::new(&qc, MinSumRule::Normalized(0.75));
    let mut codeword = BitVec::with_capacity(code.n());
    canonical.decode_codeword_into(&llrs, 10, &mut codeword);
    candidate.decode(&llrs, 10, false);

    for (index, (&got, want)) in candidate
        .posterior()
        .iter()
        .zip(canonical.posterior_llrs().iter())
        .enumerate()
    {
        assert_eq!(got.to_bits(), want.value().to_bits(), "position {index}");
    }
}

/// The AVX2 check kernel and its scalar reference agree bit for bit, including
/// on the inputs the float contract's sign and magnitude rules separate.
#[test]
fn qc_avx2_kernel_matches_its_scalar_reference_including_non_finite_inputs() {
    let z = 44;
    let qc = small_qc_code(z);
    let code = LdpcCode::from_quasi_cyclic(&qc);
    let mut vectorized = QcDecoder::with_dispatch(&qc, MinSumRule::Normalized(0.75), true);
    if !vectorized.uses_avx2() {
        return;
    }
    let mut scalar = QcDecoder::with_dispatch(&qc, MinSumRule::Normalized(0.75), false);

    let exceptional = [
        -0.0f32,
        0.0,
        f32::NAN,
        -f32::NAN,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::MIN_POSITIVE,
        -f32::MIN_POSITIVE,
    ];
    let (_, ordinary) = noisy_llrs(&code, 21, 0.85, false);
    let mut llrs = ordinary;
    for (index, value) in exceptional.iter().enumerate() {
        llrs[index * 7 % code.n()] = Llr::new(*value);
    }

    vectorized.decode(&llrs, 6, false);
    scalar.decode(&llrs, 6, false);
    for (index, (&got, &want)) in vectorized
        .posterior()
        .iter()
        .zip(scalar.posterior().iter())
        .enumerate()
    {
        assert_eq!(got.to_bits(), want.to_bits(), "position {index}");
    }
}

/// Lifting sizes that are and are not multiples of the lane count both run the
/// kernel's vector body and its tail to the same result.
#[test]
fn qc_kernel_tail_and_vector_body_agree_at_every_lifting_size() {
    for z in [1usize, 3, 7, 8, 9, 16, 17, 40] {
        let qc = small_qc_code(z);
        let code = LdpcCode::from_quasi_cyclic(&qc);
        let mut vectorized = QcDecoder::with_dispatch(&qc, MinSumRule::Plain, true);
        let mut scalar = QcDecoder::with_dispatch(&qc, MinSumRule::Plain, false);
        let (_, llrs) = noisy_llrs(&code, 31 + z as u64, 0.9, false);
        vectorized.decode(&llrs, 5, false);
        scalar.decode(&llrs, 5, false);
        assert_eq!(vectorized.posterior(), scalar.posterior(), "z {z}");
    }
}

/// Early termination stops at the first passing syndrome and leaves the same
/// decision the canonical decoder reaches.
#[test]
fn qc_early_termination_matches_the_canonical_decoder() {
    let z = 32;
    let qc = small_qc_code(z);
    let code = LdpcCode::from_quasi_cyclic(&qc);
    let mut canonical = LdpcDecoder::with_config(
        code.clone(),
        DecoderConfig::new(DecoderAlgorithm::NormalizedMinSum(0.75), true),
    );
    let mut candidate = QcDecoder::new(&qc, MinSumRule::Normalized(0.75));
    for seed in 0..6u64 {
        let (_, llrs) = noisy_llrs(&code, 100 + seed, 0.7, seed % 2 == 0);
        let mut codeword = BitVec::with_capacity(code.n());
        let want = canonical.decode_codeword_into(&llrs, 20, &mut codeword);
        let got = candidate.decode(&llrs, 20, true);
        assert_eq!(got.iterations, want.iterations, "seed {seed}");
        assert_eq!(
            got.syndrome_check_passed, want.syndrome_check_passed,
            "seed {seed}"
        );
        for index in 0..code.n() {
            assert_eq!(
                candidate.hard_bits()[index],
                codeword.get(index),
                "seed {seed} position {index}"
            );
        }
    }
}

// --- Family Q --------------------------------------------------------------

/// The integer reduction follows the canonical tie, sign and degree rules: a
/// leave-one-out reference over the same inputs reproduces every output.
#[test]
fn quantized_check_row_matches_a_leave_one_out_reference() {
    fn reference(rule: QuantizedRule, inputs: &[i16], excluded: usize) -> i16 {
        let others: Vec<i16> = inputs
            .iter()
            .enumerate()
            .filter(|(index, _)| *index != excluded)
            .map(|(_, value)| *value)
            .collect();
        if others.is_empty() {
            return 0;
        }
        let negative = others.iter().filter(|value| **value < 0).count() % 2 == 1;
        let magnitude = others
            .iter()
            .map(|value| i32::from(*value).abs())
            .min()
            .unwrap();
        let scaled = match rule {
            QuantizedRule::Plain => magnitude,
            QuantizedRule::Normalized { numerator, shift } => (magnitude * numerator) >> shift,
            QuantizedRule::Offset { offset } => (magnitude - offset).max(0),
        };
        i16::clip(if negative { -scaled } else { scaled })
    }

    let rules = [
        QuantizedRule::Plain,
        QuantizedRule::Normalized {
            numerator: 3,
            shift: 2,
        },
        QuantizedRule::Offset { offset: 2 },
    ];
    let cases: [&[i16]; 7] = [
        &[3, -2, 4],
        &[-1, -2],
        &[2, 2, 5],
        &[-2, 2, 5],
        &[0, 0, 7],
        &[32767, -32767, 5],
        &[9, -8, 7, -6, 5, -4, 3, -2, 1, 0],
    ];
    for rule in rules {
        for inputs in cases {
            let mut outputs = vec![0i16; inputs.len()];
            quantized_check_row(rule, inputs, &mut outputs);
            for (index, &got) in outputs.iter().enumerate() {
                assert_eq!(
                    got,
                    reference(rule, inputs, index),
                    "rule {rule:?} case {inputs:?} output {index}"
                );
            }
        }
    }
}

/// A degree-one check emits the zero message and a degree-zero check writes
/// nothing, as the canonical reduction does.
#[test]
fn quantized_check_row_handles_degenerate_degrees() {
    let mut outputs = [7i8];
    quantized_check_row(QuantizedRule::Plain, &[-3i8], &mut outputs);
    assert_eq!(outputs[0], 0);
    quantized_check_row(QuantizedRule::Plain, &[] as &[i8], &mut []);
}

/// The alphabet is symmetric and saturating: no accumulation leaves
/// `[-LIMIT, LIMIT]`, and the type minimum never occurs.
#[test]
fn the_quantized_alphabet_saturates_and_excludes_the_type_minimum() {
    for value in [i8::MAX, -i8::MAX, 100, -100, 0] {
        for other in [i8::MAX, -i8::MAX, 100, -100, 1] {
            let sum = Alphabet::sat_add(value, other);
            let difference = Alphabet::sat_sub(value, other);
            for result in [sum, difference] {
                assert!(
                    i32::from(result) <= <i8 as Alphabet>::LIMIT
                        && i32::from(result) >= -<i8 as Alphabet>::LIMIT,
                    "{value} and {other} produced {result}"
                );
                assert_ne!(result, i8::MIN, "the symmetric alphabet excludes i8::MIN");
            }
        }
    }
    assert_eq!(<i8 as Alphabet>::clip(-500), -127);
    assert_eq!(<i16 as Alphabet>::clip(-100_000), -32767);
}

/// Quantization rounds half away from zero and clips; a NaN channel LLR
/// enters as the zero message rather than an arbitrary pattern.
#[test]
fn quantization_rounds_and_clips_the_channel() {
    let scale = 4.0;
    assert_eq!(<i8 as Alphabet>::quantize(Llr::new(0.5), scale), 2);
    assert_eq!(<i8 as Alphabet>::quantize(Llr::new(-0.5), scale), -2);
    assert_eq!(<i8 as Alphabet>::quantize(Llr::new(0.125), scale), 1);
    assert_eq!(<i8 as Alphabet>::quantize(Llr::new(-0.125), scale), -1);
    assert_eq!(<i8 as Alphabet>::quantize(Llr::new(100.0), scale), 127);
    assert_eq!(<i8 as Alphabet>::quantize(Llr::new(-100.0), scale), -127);
    assert_eq!(<i8 as Alphabet>::quantize(Llr::new(f32::NAN), scale), 0);
    assert_eq!(
        <i8 as Alphabet>::quantize(Llr::new(f32::INFINITY), scale),
        127
    );
}

/// The quantized decoder decodes the codewords the canonical decoder decodes
/// on a clean channel, in both widths and over both codeword classes.
#[test]
fn the_quantized_decoder_decodes_a_clean_channel_in_both_widths() {
    let qc = small_qc_code(16);
    let code = LdpcCode::from_quasi_cyclic(&qc);
    let rule = QuantizedRule::Normalized {
        numerator: 3,
        shift: 2,
    };
    for all_zero in [true, false] {
        let (codeword, llrs) = noisy_llrs(&code, if all_zero { 5 } else { 6 }, 0.35, all_zero);
        let mut narrow: QuantizedDecoder<i8> = QuantizedDecoder::new(&code, rule, 4.0);
        let mut wide: QuantizedDecoder<i16> = QuantizedDecoder::new(&code, rule, 16.0);
        assert!(narrow.decode(&llrs, 30, true).syndrome_check_passed);
        assert!(wide.decode(&llrs, 30, true).syndrome_check_passed);
        for index in 0..code.n() {
            assert_eq!(
                narrow.hard_bits()[index],
                codeword.get(index),
                "i8 position {index}"
            );
            assert_eq!(
                wide.hard_bits()[index],
                codeword.get(index),
                "i16 position {index}"
            );
        }
    }
}

/// Falsification the review requires preserved: the quantized decoder is not
/// bit-exact against the canonical float decoder, and the difference is
/// measured rather than assumed.
#[test]
fn the_quantized_decoder_is_not_bit_exact_against_the_canonical_decoder() {
    let qc = small_qc_code(16);
    let code = LdpcCode::from_quasi_cyclic(&qc);
    let (_, llrs) = noisy_llrs(&code, 77, 0.95, false);

    let mut canonical = LdpcDecoder::with_config(
        code.clone(),
        DecoderConfig::new(DecoderAlgorithm::NormalizedMinSum(0.75), false),
    );
    let mut codeword = BitVec::with_capacity(code.n());
    canonical.decode_codeword_into(&llrs, 12, &mut codeword);

    let mut candidate: QuantizedDecoder<i8> = QuantizedDecoder::new(
        &code,
        QuantizedRule::Normalized {
            numerator: 3,
            shift: 2,
        },
        2.0,
    );
    candidate.decode(&llrs, 12, false);

    let differing = (0..code.n())
        .filter(|&index| candidate.hard_bits()[index] != codeword.get(index))
        .count();
    assert!(
        differing > 0,
        "a coarse i8 alphabet on a noisy frame is expected to part from the float decoder"
    );
}

// --- Family L --------------------------------------------------------------

/// A layered sweep decodes the codewords the canonical decoder decodes, over
/// both codeword classes and every min-sum rule.
#[test]
fn the_layered_decoder_decodes_a_clean_channel() {
    let qc = small_qc_code(16);
    let code = LdpcCode::from_quasi_cyclic(&qc);
    for rule in [
        MinSumRule::Plain,
        MinSumRule::Normalized(0.75),
        MinSumRule::Offset(0.5),
    ] {
        let mut decoder = LayeredDecoder::new(&code, rule);
        for all_zero in [true, false] {
            let (codeword, llrs) = noisy_llrs(&code, if all_zero { 41 } else { 42 }, 0.4, all_zero);
            assert!(
                decoder.decode(&llrs, 30, true).syndrome_check_passed,
                "rule {rule:?} all_zero {all_zero}"
            );
            for index in 0..code.n() {
                assert_eq!(
                    decoder.hard_bits()[index],
                    codeword.get(index),
                    "rule {rule:?} position {index}"
                );
            }
        }
    }
}

/// Falsification preserved: a layered sweep is not a flooding iteration, so
/// the two schedules reach different states and their counts are not
/// comparable.
#[test]
fn the_layered_schedule_is_not_the_flooding_schedule() {
    let qc = small_qc_code(16);
    let code = LdpcCode::from_quasi_cyclic(&qc);
    let (_, llrs) = noisy_llrs(&code, 88, 0.9, false);

    let mut canonical = LdpcDecoder::with_config(
        code.clone(),
        DecoderConfig::new(DecoderAlgorithm::NormalizedMinSum(0.75), false),
    );
    let mut codeword = BitVec::with_capacity(code.n());
    canonical.decode_codeword_into(&llrs, 1, &mut codeword);

    let mut layered = LayeredDecoder::new(&code, MinSumRule::Normalized(0.75));
    layered.decode(&llrs, 1, false);

    let differing = (0..code.n())
        .filter(|&index| layered.hard_bits()[index] != codeword.get(index))
        .count();
    assert!(
        differing > 0,
        "one layered sweep and one flooding iteration are expected to differ"
    );
}

/// The canonical reduction the layered decoder reuses is the production one,
/// so a layered arm changes the schedule and nothing else about the check.
#[test]
fn the_layered_decoder_reuses_the_canonical_reduction() {
    let inputs = [Llr::new(3.0), Llr::new(-2.0), Llr::new(4.0)];
    let mut outputs = [Llr::zero(); 3];
    min_sum_check_row(MinSumRule::Normalized(0.75), &inputs, &mut outputs);
    assert_eq!(outputs[0].value(), 0.75 * -2.0);
}
