//! Bit-interleaver tests on the VV001-CR35 reference stream
//! (`@/citation/DvbVerification2010`).
//!
//! VV001-CR35 is a Normal-frame, rate 3/5, 256-QAM stream, a modulation
//! `DvbT2Modulation` does not represent. These tests check the TP06 and TP07a
//! block structure and the rate 3/5 round trip on TP06 data;
//! `dvb_t2_chain_tp07a` asserts the bit-exact TP06 → TP07a mapping on the
//! 16-QAM and 64-QAM streams.

use gf2_coding::ldpc::dvb_t2::bit_interleaver::{
    DvbT2BitInterleaver, DvbT2Modcod, DvbT2Modulation,
};
use gf2_coding::ldpc::dvb_t2::FrameSize;
use gf2_coding::CodeRate;

use gf2_coding::test_support::{dvb_vectors_path, parse_tp_blocks, tp_path};

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// TP06 and TP07a have the same block count and each block contains
/// exactly 64800 bits (Normal FECFRAME size).
#[test]
#[ignore = "slow: reads the VV001-CR35 reference stream from $DVB_TEST_VECTORS_PATH"]
fn test_tp06_to_tp07a_structural_validation() {
    let config_dir = dvb_vectors_path().join("VV001-CR35_CSP");
    if !config_dir.exists() {
        eprintln!("Test vectors not found at {:?}, skipping", config_dir);
        return;
    }

    let tp06_blocks = parse_tp_blocks(&tp_path(&config_dir, "06"));
    let tp07a_blocks = parse_tp_blocks(&tp_path(&config_dir, "07a"));

    assert!(!tp06_blocks.is_empty(), "TP06 parse produced no blocks");
    assert!(!tp07a_blocks.is_empty(), "TP07a parse produced no blocks");
    assert_eq!(
        tp06_blocks.len(),
        tp07a_blocks.len(),
        "TP06 and TP07a must have the same number of blocks"
    );

    // Every block in a Normal FECFRAME is exactly 64800 bits.
    let n_fec = 64800usize;
    for (i, block) in tp06_blocks.iter().enumerate() {
        assert_eq!(
            block.len(),
            n_fec,
            "TP06 block {i} has {} bits, expected {}",
            block.len(),
            n_fec
        );
    }
    for (i, block) in tp07a_blocks.iter().enumerate() {
        assert_eq!(
            block.len(),
            n_fec,
            "TP07a block {i} has {} bits, expected {}",
            block.len(),
            n_fec
        );
    }

    eprintln!(
        "TP06/TP07a structural validation passed: {} blocks × {} bits",
        tp06_blocks.len(),
        n_fec
    );
}

/// Rate 3/5 Normal × QPSK construction smoke test.
///
/// Verifies that `DvbT2BitInterleaver::new` accepts Rate3_5 with each of
/// the three in-scope modulations and produces an interleaver of the
/// correct frame size for the Normal FECFRAME (64800 bits).
#[test]
fn test_rate3_5_normal_construction() {
    for modulation in [
        DvbT2Modulation::Qpsk,
        DvbT2Modulation::Qam16,
        DvbT2Modulation::Qam64,
    ] {
        let modcod = DvbT2Modcod::new(FrameSize::Normal, CodeRate::Rate3_5, modulation);
        let il = DvbT2BitInterleaver::new(modcod);
        assert_eq!(
            il.frame_bits(),
            64800,
            "Rate3_5 Normal {:?} interleaver must cover 64800 bits",
            modulation
        );
        assert_eq!(
            il.num_columns() * il.num_rows(),
            64800,
            "Nc × Nr must equal 64800 for Rate3_5 Normal {:?}",
            modulation
        );
    }
}

/// Rate 3/5 Normal × QPSK roundtrip identity on VV001-CR35 TP06 data.
///
/// Loads the first block of the first frame from VV001-CR35 TP06, applies
/// `interleave` then `deinterleave` with a Rate 3/5 × QPSK interleaver,
/// and asserts the result equals the original input.
///
/// This validates:
/// * The Rate 3/5 permutation tables are self-consistent (forward ×
///   inverse = identity).
/// * The interleaver handles exactly 64800-bit inputs from a real ETSI
///   reference block without panicking or corrupting data.
#[test]
#[ignore = "slow: reads the VV001-CR35 reference stream from $DVB_TEST_VECTORS_PATH"]
fn test_rate3_5_qpsk_roundtrip_on_vv001_cr35_tp06() {
    let config_dir = dvb_vectors_path().join("VV001-CR35_CSP");
    if !config_dir.exists() {
        eprintln!("Test vectors not found at {:?}, skipping", config_dir);
        return;
    }

    let tp06_blocks = parse_tp_blocks(&tp_path(&config_dir, "06"));
    assert!(!tp06_blocks.is_empty(), "TP06 parse produced no blocks");

    // Use Rate 3/5 × QPSK (Nc=2, Nr=32400, twist=[0,0]).
    let modcod = DvbT2Modcod::new(FrameSize::Normal, CodeRate::Rate3_5, DvbT2Modulation::Qpsk);
    let interleaver = DvbT2BitInterleaver::new(modcod);
    assert_eq!(interleaver.frame_bits(), 64800);

    // Test on the first block only (64800 bits — sufficient for roundtrip proof).
    let tp06_block = &tp06_blocks[0];
    assert_eq!(
        tp06_block.len(),
        interleaver.frame_bits(),
        "TP06 block 0 length must equal interleaver frame_bits()"
    );

    let interleaved = interleaver.interleave(tp06_block);
    let recovered = interleaver.deinterleave(&interleaved);
    assert_eq!(
        recovered, *tp06_block,
        "Rate3_5 QPSK roundtrip failed on VV001-CR35 TP06 block 0"
    );

    eprintln!(
        "Rate3_5 Normal QPSK roundtrip passed on {} bits of real ETSI TP06 data",
        interleaver.frame_bits()
    );
}
