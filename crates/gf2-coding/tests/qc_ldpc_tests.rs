//! Quasi-cyclic LDPC construction: circulant expansion, dimensions and input validation.

use gf2_coding::ldpc::{CirculantMatrix, LdpcCode, QuasiCyclicLdpc};
use gf2_core::BitVec;

#[test]
fn test_simple_qc_ldpc_construction() {
    let base_matrix = vec![vec![0, 1], vec![2, 0]];
    let expansion_factor = 3;

    let qc = QuasiCyclicLdpc::new(base_matrix, expansion_factor);

    assert_eq!(qc.base_rows(), 2);
    assert_eq!(qc.base_cols(), 2);
    assert_eq!(qc.expansion_factor(), 3);
    assert_eq!(qc.expanded_rows(), 6);
    assert_eq!(qc.expanded_cols(), 6);
}

#[test]
fn test_qc_ldpc_zero_blocks() {
    let base_matrix = vec![vec![0, -1], vec![-1, 1]];
    let expansion_factor = 4;

    let qc = QuasiCyclicLdpc::new(base_matrix, expansion_factor);
    let code = LdpcCode::from_quasi_cyclic(&qc);

    assert_eq!(code.n(), 8);
    assert_eq!(code.m(), 8);

    // 2 circulants × 4 ones each = 8 edges
    let edges = qc.to_edges();
    assert_eq!(edges.len(), 8);

    let mut all_zeros = BitVec::new();
    for _ in 0..8 {
        all_zeros.push_bit(false);
    }
    assert!(code.is_valid_codeword(&all_zeros));
}

#[test]
fn test_circulant_expansion() {
    let circ = CirculantMatrix::new(0, 4);
    let edges = circ.to_edges(0, 0);

    assert_eq!(edges.len(), 4);
    assert!(edges.contains(&(0, 0)));
    assert!(edges.contains(&(1, 1)));
    assert!(edges.contains(&(2, 2)));
    assert!(edges.contains(&(3, 3)));
}

#[test]
fn test_circulant_shift_right() {
    let circ = CirculantMatrix::new(1, 4);
    let edges = circ.to_edges(0, 0);

    assert_eq!(edges.len(), 4);
    assert!(edges.contains(&(0, 1)));
    assert!(edges.contains(&(1, 2)));
    assert!(edges.contains(&(2, 3)));
    assert!(edges.contains(&(3, 0)));
}

#[test]
fn test_circulant_shift_multiple() {
    let circ = CirculantMatrix::new(2, 5);
    let edges = circ.to_edges(0, 0);

    assert_eq!(edges.len(), 5);
    assert!(edges.contains(&(0, 2)));
    assert!(edges.contains(&(1, 3)));
    assert!(edges.contains(&(2, 4)));
    assert!(edges.contains(&(3, 0)));
    assert!(edges.contains(&(4, 1)));
}

#[test]
fn test_circulant_with_offset() {
    let circ = CirculantMatrix::new(1, 3);
    let edges = circ.to_edges(1, 2); // Row offset 1*3=3, col offset 2*3=6

    assert_eq!(edges.len(), 3);
    assert!(edges.contains(&(3, 7)));
    assert!(edges.contains(&(4, 8)));
    assert!(edges.contains(&(5, 6)));
}

#[test]
fn test_qc_ldpc_syndrome_all_zeros() {
    let base_matrix = vec![vec![0, 1], vec![1, 0]];
    let expansion_factor = 3;

    let qc = QuasiCyclicLdpc::new(base_matrix, expansion_factor);
    let code = LdpcCode::from_quasi_cyclic(&qc);

    let mut codeword = BitVec::new();
    for _ in 0..code.n() {
        codeword.push_bit(false);
    }

    assert!(code.is_valid_codeword(&codeword));
}

#[test]
fn test_qc_ldpc_weights() {
    let base_matrix = vec![vec![0, 1, -1], vec![1, -1, 2]];
    let expansion_factor = 4;

    let qc = QuasiCyclicLdpc::new(base_matrix, expansion_factor);
    let _code = LdpcCode::from_quasi_cyclic(&qc);

    let total_edges = qc.to_edges().len();

    // 2 base rows × 2 non-zero blocks per row × expansion_factor ones per circulant
    let expected_edges = 2 * 2 * expansion_factor;
    assert_eq!(total_edges, expected_edges);
}

#[test]
fn test_qc_ldpc_dimensions() {
    let base_matrix = vec![
        vec![0, 1, 2, 3, 4],
        vec![3, 0, 1, 2, 4],
        vec![2, 3, 0, 1, 4],
    ];
    let expansion_factor = 5;

    let qc = QuasiCyclicLdpc::new(base_matrix, expansion_factor);
    let code = LdpcCode::from_quasi_cyclic(&qc);

    assert_eq!(code.m(), 3 * 5);
    assert_eq!(code.n(), 5 * 5);
    assert_eq!(code.k(), 10); // n - m = 25 - 15
}

#[test]
#[should_panic(expected = "Base matrix must have at least one row")]
fn test_qc_ldpc_empty_base_matrix() {
    let base_matrix: Vec<Vec<i32>> = vec![];
    let expansion_factor = 4;
    let _qc = QuasiCyclicLdpc::new(base_matrix, expansion_factor);
}

#[test]
#[should_panic(expected = "All rows in base matrix must have the same length")]
fn test_qc_ldpc_inconsistent_base_matrix() {
    let base_matrix = vec![vec![0, 1], vec![1, 0, 2]];
    let expansion_factor = 4;
    let _qc = QuasiCyclicLdpc::new(base_matrix, expansion_factor);
}

#[test]
#[should_panic(expected = "Expansion factor must be positive")]
fn test_qc_ldpc_zero_expansion_factor() {
    let base_matrix = vec![vec![0, 1]];
    let expansion_factor = 0;
    let _qc = QuasiCyclicLdpc::new(base_matrix, expansion_factor);
}

#[test]
#[should_panic(expected = "Shift value")]
fn test_qc_ldpc_invalid_shift_value() {
    let base_matrix = vec![vec![0, 5]]; // Shift 5 is invalid for expansion_factor 4
    let expansion_factor = 4;
    let qc = QuasiCyclicLdpc::new(base_matrix, expansion_factor);
    let _code = LdpcCode::from_quasi_cyclic(&qc); // Should panic during expansion
}
