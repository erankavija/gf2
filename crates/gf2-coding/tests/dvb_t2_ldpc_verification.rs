//! DVB-T2 LDPC encoding verification using reference test vectors.

mod test_vectors;

use gf2_coding::ldpc::{LdpcCode, LdpcEncoder};
use gf2_coding::traits::BlockEncoder;
use gf2_coding::CodeRate;
use std::env;
use std::path::PathBuf;

#[test]
#[ignore]
fn test_ldpc_encoding_tp05_to_tp06() {
    if !test_vectors::test_vectors_available() {
        return;
    }
    
    let base_path = test_vectors::test_vectors_path();

    let vectors = test_vectors::TestVectorSet::load(&base_path, "VV001-CR35")
        .expect("Failed to load test vectors");
    let tp05 = vectors.tp05.as_ref().expect("TP05 not found");
    let tp06 = vectors.tp06.as_ref().expect("TP06 not found");

    // DVB-T2 Normal, Rate 3/5
    let code = LdpcCode::dvb_t2_normal(CodeRate::Rate3_5);
    let encoder = LdpcEncoder::new(code);

    let mut successes = 0;
    let mut failures = 0;

    // Test first 10 blocks only (encoding is slow with iterative algorithm)
    let test_blocks = 10.min(tp05.frame(0).len());
    eprintln!("Testing LDPC encoding on {} blocks...", test_blocks);
    
    for (block_idx, input_block) in tp05.frame(0).iter().take(test_blocks).enumerate() {
        let expected_output = &tp06.frame(0)[block_idx];

        let encoded = encoder.encode(&input_block.data);

        if encoded == expected_output.data {
            successes += 1;
        } else {
            failures += 1;
            eprintln!(
                "Block {}: MISMATCH (expected {} bits, got {})",
                block_idx + 1,
                expected_output.data.len(),
                encoded.len()
            );
        }
    }

    assert_eq!(
        failures,
        0,
        "LDPC encoding validation: {}/{} blocks match",
        successes,
        successes + failures
    );
}
