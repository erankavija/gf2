//! Regression tests for 5G NR LDPC rate-matched encoding and decoding.
//!
//! These test vectors are generated from our own encoder with the corrected
//! per-i_LS shift tables (3GPP TS 38.212 Tables 5.3.2-2/3). They serve as
//! regression fixtures: any future change that alters the encoder output
//! will be caught immediately.
//!
//! Each test vector verifies:
//! 1. Encoding the message produces the expected codeword (bit-exact)
//! 2. Noiseless decoding of the codeword recovers the message (0 errors)

use gf2_coding::ldpc::nr_5g::Nr5gRateMatchedDecoder;
use gf2_coding::ldpc::QuasiCyclicLdpc;
use gf2_coding::llr::Llr;
use gf2_coding::traits::{BlockEncoder, IterativeSoftDecoder};
use gf2_core::BitVec;

/// Deterministic message: bits set at positions that are multiples of `stride`.
fn deterministic_message(k: usize, stride: usize) -> BitVec {
    let mut msg = BitVec::zeros(k);
    for i in (0..k).step_by(stride) {
        msg.set(i, true);
    }
    msg
}

/// Encode a message, return the codeword as a Vec<u8> of 0/1 values.
fn encode_to_bytes(rm_code: &impl BlockEncoder, msg: &BitVec) -> Vec<u8> {
    let cw = rm_code.encode(msg);
    (0..cw.len()).map(|i| cw.get(i) as u8).collect()
}

/// Verify encode produces expected codeword and noiseless decode recovers message.
fn verify_roundtrip(bg: u8, n: usize, k: usize, stride: usize, expected_cw: &[u8]) {
    let rm_code = QuasiCyclicLdpc::nr_5g_rate_matched(bg, n, k);
    let msg = deterministic_message(k, stride);

    // 1. Encode must match expected codeword bit-exactly
    let our_cw = encode_to_bytes(&rm_code, &msg);
    assert_eq!(
        our_cw, expected_cw,
        "BG{bg} ({n},{k}) stride={stride}: encoder output changed"
    );

    // 2. Noiseless decode must recover the message
    let cw = rm_code.encode(&msg);
    let llrs: Vec<Llr> = (0..n)
        .map(|i| {
            if cw.get(i) {
                Llr::new(-10.0)
            } else {
                Llr::new(10.0)
            }
        })
        .collect();

    let mut decoder = Nr5gRateMatchedDecoder::new(rm_code);
    let result = decoder.decode_iterative(&llrs, 50);
    assert!(
        result.converged,
        "BG{bg} ({n},{k}): noiseless decode did not converge"
    );
    for i in 0..k {
        assert_eq!(
            result.decoded_bits.get(i),
            msg.get(i),
            "BG{bg} ({n},{k}): message bit {i} mismatch after noiseless decode"
        );
    }
}

// ============================================================================
// Regression vectors — generated with per-i_LS shift tables
// ============================================================================
// To regenerate: run `cargo test -p gf2-coding --test nr5g_regression --
//   generate_vectors -- --nocapture` and copy the output.

#[test]
fn test_bg2_256_121_regression() {
    // BG2 (256,121) Z=13 i_LS=6, stride=3
    verify_roundtrip(2, 256, 121, 3, &VECTOR_256_121);
}

#[test]
fn test_bg2_256_49_regression() {
    // BG2 (256,49) Z=6 i_LS=1, stride=3
    verify_roundtrip(2, 256, 49, 3, &VECTOR_256_49);
}

#[test]
fn test_bg2_625_225_regression() {
    // BG2 (625,225) Z=24 i_LS=1, stride=5
    verify_roundtrip(2, 625, 225, 5, &VECTOR_625_225);
}

#[test]
fn test_bg2_1024_441_regression() {
    // BG2 (1024,441) Z=48 i_LS=1, stride=7
    verify_roundtrip(2, 1024, 441, 7, &VECTOR_1024_441);
}

#[test]
fn test_bg1_1024_640_regression() {
    // BG1 (1024,640) Z=30 i_LS=7, stride=7
    verify_roundtrip(1, 1024, 640, 7, &VECTOR_1024_640);
}

#[test]
fn test_bg1_4096_3249_regression() {
    // BG1 (4096,3249) Z=160 i_LS=2, stride=11
    verify_roundtrip(1, 4096, 3249, 11, &VECTOR_4096_3249);
}

// Test vector generation helper — run with --nocapture to print vectors
#[test]
#[ignore]
fn generate_vectors() {
    let configs: &[(u8, usize, usize, usize)] = &[
        (2, 256, 121, 3),
        (2, 256, 49, 3),
        (2, 625, 225, 5),
        (2, 1024, 441, 7),
        (1, 1024, 640, 7),
        (1, 4096, 3249, 11),
    ];

    for &(bg, n, k, stride) in configs {
        let rm_code = QuasiCyclicLdpc::nr_5g_rate_matched(bg, n, k);
        let msg = deterministic_message(k, stride);
        let cw = encode_to_bytes(&rm_code, &msg);
        let name = format!("VECTOR_{}_{}", n, k);
        println!("const {name}: [u8; {n}] = {:?};", cw);
    }
}

// ============================================================================
// Embedded test vectors
// ============================================================================

include!("data/nr5g_regression_vectors.rs");
