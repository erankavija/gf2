//! Regression tests for 5G NR LDPC rate-matched encoding and decoding. The
//! vectors are this crate's own encoder output and pin it against change;
//! `nr5g_external_vectors.rs` holds the external conformance evidence.

use gf2_coding::ldpc::nr_5g::Nr5gRateMatchedDecoder;
use gf2_coding::ldpc::QuasiCyclicLdpc;
use gf2_coding::llr::Llr;
use gf2_coding::traits::{BlockEncoder, IterativeSoftDecoder};
use gf2_core::BitVec;

fn deterministic_message(k: usize, stride: usize) -> BitVec {
    let mut msg = BitVec::zeros(k);
    for i in (0..k).step_by(stride) {
        msg.set(i, true);
    }
    msg
}

fn encode_to_bytes(rm_code: &impl BlockEncoder, msg: &BitVec) -> Vec<u8> {
    let cw = rm_code.encode(msg);
    (0..cw.len()).map(|i| cw.get(i) as u8).collect()
}

fn verify_roundtrip(bg: u8, n: usize, k: usize, stride: usize, expected_cw: &[u8]) {
    let rm_code = QuasiCyclicLdpc::nr_5g_rate_matched(bg, n, k);
    let msg = deterministic_message(k, stride);

    let our_cw = encode_to_bytes(&rm_code, &msg);
    assert_eq!(
        our_cw, expected_cw,
        "BG{bg} ({n},{k}) stride={stride}: encoder output changed"
    );

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

#[test]
fn test_bg2_256_121_regression() {
    verify_roundtrip(2, 256, 121, 3, &VECTOR_256_121);
}

#[test]
fn test_bg2_256_49_regression() {
    verify_roundtrip(2, 256, 49, 3, &VECTOR_256_49);
}

#[test]
fn test_bg2_625_225_regression() {
    verify_roundtrip(2, 625, 225, 5, &VECTOR_625_225);
}

#[test]
fn test_bg2_1024_441_regression() {
    verify_roundtrip(2, 1024, 441, 7, &VECTOR_1024_441);
}

#[test]
fn test_bg1_1024_640_regression() {
    verify_roundtrip(1, 1024, 640, 7, &VECTOR_1024_640);
}

#[test]
fn test_bg1_4096_3249_regression() {
    verify_roundtrip(1, 4096, 3249, 11, &VECTOR_4096_3249);
}

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

include!("data/nr5g_regression_vectors.rs");
