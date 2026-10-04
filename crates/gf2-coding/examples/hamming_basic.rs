//! Single-error correction with the Hamming(7,4) code and a syndrome-table decoder.

use gf2_coding::traits::{BlockEncoder, HardDecisionDecoder};
use gf2_coding::{LinearBlockCode, SyndromeTableDecoder};
use gf2_core::BitVec;

fn main() {
    println!("=== Your First Error-Correcting Code ===\n");

    let code = LinearBlockCode::hamming(3);
    let decoder = SyndromeTableDecoder::new(code.clone());
    println!(
        "Created Hamming(7,4) code: {} data bits → {} codeword bits",
        code.k(),
        code.n()
    );

    let mut message = BitVec::zeros(4);
    message.set(0, true);
    message.set(2, true);
    print!("\n📤 Original message: [");
    for i in 0..message.len() {
        print!("{}", if message.get(i) { "1" } else { "0" });
    }
    println!("]");

    let codeword = code.encode(&message);
    print!("✅ Encoded codeword: [");
    for i in 0..codeword.len() {
        print!("{}", if codeword.get(i) { "1" } else { "0" });
    }
    println!("] (added {} parity bits)", code.n() - code.k());

    let mut received = codeword.clone();
    let error_position = 2;
    received.set(error_position, !received.get(error_position));

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

    let decoded = decoder.decode(&received);
    print!("\n🔧 After correction: [");
    for i in 0..decoded.len() {
        print!("{}", if decoded.get(i) { "1" } else { "0" });
    }
    println!("]");

    if decoded == message {
        println!("✨ Success! Original message recovered perfectly.");
    } else {
        println!("❌ Decoding failed (too many errors)");
    }

    println!("\n💡 Key insight: The decoder automatically found and fixed the error!");
    println!("   This works for ANY single-bit error in the 7-bit codeword.");

    println!("\n--- Testing error at different position ---");
    let mut received2 = codeword.clone();
    received2.set(5, !received2.get(5));
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
