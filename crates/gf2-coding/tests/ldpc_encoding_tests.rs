//! Tests for LDPC systematic encoding using Richardson-Urbanke algorithm.
//!
//! Following TDD approach: tests define expected behavior before implementation.

use gf2_coding::ldpc::LdpcCode;
use gf2_core::BitVec;

/// Helper: Create a simple (7,4) Hamming code as LDPC for testing
fn simple_ldpc_7_4() -> LdpcCode {
    // Parity-check matrix H for [7,4] Hamming code:
    // H = [1 0 1 1 1 0 0]
    //     [0 1 0 1 0 1 0]
    //     [0 0 1 1 0 0 1]
    let edges = vec![
        (0, 0), (0, 2), (0, 3), (0, 4),
        (1, 1), (1, 3), (1, 5),
        (2, 2), (2, 3), (2, 6),
    ];
    LdpcCode::from_edges(3, 7, &edges)
}

/// Helper: Generate all 4-bit messages
fn all_4_bit_messages() -> Vec<BitVec> {
    (0u8..16)
        .map(|n| {
            let mut bv = BitVec::new();
            for i in 0..4 {
                bv.push_bit((n >> i) & 1 == 1);
            }
            bv
        })
        .collect()
}

// ============================================================================
// Phase 1: Core Richardson-Urbanke Algorithm Tests
// ============================================================================

#[test]
fn test_ru_preprocess_simple_ldpc() {
    use gf2_coding::ldpc::LdpcEncoder;
    use gf2_coding::traits::BlockEncoder;
    
    let code = simple_ldpc_7_4();
    
    // Creating an encoder preprocesses the matrix
    let encoder = LdpcEncoder::new(code.clone());
    
    // Verify dimensions through encoder
    assert_eq!(encoder.n(), 7, "n should be 7");
    assert_eq!(encoder.k(), 4, "k should be 4 (7-3)");
}

#[test]
fn test_ru_encoding_produces_valid_codewords() {
    use gf2_coding::ldpc::LdpcEncoder;
    use gf2_coding::traits::BlockEncoder;
    
    let code = simple_ldpc_7_4();
    let encoder = LdpcEncoder::new(code.clone());
    
    // Test all 16 possible 4-bit messages
    for message in all_4_bit_messages() {
        let codeword = encoder.encode(&message);
        assert_eq!(codeword.len(), 7, "Codeword should be 7 bits");
        assert!(
            code.is_valid_codeword(&codeword),
            "Encoded codeword must satisfy H·c = 0"
        );
    }
}

#[test]
fn test_ru_encoding_is_systematic() {
    use gf2_coding::ldpc::LdpcEncoder;
    use gf2_coding::traits::BlockEncoder;
    
    let code = simple_ldpc_7_4();
    let encoder = LdpcEncoder::new(code);
    
    // Create 4-bit message [0,1,0,1]
    let mut message = BitVec::new();
    message.push_bit(false);
    message.push_bit(true);
    message.push_bit(false);
    message.push_bit(true);
    
    let codeword = encoder.encode(&message);
    
    // First k bits should equal the message
    for i in 0..4 {
        assert_eq!(
            codeword.get(i),
            message.get(i),
            "Bit {} should match message bit",
            i
        );
    }
}

// ============================================================================
// Phase 2: DVB-T2 Integration Tests
// ============================================================================

#[test]
#[ignore = "DVB-T2 preprocessing is slow"]
fn test_dvb_t2_preprocessing_all_configs() {
    use gf2_coding::bch::CodeRate;
    use gf2_coding::ldpc::LdpcEncoder;
    
    let rates = [
        CodeRate::Rate1_2,
        CodeRate::Rate3_5,
        CodeRate::Rate2_3,
        CodeRate::Rate3_4,
        CodeRate::Rate4_5,
        CodeRate::Rate5_6,
    ];
    
    for rate in &rates {
        // Test short frames - encoder creation does preprocessing
        let code_short = LdpcCode::dvb_t2_short(*rate);
        let _encoder = LdpcEncoder::new(code_short);
        // If we get here, preprocessing succeeded
        
        // Test normal frames (skip for now - very large)
        // Normal frames can be tested after basic implementation works
    }
}

#[test]
#[ignore = "DVB-T2 short frame preprocessing is slow"]
fn test_ldpc_encoder_creation() {
    use gf2_coding::bch::CodeRate;
    use gf2_coding::ldpc::LdpcEncoder;
    use gf2_coding::traits::BlockEncoder;
    
    let code = LdpcCode::dvb_t2_short(CodeRate::Rate1_2);
    let encoder = LdpcEncoder::new(code.clone());
    
    // Generate random message
    let mut message = BitVec::new();
    for _ in 0..code.k() {
        message.push_bit(rand::random());
    }
    
    let codeword = encoder.encode(&message);
    
    assert_eq!(codeword.len(), code.n(), "Codeword length should match n");
    
    // Check systematic form: first k bits = message
    for i in 0..code.k() {
        assert_eq!(
            codeword.get(i),
            message.get(i),
            "Systematic encoding: bit {} should match",
            i
        );
    }
}

#[test]
#[ignore = "DVB-T2 short frame preprocessing is slow"]
fn test_dvb_t2_encoded_codewords_valid() {
    use gf2_coding::bch::CodeRate;
    use gf2_coding::ldpc::LdpcEncoder;
    use gf2_coding::traits::BlockEncoder;
    
    let code = LdpcCode::dvb_t2_short(CodeRate::Rate1_2);
    let encoder = LdpcEncoder::new(code.clone());
    
    // Test 10 random messages
    for _ in 0..10 {
        let mut message = BitVec::new();
        for _ in 0..code.k() {
            message.push_bit(rand::random());
        }
        
        let codeword = encoder.encode(&message);
        assert!(
            code.is_valid_codeword(&codeword),
            "Encoded codeword must be valid (H·c = 0)"
        );
    }
}

#[test]
#[ignore = "DVB-T2 short frames take time to preprocess"]
fn test_ldpc_encode_decode_roundtrip_simple() {
    use gf2_coding::bch::CodeRate;
    use gf2_coding::ldpc::{LdpcDecoder, LdpcEncoder};
    use gf2_coding::llr::Llr;
    use gf2_coding::traits::{BlockEncoder, IterativeSoftDecoder};
    
    let code = LdpcCode::dvb_t2_short(CodeRate::Rate1_2);
    let encoder = LdpcEncoder::new(code.clone());
    let mut decoder = LdpcDecoder::new(code.clone());
    
    // Random message
    let mut message = BitVec::new();
    for _ in 0..code.k() {
        message.push_bit(rand::random());
    }
    
    // Encode
    let codeword = encoder.encode(&message);
    
    // Perfect channel: convert bits to high-confidence LLRs
    let mut llrs = Vec::with_capacity(codeword.len());
    for i in 0..codeword.len() {
        let bit = codeword.get(i);
        llrs.push(if bit {
            Llr::new(10.0) // Strong belief in '1'
        } else {
            Llr::new(-10.0) // Strong belief in '0'
        });
    }
    
    // Decode
    let result = decoder.decode_iterative(&llrs, 50);
    assert!(result.converged, "Decoding should converge");
    
    let decoded = result.decoded_bits;
    
    // Extract message from systematic position
    let mut recovered_message = BitVec::new();
    for i in 0..code.k() {
        recovered_message.push_bit(decoded.get(i));
    }
    
    assert_eq!(
        recovered_message, message,
        "Roundtrip should recover original message"
    );
}
