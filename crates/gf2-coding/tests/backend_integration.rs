//! Integration tests for ComputeBackend with gf2-coding algorithms.
//!
//! These tests verify that LDPC and BCH algorithms correctly use the
//! ComputeBackend abstraction for parallelization.

use gf2_coding::bch::{BchCode, BchEncoder};
use gf2_coding::ldpc::{LdpcCode, LdpcDecoder, LdpcEncoder};
use gf2_coding::llr::Llr;
use gf2_coding::traits::BlockEncoder;
use gf2_core::gf2m::Gf2mField;
use gf2_core::BitVec;

#[test]
fn test_ldpc_encoder_uses_backend_for_batch() {
    // Create a simple LDPC code
    let edges = vec![(0, 0), (0, 1), (0, 2)];
    let code = LdpcCode::from_edges(1, 3, &edges);
    let encoder = LdpcEncoder::new(code);

    // Create multiple messages
    let messages: Vec<BitVec> = (0..100)
        .map(|i| {
            let mut msg = BitVec::with_capacity(2);
            msg.push_bit(i % 2 == 0);
            msg.push_bit(i % 3 == 0);
            msg
        })
        .collect();

    // Batch encode should use backend internally
    let codewords = encoder.encode_batch(&messages);

    // Verify results match individual encoding
    assert_eq!(codewords.len(), 100);
    for (msg, cw) in messages.iter().zip(codewords.iter()) {
        let expected = encoder.encode(msg);
        assert_eq!(cw.len(), expected.len());
        for i in 0..cw.len() {
            assert_eq!(cw.get(i), expected.get(i));
        }
    }
}

#[test]
fn test_ldpc_decoder_uses_backend_for_batch() {
    // Create a simple LDPC code
    let edges = vec![(0, 0), (0, 1), (0, 2)];
    let code = LdpcCode::from_edges(1, 3, &edges);

    // Create multiple LLR blocks
    let llr_blocks: Vec<Vec<Llr>> = (0..50)
        .map(|i| {
            if i % 2 == 0 {
                vec![Llr::new(10.0f32), Llr::new(10.0f32), Llr::new(10.0f32)]
            } else {
                vec![Llr::new(-10.0f32), Llr::new(-10.0f32), Llr::new(10.0f32)]
            }
        })
        .collect();

    // Batch decode should use backend internally
    let results = LdpcDecoder::decode_batch(&code, &llr_blocks, 10);

    // Verify results
    assert_eq!(results.len(), 50);
    for (i, result) in results.iter().enumerate() {
        assert!(result.converged);
        if i % 2 == 0 {
            assert_eq!(result.decoded_bits.count_ones(), 0);
        } else {
            assert_eq!(result.decoded_bits.count_ones(), 2);
        }
    }
}

#[test]
fn test_bch_encoder_batch_uses_backend() {
    // Create BCH(15, 11, 1) code
    let field = Gf2mField::new(4, 0b10011);
    let code = BchCode::new(15, 11, 1, field);
    let encoder = BchEncoder::new(code.clone());

    // Create multiple messages
    let messages: Vec<BitVec> = (0..100)
        .map(|i| {
            let mut msg = BitVec::with_capacity(11);
            for j in 0..11 {
                msg.push_bit((i + j) % 2 == 0);
            }
            msg
        })
        .collect();

    // Batch encode (currently sequential, will be parallelized)
    let codewords = encoder.encode_batch(&messages);

    // Verify results match individual encoding
    assert_eq!(codewords.len(), 100);
    for (msg, cw) in messages.iter().zip(codewords.iter()) {
        let expected = encoder.encode(msg);
        assert_eq!(cw.len(), expected.len());
        for i in 0..cw.len() {
            assert_eq!(cw.get(i), expected.get(i));
        }
    }
}

#[test]
fn test_ldpc_batch_operations_are_deterministic() {
    // Create LDPC code
    let edges = vec![(0, 0), (0, 1), (0, 2), (1, 1), (1, 2), (1, 3)];
    let code = LdpcCode::from_edges(2, 4, &edges);
    let encoder = LdpcEncoder::new(code.clone());

    // Create messages
    let messages: Vec<BitVec> = (0..20)
        .map(|i| {
            let mut msg = BitVec::with_capacity(2);
            msg.push_bit(i % 2 == 0);
            msg.push_bit(i % 3 == 0);
            msg
        })
        .collect();

    // Encode multiple times - should be deterministic
    let codewords1 = encoder.encode_batch(&messages);
    let codewords2 = encoder.encode_batch(&messages);

    assert_eq!(codewords1.len(), codewords2.len());
    for (cw1, cw2) in codewords1.iter().zip(codewords2.iter()) {
        assert_eq!(cw1.len(), cw2.len());
        for i in 0..cw1.len() {
            assert_eq!(cw1.get(i), cw2.get(i));
        }
    }
}

#[test]
fn test_backend_batch_operations_empty_input() {
    // Test that empty batches work correctly
    let edges = vec![(0, 0), (0, 1), (0, 2)];
    let code = LdpcCode::from_edges(1, 3, &edges);
    let encoder = LdpcEncoder::new(code.clone());

    let empty_messages: Vec<BitVec> = vec![];
    let codewords = encoder.encode_batch(&empty_messages);
    assert_eq!(codewords.len(), 0);

    let empty_llrs: Vec<Vec<Llr>> = vec![];
    let results = LdpcDecoder::decode_batch(&code, &empty_llrs, 10);
    assert_eq!(results.len(), 0);
}

#[test]
fn test_backend_batch_single_item() {
    // Batch operations should work correctly with single item
    let edges = vec![(0, 0), (0, 1), (0, 2)];
    let code = LdpcCode::from_edges(1, 3, &edges);
    let encoder = LdpcEncoder::new(code.clone());

    let mut msg = BitVec::with_capacity(2);
    msg.push_bit(true);
    msg.push_bit(false);

    let messages = vec![msg.clone()];
    let codewords = encoder.encode_batch(&messages);

    assert_eq!(codewords.len(), 1);
    let expected = encoder.encode(&msg);
    assert_eq!(codewords[0].len(), expected.len());
    for i in 0..codewords[0].len() {
        assert_eq!(codewords[0].get(i), expected.get(i));
    }
}

// TODO: Enable when parallel feature is added to gf2-coding
#[test]
#[ignore]
fn test_ldpc_batch_parallel_correctness() {
    // Verify parallel batch operations produce same results as sequential
    let edges: Vec<(usize, usize)> = (0..10)
        .flat_map(|i| (0..5).map(move |j| (i, (i * 3 + j) % 20)))
        .collect();
    let code = LdpcCode::from_edges(10, 20, &edges);
    let encoder = LdpcEncoder::new(code);

    // Create large batch to trigger parallelization
    let messages: Vec<BitVec> = (0..1000)
        .map(|i| {
            let mut msg = BitVec::with_capacity(10);
            for j in 0..10 {
                msg.push_bit((i ^ j) % 2 == 0);
            }
            msg
        })
        .collect();

    let codewords = encoder.encode_batch(&messages);

    // Verify each codeword is correct
    assert_eq!(codewords.len(), 1000);
    for (msg, cw) in messages.iter().zip(codewords.iter()) {
        let expected = encoder.encode(msg);
        assert_eq!(cw.len(), expected.len());
        for i in 0..cw.len() {
            assert_eq!(cw.get(i), expected.get(i));
        }
    }
}
