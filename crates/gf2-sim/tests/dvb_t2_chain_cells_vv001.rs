//! Cells of the [`dvb_t2_bicm_stages`] forward chain against the external
//! VV001-CR35 reference stream (`@/citation/DvbVerification2010`): its first
//! TP04 BBFRAME (normal frame, rate 3/5, 256-QAM, `@/citation/Etsi2015`)
//! through encode, bit interleave and cell mapping gives the TP08a cells of
//! that FEC block.
//!
//! 256-QAM is the only constellation order verified here. 16-QAM and 64-QAM
//! are unverified against external vectors: no stream of those orders is
//! available to this suite.

use gf2_coding::ldpc::dvb_t2::DvbT2Modulation;
use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig};
use gf2_coding::modem::DemapMethod;
use gf2_coding::test_support::{
    dvb_vectors_path, parse_tp_block_prefix, parse_tp_complex_block_prefix, tp_path,
};
use gf2_coding::CodeRate;

use gf2_sim::batch::{BitPackedBatch, SymbolBatch};
use gf2_sim::stage::{AnyScratch, TypedBatch};
use gf2_sim::stages::{dvb_t2_bicm_stages, DEFAULT_DEMAP_NOISE_VAR};

/// The reference streams print seven significant digits.
const CELL_TOL: f64 = 2e-6;

#[test]
fn test_forward_chain_reproduces_vv001_tp08a_cells() {
    let dir = dvb_vectors_path().join("VV001-CR35_CSP");
    let (tp04, tp08a) = (tp_path(&dir, "04"), tp_path(&dir, "08a"));
    if !(tp04.is_file() && tp08a.is_file()) {
        eprintln!(
            "SKIP test_forward_chain_reproduces_vv001_tp08a_cells: VV001-CR35 TP04 and \
             TP08a streams absent under DVB_TEST_VECTORS_PATH"
        );
        return;
    }
    let bbframe = parse_tp_block_prefix(&tp04, 1).remove(0);
    let expected = parse_tp_complex_block_prefix(&tp08a, 1).remove(0);

    let stages = dvb_t2_bicm_stages(
        CodeRate::Rate3_5,
        DvbT2Modulation::Qam256,
        DecoderConfig::new(DecoderAlgorithm::SumProduct, true),
        DemapMethod::ExactLogMap,
        DEFAULT_DEMAP_NOISE_VAR,
    );
    assert_eq!(
        bbframe.len(),
        stages.codec.k_bch(),
        "TP04 carries one BBFRAME"
    );

    let input: Box<dyn TypedBatch> = Box::new(BitPackedBatch::new(vec![bbframe]));
    let output = stages.forward.iter().fold(input, |batch, stage| {
        let mut scratch: Box<dyn AnyScratch> = stage.default_scratch();
        stage
            .process_any(batch.as_ref(), scratch.as_mut())
            .expect("forward stage")
    });
    let cells = output
        .as_any()
        .downcast_ref::<SymbolBatch>()
        .expect("the forward chain ends in cells");

    assert_eq!(cells.i[0].len(), expected.len(), "one cell per TP08a line");
    let mismatched = expected
        .iter()
        .zip(cells.i[0].iter().zip(&cells.q[0]))
        .filter(|(&(re, im), (&i, &q))| {
            (f64::from(i) - re).abs() > CELL_TOL || (f64::from(q) - im).abs() > CELL_TOL
        })
        .count();
    assert_eq!(
        mismatched,
        0,
        "chain cells off TP08a out of {}",
        expected.len()
    );
}
