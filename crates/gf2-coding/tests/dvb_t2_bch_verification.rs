//! The DVB-T2 outer BCH code and its decoder against the VV001-CR35 TP04
//! BBFRAMEs and TP05 BCH codewords (`@/citation/DvbVerification2010`). Each
//! test returns early when the streams are absent at `$DVB_TEST_VECTORS_PATH`.

mod test_vectors;

use gf2_coding::bch::dvb_t2::{dvb_t2_bch_code, DvbT2BchCode, DvbT2BchDecoder};
use gf2_coding::bch::BchDecodeOutcome;
use gf2_coding::traits::block::{BlockCode, BlockEncoder};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use test_vectors::{test_vectors_available, test_vectors_path, TestVectorSet};

fn stream_code(vectors: &TestVectorSet) -> DvbT2BchCode {
    dvb_t2_bch_code(vectors.config.frame_size.to_bch(), vectors.config.code_rate)
        .expect("a standard DVB-T2 configuration")
}

#[test]
fn test_bch_encoding_tp04_to_tp05() {
    if !test_vectors_available() {
        eprintln!("Test vectors not available at {:?}", test_vectors_path());
        return;
    }

    let vectors = TestVectorSet::load(&test_vectors_path(), "VV001-CR35")
        .expect("Failed to load test vectors");

    let bch = stream_code(&vectors);

    let tp04 = vectors.tp04.as_ref().expect("TP04 not found");
    let tp05 = vectors.tp05.as_ref().expect("TP05 not found");

    let mut successes = 0;
    let mut failures = 0;

    for (block_idx, input_block) in tp04.frame(0).iter().enumerate() {
        let expected_output = &tp05.frame(0)[block_idx];

        let encoded = bch
            .encode(&input_block.data)
            .expect("a K_bch-bit BBFRAME encodes");

        if encoded == expected_output.data {
            successes += 1;
        } else {
            failures += 1;
            eprintln!(
                "Frame 1, Block {}: MISMATCH (expected {} bits, got {})",
                block_idx + 1,
                expected_output.data.len(),
                encoded.len()
            );

            let mut diff_count = 0;
            for i in 0..encoded.len().min(expected_output.data.len()) {
                if encoded.get(i) != expected_output.data.get(i) {
                    diff_count += 1;
                    if diff_count <= 5 {
                        eprintln!(
                            "  Bit {} differs: expected {}, got {}",
                            i,
                            expected_output.data.get(i) as u8,
                            encoded.get(i) as u8
                        );
                    }
                }
            }
            if diff_count > 5 {
                eprintln!("  ... and {} more differences", diff_count - 5);
            }
        }
    }

    println!(
        "BCH Encoding (Frame 1): {} successes, {} failures",
        successes, failures
    );
    assert_eq!(
        failures, 0,
        "BCH encoding validation failed on {} blocks",
        failures
    );
}

#[test]
fn test_bch_decoding_tp05_to_tp04_error_free() {
    if !test_vectors_available() {
        eprintln!("Test vectors not available at {:?}", test_vectors_path());
        return;
    }

    let vectors = TestVectorSet::load(&test_vectors_path(), "VV001-CR35")
        .expect("Failed to load test vectors");

    let bch = stream_code(&vectors);
    let decoder = DvbT2BchDecoder::new(&bch);

    let tp04 = vectors.tp04.as_ref().expect("TP04 not found");
    let tp05 = vectors.tp05.as_ref().expect("TP05 not found");

    let mut successes = 0;
    let mut failures = 0;

    for (block_idx, codeword) in tp05.frame(0).iter().enumerate() {
        let expected_message = &tp04.frame(0)[block_idx];

        let (outcome, decoded) = decoder
            .decode(&codeword.data)
            .expect("an N_bch-bit word decodes");

        if outcome == BchDecodeOutcome::NoErrors && decoded == expected_message.data {
            successes += 1;
        } else {
            failures += 1;
            eprintln!(
                "Frame 1, Block {}: MISMATCH (expected {} bits, got {})",
                block_idx + 1,
                expected_message.data.len(),
                decoded.len()
            );
        }
    }

    println!(
        "BCH Decoding (Frame 1, error-free): {} successes, {} failures",
        successes, failures
    );
    assert_eq!(
        failures, 0,
        "BCH decoding validation failed on {} blocks",
        failures
    );
}

#[test]
#[ignore = "slow: 600 seeded decodes of VV001-CR35 normal-frame words, 16 s on the reference host"]
fn test_bch_error_correction() {
    if !test_vectors_available() {
        eprintln!("Test vectors not available at {:?}", test_vectors_path());
        return;
    }

    let vectors = TestVectorSet::load(&test_vectors_path(), "VV001-CR35")
        .expect("Failed to load test vectors");

    let bch = stream_code(&vectors);
    let decoder = DvbT2BchDecoder::new(&bch);

    let tp04 = vectors.tp04.as_ref().expect("TP04 not found");
    let tp05 = vectors.tp05.as_ref().expect("TP05 not found");

    let mut rng = StdRng::seed_from_u64(0xAE03_BCD0);

    let max_errors = decoder.correction_radius();
    println!("Testing error correction up to t={} errors", max_errors);

    let num_test_blocks = 10.min(tp05.frame(0).len());
    let trials_per_block = 5;

    for num_errors in 1..=max_errors {
        let mut successes = 0;
        let mut failures = 0;

        for block_idx in 0..num_test_blocks {
            let codeword = &tp05.frame(0)[block_idx];
            let expected_message = &tp04.frame(0)[block_idx];

            for _trial in 0..trials_per_block {
                let mut corrupted = codeword.data.clone();
                let mut error_positions = Vec::new();

                while error_positions.len() < num_errors {
                    let pos = rng.gen_range(0..corrupted.len());
                    if !error_positions.contains(&pos) {
                        error_positions.push(pos);
                        corrupted.set(pos, !corrupted.get(pos));
                    }
                }

                let (outcome, decoded) = decoder
                    .decode(&corrupted)
                    .expect("an N_bch-bit word decodes");

                if outcome == (BchDecodeOutcome::Corrected { count: num_errors })
                    && decoded == expected_message.data
                {
                    successes += 1;
                } else {
                    failures += 1;
                }
            }
        }

        let total = num_test_blocks * trials_per_block;
        println!(
            "  {} errors: {}/{} corrected ({:.1}%)",
            num_errors,
            successes,
            total,
            100.0 * successes as f64 / total as f64
        );

        assert_eq!(
            failures, 0,
            "BCH failed to correct {} errors in {} trials",
            num_errors, failures
        );
    }
}

#[test]
fn test_bch_systematic_property() {
    if !test_vectors_available() {
        eprintln!("Test vectors not available at {:?}", test_vectors_path());
        return;
    }

    let vectors = TestVectorSet::load(&test_vectors_path(), "VV001-CR35")
        .expect("Failed to load test vectors");

    let bch = stream_code(&vectors);

    let tp04 = vectors.tp04.as_ref().expect("TP04 not found");
    let tp05 = vectors.tp05.as_ref().expect("TP05 not found");

    let k = bch.k();
    let n = bch.n();
    let parity_bits = n - k;

    println!(
        "Verifying systematic encoding: k={}, n={}, parity={}",
        k, n, parity_bits
    );

    for block_idx in 0..5.min(tp04.frame(0).len()) {
        let message = &tp04.frame(0)[block_idx];
        let codeword = &tp05.frame(0)[block_idx];

        assert_eq!(message.data.len(), k, "Message length mismatch");
        assert_eq!(codeword.data.len(), n, "Codeword length mismatch");

        for i in 0..k {
            assert_eq!(
                codeword.data.get(i),
                message.data.get(i),
                "Systematic property violated at block {}, bit {}",
                block_idx + 1,
                i
            );
        }
    }

    println!("✓ Systematic encoding property verified");
}

#[test]
fn test_bch_encoding_sample() {
    if !test_vectors_available() {
        eprintln!("Test vectors not available at {:?}", test_vectors_path());
        return;
    }

    let vectors = TestVectorSet::load(&test_vectors_path(), "VV001-CR35")
        .expect("Failed to load test vectors");

    let bch = stream_code(&vectors);

    let tp04 = vectors.tp04.as_ref().expect("TP04 not found");
    let tp05 = vectors.tp05.as_ref().expect("TP05 not found");

    println!("Testing sample blocks from each frame...");

    for frame_idx in 0..tp04.num_frames() {
        let frame_tp04 = tp04.frame(frame_idx);
        let frame_tp05 = tp05.frame(frame_idx);

        let test_indices = vec![0, frame_tp04.len() / 2, frame_tp04.len() - 1];

        for &block_idx in &test_indices {
            let message = &frame_tp04[block_idx];
            let expected_cw = &frame_tp05[block_idx];
            let encoded = bch
                .encode(&message.data)
                .expect("a K_bch-bit BBFRAME encodes");

            assert_eq!(
                encoded,
                expected_cw.data,
                "Frame {}, Block {} encoding mismatch",
                frame_idx + 1,
                block_idx + 1
            );
        }

        println!(
            "  Frame {}: {} sample blocks verified",
            frame_idx + 1,
            test_indices.len()
        );
    }

    println!("✓ All sample blocks match");
}
