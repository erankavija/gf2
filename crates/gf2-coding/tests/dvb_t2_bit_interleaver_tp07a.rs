//! End-to-end TP07a bit-interleaver integration tests against VV001-CR35.
//!
//! # VV001-CR35 configuration
//!
//! VV001-CR35 is the ETSI DVB-T2 conformance vector for Normal frame,
//! Rate 3/5.  Its full modulation chain uses **256-QAM** (confirmed by
//! TP08 complex symbol count: 8100 complex samples/block × 8 bits/symbol
//! = 64800 bits/block = the Normal FECFRAME size).
//!
//! # Scope note
//!
//! `DvbT2BitInterleaver` implements the bit interleaver stage (ETSI EN
//! 302 755 v1.4.1 §6.1.3) for QPSK, 16-QAM, and 64-QAM.  The VV001-CR35
//! TP07a file is produced by a combined pipeline of §6.1.3 (bit
//! interleaver), §6.1.4 (cell word demux), and §6.1.5 (cell interleaver)
//! with 256-QAM parameters.  Reproducing TP07a bit-exactly from TP06
//! therefore requires 256-QAM support plus the cell-word stages, which are
//! out of scope for this module and will be addressed in issue 4cdaf1c5.
//!
//! # What these tests validate
//!
//! 1. **Structural integrity** — TP06 and TP07a have the same block count
//!    and each block has exactly 64800 bits, as required by the Normal
//!    FECFRAME specification.
//! 2. **Rate 3/5 Normal construction** — `DvbT2BitInterleaver::new` with
//!    `CodeRate::Rate3_5` and `FrameSize::Normal` does not panic for all
//!    three modulations (QPSK, 16-QAM, 64-QAM) and produces an interleaver
//!    of the correct frame size.
//! 3. **Rate 3/5 roundtrip identity** — `deinterleave(interleave(tp06))`
//!    equals `tp06` for the first block of the first frame, using the
//!    Rate 3/5 × QPSK interleaver.  This confirms the permutation tables
//!    are consistent (forward × inverse = identity) for Rate 3/5 Normal.

use std::path::PathBuf;

use gf2_coding::ldpc::dvb_t2::bit_interleaver::{
    DvbT2BitInterleaver, DvbT2Modcod, DvbT2Modulation,
};
use gf2_coding::ldpc::dvb_t2::FrameSize;
use gf2_coding::CodeRate;

// CSP test-point file parser + path builder are factored into the
// crate's shared `test_support` module so the inline test in
// `crates/gf2-coding/src/ldpc/dvb_t2/concat.rs` (TP04 → TP06 vector
// check) and this integration test share one definition. Enabled by
// the `test-support` feature on the self-referenced dev-dependency.
use gf2_coding::test_support::{parse_tp_blocks, tp_path};

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Structural validation: TP06 and TP07a have the same block count and
/// each block contains exactly 64800 bits (Normal FECFRAME size).
///
/// This test does **not** validate the forward TP06 → TP07a mapping,
/// because VV001-CR35 uses 256-QAM + cell interleaving stages (§6.1.4/
/// §6.1.5) that are outside the scope of `DvbT2BitInterleaver`.  Full
/// forward validation is deferred to issue 4cdaf1c5.
#[test]
#[ignore = "external: requires DVB-T2 test vectors at $DVB_TEST_VECTORS_PATH or ~/dvb_test_vectors"]
fn test_tp06_to_tp07a_structural_validation() {
    let base_path = std::env::var("DVB_TEST_VECTORS_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(std::env::var("HOME").expect("HOME not set")).join("dvb_test_vectors")
        });
    let config_dir = base_path.join("VV001-CR35_CSP");
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
///
/// Note: this test does **not** validate the forward TP06 → TP07a mapping.
/// VV001-CR35 uses 256-QAM with cell interleaving stages beyond §6.1.3.
/// A QPSK interleaver is used here because it is in scope; the TP06 data
/// comes from the real ETSI reference vector irrespective of the final
/// modulation.
#[test]
#[ignore = "external: requires DVB-T2 test vectors at $DVB_TEST_VECTORS_PATH or ~/dvb_test_vectors"]
fn test_rate3_5_qpsk_roundtrip_on_vv001_cr35_tp06() {
    let base_path = std::env::var("DVB_TEST_VECTORS_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(std::env::var("HOME").expect("HOME not set")).join("dvb_test_vectors")
        });
    let config_dir = base_path.join("VV001-CR35_CSP");
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
