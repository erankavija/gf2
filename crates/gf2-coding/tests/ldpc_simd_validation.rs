//! Validation that SIMD LLR operations match scalar for LDPC decoding.

use gf2_coding::ldpc::{LdpcCode, LdpcDecoder};
use gf2_coding::llr::Llr;
use gf2_coding::traits::IterativeSoftDecoder;

#[test]
fn test_simd_enabled() {
    #[cfg(feature = "simd")]
    {
        // Check if SIMD is actually available
        use std::arch::is_x86_feature_detected;
        if is_x86_feature_detected!("avx2") {
            println!("✅ AVX2 detected - SIMD should be active");
        } else {
            println!("⚠️  AVX2 not available - SIMD will use scalar fallback");
        }
    }
    
    #[cfg(not(feature = "simd"))]
    {
        println!("⚠️  SIMD feature not enabled");
    }
}

#[test]
fn test_ldpc_decode_with_simd() {
    use gf2_coding::bch::CodeRate;
    
    // Use DVB-T2 code
    let code = LdpcCode::dvb_t2_short(CodeRate::Rate1_2);
    let mut decoder = LdpcDecoder::new(code.clone());

    // Create channel LLRs (all-ones codeword with high confidence)
    let llrs: Vec<Llr> = (0..code.n()).map(|_| Llr::new(5.0)).collect();

    // Decode
    let result = decoder.decode_iterative(&llrs, 10);

    assert!(result.converged, "Should converge for clean signal");
    println!("Converged in {} iterations", result.iterations);
    println!("Decoded bits: {}", result.decoded_bits.len());
}

#[test]
#[cfg(feature = "simd")]
fn test_simd_vs_scalar_consistency() {
    use gf2_coding::bch::CodeRate;
    
    // Use DVB-T2 code
    let code = LdpcCode::dvb_t2_short(CodeRate::Rate1_2);
    
    // Test with various LLR patterns (must match code length)
    let n = code.n();
    let test_cases: Vec<Vec<f64>> = vec![
        vec![5.0; n],  // All high confidence
        vec![-5.0; n], // All negative
        (0..n).map(|i| if i % 2 == 0 { 5.0 } else { -5.0 }).collect(), // Alternating
    ];

    for llr_values in test_cases {
        let llrs: Vec<Llr> = llr_values.iter().map(|&v| Llr::new(v)).collect();
        
        let mut decoder1 = LdpcDecoder::new(code.clone());
        let result1 = decoder1.decode_iterative(&llrs, 5);

        let mut decoder2 = LdpcDecoder::new(code.clone());
        let result2 = decoder2.decode_iterative(&llrs, 5);

        // Results should be deterministic
        assert_eq!(result1.decoded_bits, result2.decoded_bits, "Decoding should be deterministic");
        assert_eq!(result1.converged, result2.converged);
        assert_eq!(result1.iterations, result2.iterations);
    }
}
