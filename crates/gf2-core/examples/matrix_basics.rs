//! BitMatrix Basics - Essential operations tutorial
//!
//! This example demonstrates fundamental BitMatrix operations:
//! - Construction (zeros, identity, ones)
//! - Element access (get, set)
//! - Row operations (swap, XOR)
//! - Matrix operations (transpose, multiply)
//! - Row/column extraction
//!
//! Run with: `cargo run --example matrix_basics`

use gf2_core::{BitMatrix, BitVec};

fn main() {
    println!("=== BitMatrix Basics Tutorial ===\n");

    // ========================================
    // 1. Construction
    // ========================================
    println!("1. Construction");
    println!("   ----------------");

    let zeros = BitMatrix::zeros(3, 3);
    println!("   3×3 zeros:\n{}", zeros);

    let identity = BitMatrix::identity(3);
    println!("   3×3 identity:\n{}", identity);

    let ones = BitMatrix::ones(2, 4);
    println!("   2×4 ones:\n{}\n", ones);

    // ========================================
    // 2. Element Access
    // ========================================
    println!("2. Element Access");
    println!("   ----------------");

    let mut m = BitMatrix::zeros(3, 3);
    m.set(0, 0, true); // Top-left
    m.set(1, 1, true); // Center
    m.set(2, 2, true); // Bottom-right
    m.set(0, 2, true); // Top-right

    println!("   Matrix after setting elements:");
    println!("{}", m);

    println!("   m.get(0, 0) = {}", m.get(0, 0));
    println!("   m.get(0, 1) = {}", m.get(0, 1));
    println!("   m.get(0, 2) = {}\n", m.get(0, 2));

    // ========================================
    // 3. Dimensions
    // ========================================
    println!("3. Dimensions");
    println!("   ----------------");

    let m = BitMatrix::zeros(10, 20);
    println!(
        "   Matrix dimensions: {} rows × {} columns\n",
        m.rows(),
        m.cols()
    );

    // ========================================
    // 4. Row Operations (GF(2) algebra)
    // ========================================
    println!("4. Row Operations");
    println!("   ----------------");

    let mut m = BitMatrix::identity(3);
    println!("   Starting with identity:");
    println!("{}", m);

    // XOR row 1 into row 0 (Gaussian elimination step)
    m.row_xor(0, 1);
    println!("   After row[0] ^= row[1]:");
    println!("{}", m);

    // Swap rows 1 and 2
    m.swap_rows(1, 2);
    println!("   After swap_rows(1, 2):");
    println!("{}\n", m);

    // ========================================
    // 5. Transpose
    // ========================================
    println!("5. Transpose");
    println!("   ----------------");

    let mut m = BitMatrix::zeros(2, 3);
    m.set(0, 0, true);
    m.set(0, 2, true);
    m.set(1, 1, true);

    println!("   Original (2×3):");
    println!("{}", m);

    let t = m.transpose();
    println!("   Transposed (3×2):");
    println!("{}\n", t);

    // ========================================
    // 6. Matrix Multiplication
    // ========================================
    println!("6. Matrix Multiplication (M4RM algorithm)");
    println!("   ----------------");

    let a = BitMatrix::identity(3);
    let mut b = BitMatrix::zeros(3, 3);
    b.set(0, 1, true);
    b.set(1, 2, true);
    b.set(2, 0, true);

    println!("   Matrix A (identity):");
    println!("{}", a);

    println!("   Matrix B:");
    println!("{}", b);

    let c = &a * &b;
    println!("   A × B = B (identity property):");
    println!("{}\n", c);

    // Non-trivial multiplication
    let d = &b * &b;
    println!("   B × B:");
    println!("{}\n", d);

    // ========================================
    // 7. Row and Column Extraction
    // ========================================
    println!("7. Row and Column Extraction");
    println!("   ----------------");

    let mut m = BitMatrix::zeros(3, 4);
    m.set(0, 0, true);
    m.set(0, 2, true);
    m.set(1, 1, true);
    m.set(1, 3, true);
    m.set(2, 0, true);

    println!("   Matrix (3×4):");
    println!("{}", m);

    let row0 = m.row_as_bitvec(0);
    println!(
        "   row[0] as BitVec: {} bits, pattern: {:04b}",
        row0.len(),
        row0.to_bytes_le()[0] & 0xF
    );

    let col0 = m.col_as_bitvec(0);
    println!(
        "   col[0] as BitVec: {} bits, pattern: {:03b}\n",
        col0.len(),
        col0.to_bytes_le()[0] & 0x7
    );

    // ========================================
    // 8. Practical Example: Solving Ax = b
    // ========================================
    println!("8. Practical Example: Matrix-Vector Multiply");
    println!("   ----------------");

    // Create a simple parity check matrix H
    let mut h = BitMatrix::zeros(3, 7);
    // Row 0: positions 0, 1, 3, 4
    h.set(0, 0, true);
    h.set(0, 1, true);
    h.set(0, 3, true);
    h.set(0, 4, true);
    // Row 1: positions 0, 2, 3, 5
    h.set(1, 0, true);
    h.set(1, 2, true);
    h.set(1, 3, true);
    h.set(1, 5, true);
    // Row 2: positions 1, 2, 3, 6
    h.set(2, 1, true);
    h.set(2, 2, true);
    h.set(2, 3, true);
    h.set(2, 6, true);

    println!("   Parity check matrix H (3×7):");
    println!("{}", h);

    // Valid codeword (all zeros in syndrome)
    let x = BitVec::from_bytes_le(&[0b0000000]);
    let syndrome = h.matvec(&x);
    println!("   x = 0000000");
    print!("   H×x = ");
    for i in 0..syndrome.len() {
        print!("{}", if syndrome.get(i) { '1' } else { '0' });
    }
    println!(" (syndrome = 0 means valid codeword)");

    // Invalid codeword (non-zero syndrome)
    let x = BitVec::from_bytes_le(&[0b0010000]);
    let syndrome = h.matvec(&x);
    println!("   x = 0001000");
    print!("   H×x = ");
    for i in 0..syndrome.len() {
        print!("{}", if syndrome.get(i) { '1' } else { '0' });
    }
    println!(" (syndrome ≠ 0 means error detected)\n");

    println!("=== End of BitMatrix Basics Tutorial ===");
}
