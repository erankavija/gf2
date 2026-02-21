//! Your First Error-Correcting Code
//!
//! **Difficulty**: 🟢 Beginner  
//! **Estimated Time**: 5 minutes
//!
//! This example demonstrates the fundamental concept of error correction:
//! sending a message through a noisy channel and recovering the original data.
//!
//! We'll use Hamming(7,4) - a classic code that can correct 1-bit errors.
//!
//! ## Learning Objectives
//!
//! - Understand what error correction does (one concrete example)
//! - See the complete pipeline: encode → corrupt → correct → verify
//! - Experience the "aha moment" of automatic error correction
//! - Learn that error correction works for ANY single-bit error
//!
//! ## Next Steps
//!
//! After this example, try:
//! - [`block_code_intro.rs`](block_code_intro.html) - Learn about block code parameters
//! - [`hamming_7_4.rs`](hamming_7_4.html) - Advanced dive into syndrome decoding

use gf2_coding::traits::{BlockEncoder, HardDecisionDecoder};
use gf2_coding::{LinearBlockCode, SyndromeTableDecoder};
use gf2_core::BitVec;

fn main() {
    println!("=== Your First Error-Correcting Code ===\n");

    // Step 1: Create a Hamming(7,4) code
    // - Can encode 4 bits of data into 7-bit codewords
    // - Automatically corrects any single bit error
    let code = LinearBlockCode::hamming(3);
    let decoder = SyndromeTableDecoder::new(code.clone());
    println!(
        "Created Hamming(7,4) code: {} data bits → {} codeword bits",
        code.k(),
        code.n()
    );

    // Step 2: Prepare a message
    let mut message = BitVec::zeros(4);
    message.set(0, true); // Set bit 0
    message.set(2, true); // Set bit 2
                          // Result: 1010 in binary
    print!("\n📤 Original message: [");
    for i in 0..message.len() {
        print!("{}", if message.get(i) { "1" } else { "0" });
    }
    println!("]");

    // Step 3: Encode (add redundancy for error correction)
    let codeword = code.encode(&message);
    print!("✅ Encoded codeword: [");
    for i in 0..codeword.len() {
        print!("{}", if codeword.get(i) { "1" } else { "0" });
    }
    println!("] (added {} parity bits)", code.n() - code.k());

    // Step 4: Simulate transmission error
    let mut received = codeword.clone();
    let error_position = 2;
    received.set(error_position, !received.get(error_position)); // Flip bit 2

    print!("\n⚠️  Corrupted (bit {} flipped): [", error_position);
    for i in 0..received.len() {
        print!("{}", if received.get(i) { "1" } else { "0" });
    }
    println!("]");
    println!(
        "   Errors introduced: {} bit(s) changed",
        (0..received.len())
            .filter(|&i| received.get(i) != codeword.get(i))
            .count()
    );

    // Step 5: Decode and correct
    let decoded = decoder.decode(&received);
    print!("\n🔧 After correction: [");
    for i in 0..decoded.len() {
        print!("{}", if decoded.get(i) { "1" } else { "0" });
    }
    println!("]");

    // Step 6: Verify success
    if decoded == message {
        println!("✨ Success! Original message recovered perfectly.");
    } else {
        println!("❌ Decoding failed (too many errors)");
    }

    println!("\n💡 Key insight: The decoder automatically found and fixed the error!");
    println!("   This works for ANY single-bit error in the 7-bit codeword.");

    // Bonus: Test error at different position
    println!("\n--- Testing error at different position ---");
    let mut received2 = codeword.clone();
    received2.set(5, !received2.get(5)); // Flip bit 5 instead
    let decoded2 = decoder.decode(&received2);
    print!("Error at bit 5: [");
    for i in 0..received2.len() {
        print!("{}", if received2.get(i) { "1" } else { "0" });
    }
    println!("]");
    print!("Corrected:      [");
    for i in 0..decoded2.len() {
        print!("{}", if decoded2.get(i) { "1" } else { "0" });
    }
    println!("]");
    if decoded2 == message {
        println!("✓ Also corrected successfully!");
    }
}
