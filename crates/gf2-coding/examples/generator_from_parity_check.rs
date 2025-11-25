//! Computing Generator Matrix from Parity-Check Matrix
//!
//! This example demonstrates the algorithm for computing a systematic generator
//! matrix G from a parity-check matrix H such that H·G^T = 0 in GF(2).
//!
//! The algorithm uses Gaussian elimination with a critical row reordering step
//! to correctly extract parity relationships.

use gf2_coding::ldpc::encoding::RuEncodingMatrices;
use gf2_core::sparse::SpBitMatrixDual;
use gf2_core::BitVec;

fn print_matrix(name: &str, h: &SpBitMatrixDual) {
    println!("\n{} ({}×{}):", name, h.rows(), h.cols());
    for row in 0..h.rows() {
        print!("  ");
        for col in 0..h.cols() {
            let has_edge = h.row_iter(row).any(|c| c == col);
            print!("{} ", if has_edge { 1 } else { 0 });
        }
        println!();
    }
}

fn main() {
    println!("=== Generator Matrix from Parity-Check Matrix ===\n");
    
    println!("Algorithm Overview:");
    println!("------------------");
    println!("1. Gaussian elimination with column pivoting from right");
    println!("2. **CRITICAL**: Reorder rows so row i has pivot in parity_cols[i]");
    println!("3. Build G = [I_k | P] using reordered H structure");
    println!("\nThe row reordering ensures proper alignment of the identity");
    println!("structure, enabling correct extraction of parity relationships.");
    
    // Example 1: Custom Hamming [7,4]
    println!("\n{}", "=".repeat(70));
    println!("Example 1: Custom Hamming [7,4] Code");
    println!("{}", "=".repeat(70));
    
    let edges1 = vec![
        (0, 0), (0, 2), (0, 3), (0, 4),
        (1, 1), (1, 3), (1, 5),
        (2, 2), (2, 3), (2, 6),
    ];
    let h1 = SpBitMatrixDual::from_coo(3, 7, &edges1);
    
    print_matrix("Parity-check matrix H", &h1);
    
    // Preprocess to compute generator matrix
    let matrices1 = RuEncodingMatrices::preprocess(&h1).unwrap();
    println!("\n✓ Successfully computed generator matrix G");
    println!("  - Message dimension k = {}", matrices1.k());
    println!("  - Codeword length n = {}", matrices1.n());
    println!("  - Parity bits r = {}", matrices1.r());
    
    // Test encoding
    println!("\nTesting encoding for all 2^k = 16 messages:");
    let mut all_valid = true;
    for msg_val in 0u8..16 {
        let mut message = BitVec::new();
        for i in 0..4 {
            message.push_bit((msg_val >> i) & 1 == 1);
        }
        
        let codeword = matrices1.encode(&message);
        let syndrome = h1.matvec(&codeword);
        
        let valid = syndrome.count_ones() == 0;
        if !valid {
            all_valid = false;
            println!("  Message {:2}: INVALID (syndrome non-zero)", msg_val);
        }
    }
    
    if all_valid {
        println!("  ✓ All 16 codewords satisfy H·c = 0");
    } else {
        println!("  ✗ Some codewords INVALID");
    }
    
    // Example 2: Standard Hamming [7,4]
    println!("\n{}", "=".repeat(70));
    println!("Example 2: Standard Hamming [7,4] Code");
    println!("{}", "=".repeat(70));
    
    let edges2 = vec![
        (0, 0), (0, 1), (0, 3), (0, 4),
        (1, 0), (1, 2), (1, 3), (1, 5),
        (2, 1), (2, 2), (2, 3), (2, 6),
    ];
    let h2 = SpBitMatrixDual::from_coo(3, 7, &edges2);
    
    print_matrix("Parity-check matrix H", &h2);
    
    let matrices2 = RuEncodingMatrices::preprocess(&h2).unwrap();
    println!("\n✓ Successfully computed generator matrix G");
    
    // Test a few specific messages
    println!("\nEncoding examples:");
    let test_messages = vec![
        (0b0000, "0000"),
        (0b0001, "0001"),
        (0b1010, "1010"),
        (0b1111, "1111"),
    ];
    
    for (msg_val, msg_str) in test_messages {
        let mut message = BitVec::new();
        for i in 0..4 {
            message.push_bit((msg_val >> i) & 1 == 1);
        }
        
        let codeword = matrices2.encode(&message);
        let syndrome = h2.matvec(&codeword);
        
        print!("  Message {}: ", msg_str);
        for i in 0..7 {
            print!("{}", if codeword.get(i) { 1 } else { 0 });
        }
        if syndrome.count_ones() == 0 {
            println!(" ✓");
        } else {
            println!(" ✗");
        }
    }
    
    // Summary
    println!("\n{}", "=".repeat(70));
    println!("Summary");
    println!("{}", "=".repeat(70));
    println!("\nThe algorithm successfully computes generator matrices for");
    println!("different parity-check matrices, ensuring H·G^T = 0 in GF(2).");
    println!("\nKey insight: After Gaussian elimination, rows must be reordered");
    println!("to align with pivot columns. This ensures the systematic form");
    println!("G = [I_k | P] correctly encodes the parity relationships.");
}
