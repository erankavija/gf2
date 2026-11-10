//! Bit interleaver, cell-word demultiplexer and cell mapping against the
//! first FEC block of the VV001-CR35 reference stream
//! (`@/citation/DvbVerification2010`), a normal-frame, rate 3/5, 256-QAM
//! stream of `@/citation/Etsi2015`: TP06 is the FECFRAME, TP07a the
//! bit-interleaved FECFRAME, TP07 the cell words and TP08a the unrotated
//! cells.
//!
//! The 16-QAM and 64-QAM rows of the demultiplexer and their cell mapping are
//! unverified against external vectors: no stream of those orders is
//! available to this suite. Their mapping is checked against Tables 14(d) to
//! 14(g) by the `cell_mapper` unit tests.

mod common;

use gf2_coding::ldpc::dvb_t2::{
    DvbT2BitInterleaver, DvbT2CellMapper, DvbT2Modcod, DvbT2Modulation, FrameSize,
};
use gf2_coding::test_support::{parse_tp_block_prefix, parse_tp_complex_block_prefix, tp_path};
use gf2_coding::CodeRate;
use gf2_core::BitVec;

/// The reference streams print seven significant digits.
const CELL_TOL: f64 = 2e-6;

const VV001: DvbT2Modcod = DvbT2Modcod {
    frame_size: FrameSize::Normal,
    code_rate: CodeRate::Rate3_5,
    modulation: DvbT2Modulation::Qam256,
};

/// First blocks of TP06, TP07a, TP07 and TP08a.
struct Block {
    tp06: BitVec,
    tp07a: BitVec,
    tp07: BitVec,
    tp08a: Vec<(f64, f64)>,
}

/// The first FEC block, or `None` after a skip notice when the stream is
/// absent.
fn first_block(test: &str) -> Option<Block> {
    let dir = common::dvb_vectors_dir().map(|d| d.join("VV001-CR35_CSP"));
    let paths = dir
        .as_ref()
        .map(|d| ["06", "07a", "07", "08a"].map(|tp| tp_path(d, tp)));
    let Some(paths) = paths.filter(|p| p.iter().all(|p| p.is_file())) else {
        common::skip(
            test,
            "VV001-CR35 TP06, TP07a, TP07 and TP08a streams absent under DVB_TEST_VECTORS_PATH",
        );
        return None;
    };
    let bits = |i: usize| parse_tp_block_prefix(&paths[i], 1).remove(0);
    Some(Block {
        tp06: bits(0),
        tp07a: bits(1),
        tp07: bits(2),
        tp08a: parse_tp_complex_block_prefix(&paths[3], 1).remove(0),
    })
}

/// Number of cells off their TP08a value.
fn mismatched_cells(i: &[f32], q: &[f32], cells: &[(f64, f64)]) -> usize {
    assert_eq!(i.len(), cells.len());
    cells
        .iter()
        .zip(i.iter().zip(q))
        .filter(|(&(re, im), (&i, &q))| {
            (f64::from(i) - re).abs() > CELL_TOL || (f64::from(q) - im).abs() > CELL_TOL
        })
        .count()
}

#[test]
fn test_interleaver_maps_vv001_tp06_to_tp07a() {
    let Some(block) = first_block("test_interleaver_maps_vv001_tp06_to_tp07a") else {
        return;
    };
    let interleaver = DvbT2BitInterleaver::new(VV001);
    assert_eq!(interleaver.interleave(&block.tp06), block.tp07a);
    assert_eq!(interleaver.deinterleave(&block.tp07a), block.tp06);
}

#[test]
fn test_demux_maps_vv001_tp07a_to_tp07_cell_words() {
    let Some(block) = first_block("test_demux_maps_vv001_tp07a_to_tp07_cell_words") else {
        return;
    };
    let mapper = DvbT2CellMapper::new(VV001);
    assert_eq!(mapper.demux().demux(&block.tp07a), block.tp07);
    assert_eq!(mapper.demux().mux(&block.tp07), block.tp07a);
}

#[test]
fn test_cell_mapper_maps_vv001_to_tp08a_cells() {
    let Some(block) = first_block("test_cell_mapper_maps_vv001_to_tp08a_cells") else {
        return;
    };
    let mapper = DvbT2CellMapper::new(VV001);
    let cells = mapper.num_cells();
    assert_eq!(block.tp08a.len(), cells, "TP08a carries one cell per word");
    let (mut i, mut q) = (vec![0.0_f32; cells], vec![0.0_f32; cells]);

    mapper.map(&block.tp07a, &mut i, &mut q);
    assert_eq!(mismatched_cells(&i, &q, &block.tp08a), 0, "map(TP07a)");

    mapper.map_cell_words(&block.tp07, &mut i, &mut q);
    assert_eq!(
        mismatched_cells(&i, &q, &block.tp08a),
        0,
        "map_cell_words(TP07)"
    );
}
