//! [`ModemSpec::gray_square_qam`] at order 256 against the cell mapping of
//! the VV001-CR35 reference stream (`@/citation/DvbVerification2010`), a
//! 256-QAM stream of `@/citation/Etsi2015`: TP07 carries one 8-bit cell word
//! per unrotated constellation point of TP08a.

mod common;

use gf2_coding::modem::{BatchMapper, ModemSpec};
use gf2_coding::test_support::{parse_tp_block_prefix, parse_tp_complex_block_prefix, tp_path};

const BITS_PER_CELL: usize = 8;

/// The reference streams print seven significant digits.
const CELL_TOL: f64 = 2e-6;

/// Cell-word bits and unrotated `(re, im)` cells of one FEC block.
type Block = (Vec<bool>, Vec<(f64, f64)>);

/// The first FEC block, or `None` when the stream is absent.
fn first_block() -> Option<Block> {
    let config_dir = common::dvb_vectors_dir()?.join("VV001-CR35_CSP");
    let (tp07, tp08a) = (tp_path(&config_dir, "07"), tp_path(&config_dir, "08a"));
    if !(tp07.is_file() && tp08a.is_file()) {
        return None;
    }
    let words = parse_tp_block_prefix(&tp07, 1).remove(0);
    let cells = parse_tp_complex_block_prefix(&tp08a, 1).remove(0);
    assert_eq!(
        words.len(),
        cells.len() * BITS_PER_CELL,
        "TP07 carries {BITS_PER_CELL} bits per TP08a cell"
    );
    Some(((0..words.len()).map(|i| words.get(i)).collect(), cells))
}

/// Number of cells the order-256 preset maps away from the reference cell.
fn mismatched_cells(labels: &[bool], cells: &[(f64, f64)]) -> usize {
    let mapper = ModemSpec::gray_square_qam(256).preferred_mapper();
    let mut out_i = vec![0.0_f32; cells.len()];
    let mut out_q = vec![0.0_f32; cells.len()];
    mapper.map_bits(labels, &mut out_i, &mut out_q);
    cells
        .iter()
        .zip(out_i.iter().zip(&out_q))
        .filter(|(&(re, im), (&i, &q))| {
            (f64::from(i) - re).abs() > CELL_TOL || (f64::from(q) - im).abs() > CELL_TOL
        })
        .count()
}

/// The preset reads its I label from the first half of a label, the reference
/// stream from the even-indexed bits of a cell word: the preset reproduces
/// every TP08a cell from the even-indexed bits followed by the odd-indexed
/// bits, and not from the cell word itself.
#[test]
fn test_gray_square_qam_256_maps_vv001_cells_from_even_then_odd_word_bits() {
    let Some((words, cells)) = first_block() else {
        common::skip(
            "test_gray_square_qam_256_maps_vv001_cells_from_even_then_odd_word_bits",
            "VV001-CR35 TP07 and TP08a streams absent under DVB_TEST_VECTORS_PATH",
        );
        return;
    };

    let even_then_odd: Vec<bool> = words
        .chunks_exact(BITS_PER_CELL)
        .flat_map(|word| {
            let even = word.iter().step_by(2);
            let odd = word.iter().skip(1).step_by(2);
            even.chain(odd).copied().collect::<Vec<_>>()
        })
        .collect();

    assert_eq!(
        mismatched_cells(&even_then_odd, &cells),
        0,
        "even-then-odd cell-word bits must reproduce every TP08a cell"
    );
    assert_ne!(
        mismatched_cells(&words, &cells),
        0,
        "the preset label is not the VV001-CR35 cell word"
    );
}
