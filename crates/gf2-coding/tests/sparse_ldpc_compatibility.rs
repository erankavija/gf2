//! Test that gf2-core sparse matrix is suitable for LDPC code operations.

use gf2_core::sparse::SparseMatrix;
use gf2_core::BitVec;

#[test]
fn test_ldpc_regular_code_structure() {
    // Simulate a regular (3,6) LDPC code: 3 ones per column, 6 ones per row
    // Small example: 4 rows × 8 cols (rate = 1/2)
    // Each column has exactly 3 ones
    // Each row should have exactly 6 ones for regularity

    let coo = vec![
        // Column 0: rows 0,1,2
        (0, 0),
        (1, 0),
        (2, 0),
        // Column 1: rows 0,1,3
        (0, 1),
        (1, 1),
        (3, 1),
        // Column 2: rows 0,2,3
        (0, 2),
        (2, 2),
        (3, 2),
        // Column 3: rows 1,2,3
        (1, 3),
        (2, 3),
        (3, 3),
        // Column 4: rows 0,1,2
        (0, 4),
        (1, 4),
        (2, 4),
        // Column 5: rows 0,1,3
        (0, 5),
        (1, 5),
        (3, 5),
        // Column 6: rows 0,2,3
        (0, 6),
        (2, 6),
        (3, 6),
        // Column 7: rows 1,2,3
        (1, 7),
        (2, 7),
        (3, 7),
    ];

    let h = SparseMatrix::from_coo(4, 8, &coo);

    // Verify structure
    assert_eq!(h.rows(), 4); // Check nodes
    assert_eq!(h.cols(), 8); // Variable nodes
    assert_eq!(h.nnz(), 24); // Total edges

    // Check row weights (check node degree)
    for r in 0..4 {
        let degree: usize = h.row_iter(r).count();
        assert_eq!(degree, 6, "Row {} should have degree 6", r);
    }

    // Check column weights by transposing (variable node degree)
    let h_t = h.transpose();
    for c in 0..8 {
        let degree: usize = h_t.row_iter(c).count();
        assert_eq!(degree, 3, "Column {} should have degree 3", c);
    }
}

#[test]
fn test_ldpc_syndrome_computation() {
    // Simple 3×6 parity check matrix
    let coo = vec![
        (0, 0),
        (0, 1),
        (0, 2), // Check 0: c0 + c1 + c2 = 0
        (1, 2),
        (1, 3),
        (1, 4), // Check 1: c2 + c3 + c4 = 0
        (2, 0),
        (2, 4),
        (2, 5), // Check 2: c0 + c4 + c5 = 0
    ];

    let h = SparseMatrix::from_coo(3, 6, &coo);

    // Valid codeword: [0,0,0,0,0,0] should give zero syndrome
    let mut valid_codeword = BitVec::new();
    for _ in 0..6 {
        valid_codeword.push_bit(false);
    }
    let syndrome = h.matvec(&valid_codeword);
    assert_eq!(syndrome.len(), 3);
    assert!(!syndrome.get(0) && !syndrome.get(1) && !syndrome.get(2));

    // Another valid codeword: [1,1,0,0,0,1]
    // Check 0: 1+1+0 = 0 ✓
    // Check 1: 0+0+0 = 0 ✓
    // Check 2: 1+0+1 = 0 ✓
    let mut valid2 = BitVec::new();
    for &b in &[true, true, false, false, false, true] {
        valid2.push_bit(b);
    }
    let syndrome2 = h.matvec(&valid2);
    assert!(!syndrome2.get(0) && !syndrome2.get(1) && !syndrome2.get(2));

    // Invalid codeword: [1,0,0,0,0,0] should fail check 0
    // Check 0: 1+0+0 = 1 ✗
    // Check 1: 0+0+0 = 0 ✓
    // Check 2: 1+0+0 = 1 ✗
    let mut invalid = BitVec::new();
    invalid.push_bit(true);
    for _ in 0..5 {
        invalid.push_bit(false);
    }
    let syndrome3 = h.matvec(&invalid);
    assert!(syndrome3.get(0)); // Check 0 fails
    assert!(!syndrome3.get(1)); // Check 1 passes
    assert!(syndrome3.get(2)); // Check 2 fails
}

#[test]
fn test_ldpc_neighbor_iteration_performance() {
    // Create a sparse matrix with typical LDPC density
    // 1000 variable nodes, 500 check nodes, column weight 3
    let mut coo = Vec::new();

    for col in 0..1000 {
        // Each variable node connects to 3 check nodes
        // Distribute somewhat evenly
        let check1 = (col * 3) % 500;
        let check2 = (col * 3 + 1) % 500;
        let check3 = (col * 3 + 2) % 500;
        coo.push((check1, col));
        coo.push((check2, col));
        coo.push((check3, col));
    }

    let h = SparseMatrix::from_coo(500, 1000, &coo);

    // Should be very sparse
    let density = h.nnz() as f64 / (500.0 * 1000.0);
    assert!(density < 0.01, "Density should be < 1% for LDPC");

    // Verify we can efficiently iterate neighbors
    // This is critical for belief propagation
    let mut total_neighbors = 0;
    for r in 0..500 {
        total_neighbors += h.row_iter(r).count();
    }
    assert_eq!(total_neighbors, h.nnz());

    // Check column iteration (via transpose)
    let h_t = h.transpose();
    let mut col_neighbors = 0;
    for c in 0..1000 {
        let degree = h_t.row_iter(c).count();
        col_neighbors += degree;
        // Most columns should have degree 3
        assert!(
            (2..=4).contains(&degree),
            "Column {} has unexpected degree {}",
            c,
            degree
        );
    }
    assert_eq!(col_neighbors, h.nnz());
}

#[test]
fn test_sparse_row_and_col_access() {
    // Small example to test both row and column iteration
    let coo = vec![(0, 1), (0, 3), (1, 2), (1, 3), (2, 0), (2, 1)];

    let h = SparseMatrix::from_coo(3, 4, &coo);

    // Test row iteration (check nodes in LDPC)
    let row0: Vec<_> = h.row_iter(0).collect();
    assert_eq!(row0, vec![1, 3]);

    let row1: Vec<_> = h.row_iter(1).collect();
    assert_eq!(row1, vec![2, 3]);

    let row2: Vec<_> = h.row_iter(2).collect();
    assert_eq!(row2, vec![0, 1]);

    // Test column iteration (variable nodes in LDPC)
    let col0: Vec<_> = h.col_iter(0).into_iter().collect();
    assert_eq!(col0, vec![2]);

    let col1: Vec<_> = h.col_iter(1).into_iter().collect();
    assert_eq!(col1, vec![0, 2]);

    let col2: Vec<_> = h.col_iter(2).into_iter().collect();
    assert_eq!(col2, vec![1]);

    let col3: Vec<_> = h.col_iter(3).into_iter().collect();
    assert_eq!(col3, vec![0, 1]);
}
