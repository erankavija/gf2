//! Understanding Block Codes
//!
//! **Difficulty**: 🟢 Beginner  
//! **Estimated Time**: 8 minutes
//!
//! Block codes encode fixed-size chunks (blocks) of data by adding redundancy.
//! This example explains the fundamental parameters and structure.
//!
//! ## Learning Objectives
//!
//! - Understand what a block code is (fixed-size data chunks)
//! - Learn about code parameters (n, k, rate)
//! - See how parity check matrix validates codewords
//! - Grasp the concept of systematic encoding
//! - Understand syndrome-based error detection
//!
//! ## Prerequisites
//!
//! If you haven't already, start with:
//! - [`hamming_basic.rs`](hamming_basic.html) - Your first error-correcting code
//!
//! ## Next Steps
//!
//! After this example, explore:
//! - [`hamming_7_4.rs`](hamming_7_4.html) - Deep dive into syndrome decoding
//! - [`dvb_t2_ldpc_basic.rs`](dvb_t2_ldpc_basic.html) - Real-world LDPC codes

use gf2_coding::traits::{BlockEncoder, GeneratorMatrixAccess};
use gf2_coding::LinearBlockCode;
use gf2_core::BitVec;

fn main() {
    println!("=== Understanding Block Codes ===\n");

    // Create Hamming(7,4) code
    let code = LinearBlockCode::hamming(3);

    // === CODE PARAMETERS ===
    println!("📊 Code Parameters:");
    println!(
        "   n = {} (codeword length - what gets transmitted)",
        code.n()
    );
    println!("   k = {} (message length - your actual data)", code.k());
    let rate = code.k() as f64 / code.n() as f64;
    println!(
        "   Rate = {:.2} ({}/{} = fraction of useful data)",
        rate,
        code.k(),
        code.n()
    );

    let g = code.generator_matrix();
    println!("   Generator matrix: {} × {} matrix", g.rows(), g.cols());

    // === SYSTEMATIC ENCODING ===
    println!("\n🔧 Systematic Encoding:");
    println!("   Codeword format: [message | parity]");
    println!("   First {} bits = your data (unchanged)", code.k());
    println!(
        "   Last {} bits = computed parity checks",
        code.n() - code.k()
    );

    // Demonstrate with example
    let mut message = BitVec::zeros(4);
    message.set(0, true); // Binary: 1000

    let codeword = code.encode(&message);
    println!("\n   Example with message [1 0 0 0]:");
    print!("   Message:  [");
    for i in 0..code.k() {
        print!("{}", if codeword.get(i) { "1" } else { "0" });
        if i < code.k() - 1 {
            print!(" ");
        }
    }
    println!("] (your data)");

    print!("   Parity:          [");
    for i in code.k()..code.n() {
        print!("{}", if codeword.get(i) { "1" } else { "0" });
        if i < code.n() - 1 {
            print!(" ");
        }
    }
    println!("] (added redundancy)");

    // === VALIDATION ===
    println!("\n✅ Parity Check Validation:");
    println!("   Valid codewords satisfy: H × c = 0 (over GF(2))");
    println!("   where H is the parity-check matrix");

    let syndrome = code.syndrome(&codeword).unwrap_or_else(|| BitVec::zeros(0));

    if syndrome.count_ones() == 0 {
        println!("   ✓ Syndrome is zero → valid codeword");
    } else {
        println!("   ✗ Non-zero syndrome → errors detected");
    }

    // === ERROR DETECTION ===
    println!("\n🔍 Error Detection:");
    let mut corrupted = codeword.clone();
    corrupted.set(0, !corrupted.get(0));

    print!("   Original:  [");
    for i in 0..codeword.len() {
        print!("{}", if codeword.get(i) { "1" } else { "0" });
    }
    println!("]");
    print!("   Corrupted: [");
    for i in 0..corrupted.len() {
        print!("{}", if corrupted.get(i) { "1" } else { "0" });
    }
    println!("] (flipped bit 0)");

    let syndrome2 = code.syndrome(&corrupted).unwrap();
    print!("   Syndrome:  [");
    for i in 0..syndrome2.len() {
        print!("{}", if syndrome2.get(i) { "1" } else { "0" });
    }
    println!("]");

    if syndrome2.count_ones() > 0 {
        println!("   ✓ Non-zero syndrome → error detected!");
    }

    // === MULTIPLE CODEWORDS ===
    println!("\n📦 Block Processing:");
    println!("   Block codes process data in fixed-size chunks");
    println!("   For long messages, split into {}-bit blocks", code.k());

    let messages = [
        {
            let mut bv = BitVec::from_bytes_le(&[0b0000]);
            bv.resize(4, false);
            bv
        },
        {
            let mut bv = BitVec::from_bytes_le(&[0b1111]);
            bv.resize(4, false);
            bv
        },
        {
            let mut bv = BitVec::from_bytes_le(&[0b1010]);
            bv.resize(4, false);
            bv
        },
    ];

    println!("\n   Example: Encoding 3 blocks (12 bits total)");
    for (i, msg) in messages.iter().enumerate() {
        let cw = code.encode(msg);
        print!("   Block {}: [", i + 1);
        for j in 0..msg.len() {
            print!("{}", if msg.get(j) { "1" } else { "0" });
        }
        print!("] → [");
        for j in 0..cw.len() {
            print!("{}", if cw.get(j) { "1" } else { "0" });
        }
        println!("]");
    }

    // === SUMMARY ===
    println!("\n💡 Key Concepts:");
    println!("   • Block codes operate on fixed-size chunks");
    println!("   • Rate k/n determines bandwidth efficiency");
    println!("   • Systematic encoding preserves message bits");
    println!("   • Syndrome checks detect (and help correct) errors");
    println!("   • Higher redundancy (lower rate) → better correction");
}
