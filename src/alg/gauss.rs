//! Gauss-Jordan elimination for matrix inversion over GF(2).
//!
//! This module implements matrix inversion using Gauss-Jordan elimination with
//! full pivoting over the binary field GF(2).

use crate::kernels::ops::xor_inplace;
use crate::matrix::BitMatrix;

/// Computes the inverse of a square matrix over GF(2) using Gauss-Jordan elimination.
///
/// Returns `None` if the matrix is singular (not invertible).
///
/// # Algorithm
///
/// 1. Create an augmented matrix [A | I] where I is the identity matrix
/// 2. For each column i (left to right):
///    a. Find a pivot row at or below row i that has a 1 in column i
///    b. If no pivot found, matrix is singular → return None
///    c. Swap pivot row with row i
///    d. For all other rows j ≠ i: if row j has a 1 in column i, XOR row i into row j
/// 3. The right half of the augmented matrix is now A^(-1)
///
/// # Arguments
///
/// * `m` - Square matrix to invert
///
/// # Returns
///
/// * `Some(inverse)` - The inverse matrix if it exists
/// * `None` - If the matrix is singular or non-square
///
/// # Examples
///
/// ```
/// use gf2::matrix::BitMatrix;
/// use gf2::alg::gauss::invert;
///
/// let id = BitMatrix::identity(3);
/// let inv = invert(&id).unwrap();
/// // inv equals id for identity matrix
/// ```
pub fn invert(m: &BitMatrix) -> Option<BitMatrix> {
    let n = m.rows();
    
    // Must be square
    if n != m.cols() {
        return None;
    }
    
    if n == 0 {
        return Some(BitMatrix::new_zero(0, 0));
    }
    
    // Create augmented matrix [A | I]
    // Each row has 2n columns
    let mut aug = BitMatrix::new_zero(n, 2 * n);
    
    // Copy A into left half
    for r in 0..n {
        for c in 0..n {
            aug.set(r, c, m.get(r, c));
        }
    }
    
    // Set right half to identity
    for i in 0..n {
        aug.set(i, n + i, true);
    }
    
    // Gauss-Jordan elimination
    for col in 0..n {
        // Find pivot row (any row >= col with a 1 in column col)
        let mut pivot_row = None;
        for r in col..n {
            if aug.get(r, col) {
                pivot_row = Some(r);
                break;
            }
        }
        
        let pivot_row = match pivot_row {
            Some(r) => r,
            None => return None, // No pivot found, matrix is singular
        };
        
        // Swap pivot row with current row
        if pivot_row != col {
            aug.swap_rows(col, pivot_row);
        }
        
        // Eliminate column col from all other rows
        for r in 0..n {
            if r != col && aug.get(r, col) {
                // XOR row col into row r
                // We need to XOR the entire rows
                let row_col_words: Vec<u64> = aug.row_words(col).to_vec();
                let row_r = aug.row_words_mut(r);
                xor_inplace(row_r, &row_col_words);
            }
        }
    }
    
    // Extract right half (the inverse)
    let mut inv = BitMatrix::new_zero(n, n);
    for r in 0..n {
        for c in 0..n {
            inv.set(r, c, aug.get(r, n + c));
        }
    }
    
    Some(inv)
}
