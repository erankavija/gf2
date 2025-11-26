//! TDD tests for LDPC systematic encoding.
//!
//! DVB-T2 LDPC codes use systematic encoding: codeword = [message | parity]
//! where the first k bits are the message and the last m bits are parity.
//!
//! Test-driven development approach:
//! 1. Write tests defining expected behavior
//! 2. Tests initially fail
//! 3. Implement minimal code to make tests pass
//! 4. Refactor while keeping tests green

use gf2_coding::ldpc::LdpcCode;
use gf2_coding::traits::BlockEncoder;
use gf2_coding::CodeRate;
use gf2_core::BitVec;

#[cfg(test)]
mod systematic_encoding_basic_tests {
    use super::*;

    /// Test that LdpcEncoder exists and can be constructed
    #[test]
    #[ignore = "Slow: DVB-T2 preprocessing (~2 seconds)"]
    fn test_ldpc_encoder_construction() {
        let code = LdpcCode::dvb_t2_short(CodeRate::Rate1_2);
        let encoder = gf2_coding::ldpc::LdpcEncoder::new(code);

        assert_eq!(encoder.k(), 7200);
        assert_eq!(encoder.n(), 16200);
    }

    /// Test systematic encoding: message appears in first k positions
    #[test]
    #[ignore = "Slow: DVB-T2 preprocessing (~2 seconds)"]
    fn test_systematic_form() {
        let code = LdpcCode::dvb_t2_short(CodeRate::Rate1_2);
        let encoder = gf2_coding::ldpc::LdpcEncoder::new(code.clone());

        // Create a simple message
        let mut message = BitVec::zeros(encoder.k());
        message.set(0, true);
        message.set(10, true);
        message.set(100, true);

        let codeword = encoder.encode(&message);

        // First k bits should match message (systematic form)
        for i in 0..encoder.k() {
            assert_eq!(
                codeword.get(i),
                message.get(i),
                "Message bit {} not preserved in systematic position",
                i
            );
        }

        // Codeword length should be n
        assert_eq!(codeword.len(), encoder.n());
    }

    /// Test that encoded codeword is valid (syndrome is zero)
    #[test]
    #[ignore = "Slow: DVB-T2 preprocessing (~2 seconds)"]
    fn test_encoded_codeword_is_valid() {
        let code = LdpcCode::dvb_t2_short(CodeRate::Rate1_2);
        let encoder = gf2_coding::ldpc::LdpcEncoder::new(code.clone());

        // Encode zero message
        let message = BitVec::zeros(encoder.k());
        let codeword = encoder.encode(&message);

        // Should be a valid codeword
        assert!(
            code.is_valid_codeword(&codeword),
            "Encoded codeword must be valid (zero syndrome)"
        );
    }

    /// Test encoding multiple messages
    #[test]
    #[ignore = "Slow: DVB-T2 preprocessing (~2 seconds)"]
    fn test_encode_multiple_messages() {
        let code = LdpcCode::dvb_t2_short(CodeRate::Rate1_2);
        let encoder = gf2_coding::ldpc::LdpcEncoder::new(code.clone());

        let test_messages = vec![
            BitVec::zeros(encoder.k()),
            {
                let mut bv = BitVec::zeros(encoder.k());
                bv.set(0, true);
                bv
            },
            {
                let mut bv = BitVec::zeros(encoder.k());
                for i in 0..encoder.k() {
                    bv.set(i, i % 2 == 0);
                }
                bv
            },
        ];

        for message in test_messages {
            let codeword = encoder.encode(&message);

            // Check systematic form
            for i in 0..encoder.k() {
                assert_eq!(codeword.get(i), message.get(i));
            }

            // Check validity
            assert!(code.is_valid_codeword(&codeword));
        }
    }
}

#[cfg(test)]
mod systematic_encoding_linearity_tests {
    use super::*;

    /// Test linearity: encode(m1 ⊕ m2) = encode(m1) ⊕ encode(m2)
    #[test]
    #[ignore = "Slow: DVB-T2 preprocessing (~2 seconds)"]
    fn test_encoding_linearity() {
        let code = LdpcCode::dvb_t2_short(CodeRate::Rate1_2);
        let encoder = gf2_coding::ldpc::LdpcEncoder::new(code);

        // Two messages
        let mut m1 = BitVec::zeros(encoder.k());
        m1.set(5, true);
        m1.set(10, true);

        let mut m2 = BitVec::zeros(encoder.k());
        m2.set(10, true);
        m2.set(20, true);

        // Encode separately
        let c1 = encoder.encode(&m1);
        let c2 = encoder.encode(&m2);

        // XOR messages
        let mut m_xor = m1.clone();
        for i in 0..encoder.k() {
            m_xor.set(i, m1.get(i) ^ m2.get(i));
        }

        // Encode XORed message
        let c_xor = encoder.encode(&m_xor);

        // XOR codewords
        let mut c1_xor_c2 = c1.clone();
        for i in 0..encoder.n() {
            c1_xor_c2.set(i, c1.get(i) ^ c2.get(i));
        }

        // Should be equal: encode(m1 ⊕ m2) = encode(m1) ⊕ encode(m2)
        assert_eq!(
            c_xor.to_bytes_le(),
            c1_xor_c2.to_bytes_le(),
            "Encoding must be linear"
        );
    }

    /// Test that encoding zero message produces zero codeword
    #[test]
    #[ignore = "Slow: DVB-T2 preprocessing (~2 seconds)"]
    fn test_zero_message_encodes_to_zero() {
        let code = LdpcCode::dvb_t2_short(CodeRate::Rate1_2);
        let encoder = gf2_coding::ldpc::LdpcEncoder::new(code);

        let zero_message = BitVec::zeros(encoder.k());
        let codeword = encoder.encode(&zero_message);

        assert_eq!(
            codeword.count_ones(),
            0,
            "Zero message should encode to all-zero codeword"
        );
    }
}

#[cfg(test)]
mod dvb_t2_encoding_tests {
    use super::*;

    /// Test all DVB-T2 Normal frame configurations
    #[test]
    #[ignore = "Very slow: preprocesses all 6 DVB-T2 Normal configs (~30-60 seconds)"]
    fn test_dvb_t2_normal_all_rates() {
        let rates = vec![
            CodeRate::Rate1_2,
            CodeRate::Rate3_5,
            CodeRate::Rate2_3,
            CodeRate::Rate3_4,
            CodeRate::Rate4_5,
            CodeRate::Rate5_6,
        ];

        for rate in rates {
            let code = LdpcCode::dvb_t2_normal(rate);
            let encoder = gf2_coding::ldpc::LdpcEncoder::new(code.clone());

            // Encode a simple message
            let message = BitVec::zeros(encoder.k());
            let codeword = encoder.encode(&message);

            // Verify systematic form and validity
            assert_eq!(codeword.len(), 64800);
            assert!(code.is_valid_codeword(&codeword));
        }
    }

    /// Test all DVB-T2 Short frame configurations
    #[test]
    #[ignore = "Slow: preprocesses all 6 DVB-T2 Short configs (~8-10 seconds)"]
    fn test_dvb_t2_short_all_rates() {
        let rates = vec![
            CodeRate::Rate1_2,
            CodeRate::Rate3_5,
            CodeRate::Rate2_3,
            CodeRate::Rate3_4,
            CodeRate::Rate4_5,
            CodeRate::Rate5_6,
        ];

        for rate in rates {
            let code = LdpcCode::dvb_t2_short(rate);
            let encoder = gf2_coding::ldpc::LdpcEncoder::new(code.clone());

            // Encode a simple message
            let message = BitVec::zeros(encoder.k());
            let codeword = encoder.encode(&message);

            // Verify systematic form and validity
            assert_eq!(codeword.len(), 16200);
            assert!(code.is_valid_codeword(&codeword));
        }
    }
}

#[cfg(test)]
mod parity_computation_tests {
    use super::*;

    /// Test that parity bits are actually computed (not all zero for non-zero message)
    #[test]
    #[ignore = "Slow: DVB-T2 preprocessing (~2 seconds)"]
    fn test_parity_bits_computed() {
        let code = LdpcCode::dvb_t2_short(CodeRate::Rate1_2);
        let encoder = gf2_coding::ldpc::LdpcEncoder::new(code);

        // Non-zero message
        let mut message = BitVec::zeros(encoder.k());
        message.set(0, true);
        message.set(100, true);

        let codeword = encoder.encode(&message);

        // Count ones in parity region [k, n)
        let mut parity_ones = 0;
        for i in encoder.k()..encoder.n() {
            if codeword.get(i) {
                parity_ones += 1;
            }
        }

        // Parity region should not be all zeros for non-zero message
        // (this would indicate parity is not being computed)
        assert!(
            parity_ones > 0,
            "Parity bits should be computed for non-zero message"
        );
    }

    /// Test parity structure for DVB-T2 (dual-diagonal property)
    #[test]
    #[ignore = "Slow: DVB-T2 preprocessing (~2 seconds)"]
    fn test_dvb_t2_parity_structure() {
        let code = LdpcCode::dvb_t2_short(CodeRate::Rate1_2);
        let encoder = gf2_coding::ldpc::LdpcEncoder::new(code.clone());

        // Encode a message
        let mut message = BitVec::zeros(encoder.k());
        message.set(50, true);

        let codeword = encoder.encode(&message);

        // DVB-T2 uses dual-diagonal parity structure
        // Verify that parity satisfies: H · c = 0
        assert!(
            code.is_valid_codeword(&codeword),
            "Codeword must satisfy dual-diagonal parity constraints"
        );
    }
}

#[cfg(test)]
mod edge_case_tests {
    use super::*;

    /// Test encoding with message length exactly k
    #[test]
    #[ignore = "Slow: DVB-T2 preprocessing (~2 seconds)"]
    fn test_encode_exact_length() {
        let code = LdpcCode::dvb_t2_short(CodeRate::Rate1_2);
        let encoder = gf2_coding::ldpc::LdpcEncoder::new(code);

        let message = BitVec::zeros(encoder.k());
        let codeword = encoder.encode(&message);

        assert_eq!(codeword.len(), encoder.n());
    }

    /// Test encoding preserves all message bits
    #[test]
    #[ignore = "Slow: DVB-T2 preprocessing (~2 seconds)"]
    fn test_all_message_bits_preserved() {
        let code = LdpcCode::dvb_t2_short(CodeRate::Rate1_2);
        let encoder = gf2_coding::ldpc::LdpcEncoder::new(code);

        // Pattern with bits set throughout message
        let mut message = BitVec::zeros(encoder.k());
        for i in (0..encoder.k()).step_by(100) {
            message.set(i, true);
        }

        let codeword = encoder.encode(&message);

        // Verify every message bit is preserved
        for i in 0..encoder.k() {
            assert_eq!(
                codeword.get(i),
                message.get(i),
                "Message bit {} not preserved",
                i
            );
        }
    }
}
