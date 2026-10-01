//! Binary BCH construction, encoding, and decoding laws on the canonical model.
//!
//! The codes are primitive narrow-sense constructions over
//! $\mathrm{GF}(2^4) = \mathrm{GF}(2)\[x\]/(x^4 + x + 1)$ and the production
//! DVB-T2 outer codes. Laws shared with every base-field class, including the
//! typed errors of invalid constructions, live in `bch_conformance.rs`.
//!
//! [`BinaryBchDecoder`] reads internal coordinates, where coordinate $i$ is the
//! coefficient of $x^i$, and exposes its syndromes, error locator, and Chien
//! search only through the outcome, the corrected word, and the corrected
//! coordinates of its diagnostic report. The syndrome, Berlekamp-Massey, and
//! Chien-search groups below assert those observables.

use gf2_coding::bch::dvb_t2::{dvb_t2_bch_code, DvbT2BchCode, DvbT2BchDecoder, FrameSize};
use gf2_coding::bch::error::BchError;
use gf2_coding::bch::spec::{BinaryBchCode, DesignedDistance};
use gf2_coding::bch::{BchDecodeOutcome, BinaryBchDecoder, SystematicLayout};
use gf2_coding::error::CodeError;
use gf2_coding::traits::block::{BlockCode, BlockEncoder};
use gf2_coding::CodeRate;
use gf2_core::field::extension::{BinaryPrimeExt, FieldExtension};
use gf2_core::field::FieldPoly;
use gf2_core::gf2m::Gf2mField;
use gf2_core::gfp::Fp;
use gf2_core::BitVec;

/// The primitive narrow-sense code over $\mathrm{GF}(2^4)$ with designed
/// distance $2t + 1$: BCH(15, 11) for $t = 1$ and BCH(15, 7) for $t = 2$.
fn gf16_code(t: u64) -> BinaryBchCode {
    let extension = BinaryPrimeExt::new(Gf2mField::new(4, 0b10011).with_tables())
        .expect("x^4 + x + 1 presents GF(16)");
    let designed_distance =
        DesignedDistance::try_from(2 * t + 1).expect("a positive designed distance");
    BinaryBchCode::primitive_narrow_sense(extension, designed_distance)
        .expect("a primitive narrow-sense code over GF(16)")
}

/// Encodes `message` in the default systematic layout.
fn encode(code: &BinaryBchCode, message: &BitVec) -> BitVec {
    code.encode(message).expect("a k-bit message encodes")
}

/// Returns the codeword polynomial a default-layout word carries.
fn codeword_polynomial(code: &BinaryBchCode, codeword: &BitVec) -> FieldPoly<Fp<2>> {
    let plan = code.systematic_plan(SystematicLayout::default());
    let coefficients = (0..code.n())
        .map(|degree| {
            let user = plan
                .user_coordinate(degree)
                .expect("a degree below the length");
            Fp::<2>::new(u64::from(codeword.get(user)))
        })
        .collect();
    FieldPoly::new(coefficients)
}

/// Asserts that the generator vanishes at $\alpha^1, \dots, \alpha^{2t}$ for
/// the primitive element $\alpha$ of the code's splitting field.
fn assert_generator_vanishes_at_consecutive_powers(code: &BinaryBchCode, t: usize) {
    let extension = code.extension();
    let lifted = FieldPoly::new(
        code.generator()
            .iter()
            .map(|coefficient| extension.embed(coefficient))
            .collect(),
    );
    let alpha = extension
        .field()
        .primitive_element()
        .expect("GF(16) has a primitive element");
    let mut alpha_power = alpha.clone();
    for i in 1..=(2 * t) {
        let eval = lifted.eval(&alpha_power);
        assert!(
            eval.is_zero(),
            "Generator must vanish at α^{} (evaluation: {:?})",
            i,
            eval.value()
        );
        alpha_power = &alpha_power * &alpha;
    }
}

/// What the canonical decoder established about one default-layout word.
struct LayoutDecode {
    outcome: BchDecodeOutcome,
    /// The corrected word's message, or `None` without a verified correction.
    message: Option<BitVec>,
    /// The corrected user coordinates, ascending.
    error_positions: Vec<usize>,
}

/// Decodes a default-layout word with [`BinaryBchDecoder`].
///
/// The decoder reads internal coordinates, so the received word crosses the
/// layout's coordinate map on the way in, and the corrected word and its
/// corrected coordinates cross it on the way out.
fn decode(code: &BinaryBchCode, received: &BitVec) -> LayoutDecode {
    let plan = code.systematic_plan(SystematicLayout::default());
    let internal_of = |user: usize| {
        plan.internal_coordinate(user)
            .expect("a user coordinate below the length")
    };
    let mut internal = BitVec::zeros(code.n());
    for user in 0..code.n() {
        internal.set(internal_of(user), received.get(user));
    }

    let report = BinaryBchDecoder::new(code)
        .decode(&internal)
        .expect("a length-n word decodes");
    let message = report.codeword().map(|corrected| {
        let mut message = BitVec::zeros(code.k());
        for user in 0..code.k() {
            message.set(user, corrected.get(internal_of(user)));
        }
        message
    });
    let mut error_positions: Vec<usize> = report
        .error_positions()
        .iter()
        .map(|&degree| {
            plan.user_coordinate(degree)
                .expect("a corrected coordinate below the length")
        })
        .collect();
    error_positions.sort_unstable();

    LayoutDecode {
        outcome: report.outcome(),
        message,
        error_positions,
    }
}

/// Returns the decoded message, which a correctable word always produces.
fn decode_message(code: &BinaryBchCode, received: &BitVec) -> BitVec {
    decode(code, received)
        .message
        .expect("a word within the correction radius decodes")
}

mod bch_construction_tests {
    use super::*;

    #[test]
    fn test_bch_code_new() {
        let code = gf16_code(1);

        assert_eq!(code.n(), 15);
        assert_eq!(code.k(), 11);
        assert_eq!(code.correction_radius(), 1);
    }

    #[test]
    fn test_generator_polynomial_degree() {
        let code = gf16_code(1);

        // For t=1, generator has degree at most 2t*m where m is extension degree
        // In practice, should be around n-k = 4
        assert!(code.generator().degree().unwrap() <= 8);
        assert!(code.generator().degree().unwrap() >= 2); // At least 2 for t=1
    }

    #[test]
    fn test_generator_has_consecutive_roots() {
        // Generator should have α, α^2, α^3, α^4 as roots (for t=2)
        let code = gf16_code(2);
        assert_generator_vanishes_at_consecutive_powers(&code, code.correction_radius());
    }

    /// The canonical code records the witnessed distance bound rather than
    /// the requested designed distance; for BCH(15, 11) the two agree.
    #[test]
    fn test_bch_code_parameters_valid() {
        let code = gf16_code(1);

        assert_eq!(code.distance_bound().minimum_distance_lower_bound(), 3); // 2t + 1 = 3
        assert_eq!(code.n() - code.k(), 4); // Parity bits
    }
}

mod dvb_t2_parameter_tests {
    use super::*;

    #[test]
    fn test_dvb_t2_short_rate_half() {
        let code = dvb_t2_bch_code(FrameSize::Short, CodeRate::Rate1_2).unwrap();

        assert_eq!(code.n(), 7200); // BCH output = LDPC k
        assert_eq!(code.k(), 7032); // BCH input = Kbch
        assert_eq!(code.mother().code().correction_radius(), 12);
    }

    #[test]
    fn test_dvb_t2_short_all_rates() {
        let rates = vec![
            (CodeRate::Rate1_2, 7032),
            (CodeRate::Rate3_5, 9552),
            (CodeRate::Rate2_3, 10632),
            (CodeRate::Rate3_4, 11712),
            (CodeRate::Rate4_5, 12432),
            (CodeRate::Rate5_6, 13152),
        ];

        for (rate, expected_k) in rates {
            let code = dvb_t2_bch_code(FrameSize::Short, rate).unwrap();
            assert_eq!(code.k(), expected_k);
            assert_eq!(code.mother().code().correction_radius(), 12);
        }
    }

    #[test]
    fn test_dvb_t2_normal_rate_half() {
        let code = dvb_t2_bch_code(FrameSize::Normal, CodeRate::Rate1_2).unwrap();

        assert_eq!(code.n(), 32400); // BCH output = LDPC k
        assert_eq!(code.k(), 32208); // BCH input = Kbch
        assert_eq!(code.mother().code().correction_radius(), 12);
    }

    #[test]
    fn test_dvb_t2_normal_all_rates() {
        let rates = vec![
            (CodeRate::Rate1_2, 32208, 12),
            (CodeRate::Rate3_5, 38688, 12),
            (CodeRate::Rate2_3, 43040, 10), // t=10 for this rate
            (CodeRate::Rate3_4, 48408, 12),
            (CodeRate::Rate4_5, 51648, 12),
            (CodeRate::Rate5_6, 53840, 10), // t=10 for this rate
        ];

        for (rate, expected_k, expected_t) in rates {
            let code = dvb_t2_bch_code(FrameSize::Normal, rate).unwrap();
            assert_eq!(code.k(), expected_k);
            assert_eq!(code.mother().code().correction_radius(), expected_t);
        }
    }
}

mod encoding_tests {
    use super::*;

    #[test]
    fn test_encoder_creates_valid_codeword_length() {
        let code = gf16_code(1);

        let mut msg = BitVec::zeros(11);
        for i in 0..11 {
            msg.set(i, i % 2 == 0);
        }
        let cw = encode(&code, &msg);

        assert_eq!(cw.len(), 15);
    }

    #[test]
    fn test_systematic_encoding_preserves_message() {
        let code = gf16_code(1);

        let mut msg = BitVec::zeros(11);
        for i in 0..11 {
            msg.set(i, (i / 2) % 2 == 0);
        }
        let cw = encode(&code, &msg);

        // In systematic form [message | parity], message appears in first k positions
        for i in 0..11 {
            assert_eq!(
                cw.get(i),
                msg.get(i),
                "Message bit {} not preserved at position {}",
                i,
                i
            );
        }
    }

    #[test]
    fn test_zero_message_encodes_to_zero() {
        let code = gf16_code(1);

        let msg = BitVec::zeros(11);
        let cw = encode(&code, &msg);

        assert_eq!(cw, BitVec::zeros(15));
    }

    /// A message of the wrong length is a typed buffer error naming the
    /// expected and supplied lengths.
    #[test]
    fn test_encoder_rejects_wrong_message_length() {
        let code = gf16_code(1);

        let msg = BitVec::zeros(10); // Wrong length
        assert_eq!(
            code.encode(&msg),
            Err(CodeError::BufferLengthMismatch {
                expected: 11,
                actual: 10
            })
        );
    }

    #[test]
    fn test_all_ones_message() {
        let code = gf16_code(1);

        let msg = BitVec::ones(11);
        let cw = encode(&code, &msg);

        assert_eq!(cw.len(), 15);
        // Message part should be all ones
        for i in 4..15 {
            assert!(cw.get(i), "Message bit should be 1 at position {}", i);
        }
    }

    #[test]
    fn test_encoded_codeword_is_valid() {
        let code = gf16_code(1);

        let mut msg = BitVec::from_bytes_le(&[0b10101010, 0b101]);
        msg.resize(11, false); // Trim to exactly 11 bits
        let cw = encode(&code, &msg);

        let (_, remainder) = codeword_polynomial(&code, &cw).div_rem(code.generator());

        assert!(
            remainder.is_zero(),
            "Codeword must be divisible by generator polynomial"
        );
    }
}

/// A zero syndrome is observable as [`BchDecodeOutcome::NoErrors`], and a
/// nonzero one as any other outcome.
mod syndrome_tests {
    use super::*;

    #[test]
    fn test_syndrome_zero_for_valid_codeword() {
        let code = gf16_code(1);

        let mut msg = BitVec::zeros(11);
        for i in 0..11 {
            msg.set(i, i % 3 == 0);
        }
        let cw = encode(&code, &msg);

        let decoded = decode(&code, &cw);
        assert_eq!(
            decoded.outcome,
            BchDecodeOutcome::NoErrors,
            "Syndrome must be zero for valid codeword"
        );
        assert!(decoded.error_positions.is_empty());
    }

    #[test]
    fn test_syndrome_nonzero_with_error() {
        let code = gf16_code(1);

        let mut msg = BitVec::zeros(11);
        for i in 0..11 {
            msg.set(i, (i / 2) % 2 == 0);
        }
        let mut cw = encode(&code, &msg);

        // Introduce single-bit error
        cw.set(5, !cw.get(5));

        assert_ne!(
            decode(&code, &cw).outcome,
            BchDecodeOutcome::NoErrors,
            "Syndrome must detect error"
        );
    }

    /// The decoder evaluates the syndromes the witnessed run of $2t$
    /// consecutive roots names, and its correction radius is that run's
    /// $t$.
    #[test]
    fn test_syndrome_length() {
        let code = gf16_code(1);
        let decoder = BinaryBchDecoder::new(&code);

        assert_eq!(code.distance_bound().consecutive_root_count(), 2);
        assert_eq!(decoder.correction_radius(), 1);
    }

    #[test]
    fn test_syndrome_multiple_errors() {
        let code = gf16_code(2);

        let msg = BitVec::zeros(7);
        let mut cw = encode(&code, &msg);

        // Introduce 2 errors
        cw.set(3, !cw.get(3));
        cw.set(10, !cw.get(10));

        assert_ne!(
            decode(&code, &cw).outcome,
            BchDecodeOutcome::NoErrors,
            "Syndromes must detect multiple errors"
        );
    }

    /// A received word of the wrong length is a typed decode error.
    #[test]
    fn test_syndrome_wrong_length_rejected() {
        let code = gf16_code(1);
        let decoder = BinaryBchDecoder::new(&code);

        let cw = BitVec::zeros(14); // Wrong length
        assert_eq!(
            decoder.decode(&cw),
            Err(BchError::Decode(CodeError::BufferLengthMismatch {
                expected: 15,
                actual: 14
            }))
        );
    }
}

/// The error-locator degree is observable as the corrected count.
mod berlekamp_massey_tests {
    use super::*;

    #[test]
    fn test_berlekamp_massey_no_errors() {
        let code = gf16_code(1);

        // The zero word has all-zero syndromes, so Λ(x) = 1.
        let decoded = decode(&code, &BitVec::zeros(15));
        assert_eq!(decoded.outcome.corrected_count(), Some(0));
    }

    #[test]
    fn test_berlekamp_massey_single_error() {
        let code = gf16_code(1);

        let msg = BitVec::ones(11);
        let mut cw = encode(&code, &msg);
        cw.set(5, !cw.get(5)); // Single error at position 5

        // For single error, degree should be 1
        assert_eq!(
            decode(&code, &cw).outcome,
            BchDecodeOutcome::Corrected { count: 1 }
        );
    }

    #[test]
    fn test_berlekamp_massey_two_errors() {
        let code = gf16_code(2);

        let msg = BitVec::zeros(7);
        let mut cw = encode(&code, &msg);

        // Inject 2 errors
        cw.set(3, !cw.get(3));
        cw.set(10, !cw.get(10));

        // For 2 errors, degree should be 2
        assert_eq!(
            decode(&code, &cw).outcome,
            BchDecodeOutcome::Corrected { count: 2 }
        );
    }

    #[test]
    fn test_berlekamp_massey_degree_bound() {
        let code = gf16_code(2);

        let msg = BitVec::ones(7);
        let mut cw = encode(&code, &msg);

        // Inject t errors
        cw.set(1, !cw.get(1));
        cw.set(8, !cw.get(8));

        // Degree should be at most t
        let count = decode(&code, &cw)
            .outcome
            .corrected_count()
            .expect("t errors are corrected");
        assert!(count <= code.correction_radius());
    }
}

/// The Chien-search roots are observable as the corrected coordinates.
mod chien_search_tests {
    use super::*;

    #[test]
    fn test_chien_search_no_errors() {
        let code = gf16_code(1);

        // Λ(x) = 1 means no errors
        let decoded = decode(&code, &BitVec::zeros(15));
        assert_eq!(decoded.error_positions.len(), 0);
    }

    #[test]
    fn test_chien_search_single_error() {
        let code = gf16_code(1);

        let msg = BitVec::ones(11);
        let mut cw = encode(&code, &msg);

        // Inject error at bitvec position 5
        let bitvec_error_pos = 5;
        cw.set(bitvec_error_pos, !cw.get(bitvec_error_pos));

        let bitvec_positions = decode(&code, &cw).error_positions;

        assert_eq!(bitvec_positions.len(), 1);
        assert_eq!(bitvec_positions[0], bitvec_error_pos);
    }

    #[test]
    fn test_chien_search_multiple_errors() {
        let code = gf16_code(2);

        let msg = BitVec::zeros(7);
        let mut cw = encode(&code, &msg);

        // Inject 2 errors at bitvec positions
        let bitvec_errors = vec![3, 10];
        for &pos in &bitvec_errors {
            cw.set(pos, !cw.get(pos));
        }

        let bitvec_positions = decode(&code, &cw).error_positions;

        assert_eq!(bitvec_positions.len(), 2);
        assert_eq!(bitvec_positions, bitvec_errors);
    }

    #[test]
    fn test_chien_search_correctable_errors() {
        let code = gf16_code(2);

        let msg = BitVec::ones(7);
        let mut cw = encode(&code, &msg);

        // Inject exactly t errors
        cw.set(0, !cw.get(0));
        cw.set(14, !cw.get(14));

        // Should find exactly t error positions
        let positions = decode(&code, &cw).error_positions;
        assert_eq!(positions.len(), code.correction_radius());
    }
}

mod decoder_integration_tests {
    use super::*;

    #[test]
    fn test_decode_no_errors() {
        let code = gf16_code(1);

        let msg = BitVec::ones(11);
        let cw = encode(&code, &msg);
        let decoded = decode_message(&code, &cw);

        assert_eq!(decoded, msg);
    }

    #[test]
    fn test_decode_single_error() {
        let code = gf16_code(1);

        let mut msg = BitVec::zeros(11);
        for i in 0..11 {
            msg.set(i, i % 3 == 0);
        }
        let mut cw = encode(&code, &msg);

        // Inject single error
        cw.set(7, !cw.get(7));

        let decoded = decode_message(&code, &cw);
        assert_eq!(decoded, msg);
    }

    #[test]
    fn test_decode_multiple_errors() {
        let code = gf16_code(2);

        let msg = BitVec::ones(7);
        let mut cw = encode(&code, &msg);

        // Inject 2 errors (within correction capability)
        cw.set(2, !cw.get(2));
        cw.set(12, !cw.get(12));

        let decoded = decode_message(&code, &cw);
        assert_eq!(decoded, msg);
    }

    #[test]
    fn test_decode_roundtrip_various_messages() {
        let code = gf16_code(1);

        // Test various message patterns
        let test_messages = vec![BitVec::zeros(11), BitVec::ones(11), {
            let mut msg = BitVec::zeros(11);
            for i in 0..11 {
                msg.set(i, i % 2 == 0);
            }
            msg
        }];

        for msg in test_messages {
            let cw = encode(&code, &msg);
            let decoded = decode_message(&code, &cw);
            assert_eq!(decoded, msg, "Roundtrip failed for message");
        }
    }

    #[test]
    fn test_decode_corrects_up_to_t_errors() {
        let code = gf16_code(2);

        let msg = BitVec::zeros(7);
        let mut cw = encode(&code, &msg);

        // Inject exactly t errors
        cw.set(1, !cw.get(1));
        cw.set(8, !cw.get(8));

        let decoded = decode_message(&code, &cw);
        assert_eq!(decoded, msg);
    }
}

mod known_bch_codes {
    use super::*;

    /// Test BCH(15, 7, 2) - well-documented in literature
    /// Generator polynomial: x^8 + x^7 + x^6 + x^4 + 1 (over GF(2^4))
    #[test]
    fn test_bch_15_7_2_properties() {
        let code = gf16_code(2); // x^4 + x + 1

        // Verify parameters
        assert_eq!(code.n(), 15);
        assert_eq!(code.k(), 7);
        assert_eq!(code.correction_radius(), 2);
        assert_eq!(code.distance_bound().minimum_distance_lower_bound(), 5); // 2t + 1

        // Verify generator polynomial degree
        assert_eq!(code.generator().degree(), Some(8)); // n - k = 15 - 7 = 8

        // Generator should have roots at α, α^2, α^3, α^4
        assert_generator_vanishes_at_consecutive_powers(&code, 2);
    }

    /// Test BCH(15, 11, 1) - single error correcting
    /// This is equivalent to Hamming(15, 11)
    #[test]
    fn test_bch_15_11_1_hamming_equivalence() {
        let code = gf16_code(1);

        assert_eq!(code.n(), 15);
        assert_eq!(code.k(), 11);
        assert_eq!(code.correction_radius(), 1);
        assert_eq!(code.distance_bound().minimum_distance_lower_bound(), 3); // Hamming distance

        // Generator polynomial should have degree n - k = 4
        assert_eq!(code.generator().degree(), Some(4));
    }

    /// Test linearity: c1 + c2 should be a valid codeword if c1, c2 are
    #[test]
    fn test_linearity_property() {
        let code = gf16_code(1);

        // Encode two different messages
        let mut m1 = BitVec::zeros(11);
        for i in 0..11 {
            m1.set(i, i % 2 == 0);
        }

        let mut m2 = BitVec::zeros(11);
        for i in 0..11 {
            m2.set(i, i % 3 == 0);
        }

        let c1 = encode(&code, &m1);
        let c2 = encode(&code, &m2);

        // c1 XOR c2 should decode to m1 XOR m2
        let mut c_sum = BitVec::zeros(15);
        for i in 0..15 {
            c_sum.set(i, c1.get(i) ^ c2.get(i));
        }

        let mut m_sum = BitVec::zeros(11);
        for i in 0..11 {
            m_sum.set(i, m1.get(i) ^ m2.get(i));
        }

        let decoded_sum = decode_message(&code, &c_sum);
        assert_eq!(decoded_sum, m_sum, "Linearity property violated");
    }
}

mod error_correction_limits {
    use super::*;
    use rand::rngs::StdRng;
    use rand::{Rng, SeedableRng};

    /// Test that exactly t errors can be corrected
    #[test]
    fn test_corrects_exactly_t_errors() {
        let code = gf16_code(2);

        let msg = BitVec::ones(7);
        let cw = encode(&code, &msg);

        // Test with exactly t = 2 errors at various positions
        let error_patterns = vec![(0, 5), (1, 14), (3, 10), (7, 12)];

        for (pos1, pos2) in error_patterns {
            let mut received = cw.clone();
            received.set(pos1, !received.get(pos1));
            received.set(pos2, !received.get(pos2));

            let decoded = decode_message(&code, &received);
            assert_eq!(
                decoded, msg,
                "Failed to correct errors at positions {} and {}",
                pos1, pos2
            );
        }
    }

    /// Test multiple seeded random error patterns within correction capability
    #[test]
    fn test_random_correctable_errors() {
        let mut rng = StdRng::seed_from_u64(0xAE03_BCD0);

        let code = gf16_code(1);

        // Test 20 random single-error patterns
        for _ in 0..20 {
            let msg = BitVec::ones(11);
            let mut cw = encode(&code, &msg);

            // Inject single error at random position
            let error_pos = rng.gen_range(0..15);
            cw.set(error_pos, !cw.get(error_pos));

            let decoded = decode_message(&code, &cw);
            assert_eq!(
                decoded, msg,
                "Failed to correct error at position {}",
                error_pos
            );
        }
    }
}

mod systematic_encoding_validation {
    use super::*;

    /// Verify systematic form: message appears in first k positions
    #[test]
    fn test_systematic_form() {
        let code = gf16_code(1);

        let mut msg = BitVec::zeros(11);
        for i in 0..11 {
            msg.set(i, i % 2 == 1);
        }

        let cw = encode(&code, &msg);

        // Message should appear in positions [0, k) - systematic [message | parity] format
        for i in 0..11 {
            assert_eq!(
                cw.get(i),
                msg.get(i),
                "Message bit {} not in systematic position",
                i
            );
        }
    }

    /// Verify codeword is divisible by generator polynomial
    #[test]
    fn test_codeword_divisibility() {
        let code = gf16_code(2);

        // Test multiple messages
        for pattern in [0b0000000, 0b1111111, 0b1010101, 0b0110011] {
            let mut msg = BitVec::zeros(7);
            for i in 0..7 {
                msg.set(i, (pattern >> i) & 1 == 1);
            }

            let cw = encode(&code, &msg);

            // Should be divisible by generator
            let (_, remainder) = codeword_polynomial(&code, &cw).div_rem(code.generator());
            assert!(
                remainder.is_zero(),
                "Codeword not divisible by generator for message pattern {:07b}",
                pattern
            );
        }
    }
}

mod dvb_t2_validation {
    use super::*;
    use rand::rngs::StdRng;
    use rand::{Rng, SeedableRng};

    /// Verify DVB-T2 Short frame parameters match ETSI EN 302 755 specification
    #[test]
    fn test_dvb_t2_short_parameters() {
        let expected = vec![
            (CodeRate::Rate1_2, 7200, 7032, 12),
            (CodeRate::Rate3_5, 9720, 9552, 12),
            (CodeRate::Rate2_3, 10800, 10632, 12),
            (CodeRate::Rate3_4, 11880, 11712, 12),
            (CodeRate::Rate4_5, 12600, 12432, 12),
            (CodeRate::Rate5_6, 13320, 13152, 12),
        ];

        for (rate, n, k, t) in expected {
            let code = dvb_t2_bch_code(FrameSize::Short, rate).unwrap();
            assert_eq!(code.n(), n, "Wrong n for {:?}", rate);
            assert_eq!(code.k(), k, "Wrong k for {:?}", rate);
            assert_eq!(
                code.mother().code().correction_radius(),
                t,
                "Wrong t for {:?}",
                rate
            );

            // Verify generator polynomial degree equals BCH parity bits
            let deg = code.mother().code().generator().degree().unwrap();
            assert_eq!(
                deg,
                n - k,
                "Generator degree {} should equal parity bits {} for {:?}",
                deg,
                n - k,
                rate
            );
        }
    }

    /// Verify DVB-T2 Normal frame parameters match ETSI EN 302 755 specification
    #[test]
    fn test_dvb_t2_normal_parameters() {
        let expected = vec![
            (CodeRate::Rate1_2, 32400, 32208, 12),
            (CodeRate::Rate3_5, 38880, 38688, 12),
            (CodeRate::Rate2_3, 43200, 43040, 10), // t=10 for rate 2/3
            (CodeRate::Rate3_4, 48600, 48408, 12),
            (CodeRate::Rate4_5, 51840, 51648, 12),
            (CodeRate::Rate5_6, 54000, 53840, 10), // t=10 for rate 5/6
        ];

        for (rate, n, k, t) in expected {
            let code = dvb_t2_bch_code(FrameSize::Normal, rate).unwrap();
            assert_eq!(code.n(), n, "Wrong n for {:?}", rate);
            assert_eq!(code.k(), k, "Wrong k for {:?}", rate);
            assert_eq!(
                code.mother().code().correction_radius(),
                t,
                "Wrong t for {:?}",
                rate
            );

            // Verify generator polynomial degree equals BCH parity bits
            let deg = code.mother().code().generator().degree().unwrap();
            assert_eq!(
                deg,
                n - k,
                "Generator degree {} should equal parity bits {} for {:?}",
                deg,
                n - k,
                rate
            );
        }
    }

    /// Test DVB-T2 short frame encode/decode
    #[test]
    fn test_dvb_t2_short_encode_decode() {
        let code = dvb_t2_bch_code(FrameSize::Short, CodeRate::Rate1_2).unwrap();
        let decoder = DvbT2BchDecoder::new(&code);

        // Create test message (all zeros for simplicity)
        let msg = BitVec::zeros(code.k());

        // Encode
        let cw = code.encode(&msg).unwrap();
        assert_eq!(cw.len(), code.n());

        // Decode without errors
        let (outcome, decoded) = decoder.decode(&cw).unwrap();
        assert_eq!(outcome, BchDecodeOutcome::NoErrors);
        assert_eq!(decoded, msg);
    }

    /// Test DVB-T2 normal frame encode/decode
    #[test]
    fn test_dvb_t2_normal_encode_decode() {
        let code = dvb_t2_bch_code(FrameSize::Normal, CodeRate::Rate1_2).unwrap();
        let decoder = DvbT2BchDecoder::new(&code);

        // Create test message (all zeros for simplicity)
        let msg = BitVec::zeros(code.k());

        // Encode
        let cw = code.encode(&msg).unwrap();
        assert_eq!(cw.len(), code.n());

        // Decode without errors
        let (outcome, decoded) = decoder.decode(&cw).unwrap();
        assert_eq!(outcome, BchDecodeOutcome::NoErrors);
        assert_eq!(decoded, msg);
    }

    /// Injects `num_errors` distinct seeded errors into `codeword` and
    /// asserts that the decoder corrects all of them.
    fn assert_corrects(
        code: &DvbT2BchCode,
        msg: &BitVec,
        codeword: &BitVec,
        num_errors: usize,
        rng: &mut StdRng,
    ) {
        let mut corrupted = codeword.clone();
        let mut positions = Vec::new();

        // Inject errors at random positions
        for _ in 0..num_errors {
            loop {
                let pos = rng.gen_range(0..code.n());
                if !positions.contains(&pos) {
                    positions.push(pos);
                    corrupted.set(pos, !corrupted.get(pos));
                    break;
                }
            }
        }

        let (outcome, decoded) = DvbT2BchDecoder::new(code).decode(&corrupted).unwrap();
        assert_eq!(outcome, BchDecodeOutcome::Corrected { count: num_errors });
        assert_eq!(
            decoded,
            *msg,
            "Failed to correct {} errors (t={})",
            num_errors,
            code.mother().code().correction_radius()
        );
    }

    /// Test DVB-T2 short frame error correction capability
    #[test]
    fn test_dvb_t2_short_error_correction() {
        let code = dvb_t2_bch_code(FrameSize::Short, CodeRate::Rate1_2).unwrap();
        let t = code.mother().code().correction_radius();

        let mut rng = StdRng::seed_from_u64(54321);
        let msg = BitVec::random(code.k(), &mut rng);
        let cw = code.encode(&msg).unwrap();

        // Test correction of 1, t/2, and t errors
        for num_errors in [1, t / 2, t] {
            assert_corrects(&code, &msg, &cw, num_errors, &mut rng);
        }
    }

    /// Test DVB-T2 normal frame error correction capability
    #[test]
    fn test_dvb_t2_normal_error_correction() {
        let code = dvb_t2_bch_code(FrameSize::Normal, CodeRate::Rate1_2).unwrap();
        let t = code.mother().code().correction_radius();

        let mut rng = StdRng::seed_from_u64(98765);
        let msg = BitVec::random(code.k(), &mut rng);
        let cw = code.encode(&msg).unwrap();

        // Test correction of 1, t/2, and t errors
        for num_errors in [1, t / 2, t] {
            assert_corrects(&code, &msg, &cw, num_errors, &mut rng);
        }
    }

    /// Test DVB-T2 short frame - all code rates with error correction
    #[test]
    fn test_dvb_t2_short_all_rates_error_correction() {
        let rates = [
            CodeRate::Rate1_2,
            CodeRate::Rate3_5,
            CodeRate::Rate2_3,
            CodeRate::Rate3_4,
            CodeRate::Rate4_5,
            CodeRate::Rate5_6,
        ];

        for rate in rates {
            let code = dvb_t2_bch_code(FrameSize::Short, rate).unwrap();

            let mut rng = StdRng::seed_from_u64(11111);
            let msg = BitVec::random(code.k(), &mut rng);
            let cw = code.encode(&msg).unwrap();

            // Test with single error
            assert_corrects(&code, &msg, &cw, 1, &mut rng);
        }
    }

    /// Test DVB-T2 normal frame - all code rates with error correction
    #[test]
    fn test_dvb_t2_normal_all_rates_error_correction() {
        let rates = [
            CodeRate::Rate1_2,
            CodeRate::Rate3_5,
            CodeRate::Rate2_3,
            CodeRate::Rate3_4,
            CodeRate::Rate4_5,
            CodeRate::Rate5_6,
        ];

        for rate in rates {
            let code = dvb_t2_bch_code(FrameSize::Normal, rate).unwrap();

            let mut rng = StdRng::seed_from_u64(22222);
            let msg = BitVec::random(code.k(), &mut rng);
            let cw = code.encode(&msg).unwrap();

            // Test with single error
            assert_corrects(&code, &msg, &cw, 1, &mut rng);
        }
    }
}
