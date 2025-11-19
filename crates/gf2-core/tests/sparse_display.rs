use gf2_core::sparse::{SpBitMatrix, SpBitMatrixDual};

#[test]
fn test_sparse_matrix_display_basic() {
    let coo = vec![(0, 0), (0, 3), (1, 1), (2, 2)];
    let s = SpBitMatrix::from_coo(3, 4, &coo);

    let output = format!("{}", s);
    let lines: Vec<&str> = output.lines().collect();

    assert_eq!(lines.len(), 5); // top border + 3 rows + bottom border
    assert!(lines[0].contains("┌"));
    assert!(lines[0].contains("┐"));
    assert!(lines[1].contains("│ 1 0 0 1 │"));
    assert!(lines[2].contains("│ 0 1 0 0 │"));
    assert!(lines[3].contains("│ 0 0 1 0 │"));
    assert!(lines[4].contains("└"));
    assert!(lines[4].contains("┘"));
}

#[test]
fn test_sparse_matrix_display_empty() {
    let s = SpBitMatrix::zeros(0, 0);
    let output = format!("{}", s);
    assert_eq!(output, "[ ]");
}

#[test]
fn test_sparse_matrix_display_zero_rows() {
    let s = SpBitMatrix::zeros(0, 5);
    let output = format!("{}", s);
    assert_eq!(output, "[ ]");
}

#[test]
fn test_sparse_matrix_display_zero_cols() {
    let s = SpBitMatrix::zeros(3, 0);
    let output = format!("{}", s);
    assert_eq!(output, "[ ]");
}

#[test]
fn test_sparse_matrix_display_identity() {
    let s = SpBitMatrix::identity(3);
    let output = format!("{}", s);
    let lines: Vec<&str> = output.lines().collect();

    assert!(lines[1].contains("│ 1 0 0 │"));
    assert!(lines[2].contains("│ 0 1 0 │"));
    assert!(lines[3].contains("│ 0 0 1 │"));
}

#[test]
fn test_sparse_matrix_display_all_zeros() {
    let s = SpBitMatrix::zeros(2, 3);
    let output = format!("{}", s);
    let lines: Vec<&str> = output.lines().collect();

    assert!(lines[1].contains("│ 0 0 0 │"));
    assert!(lines[2].contains("│ 0 0 0 │"));
}

#[test]
fn test_sparse_matrix_dual_display() {
    let coo = vec![(0, 0), (0, 3), (1, 1), (2, 2)];
    let s = SpBitMatrixDual::from_coo(3, 4, &coo);

    let output = format!("{}", s);
    let lines: Vec<&str> = output.lines().collect();

    assert_eq!(lines.len(), 5);
    assert!(lines[1].contains("│ 1 0 0 1 │"));
    assert!(lines[2].contains("│ 0 1 0 0 │"));
    assert!(lines[3].contains("│ 0 0 1 0 │"));
}
