//! Integration tests for BCH codes.

use gf2_coding::bch::dvb_t2::FrameSize;
use gf2_coding::bch::{BchCode, BchDecoder, BchEncoder, CodeRate};
use gf2_coding::traits::BlockEncoder;
use gf2_core::gf2m::{Gf2mElement, Gf2mField, Gf2mPoly};
use gf2_core::BitVec;

#[cfg(test)]
mod bch_construction_tests {
    use super::*;

    #[test]
    fn test_bch_code_new() {
        let field = Gf2mField::new(4, 0b10011);
        let code = BchCode::new(15, 11, 1, field);

        assert_eq!(code.n(), 15);
        assert_eq!(code.k(), 11);
        assert_eq!(code.t(), 1);
    }

    #[test]
    fn test_generator_polynomial_degree() {
        let field = Gf2mField::new(4, 0b10011);
        let code = BchCode::new(15, 11, 1, field);

        // For t=1, generator has degree at most 2t*m where m is extension degree
        // In practice, should be around n-k = 4
        assert!(code.generator().degree().unwrap() <= 8);
        assert!(code.generator().degree().unwrap() >= 2); // At least 2 for t=1
    }

    #[test]
    fn test_generator_has_consecutive_roots() {
        let field = Gf2mField::new(4, 0b10011).with_tables();
        let code = BchCode::new(15, 7, 2, field.clone());
        let g = code.generator();

        // Generator should have α, α^2, α^3, α^4 as roots (for t=2)
        let alpha = field.primitive_element().unwrap();
        let mut alpha_power = alpha.clone();

        for i in 1..=(2 * code.t()) {
            let eval = g.eval(&alpha_power);
            assert!(
                eval.is_zero(),
                "Generator must vanish at α^{} (evaluation: {:?})",
                i,
                eval.value()
            );
            alpha_power = &alpha_power * &alpha;
        }
    }

    #[test]
    fn test_bch_code_parameters_valid() {
        let field = Gf2mField::new(4, 0b10011);
        let code = BchCode::new(15, 11, 1, field);

        assert_eq!(code.designed_distance(), 3); // 2t + 1 = 3
        assert_eq!(code.n() - code.k(), 4); // Parity bits
    }

    #[test]
    #[should_panic(expected = "Codeword length must exceed message length")]
    fn test_invalid_k_greater_than_n() {
        let field = Gf2mField::new(4, 0b10011);
        BchCode::new(15, 16, 1, field);
    }

    #[test]
    #[should_panic(expected = "must divide")]
    fn test_invalid_n_too_large() {
        let field = Gf2mField::new(4, 0b10011);
        BchCode::new(20, 10, 1, field); // 20 > 2^4 - 1 = 15
    }

    #[test]
    #[should_panic(expected = "Error correction capability must be positive")]
    fn test_invalid_t_zero() {
        let field = Gf2mField::new(4, 0b10011);
        BchCode::new(15, 11, 0, field);
    }
}

#[cfg(test)]
mod dvb_t2_parameter_tests {
    use super::*;

    #[test]
    fn test_dvb_t2_short_rate_half() {
        let code = BchCode::dvb_t2(FrameSize::Short, CodeRate::Rate1_2);

        assert_eq!(code.n(), 7200); // BCH output = LDPC k
        assert_eq!(code.k(), 7032); // BCH input = Kbch
        assert_eq!(code.t(), 12);
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
            let code = BchCode::dvb_t2(FrameSize::Short, rate);
            assert_eq!(code.k(), expected_k);
            assert_eq!(code.t(), 12);
        }
    }

    #[test]
    fn test_dvb_t2_normal_rate_half() {
        let code = BchCode::dvb_t2(FrameSize::Normal, CodeRate::Rate1_2);

        assert_eq!(code.n(), 32400); // BCH output = LDPC k
        assert_eq!(code.k(), 32208); // BCH input = Kbch
        assert_eq!(code.t(), 12);
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
            let code = BchCode::dvb_t2(FrameSize::Normal, rate);
            assert_eq!(code.k(), expected_k);
            assert_eq!(code.t(), expected_t);
        }
    }
}

#[cfg(test)]
mod encoding_tests {
    use super::*;

    #[test]
    fn test_encoder_creates_valid_codeword_length() {
        let field = Gf2mField::new(4, 0b10011);
        let code = BchCode::new(15, 11, 1, field);
        let encoder = BchEncoder::new(code.clone());

        let mut msg = BitVec::zeros(11);
        for i in 0..11 {
            msg.set(i, i % 2 == 0);
        }
        let cw = encoder.encode(&msg);

        assert_eq!(cw.len(), 15);
    }

    #[test]
    fn test_systematic_encoding_preserves_message() {
        let field = Gf2mField::new(4, 0b10011);
        let code = BchCode::new(15, 11, 1, field);
        let encoder = BchEncoder::new(code.clone());

        let mut msg = BitVec::zeros(11);
        for i in 0..11 {
            msg.set(i, (i / 2) % 2 == 0);
        }
        let cw = encoder.encode(&msg);

        // In systematic form, message appears in the last k positions
        for i in 0..11 {
            assert_eq!(
                cw.get(4 + i),
                msg.get(i),
                "Message bit {} not preserved at position {}",
                i,
                4 + i
            );
        }
    }

    #[test]
    fn test_zero_message_encodes_to_zero() {
        let field = Gf2mField::new(4, 0b10011);
        let code = BchCode::new(15, 11, 1, field);
        let encoder = BchEncoder::new(code);

        let msg = BitVec::zeros(11);
        let cw = encoder.encode(&msg);

        assert_eq!(cw, BitVec::zeros(15));
    }

    #[test]
    #[should_panic(expected = "Message must have length k")]
    fn test_encoder_rejects_wrong_message_length() {
        let field = Gf2mField::new(4, 0b10011);
        let code = BchCode::new(15, 11, 1, field);
        let encoder = BchEncoder::new(code);

        let msg = BitVec::zeros(10); // Wrong length
        encoder.encode(&msg);
    }

    #[test]
    fn test_all_ones_message() {
        let field = Gf2mField::new(4, 0b10011);
        let code = BchCode::new(15, 11, 1, field);
        let encoder = BchEncoder::new(code);

        let msg = BitVec::ones(11);
        let cw = encoder.encode(&msg);

        assert_eq!(cw.len(), 15);
        // Message part should be all ones
        for i in 4..15 {
            assert!(cw.get(i), "Message bit should be 1 at position {}", i);
        }
    }

    #[test]
    fn test_encoded_codeword_is_valid() {
        let field = Gf2mField::new(4, 0b10011).with_tables();
        let code = BchCode::new(15, 11, 1, field.clone());
        let encoder = BchEncoder::new(code.clone());

        let mut msg = BitVec::from_bytes_le(&[0b10101010, 0b101]);
        msg.resize(11, false); // Trim to exactly 11 bits
        let cw = encoder.encode(&msg);

        // Convert codeword to polynomial and check it's divisible by generator
        let cw_poly = bitvec_to_poly(&cw, &field);
        let (_, remainder) = cw_poly.div_rem(code.generator());

        assert!(
            remainder.is_zero(),
            "Codeword must be divisible by generator polynomial"
        );
    }
}

// Helper function for tests
#[cfg(test)]
fn bitvec_to_poly(bits: &BitVec, field: &Gf2mField) -> Gf2mPoly {
    let coeffs: Vec<Gf2mElement> = (0..bits.len())
        .map(|i| {
            if bits.get(i) {
                field.one()
            } else {
                field.zero()
            }
        })
        .collect();

    Gf2mPoly::new(coeffs)
}

#[cfg(test)]
mod syndrome_tests {
    use super::*;

    #[test]
    fn test_syndrome_zero_for_valid_codeword() {
        let field = Gf2mField::new(4, 0b10011);
        let code = BchCode::new(15, 11, 1, field);
        let encoder = BchEncoder::new(code.clone());
        let decoder = BchDecoder::new(code);

        let mut msg = BitVec::zeros(11);
        for i in 0..11 {
            msg.set(i, i % 3 == 0);
        }
        let cw = encoder.encode(&msg);

        let syndromes = decoder.compute_syndromes(&cw);

        // All syndromes should be zero for valid codeword
        for (i, s) in syndromes.iter().enumerate() {
            assert!(
                s.is_zero(),
                "Syndrome S_{} must be zero for valid codeword",
                i + 1
            );
        }
    }

    #[test]
    fn test_syndrome_nonzero_with_error() {
        let field = Gf2mField::new(4, 0b10011);
        let code = BchCode::new(15, 11, 1, field);
        let encoder = BchEncoder::new(code.clone());
        let decoder = BchDecoder::new(code);

        let mut msg = BitVec::zeros(11);
        for i in 0..11 {
            msg.set(i, (i / 2) % 2 == 0);
        }
        let mut cw = encoder.encode(&msg);

        // Introduce single-bit error
        cw.set(5, !cw.get(5));

        let syndromes = decoder.compute_syndromes(&cw);

        // At least one syndrome should be non-zero
        let has_nonzero = syndromes.iter().any(|s| !s.is_zero());
        assert!(has_nonzero, "Syndrome must detect error");
    }

    #[test]
    fn test_syndrome_length() {
        let field = Gf2mField::new(4, 0b10011);
        let code = BchCode::new(15, 11, 1, field);
        let decoder = BchDecoder::new(code.clone());

        let cw = BitVec::zeros(15);
        let syndromes = decoder.compute_syndromes(&cw);

        // Should compute 2t syndromes
        assert_eq!(syndromes.len(), 2 * code.t());
    }

    #[test]
    fn test_syndrome_multiple_errors() {
        let field = Gf2mField::new(4, 0b10011);
        let code = BchCode::new(15, 7, 2, field);
        let encoder = BchEncoder::new(code.clone());
        let decoder = BchDecoder::new(code);

        let msg = BitVec::zeros(7);
        let mut cw = encoder.encode(&msg);

        // Introduce 2 errors
        cw.set(3, !cw.get(3));
        cw.set(10, !cw.get(10));

        let syndromes = decoder.compute_syndromes(&cw);

        // Syndromes should be non-zero
        let has_nonzero = syndromes.iter().any(|s| !s.is_zero());
        assert!(has_nonzero, "Syndromes must detect multiple errors");
    }

    #[test]
    #[should_panic(expected = "Received vector must have length")]
    fn test_syndrome_wrong_length_panics() {
        let field = Gf2mField::new(4, 0b10011);
        let code = BchCode::new(15, 11, 1, field);
        let decoder = BchDecoder::new(code);

        let cw = BitVec::zeros(14); // Wrong length
        decoder.compute_syndromes(&cw);
    }
}

#[cfg(test)]
mod berlekamp_massey_tests {
    use super::*;

    #[test]
    fn test_berlekamp_massey_no_errors() {
        let field = Gf2mField::new(4, 0b10011);
        let code = BchCode::new(15, 11, 1, field.clone());
        let decoder = BchDecoder::new(code);

        // All-zero syndromes (no errors)
        let syndromes = vec![field.zero(); 2];
        let lambda = decoder.berlekamp_massey(&syndromes);

        // Error locator should be Λ(x) = 1 (constant polynomial)
        assert_eq!(lambda.degree(), Some(0));
        assert!(lambda.coeff(0).is_one());
    }

    #[test]
    fn test_berlekamp_massey_single_error() {
        let field = Gf2mField::new(4, 0b10011);
        let code = BchCode::new(15, 11, 1, field.clone());
        let encoder = BchEncoder::new(code.clone());
        let decoder = BchDecoder::new(code);

        let msg = BitVec::ones(11);
        let mut cw = encoder.encode(&msg);
        cw.set(5, !cw.get(5)); // Single error at position 5

        let syndromes = decoder.compute_syndromes(&cw);
        let lambda = decoder.berlekamp_massey(&syndromes);

        // For single error, degree should be 1
        assert_eq!(lambda.degree(), Some(1));
    }

    #[test]
    fn test_berlekamp_massey_two_errors() {
        let field = Gf2mField::new(4, 0b10011);
        let code = BchCode::new(15, 7, 2, field.clone());
        let encoder = BchEncoder::new(code.clone());
        let decoder = BchDecoder::new(code);

        let msg = BitVec::zeros(7);
        let mut cw = encoder.encode(&msg);

        // Inject 2 errors
        cw.set(3, !cw.get(3));
        cw.set(10, !cw.get(10));

        let syndromes = decoder.compute_syndromes(&cw);
        let lambda = decoder.berlekamp_massey(&syndromes);

        // For 2 errors, degree should be 2
        assert_eq!(lambda.degree(), Some(2));
    }

    #[test]
    fn test_berlekamp_massey_degree_bound() {
        let field = Gf2mField::new(4, 0b10011);
        let code = BchCode::new(15, 7, 2, field.clone());
        let encoder = BchEncoder::new(code.clone());
        let decoder = BchDecoder::new(code.clone());

        let msg = BitVec::ones(7);
        let mut cw = encoder.encode(&msg);

        // Inject t errors
        cw.set(1, !cw.get(1));
        cw.set(8, !cw.get(8));

        let syndromes = decoder.compute_syndromes(&cw);
        let lambda = decoder.berlekamp_massey(&syndromes);

        // Degree should be at most t
        assert!(lambda.degree().unwrap_or(0) <= code.t());
    }
}

#[cfg(test)]
mod chien_search_tests {
    use super::*;

    #[test]
    fn test_chien_search_no_errors() {
        let field = Gf2mField::new(4, 0b10011);
        let code = BchCode::new(15, 11, 1, field.clone());
        let decoder = BchDecoder::new(code);

        // Lambda(x) = 1 means no errors
        let lambda = Gf2mPoly::constant(field.one());
        let positions = decoder.chien_search(&lambda);

        assert_eq!(positions.len(), 0);
    }

    #[test]
    fn test_chien_search_single_error() {
        let field = Gf2mField::new(4, 0b10011);
        let code = BchCode::new(15, 11, 1, field.clone());
        let encoder = BchEncoder::new(code.clone());
        let decoder = BchDecoder::new(code);

        let msg = BitVec::ones(11);
        let mut cw = encoder.encode(&msg);

        let error_pos = 5;
        cw.set(error_pos, !cw.get(error_pos));

        let syndromes = decoder.compute_syndromes(&cw);
        let lambda = decoder.berlekamp_massey(&syndromes);
        let positions = decoder.chien_search(&lambda);

        assert_eq!(positions.len(), 1);
        assert_eq!(positions[0], error_pos);
    }

    #[test]
    fn test_chien_search_multiple_errors() {
        let field = Gf2mField::new(4, 0b10011);
        let code = BchCode::new(15, 7, 2, field.clone());
        let encoder = BchEncoder::new(code.clone());
        let decoder = BchDecoder::new(code);

        let msg = BitVec::zeros(7);
        let mut cw = encoder.encode(&msg);

        // Inject 2 errors
        cw.set(3, !cw.get(3));
        cw.set(10, !cw.get(10));

        let syndromes = decoder.compute_syndromes(&cw);
        let lambda = decoder.berlekamp_massey(&syndromes);
        let mut positions = decoder.chien_search(&lambda);
        positions.sort();

        assert_eq!(positions.len(), 2);
        assert_eq!(positions, vec![3, 10]);
    }

    #[test]
    fn test_chien_search_correctable_errors() {
        let field = Gf2mField::new(4, 0b10011);
        let code = BchCode::new(15, 7, 2, field.clone());
        let encoder = BchEncoder::new(code.clone());
        let decoder = BchDecoder::new(code.clone());

        let msg = BitVec::ones(7);
        let mut cw = encoder.encode(&msg);

        // Inject exactly t errors
        cw.set(0, !cw.get(0));
        cw.set(14, !cw.get(14));

        let syndromes = decoder.compute_syndromes(&cw);
        let lambda = decoder.berlekamp_massey(&syndromes);
        let positions = decoder.chien_search(&lambda);

        // Should find exactly t error positions
        assert_eq!(positions.len(), code.t());
    }
}

#[cfg(test)]
mod decoder_integration_tests {
    use super::*;
    use gf2_coding::traits::HardDecisionDecoder;

    #[test]
    fn test_decode_no_errors() {
        let field = Gf2mField::new(4, 0b10011);
        let code = BchCode::new(15, 11, 1, field);
        let encoder = BchEncoder::new(code.clone());
        let decoder = BchDecoder::new(code);

        let msg = BitVec::ones(11);
        let cw = encoder.encode(&msg);
        let decoded = decoder.decode(&cw);

        assert_eq!(decoded, msg);
    }

    #[test]
    fn test_decode_single_error() {
        let field = Gf2mField::new(4, 0b10011);
        let code = BchCode::new(15, 11, 1, field);
        let encoder = BchEncoder::new(code.clone());
        let decoder = BchDecoder::new(code);

        let mut msg = BitVec::zeros(11);
        for i in 0..11 {
            msg.set(i, i % 3 == 0);
        }
        let mut cw = encoder.encode(&msg);

        // Inject single error
        cw.set(7, !cw.get(7));

        let decoded = decoder.decode(&cw);
        assert_eq!(decoded, msg);
    }

    #[test]
    fn test_decode_multiple_errors() {
        let field = Gf2mField::new(4, 0b10011);
        let code = BchCode::new(15, 7, 2, field);
        let encoder = BchEncoder::new(code.clone());
        let decoder = BchDecoder::new(code);

        let msg = BitVec::ones(7);
        let mut cw = encoder.encode(&msg);

        // Inject 2 errors (within correction capability)
        cw.set(2, !cw.get(2));
        cw.set(12, !cw.get(12));

        let decoded = decoder.decode(&cw);
        assert_eq!(decoded, msg);
    }

    #[test]
    fn test_decode_roundtrip_various_messages() {
        let field = Gf2mField::new(4, 0b10011);
        let code = BchCode::new(15, 11, 1, field);
        let encoder = BchEncoder::new(code.clone());
        let decoder = BchDecoder::new(code);

        // Test various message patterns
        let test_messages = vec![BitVec::zeros(11), BitVec::ones(11), {
            let mut msg = BitVec::zeros(11);
            for i in 0..11 {
                msg.set(i, i % 2 == 0);
            }
            msg
        }];

        for msg in test_messages {
            let cw = encoder.encode(&msg);
            let decoded = decoder.decode(&cw);
            assert_eq!(decoded, msg, "Roundtrip failed for message");
        }
    }

    #[test]
    fn test_decode_corrects_up_to_t_errors() {
        let field = Gf2mField::new(4, 0b10011);
        let code = BchCode::new(15, 7, 2, field);
        let encoder = BchEncoder::new(code.clone());
        let decoder = BchDecoder::new(code.clone());

        let msg = BitVec::zeros(7);
        let mut cw = encoder.encode(&msg);

        // Inject exactly t errors
        cw.set(1, !cw.get(1));
        cw.set(8, !cw.get(8));

        let decoded = decoder.decode(&cw);
        assert_eq!(decoded, msg);
    }
}
#[cfg(test)]
mod known_bch_codes {
    use super::*;
    use gf2_coding::traits::HardDecisionDecoder;

    /// Test BCH(15, 7, 2) - well-documented in literature
    /// Generator polynomial: x^8 + x^7 + x^6 + x^4 + 1 (over GF(2^4))
    #[test]
    fn test_bch_15_7_2_properties() {
        let field = Gf2mField::new(4, 0b10011).with_tables(); // x^4 + x + 1
        let code = BchCode::new(15, 7, 2, field.clone());

        // Verify parameters
        assert_eq!(code.n(), 15);
        assert_eq!(code.k(), 7);
        assert_eq!(code.t(), 2);
        assert_eq!(code.designed_distance(), 5); // 2t + 1

        // Verify generator polynomial degree
        let g = code.generator();
        assert_eq!(g.degree(), Some(8)); // n - k = 15 - 7 = 8

        // Generator should have roots at α, α^2, α^3, α^4
        let alpha = field.primitive_element().unwrap();
        let mut alpha_power = alpha.clone();
        for i in 1..=4 {
            let eval = g.eval(&alpha_power);
            assert!(eval.is_zero(), "Generator must have α^{} as root", i);
            alpha_power = &alpha_power * &alpha;
        }
    }

    /// Test BCH(15, 11, 1) - single error correcting
    /// This is equivalent to Hamming(15, 11)
    #[test]
    fn test_bch_15_11_1_hamming_equivalence() {
        let field = Gf2mField::new(4, 0b10011);
        let code = BchCode::new(15, 11, 1, field);

        assert_eq!(code.n(), 15);
        assert_eq!(code.k(), 11);
        assert_eq!(code.t(), 1);
        assert_eq!(code.designed_distance(), 3); // Hamming distance

        // Generator polynomial should have degree n - k = 4
        assert_eq!(code.generator().degree(), Some(4));
    }

    /// Test linearity: c1 + c2 should be a valid codeword if c1, c2 are
    #[test]
    fn test_linearity_property() {
        let field = Gf2mField::new(4, 0b10011);
        let code = BchCode::new(15, 11, 1, field);
        let encoder = BchEncoder::new(code.clone());
        let decoder = BchDecoder::new(code);

        // Encode two different messages
        let mut m1 = BitVec::zeros(11);
        for i in 0..11 {
            m1.set(i, i % 2 == 0);
        }

        let mut m2 = BitVec::zeros(11);
        for i in 0..11 {
            m2.set(i, i % 3 == 0);
        }

        let c1 = encoder.encode(&m1);
        let c2 = encoder.encode(&m2);

        // c1 XOR c2 should decode to m1 XOR m2
        let mut c_sum = BitVec::zeros(15);
        for i in 0..15 {
            c_sum.set(i, c1.get(i) ^ c2.get(i));
        }

        let mut m_sum = BitVec::zeros(11);
        for i in 0..11 {
            m_sum.set(i, m1.get(i) ^ m2.get(i));
        }

        let decoded_sum = decoder.decode(&c_sum);
        assert_eq!(decoded_sum, m_sum, "Linearity property violated");
    }
}

#[cfg(test)]
mod error_correction_limits {
    use super::*;
    use gf2_coding::traits::HardDecisionDecoder;

    /// Test that exactly t errors can be corrected
    #[test]
    fn test_corrects_exactly_t_errors() {
        let field = Gf2mField::new(4, 0b10011);
        let code = BchCode::new(15, 7, 2, field);
        let encoder = BchEncoder::new(code.clone());
        let decoder = BchDecoder::new(code.clone());

        let msg = BitVec::ones(7);
        let cw = encoder.encode(&msg);

        // Test with exactly t = 2 errors at various positions
        let error_patterns = vec![(0, 5), (1, 14), (3, 10), (7, 12)];

        for (pos1, pos2) in error_patterns {
            let mut received = cw.clone();
            received.set(pos1, !received.get(pos1));
            received.set(pos2, !received.get(pos2));

            let decoded = decoder.decode(&received);
            assert_eq!(
                decoded, msg,
                "Failed to correct errors at positions {} and {}",
                pos1, pos2
            );
        }
    }

    /// Test multiple random error patterns within correction capability
    #[test]
    fn test_random_correctable_errors() {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        let field = Gf2mField::new(4, 0b10011);
        let code = BchCode::new(15, 11, 1, field);
        let encoder = BchEncoder::new(code.clone());
        let decoder = BchDecoder::new(code);

        // Test 20 random single-error patterns
        for _ in 0..20 {
            let msg = BitVec::ones(11);
            let mut cw = encoder.encode(&msg);

            // Inject single error at random position
            let error_pos = rng.gen_range(0..15);
            cw.set(error_pos, !cw.get(error_pos));

            let decoded = decoder.decode(&cw);
            assert_eq!(
                decoded, msg,
                "Failed to correct error at position {}",
                error_pos
            );
        }
    }
}

#[cfg(test)]
mod systematic_encoding_validation {
    use super::*;
    use gf2_core::gf2m::Gf2mPoly;

    /// Verify systematic form: message appears in last k positions
    #[test]
    fn test_systematic_form() {
        let field = Gf2mField::new(4, 0b10011);
        let code = BchCode::new(15, 11, 1, field);
        let encoder = BchEncoder::new(code.clone());

        let mut msg = BitVec::zeros(11);
        for i in 0..11 {
            msg.set(i, i % 2 == 1);
        }

        let cw = encoder.encode(&msg);

        // Message should appear in positions [n-k, n)
        let r = code.n() - code.k();
        for i in 0..11 {
            assert_eq!(
                cw.get(r + i),
                msg.get(i),
                "Message bit {} not in systematic position",
                i
            );
        }
    }

    /// Verify codeword is divisible by generator polynomial
    #[test]
    fn test_codeword_divisibility() {
        let field = Gf2mField::new(4, 0b10011).with_tables();
        let code = BchCode::new(15, 7, 2, field.clone());
        let encoder = BchEncoder::new(code.clone());

        // Test multiple messages
        for pattern in [0b0000000, 0b1111111, 0b1010101, 0b0110011] {
            let mut msg = BitVec::zeros(7);
            for i in 0..7 {
                msg.set(i, (pattern >> i) & 1 == 1);
            }

            let cw = encoder.encode(&msg);

            // Convert to polynomial
            let mut coeffs = Vec::new();
            for i in 0..15 {
                coeffs.push(if cw.get(i) { field.one() } else { field.zero() });
            }
            let cw_poly = Gf2mPoly::new(coeffs);

            // Should be divisible by generator
            let (_, remainder) = cw_poly.div_rem(code.generator());
            assert!(
                remainder.is_zero(),
                "Codeword not divisible by generator for message pattern {:07b}",
                pattern
            );
        }
    }
}

#[cfg(test)]
mod dvb_t2_validation {
    use super::*;
    use gf2_coding::bch::CodeRate;
    use gf2_coding::traits::HardDecisionDecoder;
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
            let code = BchCode::dvb_t2(FrameSize::Short, rate);
            assert_eq!(code.n(), n, "Wrong n for {:?}", rate);
            assert_eq!(code.k(), k, "Wrong k for {:?}", rate);
            assert_eq!(code.t(), t, "Wrong t for {:?}", rate);

            // Verify generator polynomial degree equals BCH parity bits
            let deg = code.generator().degree().unwrap();
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
            let code = BchCode::dvb_t2(FrameSize::Normal, rate);
            assert_eq!(code.n(), n, "Wrong n for {:?}", rate);
            assert_eq!(code.k(), k, "Wrong k for {:?}", rate);
            assert_eq!(code.t(), t, "Wrong t for {:?}", rate);

            // Verify generator polynomial degree equals BCH parity bits
            let deg = code.generator().degree().unwrap();
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
        let code = BchCode::dvb_t2(FrameSize::Short, CodeRate::Rate1_2);
        let encoder = BchEncoder::new(code.clone());
        let decoder = BchDecoder::new(code.clone());

        // Create test message (all zeros for simplicity)
        let msg = BitVec::zeros(code.k());

        // Encode
        let cw = encoder.encode(&msg);
        assert_eq!(cw.len(), code.n());

        // Decode without errors
        let decoded = decoder.decode(&cw);
        assert_eq!(decoded, msg);
    }

    /// Test DVB-T2 normal frame encode/decode
    #[test]
    fn test_dvb_t2_normal_encode_decode() {
        let code = BchCode::dvb_t2(FrameSize::Normal, CodeRate::Rate1_2);
        let encoder = BchEncoder::new(code.clone());
        let decoder = BchDecoder::new(code.clone());

        // Create test message (all zeros for simplicity)
        let msg = BitVec::zeros(code.k());

        // Encode
        let cw = encoder.encode(&msg);
        assert_eq!(cw.len(), code.n());

        // Decode without errors
        let decoded = decoder.decode(&cw);
        assert_eq!(decoded, msg);
    }

    /// Test DVB-T2 short frame error correction capability
    #[test]
    fn test_dvb_t2_short_error_correction() {
        let code = BchCode::dvb_t2(FrameSize::Short, CodeRate::Rate1_2);
        let encoder = BchEncoder::new(code.clone());
        let decoder = BchDecoder::new(code.clone());

        let mut rng = rand::rngs::StdRng::seed_from_u64(54321);
        let msg = BitVec::random(code.k(), &mut rng);
        let cw = encoder.encode(&msg);

        // Test correction of 1, t/2, and t errors
        for num_errors in [1, code.t() / 2, code.t()] {
            let mut corrupted = cw.clone();
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

            let decoded = decoder.decode(&corrupted);
            assert_eq!(
                decoded,
                msg,
                "Failed to correct {} errors (t={})",
                num_errors,
                code.t()
            );
        }
    }

    /// Test DVB-T2 normal frame error correction capability
    #[test]
    fn test_dvb_t2_normal_error_correction() {
        let code = BchCode::dvb_t2(FrameSize::Normal, CodeRate::Rate1_2);
        let encoder = BchEncoder::new(code.clone());
        let decoder = BchDecoder::new(code.clone());

        let mut rng = rand::rngs::StdRng::seed_from_u64(98765);
        let msg = BitVec::random(code.k(), &mut rng);
        let cw = encoder.encode(&msg);

        // Test correction of 1, t/2, and t errors
        for num_errors in [1, code.t() / 2, code.t()] {
            let mut corrupted = cw.clone();
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

            let decoded = decoder.decode(&corrupted);
            assert_eq!(
                decoded,
                msg,
                "Failed to correct {} errors (t={})",
                num_errors,
                code.t()
            );
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
            let code = BchCode::dvb_t2(FrameSize::Short, rate);
            let encoder = BchEncoder::new(code.clone());
            let decoder = BchDecoder::new(code.clone());

            let mut rng = rand::rngs::StdRng::seed_from_u64(11111);
            let msg = BitVec::random(code.k(), &mut rng);
            let mut cw = encoder.encode(&msg);

            // Test with single error
            let error_pos = rng.gen_range(0..code.n());
            cw.set(error_pos, !cw.get(error_pos));

            let decoded = decoder.decode(&cw);
            assert_eq!(
                decoded, msg,
                "Short frame rate {:?} failed single error correction",
                rate
            );
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
            let code = BchCode::dvb_t2(FrameSize::Normal, rate);
            let encoder = BchEncoder::new(code.clone());
            let decoder = BchDecoder::new(code.clone());

            let mut rng = rand::rngs::StdRng::seed_from_u64(22222);
            let msg = BitVec::random(code.k(), &mut rng);
            let mut cw = encoder.encode(&msg);

            // Test with single error
            let error_pos = rng.gen_range(0..code.n());
            cw.set(error_pos, !cw.get(error_pos));

            let decoded = decoder.decode(&cw);
            assert_eq!(
                decoded, msg,
                "Normal frame rate {:?} failed single error correction",
                rate
            );
        }
    }
}
