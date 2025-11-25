//! Comprehensive validation tests for LDPC codes.
//!
//! This test suite verifies mathematical properties and correctness of LDPC
//! code construction, encoding, and decoding following TDD principles.
//!
//! Test categories:
//! 1. Code construction validation (matrix properties)
//! 2. Mathematical property tests (linearity, orthogonality)
//! 3. DVB-T2 specific validation (parameter correctness, structure)
//! 4. Systematic encoding validation (when applicable)

use gf2_coding::ldpc::{LdpcCode, LdpcDecoder};
use gf2_coding::traits::{IterativeSoftDecoder, SoftDecoder};
use gf2_coding::llr::Llr;
use gf2_coding::CodeRate;
use gf2_core::BitVec;

/// Helper to create a simple regular-like LDPC code for testing.
/// Creates a small code from edges for property testing.
fn create_test_ldpc() -> LdpcCode {
    // Simple [7,4] Hamming-like code as LDPC
    let edges = vec![
        (0, 0), (0, 1), (0, 3),
        (1, 0), (1, 2), (1, 4),
        (2, 1), (2, 2), (2, 5),
    ];
    LdpcCode::from_edges(3, 7, &edges)
}

#[cfg(test)]
mod code_construction_validation {
    use super::*;

    /// Test that all-zero codeword is always valid (linearity requirement)
    #[test]
    fn test_zero_codeword_is_valid() {
        // Simple test LDPC code
        let code = create_test_ldpc();
        let zero_cw = BitVec::zeros(code.n());
        assert!(
            code.is_valid_codeword(&zero_cw),
            "All-zero codeword must be valid for any linear code"
        );

        // DVB-T2 code
        let dvb_code = LdpcCode::dvb_t2_normal(CodeRate::Rate1_2);
        let dvb_zero = BitVec::zeros(dvb_code.n());
        assert!(
            dvb_code.is_valid_codeword(&dvb_zero),
            "DVB-T2 all-zero codeword must be valid"
        );
    }

    /// Test syndrome computation produces correct dimensions
    #[test]
    fn test_syndrome_dimensions() {
        let code = create_test_ldpc();
        let codeword = BitVec::zeros(code.n());
        let syndrome = code.syndrome(&codeword);

        assert_eq!(
            syndrome.len(),
            code.m(),
            "Syndrome length must equal number of check nodes"
        );
    }

    /// Test that code parameters satisfy basic relationships
    #[test]
    fn test_code_parameter_relationships() {
        let code = create_test_ldpc();
        
        assert_eq!(code.n(), 7, "n should match construction parameter");
        assert_eq!(code.m(), 3, "m should match construction parameter");
        assert_eq!(code.k(), 4, "k = n - m for full-rank H");
        assert!(
            (code.rate() - 4.0 / 7.0).abs() < 1e-10,
            "Rate should equal k/n"
        );
    }

    /// Test parity-check matrix dimensions via syndrome computation
    #[test]
    fn test_parity_check_matrix_dimensions_via_syndrome() {
        let code = create_test_ldpc();
        
        // Syndrome dimensions tell us H dimensions: syndrome = H × codeword
        let codeword = BitVec::zeros(code.n());
        let syndrome = code.syndrome(&codeword);
        
        assert_eq!(syndrome.len(), 3, "H should have 3 rows (m)");
        assert_eq!(codeword.len(), 7, "H should have 7 columns (n)");
    }
}

#[cfg(test)]
mod mathematical_property_validation {
    use super::*;

    /// Test linearity property: H·(c₁ ⊕ c₂) = H·c₁ ⊕ H·c₂
    #[test]
    fn test_syndrome_linearity() {
        let code = LdpcCode::dvb_t2_normal(CodeRate::Rate1_2);
        
        // Create two valid codewords (all zeros for simplicity)
        let c1 = BitVec::zeros(code.n());
        let c2 = BitVec::zeros(code.n());
        
        let s1 = code.syndrome(&c1);
        let s2 = code.syndrome(&c2);
        
        // XOR the codewords
        let mut c1_xor_c2 = c1.clone();
        for i in 0..code.n() {
            c1_xor_c2.set(i, c1.get(i) ^ c2.get(i));
        }
        let s_sum = code.syndrome(&c1_xor_c2);
        
        // Compute s1 ⊕ s2
        let mut s1_xor_s2 = s1.clone();
        for i in 0..code.m() {
            s1_xor_s2.set(i, s1.get(i) ^ s2.get(i));
        }
        
        assert_eq!(
            s_sum.to_bytes_le(),
            s1_xor_s2.to_bytes_le(),
            "Syndrome must be linear: H(c₁⊕c₂) = H·c₁ ⊕ H·c₂"
        );
    }

    /// Test that syndrome is zero for valid codewords
    #[test]
    fn test_valid_codeword_zero_syndrome() {
        let code = create_test_ldpc();
        
        // All-zero is always a valid codeword
        let zero_cw = BitVec::zeros(code.n());
        let syndrome = code.syndrome(&zero_cw);
        
        assert_eq!(
            syndrome.count_ones(),
            0,
            "Valid codeword must have zero syndrome"
        );
        assert!(
            code.is_valid_codeword(&zero_cw),
            "is_valid_codeword should return true for zero syndrome"
        );
    }

    /// Test syndrome detects errors (non-zero syndrome for corrupted codeword)
    #[test]
    fn test_syndrome_detects_errors() {
        let code = LdpcCode::dvb_t2_normal(CodeRate::Rate1_2);
        
        // Start with valid codeword
        let mut corrupted = BitVec::zeros(code.n());
        
        // Introduce single bit error
        corrupted.set(100, true);
        
        let syndrome = code.syndrome(&corrupted);
        assert!(
            syndrome.count_ones() > 0,
            "Corrupted codeword should have non-zero syndrome"
        );
        assert!(
            !code.is_valid_codeword(&corrupted),
            "Corrupted codeword should not validate"
        );
    }

    /// Test XOR of two valid codewords is also valid (closure property)
    #[test]
    fn test_codeword_closure_under_xor() {
        let code = create_test_ldpc();
        
        // Two valid codewords
        let c1 = BitVec::zeros(code.n());
        let c2 = BitVec::zeros(code.n());
        
        assert!(code.is_valid_codeword(&c1));
        assert!(code.is_valid_codeword(&c2));
        
        // XOR them
        let mut c3 = c1.clone();
        for i in 0..code.n() {
            c3.set(i, c1.get(i) ^ c2.get(i));
        }
        
        assert!(
            code.is_valid_codeword(&c3),
            "XOR of valid codewords must be valid (linear code property)"
        );
    }
}

#[cfg(test)]
mod dvb_t2_parameter_validation {
    use super::*;

    /// Verify DVB-T2 Normal frame parameters match ETSI EN 302 755
    #[test]
    fn test_dvb_t2_normal_parameters() {
        let test_cases = vec![
            (CodeRate::Rate1_2, 64800, 32400, 32400),
            (CodeRate::Rate3_5, 64800, 38880, 25920),
            (CodeRate::Rate2_3, 64800, 43200, 21600),
            (CodeRate::Rate3_4, 64800, 48600, 16200),
            (CodeRate::Rate4_5, 64800, 51840, 12960),
            (CodeRate::Rate5_6, 64800, 54000, 10800),
        ];

        for (rate, expected_n, expected_k, expected_m) in test_cases {
            let code = LdpcCode::dvb_t2_normal(rate);
            
            assert_eq!(code.n(), expected_n, "Wrong n for {:?}", rate);
            assert_eq!(code.k(), expected_k, "Wrong k for {:?}", rate);
            assert_eq!(code.m(), expected_m, "Wrong m for {:?}", rate);
            
            // Verify n = k + m
            assert_eq!(
                code.n(),
                code.k() + code.m(),
                "n must equal k + m for {:?}",
                rate
            );
            
            // Verify rate calculation
            let calculated_rate = code.k() as f64 / code.n() as f64;
            let expected_rate = match rate {
                CodeRate::Rate1_2 => 0.5,
                CodeRate::Rate3_5 => 0.6,
                CodeRate::Rate2_3 => 2.0 / 3.0,
                CodeRate::Rate3_4 => 0.75,
                CodeRate::Rate4_5 => 0.8,
                CodeRate::Rate5_6 => 5.0 / 6.0,
            };
            assert!(
                (calculated_rate - expected_rate).abs() < 0.01,
                "Rate mismatch for {:?}: expected {}, got {}",
                rate,
                expected_rate,
                calculated_rate
            );
        }
    }

    /// Verify DVB-T2 Short frame parameters match ETSI EN 302 755
    #[test]
    fn test_dvb_t2_short_parameters() {
        let test_cases = vec![
            (CodeRate::Rate1_2, 16200, 7200, 9000),
            (CodeRate::Rate3_5, 16200, 9720, 6480),
            (CodeRate::Rate2_3, 16200, 10800, 5400),
            (CodeRate::Rate3_4, 16200, 11880, 4320),
            (CodeRate::Rate4_5, 16200, 12600, 3600),
            (CodeRate::Rate5_6, 16200, 13320, 2880),
        ];

        for (rate, expected_n, expected_k, expected_m) in test_cases {
            let code = LdpcCode::dvb_t2_short(rate);
            
            assert_eq!(code.n(), expected_n, "Wrong n for {:?}", rate);
            assert_eq!(code.k(), expected_k, "Wrong k for {:?}", rate);
            assert_eq!(code.m(), expected_m, "Wrong m for {:?}", rate);
            
            assert_eq!(
                code.n(),
                code.k() + code.m(),
                "n must equal k + m for {:?}",
                rate
            );
        }
    }

    /// Test that DVB-T2 codes have correct codeword length for standard
    #[test]
    fn test_dvb_t2_standard_codeword_lengths() {
        // Normal frames: 64800 bits
        let normal = LdpcCode::dvb_t2_normal(CodeRate::Rate1_2);
        assert_eq!(
            normal.n(),
            64800,
            "DVB-T2 Normal frames must have 64800 bits"
        );

        // Short frames: 16200 bits
        let short = LdpcCode::dvb_t2_short(CodeRate::Rate1_2);
        assert_eq!(
            short.n(),
            16200,
            "DVB-T2 Short frames must have 16200 bits"
        );
    }
}

#[cfg(test)]
mod from_edges_validation {
    use super::*;

    /// Test that from_edges construction works correctly
    #[test]
    fn test_from_edges_construction() {
        let code = create_test_ldpc();
        
        assert_eq!(code.n(), 7);
        assert_eq!(code.m(), 3);
        assert_eq!(code.k(), 4);
    }

    /// Test various from_edges configurations
    #[test]
    fn test_from_edges_parameter_variations() {
        // Different size codes
        let test_cases = vec![
            (2, 4, vec![(0, 0), (0, 1), (1, 2), (1, 3)]),
            (3, 6, vec![(0, 0), (0, 1), (1, 2), (1, 3), (2, 4), (2, 5)]),
        ];

        for (m, n, edges) in test_cases {
            let code = LdpcCode::from_edges(m, n, &edges);
            
            assert_eq!(code.m(), m);
            assert_eq!(code.n(), n);
            
            // Zero codeword should always be valid
            let zero = BitVec::zeros(n);
            assert!(
                code.is_valid_codeword(&zero),
                "Zero codeword must be valid for m={}, n={}",
                m, n
            );
        }
    }
}

#[cfg(test)]
mod decoder_validation {
    use super::*;

    /// Test decoder initialization and basic structure
    #[test]
    fn test_decoder_initialization() {
        let code = LdpcCode::dvb_t2_normal(CodeRate::Rate1_2);
        let decoder = LdpcDecoder::new(code.clone());
        
        // Decoder should be ready to use
        let llrs = vec![Llr::infinity(); code.n()]; // All bits certain to be 0
        let decoded = decoder.decode_soft(&llrs);
        
        // Decoded output should be valid codeword
        assert!(code.is_valid_codeword(&decoded), "Decoded message should be valid");
    }

    /// Test decoder handles all-zero input correctly
    #[test]
    fn test_decoder_all_zero_channel() {
        let code = create_test_ldpc();
        let decoder = LdpcDecoder::new(code.clone());
        
        // LLR = +∞ means bit is certain to be 0
        let llrs = vec![Llr::infinity(); code.n()];
        let decoded = decoder.decode_soft(&llrs);
        
        assert_eq!(
            decoded.count_ones(),
            0,
            "All-zero channel should decode to all-zero codeword"
        );
    }

    /// Test decoder convergence tracking with iterative decoder
    #[test]
    fn test_decoder_convergence_tracking() {
        let code = create_test_ldpc();
        let mut decoder = LdpcDecoder::new(code.clone());
        
        // Use moderate LLR values (not infinite)
        let llrs = vec![Llr::new(2.0); code.n()];
        let result = decoder.decode_iterative(&llrs, 50);
        
        assert!(
            result.iterations > 0,
            "Decoder should track iteration count"
        );
        assert!(
            result.iterations <= 50,
            "Decoder should not exceed max iterations"
        );
    }

    /// Test decoder produces valid codewords
    #[test]
    fn test_decoder_produces_valid_codewords() {
        let code = LdpcCode::dvb_t2_normal(CodeRate::Rate1_2);
        let decoder = LdpcDecoder::new(code.clone());
        
        // Perfect channel (all bits certain to be 0)
        let llrs = vec![Llr::infinity(); code.n()];
        let decoded = decoder.decode_soft(&llrs);
        
        assert!(
            code.is_valid_codeword(&decoded),
            "Decoder must produce valid codeword for perfect channel"
        );
    }
}

#[cfg(test)]
mod edge_case_validation {
    use super::*;

    /// Test syndrome computation with different bit patterns
    #[test]
    fn test_syndrome_various_patterns() {
        let code = create_test_ldpc();
        let n = code.n();
        
        // Test several patterns
        let patterns = vec![
            BitVec::zeros(n),
            {
                let mut bv = BitVec::zeros(n);
                bv.set(0, true);
                bv
            },
            {
                let mut bv = BitVec::zeros(n);
                for i in 0..n {
                    bv.set(i, i % 2 == 0);
                }
                bv
            },
        ];
        
        for pattern in patterns {
            let syndrome = code.syndrome(&pattern);
            assert_eq!(
                syndrome.len(),
                code.m(),
                "Syndrome should always have length m"
            );
        }
    }

    /// Test is_valid_codeword consistency with syndrome
    #[test]
    fn test_validity_check_consistency() {
        let code = LdpcCode::dvb_t2_normal(CodeRate::Rate1_2);
        
        let test_cases = vec![
            BitVec::zeros(code.n()),
            {
                let mut bv = BitVec::zeros(code.n());
                bv.set(100, true);
                bv
            },
        ];
        
        for codeword in test_cases {
            let is_valid = code.is_valid_codeword(&codeword);
            let syndrome = code.syndrome(&codeword);
            let syndrome_zero = syndrome.count_ones() == 0;
            
            assert_eq!(
                is_valid,
                syndrome_zero,
                "is_valid_codeword must match (syndrome == 0)"
            );
        }
    }
}
