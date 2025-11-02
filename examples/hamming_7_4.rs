//! Hamming (7,4) Code Example
//!
//! This example demonstrates the use of BitVec and BitMatrix for implementing
//! the Hamming (7,4) error-correcting code. This code encodes 4 data bits into
//! 7 bits by adding 3 parity bits, and can detect and correct single-bit errors.
//!
//! The example includes:
//! - Encoding messages using the generator matrix
//! - Simulating transmission through a Binary Symmetric Channel (BSC)
//! - Decoding and error correction using the parity-check matrix
//!
//! This demonstrates the complete flow:
//! message → encode → channel → received → decode
//!
//! The generator matrix G is 4x7 and the parity-check matrix H is 3x7.
//! For standard Hamming (7,4):
//!
//! Generator matrix G (4x7):
//! ```text
//!   ┌             ┐
//!   │ 1 0 0 0 1 1 0 │
//!   │ 0 1 0 0 1 0 1 │
//!   │ 0 0 1 0 0 1 1 │
//!   │ 0 0 0 1 1 1 1 │
//!   └             ┘
//! ```
//!
//! Parity-check matrix H (3x7):
//! ```text
//!   ┌             ┐
//!   │ 1 1 0 1 1 0 0 │
//!   │ 1 0 1 1 0 1 0 │
//!   │ 0 1 1 1 0 0 1 │
//!   └             ┘
//! ```

use gf2::alg::m4rm::multiply;
use gf2::matrix::BitMatrix;
use gf2::BitVec;
use rand::Rng;

/// Creates the generator matrix G for Hamming (7,4) code
fn create_generator_matrix() -> BitMatrix {
    gf2::bitmatrix![
        1, 0, 0, 0, 1, 1, 0;
        0, 1, 0, 0, 1, 0, 1;
        0, 0, 1, 0, 0, 1, 1;
        0, 0, 0, 1, 1, 1, 1;
    ]
}

/// Creates the parity-check matrix H for Hamming (7,4) code
fn create_parity_check_matrix() -> BitMatrix {
    gf2::bitmatrix![
        1, 1, 0, 1, 1, 0, 0;
        1, 0, 1, 1, 0, 1, 0;
        0, 1, 1, 1, 0, 0, 1;
    ]
}

/// Encodes a 4-bit message using the generator matrix
fn encode(message: &BitVec, g: &BitMatrix) -> BitVec {
    // Convert message to 1x4 matrix
    let mut msg_matrix = BitMatrix::new_zero(1, 4);
    for i in 0..4 {
        msg_matrix.set(0, i, message.get(i));
    }

    // Multiply: codeword = message × G
    let codeword_matrix = multiply(&msg_matrix, g);

    // Extract result as BitVec
    let mut codeword = BitVec::new();
    for i in 0..7 {
        codeword.push_bit(codeword_matrix.get(0, i));
    }

    codeword
}

/// Computes the syndrome of a received codeword
fn syndrome(received: &BitVec, h: &BitMatrix) -> BitVec {
    // Convert received to 7x1 column vector (transposed)
    let mut received_matrix = BitMatrix::new_zero(7, 1);
    for i in 0..7 {
        received_matrix.set(i, 0, received.get(i));
    }

    // Compute syndrome: s = H × received^T
    let syndrome_matrix = multiply(h, &received_matrix);

    // Extract syndrome as BitVec
    let mut s = BitVec::new();
    for i in 0..3 {
        s.push_bit(syndrome_matrix.get(i, 0));
    }

    s
}

/// Decodes a received 7-bit codeword, correcting single-bit errors
fn decode(received: &BitVec, h: &BitMatrix) -> BitVec {
    let s = syndrome(received, h);

    // Check if syndrome is zero (no error)
    let mut corrected = received.clone();
    if s.count_ones() > 0 {
        // Non-zero syndrome indicates an error
        // For Hamming (7,4), the syndrome directly gives the error position
        // We need to find which column of H matches the syndrome
        for i in 0..7 {
            let mut col = BitVec::new();
            for j in 0..3 {
                col.push_bit(h.get(j, i));
            }
            if col == s {
                // Flip bit at position i
                corrected.set(i, !corrected.get(i));
                println!("  Error detected and corrected at position {}", i);
                break;
            }
        }
    } else {
        println!("  No errors detected");
    }

    // Extract the first 4 bits (the data bits)
    let mut decoded = BitVec::new();
    for i in 0..4 {
        decoded.push_bit(corrected.get(i));
    }

    decoded
}

/// Simulates a binary symmetric channel (BSC) with error probability p.
///
/// Each bit in the codeword is flipped independently with probability `error_prob`.
/// This models a noisy communication channel where transmission errors occur randomly.
///
/// # Arguments
///
/// * `codeword` - The bit vector to transmit through the channel
/// * `error_prob` - Probability of bit flip (0.0 to 1.0)
///
/// # Returns
///
/// A new bit vector representing the received codeword after channel transmission
///
/// # Panics
///
/// Panics if `error_prob` is not in the range [0.0, 1.0]
fn binary_symmetric_channel(codeword: &BitVec, error_prob: f64) -> BitVec {
    assert!(
        (0.0..=1.0).contains(&error_prob),
        "error_prob must be between 0.0 and 1.0"
    );

    let mut rng = rand::thread_rng();
    let mut received = codeword.clone();

    for i in 0..received.len() {
        // Flip bit with probability error_prob
        if rng.gen::<f64>() < error_prob {
            received.set(i, !received.get(i));
        }
    }

    received
}

fn main() {
    println!("=== Hamming (7,4) Error-Correcting Code Demo ===\n");

    // Create the generator and parity-check matrices
    let g = create_generator_matrix();
    let h = create_parity_check_matrix();

    println!("Generator Matrix G (4x7):");
    println!("{}\n", g);

    println!("Parity-Check Matrix H (3x7):");
    println!("{}\n", h);

    // Example 1: Encode and decode without errors
    println!("--- Example 1: Encoding and decoding without errors ---");
    let mut message1 = BitVec::new();
    message1.push_bit(true);
    message1.push_bit(false);
    message1.push_bit(true);
    message1.push_bit(false);
    println!("Message:  {}", message1);

    let codeword1 = encode(&message1, &g);
    println!("Encoded:  {}", codeword1);

    let decoded1 = decode(&codeword1, &h);
    println!("Decoded:  {}", decoded1);
    assert_eq!(message1, decoded1);
    println!("✓ Decoding successful!\n");

    // Example 2: Introduce a single-bit error and correct it
    println!("--- Example 2: Single-bit error correction ---");
    let mut message2 = BitVec::new();
    message2.push_bit(true);
    message2.push_bit(true);
    message2.push_bit(false);
    message2.push_bit(true);
    println!("Message:  {}", message2);

    let mut codeword2 = encode(&message2, &g);
    println!("Encoded:  {}", codeword2);

    // Introduce an error at position 2
    println!("Corrupting bit at position 2...");
    codeword2.set(2, !codeword2.get(2));
    println!("Received: {}", codeword2);

    let decoded2 = decode(&codeword2, &h);
    println!("Decoded:  {}", decoded2);
    assert_eq!(message2, decoded2);
    println!("✓ Error corrected successfully!\n");

    // Example 3: Another message
    println!("--- Example 3: Another encoding/decoding example ---");
    let mut message3 = BitVec::new();
    message3.push_bit(false);
    message3.push_bit(false);
    message3.push_bit(false);
    message3.push_bit(true);
    println!("Message:  {}", message3);

    let codeword3 = encode(&message3, &g);
    println!("Encoded:  {}", codeword3);

    // No error this time
    let decoded3 = decode(&codeword3, &h);
    println!("Decoded:  {}", decoded3);
    assert_eq!(message3, decoded3);
    println!("✓ Decoding successful!\n");

    // Example 4: Error at a different position
    println!("--- Example 4: Error correction at position 5 ---");
    let mut message4 = BitVec::new();
    message4.push_bit(true);
    message4.push_bit(true);
    message4.push_bit(true);
    message4.push_bit(true);
    println!("Message:  {}", message4);

    let mut codeword4 = encode(&message4, &g);
    println!("Encoded:  {}", codeword4);

    // Introduce an error at position 5
    println!("Corrupting bit at position 5...");
    codeword4.set(5, !codeword4.get(5));
    println!("Received: {}", codeword4);

    let decoded4 = decode(&codeword4, &h);
    println!("Decoded:  {}", decoded4);
    assert_eq!(message4, decoded4);
    println!("✓ Error corrected successfully!\n");

    // Example 5: Using Binary Symmetric Channel
    println!("--- Example 5: Binary Symmetric Channel with p=0.1 ---");
    let mut message5 = BitVec::new();
    message5.push_bit(true);
    message5.push_bit(false);
    message5.push_bit(true);
    message5.push_bit(true);
    println!("Message:         {}", message5);

    let codeword5 = encode(&message5, &g);
    println!("Encoded:         {}", codeword5);

    // Pass through BSC with 10% error probability
    let received5 = binary_symmetric_channel(&codeword5, 0.1);
    println!("After Channel:   {}", received5);

    // Check if any errors occurred
    let mut errors = Vec::new();
    for i in 0..7 {
        if codeword5.get(i) != received5.get(i) {
            errors.push(i);
        }
    }
    if !errors.is_empty() {
        println!("Channel introduced errors at positions: {:?}", errors);
    } else {
        println!("Channel transmitted without errors");
    }
    let decoded5 = decode(&received5, &h);
    println!("Decoded:         {}", decoded5);
    if message5 == decoded5 {
        println!("✓ Message recovered successfully!\n");
    } else {
        println!("✗ Decoding failed (too many errors)\n");
    }

    // Example 6: Multiple transmissions through BSC
    println!("--- Example 6: Multiple transmissions through BSC (p=0.15) ---");
    let mut message6 = BitVec::new();
    message6.push_bit(false);
    message6.push_bit(true);
    message6.push_bit(true);
    message6.push_bit(false);
    println!("Original Message: {}", message6);

    let codeword6 = encode(&message6, &g);
    println!("Encoded:          {}", codeword6);
    
    let mut successes = 0;
    let num_trials = 10;
    println!("\nTransmitting {} times through BSC with p=0.15:", num_trials);
    for trial in 1..=num_trials {
        let received = binary_symmetric_channel(&codeword6, 0.15);
        let decoded = decode(&received, &h);
        
        let mut error_positions = Vec::new();
        for i in 0..7 {
            if codeword6.get(i) != received.get(i) {
                error_positions.push(i);
            }
        }
        
        let success = message6 == decoded;
        if success {
            successes += 1;
        }
        
        print!("  Trial {:2}: ", trial);
        if error_positions.is_empty() {
            print!("No errors");
        } else if error_positions.len() == 1 {
            print!("1 error at pos {}", error_positions[0]);
        } else {
            print!("{} errors at pos {:?}", error_positions.len(), error_positions);
        }
        
        if success {
            println!(" → ✓ Decoded successfully");
        } else {
            println!(" → ✗ Decoding failed");
        }
    }
    println!("\nSuccess rate: {}/{} ({:.1}%)\n", successes, num_trials, (successes as f64 / num_trials as f64) * 100.0);

    println!("=== All examples completed successfully! ===");
}
