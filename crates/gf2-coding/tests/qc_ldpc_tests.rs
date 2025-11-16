//! Tests for quasi-cyclic LDPC codes.
//!
//! Quasi-cyclic LDPC codes are structured codes where the parity-check matrix
//! is composed of circulant submatrices. This structure is used in standards
//! like DVB-T2, 5G NR, WiFi 802.11n, etc.

use gf2_coding::ldpc::{CirculantMatrix, LdpcCode, QuasiCyclicLdpc};
use gf2_core::BitVec;

/// Test creating a simple 2x2 QC-LDPC with identity and shifted circulants.
#[test]
fn test_simple_qc_ldpc_construction() {
    // Base matrix with 2x2 blocks, shift values:
    // [I, α¹]  where I is identity (shift 0) and α¹ is shift by 1
    // [α², I]  where α² is shift by 2
    let base_matrix = vec![vec![0, 1], vec![2, 0]];
    let expansion_factor = 3; // 3x3 circulant blocks

    let qc = QuasiCyclicLdpc::new(base_matrix, expansion_factor);

    assert_eq!(qc.base_rows(), 2);
    assert_eq!(qc.base_cols(), 2);
    assert_eq!(qc.expansion_factor(), 3);
    assert_eq!(qc.expanded_rows(), 6); // 2 * 3
    assert_eq!(qc.expanded_cols(), 6); // 2 * 3
}

/// Test that -1 in base matrix represents zero (no edge) blocks.
#[test]
fn test_qc_ldpc_zero_blocks() {
    // Base matrix with some zero blocks (-1 means no block)
    // [0, -1]
    // [-1, 1]
    let base_matrix = vec![vec![0, -1], vec![-1, 1]];
    let expansion_factor = 4;

    let qc = QuasiCyclicLdpc::new(base_matrix, expansion_factor);
    let code = LdpcCode::from_quasi_cyclic(&qc);

    // Expanded matrix should be 8x8
    assert_eq!(code.n(), 8);
    assert_eq!(code.m(), 8);

    // Verify structure by checking edge count
    // First base row has 1 non-zero block (identity at [0,0])
    // Second base row has 1 non-zero block (shift-1 at [1,1])
    // Total: 2 circulants × 4 ones each = 8 edges
    let edges = qc.to_edges();
    assert_eq!(edges.len(), 8);

    // Verify all-zeros is valid
    let mut all_zeros = BitVec::new();
    for _ in 0..8 {
        all_zeros.push_bit(false);
    }
    assert!(code.is_valid_codeword(&all_zeros));
}

/// Test circulant matrix expansion.
#[test]
fn test_circulant_expansion() {
    // Identity circulant (shift 0)
    let circ = CirculantMatrix::new(0, 4);
    let edges = circ.to_edges(0, 0);

    // Should create identity: (0,0), (1,1), (2,2), (3,3)
    assert_eq!(edges.len(), 4);
    assert!(edges.contains(&(0, 0)));
    assert!(edges.contains(&(1, 1)));
    assert!(edges.contains(&(2, 2)));
    assert!(edges.contains(&(3, 3)));
}

/// Test right-shifted circulant matrix.
#[test]
fn test_circulant_shift_right() {
    // Shift by 1 with size 4:
    // [0 1 0 0]   (first row has 1 in position 1)
    // [0 0 1 0]   (second row shifts right)
    // [0 0 0 1]
    // [1 0 0 0]   (wraps around)
    let circ = CirculantMatrix::new(1, 4);
    let edges = circ.to_edges(0, 0);

    assert_eq!(edges.len(), 4);
    assert!(edges.contains(&(0, 1)));
    assert!(edges.contains(&(1, 2)));
    assert!(edges.contains(&(2, 3)));
    assert!(edges.contains(&(3, 0))); // Wrap around
}

/// Test circulant with larger shift.
#[test]
fn test_circulant_shift_multiple() {
    let circ = CirculantMatrix::new(2, 5);
    let edges = circ.to_edges(0, 0);

    assert_eq!(edges.len(), 5);
    assert!(edges.contains(&(0, 2)));
    assert!(edges.contains(&(1, 3)));
    assert!(edges.contains(&(2, 4)));
    assert!(edges.contains(&(3, 0))); // Wrap
    assert!(edges.contains(&(4, 1))); // Wrap
}

/// Test circulant block offset in larger matrix.
#[test]
fn test_circulant_with_offset() {
    // Place a 3x3 circulant at position (1,2) in a larger matrix
    let circ = CirculantMatrix::new(1, 3);
    let edges = circ.to_edges(1, 2); // Row offset 1*3=3, col offset 2*3=6

    // Circulant at offset should have edges at (3+i, 6+j)
    assert_eq!(edges.len(), 3);
    assert!(edges.contains(&(3, 7))); // (0+3, 1+6)
    assert!(edges.contains(&(4, 8))); // (1+3, 2+6)
    assert!(edges.contains(&(5, 6))); // (2+3, 0+6) wrap in col
}

/// Test QC-LDPC syndrome computation on valid codeword.
#[test]
fn test_qc_ldpc_syndrome_all_zeros() {
    let base_matrix = vec![vec![0, 1], vec![1, 0]];
    let expansion_factor = 3;

    let qc = QuasiCyclicLdpc::new(base_matrix, expansion_factor);
    let code = LdpcCode::from_quasi_cyclic(&qc);

    // All-zeros is always a valid codeword
    let mut codeword = BitVec::new();
    for _ in 0..code.n() {
        codeword.push_bit(false);
    }

    assert!(code.is_valid_codeword(&codeword));
}

/// Test that row and column weights are consistent with base matrix structure.
#[test]
fn test_qc_ldpc_weights() {
    // Base matrix with specific structure
    // [0, 1, -1]  -> row has 2 non-zero blocks
    // [1, -1, 2]  -> row has 2 non-zero blocks
    let base_matrix = vec![vec![0, 1, -1], vec![1, -1, 2]];
    let expansion_factor = 4;

    let qc = QuasiCyclicLdpc::new(base_matrix, expansion_factor);
    let _code = LdpcCode::from_quasi_cyclic(&qc);

    // Each base matrix row with 2 non-zero blocks creates expansion_factor
    // rows, each with weight 2*expansion_factor in the parity-check matrix
    // We can verify this by checking the number of edges
    let total_edges = qc.to_edges().len();

    // 2 base rows × 2 non-zero blocks per row × expansion_factor ones per circulant
    let expected_edges = 2 * 2 * expansion_factor;
    assert_eq!(total_edges, expected_edges);
}

/// Test dimensions match specification.
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

    assert_eq!(code.m(), 3 * 5); // 3 base rows * 5
    assert_eq!(code.n(), 5 * 5); // 5 base cols * 5
    assert_eq!(code.k(), 10); // n - m = 25 - 15
}

/// Test that base matrix validation catches invalid inputs.
#[test]
#[should_panic(expected = "Base matrix must have at least one row")]
fn test_qc_ldpc_empty_base_matrix() {
    let base_matrix: Vec<Vec<i32>> = vec![];
    let expansion_factor = 4;
    let _qc = QuasiCyclicLdpc::new(base_matrix, expansion_factor);
}

/// Test that base matrix validation catches inconsistent row lengths.
#[test]
#[should_panic(expected = "All rows in base matrix must have the same length")]
fn test_qc_ldpc_inconsistent_base_matrix() {
    let base_matrix = vec![vec![0, 1], vec![1, 0, 2]]; // Second row has 3 elements
    let expansion_factor = 4;
    let _qc = QuasiCyclicLdpc::new(base_matrix, expansion_factor);
}

/// Test that expansion factor must be positive.
#[test]
#[should_panic(expected = "Expansion factor must be positive")]
fn test_qc_ldpc_zero_expansion_factor() {
    let base_matrix = vec![vec![0, 1]];
    let expansion_factor = 0;
    let _qc = QuasiCyclicLdpc::new(base_matrix, expansion_factor);
}

/// Test that shift values are validated.
#[test]
#[should_panic(expected = "Shift value")]
fn test_qc_ldpc_invalid_shift_value() {
    let base_matrix = vec![vec![0, 5]]; // Shift 5 is invalid for expansion_factor 4
    let expansion_factor = 4;
    let qc = QuasiCyclicLdpc::new(base_matrix, expansion_factor);
    let _code = LdpcCode::from_quasi_cyclic(&qc); // Should panic during expansion
}
