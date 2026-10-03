//! DVB-T2 LDPC encoding of the first 10 VV001-CR35 TP05 blocks against TP06
//! (`@/citation/DvbVerification2010`).

mod test_vectors;

use gf2_coding::ldpc::encoding::EncodingCache;
use gf2_coding::ldpc::{LdpcCode, LdpcEncoder};
use gf2_coding::traits::BlockEncoder;
use gf2_coding::CodeRate;
use std::path::PathBuf;

fn try_load_cache() -> Option<EncodingCache> {
    let cache_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/ldpc/dvb_t2");
    if cache_dir.exists() {
        EncodingCache::from_directory(&cache_dir).ok()
    } else {
        None
    }
}

fn create_encoder(code: LdpcCode, cache: Option<&EncodingCache>) -> LdpcEncoder {
    match cache {
        Some(c) => LdpcEncoder::with_cache(code, c),
        None => {
            eprintln!("Creating encoder without cache (this may take time)...");
            LdpcEncoder::new(code)
        }
    }
}

#[test]
#[ignore = "slow: reads the VV001-CR35 reference stream from $DVB_TEST_VECTORS_PATH"]
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
    let cache = try_load_cache();
    let code = LdpcCode::dvb_t2_normal(CodeRate::Rate3_5);
    let encoder = create_encoder(code, cache.as_ref());

    let mut successes = 0;
    let mut failures = 0;

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
