//! LDPC systematic encoding: codeword validity and systematic form.

use gf2_coding::ldpc::LdpcCode;
use gf2_core::BitVec;

fn simple_ldpc_7_4() -> LdpcCode {
    let edges = vec![
        (0, 0),
        (0, 2),
        (0, 3),
        (0, 4),
        (1, 1),
        (1, 3),
        (1, 5),
        (2, 2),
        (2, 3),
        (2, 6),
    ];
    LdpcCode::from_edges(3, 7, &edges)
}

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

#[test]
fn test_ru_preprocess_simple_ldpc() {
    use gf2_coding::ldpc::LdpcEncoder;
    use gf2_coding::traits::BlockEncoder;

    let code = simple_ldpc_7_4();

    let encoder = LdpcEncoder::new(code.clone());

    assert_eq!(encoder.n(), 7, "n should be 7");
    assert_eq!(encoder.k(), 4, "k should be 4 (7-3)");
}

#[test]
fn test_ru_encoding_produces_valid_codewords() {
    use gf2_coding::ldpc::LdpcEncoder;
    use gf2_coding::traits::BlockEncoder;

    let code = simple_ldpc_7_4();
    let encoder = LdpcEncoder::new(code.clone());

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

    let mut message = BitVec::new();
    message.push_bit(false);
    message.push_bit(true);
    message.push_bit(false);
    message.push_bit(true);

    let codeword = encoder.encode(&message);

    for i in 0..4 {
        assert_eq!(
            codeword.get(i),
            message.get(i),
            "Bit {} should match message bit",
            i
        );
    }
}

#[test]
fn test_dvb_t2_preprocessing_all_configs() {
    use gf2_coding::bch::CodeRate;
    use gf2_coding::ldpc::{encoding::EncodingCache, LdpcEncoder};
    use gf2_coding::traits::BlockEncoder;

    let cache = EncodingCache::new();

    let rates = [
        CodeRate::Rate1_2,
        CodeRate::Rate3_5,
        CodeRate::Rate2_3,
        CodeRate::Rate3_4,
        CodeRate::Rate4_5,
        CodeRate::Rate5_6,
    ];

    for rate in &rates {
        let code_short = LdpcCode::dvb_t2_short(*rate);
        let encoder = LdpcEncoder::with_cache(code_short.clone(), &cache);
        assert!(encoder.is_ira(), "DVB-T2 short should use IRA encoder");

        let msg = BitVec::zeros(code_short.k());
        let cw = encoder.encode(&msg);
        assert!(code_short.is_valid_codeword(&cw), "H·c must be zero");
    }

    assert_eq!(
        cache.stats().entries,
        0,
        "IRA encoders do not populate the RREF cache"
    );
}

#[test]
fn test_ldpc_encoder_creation() {
    use gf2_coding::bch::CodeRate;
    use gf2_coding::ldpc::{encoding::EncodingCache, LdpcEncoder};
    use gf2_coding::traits::BlockEncoder;

    let cache = EncodingCache::new();
    let code = LdpcCode::dvb_t2_short(CodeRate::Rate1_2);
    let encoder = LdpcEncoder::with_cache(code.clone(), &cache);

    let mut message = BitVec::new();
    for _ in 0..code.k() {
        message.push_bit(rand::random());
    }

    let codeword = encoder.encode(&message);

    assert_eq!(codeword.len(), code.n(), "Codeword length should match n");

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
fn test_dvb_t2_encoded_codewords_valid() {
    use gf2_coding::bch::CodeRate;
    use gf2_coding::ldpc::{encoding::EncodingCache, LdpcEncoder};
    use gf2_coding::traits::BlockEncoder;

    let cache = EncodingCache::new();
    let code = LdpcCode::dvb_t2_short(CodeRate::Rate1_2);
    let encoder = LdpcEncoder::with_cache(code.clone(), &cache);

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
#[ignore = "slow: DVB-T2 Short encode + 50-iteration BP decode"]
fn test_ldpc_encode_decode_roundtrip_simple() {
    use gf2_coding::bch::CodeRate;
    use gf2_coding::ldpc::{LdpcDecoder, LdpcEncoder};
    use gf2_coding::llr::Llr;
    use gf2_coding::traits::{BlockEncoder, IterativeSoftDecoder};

    let code = LdpcCode::dvb_t2_short(CodeRate::Rate1_2);
    let encoder = LdpcEncoder::new(code.clone());
    let mut decoder = LdpcDecoder::new(code.clone());

    let mut message = BitVec::new();
    for _ in 0..code.k() {
        message.push_bit(rand::random());
    }

    let codeword = encoder.encode(&message);

    // L = log(P(0)/P(1)): a set bit maps to a negative LLR.
    let mut llrs = Vec::with_capacity(codeword.len());
    for i in 0..codeword.len() {
        let bit = codeword.get(i);
        llrs.push(if bit {
            Llr::new(-10.0f32)
        } else {
            Llr::new(10.0f32)
        });
    }

    let result = decoder.decode_iterative(&llrs, 50);
    assert!(result.converged, "Decoding should converge");

    let decoded = result.decoded_bits;

    let mut recovered_message = BitVec::new();
    for i in 0..code.k() {
        recovered_message.push_bit(decoded.get(i));
    }

    assert_eq!(
        recovered_message, message,
        "Roundtrip should recover original message"
    );
}
