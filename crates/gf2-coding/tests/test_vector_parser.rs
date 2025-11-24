//! Test vector parser integration tests
//!
//! These tests verify the test vector parsing infrastructure works correctly.
//! Tests marked with #[ignore] require external DVB test vectors.

mod test_vectors;

use test_vectors::{test_vectors_available, test_vectors_path, TestVectorSet};

#[test]
fn test_parser_module_available() {
    // Basic smoke test - module should compile and be accessible
    // If this test runs, the module compiled successfully
}

#[test]
#[ignore]
fn test_parse_vv001_cr35_all_test_points() {
    if !test_vectors_available() {
        eprintln!("Test vectors not available at {:?}", test_vectors_path());
        eprintln!("Set DVB_TEST_VECTORS_PATH environment variable to the test vectors directory");
        return;
    }

    let vectors = TestVectorSet::load(&test_vectors_path(), "VV001-CR35")
        .expect("Failed to load VV001-CR35 test vectors");

    assert_eq!(vectors.config.name, "VV001-CR35");
    assert!(vectors.tp04.is_some(), "TP04 should be present");
    assert!(vectors.tp05.is_some(), "TP05 should be present");
    assert!(vectors.tp06.is_some(), "TP06 should be present");
    assert!(vectors.tp07a.is_some(), "TP07a should be present");

    println!("✓ Successfully loaded all test points for VV001-CR35");
}

#[test]
#[ignore]
fn test_tp04_structure() {
    if !test_vectors_available() {
        eprintln!("Test vectors not available at {:?}", test_vectors_path());
        return;
    }

    let vectors = TestVectorSet::load(&test_vectors_path(), "VV001-CR35")
        .expect("Failed to load test vectors");

    let tp04 = vectors.tp04.expect("TP04 should be present");

    // Verify structure
    assert!(tp04.num_frames() > 0, "Should have at least one frame");
    println!("TP04: {} frames", tp04.num_frames());

    let frame0 = tp04.frame(0);
    assert!(!frame0.is_empty(), "Frame 0 should have blocks");
    println!("Frame 0: {} blocks", frame0.len());

    if let Some(first_block) = frame0.first() {
        println!("First block:");
        println!("  Frame: {}", first_block.frame_number);
        println!(
            "  Block: {} of {}",
            first_block.block_number, first_block.total_blocks
        );
        println!("  Bits: {}", first_block.data.len());

        assert_eq!(first_block.frame_number, 1);
        assert_eq!(first_block.block_number, 1);
        assert!(first_block.total_blocks > 0);
        assert!(!first_block.data.is_empty());

        // Verify block count matches
        assert_eq!(
            frame0.len(),
            first_block.total_blocks,
            "Frame should contain declared number of blocks"
        );
    }

    println!("✓ TP04 structure validated");
}

#[test]
#[ignore]
fn test_all_test_points_consistent_structure() {
    if !test_vectors_available() {
        eprintln!("Test vectors not available at {:?}", test_vectors_path());
        return;
    }

    let vectors = TestVectorSet::load(&test_vectors_path(), "VV001-CR35")
        .expect("Failed to load test vectors");

    let tp04 = vectors.tp04.as_ref().expect("TP04 should be present");
    let tp05 = vectors.tp05.as_ref().expect("TP05 should be present");
    let tp06 = vectors.tp06.as_ref().expect("TP06 should be present");
    let tp07a = vectors.tp07a.as_ref().expect("TP07a should be present");

    println!("Checking consistency across test points...");
    println!("TP04:  {} frames", tp04.num_frames());
    println!("TP05:  {} frames", tp05.num_frames());
    println!("TP06:  {} frames", tp06.num_frames());
    println!("TP07a: {} frames", tp07a.num_frames());

    // All test points should have same number of frames
    assert_eq!(
        tp04.num_frames(),
        tp05.num_frames(),
        "TP04 and TP05 frame count mismatch"
    );
    assert_eq!(
        tp04.num_frames(),
        tp06.num_frames(),
        "TP04 and TP06 frame count mismatch"
    );
    assert_eq!(
        tp04.num_frames(),
        tp07a.num_frames(),
        "TP04 and TP07a frame count mismatch"
    );

    // Check first frame block counts
    let frame0_blocks_tp04 = tp04.frame(0).len();
    let frame0_blocks_tp05 = tp05.frame(0).len();
    let frame0_blocks_tp06 = tp06.frame(0).len();
    let frame0_blocks_tp07a = tp07a.frame(0).len();

    println!("Frame 0 blocks:");
    println!("  TP04:  {}", frame0_blocks_tp04);
    println!("  TP05:  {}", frame0_blocks_tp05);
    println!("  TP06:  {}", frame0_blocks_tp06);
    println!("  TP07a: {}", frame0_blocks_tp07a);

    assert_eq!(
        frame0_blocks_tp04, frame0_blocks_tp05,
        "TP04 and TP05 block count mismatch"
    );
    assert_eq!(
        frame0_blocks_tp04, frame0_blocks_tp06,
        "TP04 and TP06 block count mismatch"
    );
    assert_eq!(
        frame0_blocks_tp04, frame0_blocks_tp07a,
        "TP04 and TP07a block count mismatch"
    );

    println!("✓ All test points have consistent structure");
}

#[test]
#[ignore]
fn test_bit_lengths_match_encoding_stages() {
    if !test_vectors_available() {
        eprintln!("Test vectors not available at {:?}", test_vectors_path());
        return;
    }

    let vectors = TestVectorSet::load(&test_vectors_path(), "VV001-CR35")
        .expect("Failed to load test vectors");

    let tp04 = vectors.tp04.as_ref().expect("TP04 should be present");
    let tp05 = vectors.tp05.as_ref().expect("TP05 should be present");
    let tp06 = vectors.tp06.as_ref().expect("TP06 should be present");

    let block04 = &tp04.frame(0)[0];
    let block05 = &tp05.frame(0)[0];
    let block06 = &tp06.frame(0)[0];

    println!("Block bit lengths:");
    println!("  TP04 (BCH input):  {} bits", block04.data.len());
    println!("  TP05 (BCH output): {} bits", block05.data.len());
    println!("  TP06 (LDPC output): {} bits", block06.data.len());

    // TP05 should be longer than TP04 (BCH adds parity bits)
    assert!(
        block05.data.len() > block04.data.len(),
        "BCH encoding should increase bit length"
    );

    // TP06 should be longer than TP05 (LDPC adds parity bits)
    assert!(
        block06.data.len() > block05.data.len(),
        "LDPC encoding should increase bit length"
    );

    println!("✓ Bit lengths follow expected encoding progression");
}
