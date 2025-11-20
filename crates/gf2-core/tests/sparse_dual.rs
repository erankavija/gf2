use gf2_core::matrix::BitMatrix;
use gf2_core::sparse::{SpBitMatrix, SpBitMatrixDual};
use gf2_core::BitVec;

#[test]
fn test_dual_from_dense_roundtrip() {
    let mut m = BitMatrix::zeros(3, 5);
    m.set(0, 1, true);
    m.set(0, 3, true);
    m.set(1, 2, true);
    m.set(2, 0, true);
    m.set(2, 4, true);

    let dual = SpBitMatrixDual::from_dense(&m);
    assert_eq!(dual.rows(), 3);
    assert_eq!(dual.cols(), 5);
    assert_eq!(dual.nnz(), 5);

    let d = dual.to_dense();
    assert_eq!(d, m);
}

#[test]
fn test_dual_row_iter_matches_csr() {
    let coo = vec![(0, 2), (0, 4), (1, 1), (2, 0), (2, 3)];
    let dual = SpBitMatrixDual::from_coo(3, 5, &coo);
    let single = SpBitMatrix::from_coo(3, 5, &coo);

    for r in 0..3 {
        let dual_cols: Vec<_> = dual.row_iter(r).collect();
        let single_cols: Vec<_> = single.row_iter(r).collect();
        assert_eq!(dual_cols, single_cols);
    }
}

#[test]
fn test_dual_col_iter_no_transpose() {
    let mut m = BitMatrix::zeros(4, 3);
    m.set(0, 1, true);
    m.set(2, 1, true);
    m.set(3, 2, true);

    let dual = SpBitMatrixDual::from_dense(&m);

    // Column 1 has rows [0, 2]
    let col1_rows: Vec<_> = dual.col_iter(1).collect();
    assert_eq!(col1_rows, vec![0, 2]);

    // Column 2 has rows [3]
    let col2_rows: Vec<_> = dual.col_iter(2).collect();
    assert_eq!(col2_rows, vec![3]);

    // Column 0 is empty
    let col0_rows: Vec<_> = dual.col_iter(0).collect();
    assert_eq!(col0_rows, Vec::<usize>::new());
}

#[test]
fn test_dual_matvec_matches_single() {
    let coo = vec![(0, 1), (0, 3), (1, 0), (1, 4), (2, 2)];
    let dual = SpBitMatrixDual::from_coo(3, 5, &coo);
    let single = SpBitMatrix::from_coo(3, 5, &coo);

    let mut x = BitVec::new();
    for b in [false, true, true, false, true] {
        x.push_bit(b);
    }

    let y_dual = dual.matvec(&x);
    let y_single = single.matvec(&x);
    assert_eq!(y_dual, y_single);
}

#[test]
fn test_dual_matvec_transpose() {
    let mut m = BitMatrix::zeros(3, 5);
    m.set(0, 1, true);
    m.set(0, 3, true);
    m.set(1, 0, true);
    m.set(1, 4, true);
    m.set(2, 2, true);

    let dual = SpBitMatrixDual::from_dense(&m);

    let mut x = BitVec::new();
    for _ in 0..3 {
        x.push_bit(true);
    }

    let y = dual.matvec_transpose(&x);
    assert_eq!(y.len(), 5);

    // Check against manual transpose
    let mt = m.transpose();
    let single_t = SpBitMatrix::from_dense(&mt);
    let y_expected = single_t.matvec(&x);
    assert_eq!(y, y_expected);
}

#[test]
fn test_dual_bidirectional_sweep() {
    // Simulate alternating row/column access pattern
    let mut m = BitMatrix::zeros(4, 6);
    m.set(0, 1, true);
    m.set(1, 2, true);
    m.set(2, 3, true);
    m.set(3, 4, true);

    let dual = SpBitMatrixDual::from_dense(&m);

    // Row sweep
    let mut row_sum = 0;
    for r in 0..dual.rows() {
        row_sum += dual.row_iter(r).count();
    }
    assert_eq!(row_sum, 4);

    // Column sweep (no transpose!)
    let mut col_sum = 0;
    for c in 0..dual.cols() {
        col_sum += dual.col_iter(c).count();
    }
    assert_eq!(col_sum, 4);
}

// ============================================================================
// Deduplication Tests for SpBitMatrixDual
// ============================================================================

#[test]
fn test_dual_from_coo_deduplicated_basic() {
    let edges = vec![(0, 0), (0, 1), (0, 1), (1, 2)];
    let dual = SpBitMatrixDual::from_coo_deduplicated(2, 3, &edges);

    let d = dual.to_dense();
    assert!(d.get(0, 0));
    assert!(d.get(0, 1));
    assert!(d.get(1, 2));
    assert_eq!(dual.nnz(), 3);
}

#[test]
fn test_dual_from_coo_deduplicated_vs_xor() {
    let edges = vec![(0, 1), (0, 1), (1, 2)];

    let xor_dual = SpBitMatrixDual::from_coo(2, 3, &edges);
    let xor_d = xor_dual.to_dense();
    assert!(!xor_d.get(0, 1));
    assert_eq!(xor_dual.nnz(), 1);

    let dedup_dual = SpBitMatrixDual::from_coo_deduplicated(2, 3, &edges);
    let dedup_d = dedup_dual.to_dense();
    assert!(dedup_d.get(0, 1));
    assert_eq!(dedup_dual.nnz(), 2);
}

#[test]
fn test_dual_from_coo_deduplicated_row_col_consistency() {
    // Test that both CSR and CSC views are consistent after deduplication
    let edges = vec![
        (0, 1),
        (0, 1), // Row 0, col 1 (duplicate)
        (0, 2), // Row 0, col 2
        (1, 1),
        (1, 1), // Row 1, col 1 (duplicate)
        (2, 0), // Row 2, col 0
    ];
    let dual = SpBitMatrixDual::from_coo_deduplicated(3, 3, &edges);

    // Check row iteration
    let row0: Vec<_> = dual.row_iter(0).collect();
    assert_eq!(row0, vec![1, 2]);

    let row1: Vec<_> = dual.row_iter(1).collect();
    assert_eq!(row1, vec![1]);

    let row2: Vec<_> = dual.row_iter(2).collect();
    assert_eq!(row2, vec![0]);

    // Check column iteration
    let col0: Vec<_> = dual.col_iter(0).collect();
    assert_eq!(col0, vec![2]);

    let col1: Vec<_> = dual.col_iter(1).collect();
    assert_eq!(col1, vec![0, 1]);

    let col2: Vec<_> = dual.col_iter(2).collect();
    assert_eq!(col2, vec![0]);

    assert_eq!(dual.nnz(), 4);
}

#[test]
fn test_dual_from_coo_deduplicated_dvb_t2_scenario() {
    // Realistic scenario: dual-diagonal parity overlaps with info connections
    let edges = vec![
        // Information connections
        (0, 2),
        (0, 5),
        (1, 3),
        (1, 4),
        // Dual-diagonal parity structure
        (0, 4), // P[0]
        (1, 4), // P[1] = P[0] (dual-diagonal)
        (1, 5), // P[2]
        // Accidental duplicate from table expansion
        (0, 2), // Duplicate of first entry
    ];

    let dual = SpBitMatrixDual::from_coo_deduplicated(2, 6, &edges);

    // After deduplication: 6 unique edges
    assert_eq!(dual.nnz(), 6);

    // Verify structure
    let d = dual.to_dense();
    assert!(d.get(0, 2)); // Info
    assert!(d.get(0, 5)); // Info
    assert!(d.get(0, 4)); // Parity
    assert!(d.get(1, 3)); // Info
    assert!(d.get(1, 4)); // Parity (dual-diagonal)
    assert!(d.get(1, 5)); // Parity
}
