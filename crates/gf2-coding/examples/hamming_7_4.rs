//! Hamming (7,4) encoding and syndrome decoding with explicit generator and parity-check matrices
//! over a binary symmetric channel.

use gf2_core::BitMatrix;
use gf2_core::BitVec;
use rand::Rng;

fn create_generator_matrix() -> BitMatrix {
    gf2_core::bitmatrix![
        1, 0, 0, 0, 1, 1, 0;
        0, 1, 0, 0, 1, 0, 1;
        0, 0, 1, 0, 0, 1, 1;
        0, 0, 0, 1, 1, 1, 1;
    ]
}

fn create_parity_check_matrix() -> BitMatrix {
    gf2_core::bitmatrix![
        1, 1, 0, 1, 1, 0, 0;
        1, 0, 1, 1, 0, 1, 0;
        0, 1, 1, 1, 0, 0, 1;
    ]
}

fn encode(message: &BitVec, g: &BitMatrix) -> BitVec {
    let mut msg_matrix = BitMatrix::zeros(1, 4);
    for i in 0..4 {
        msg_matrix.set(0, i, message.get(i));
    }

    let codeword_matrix = &msg_matrix * g;

    let mut codeword = BitVec::new();
    for i in 0..7 {
        codeword.push_bit(codeword_matrix.get(0, i));
    }

    codeword
}

fn syndrome(received: &BitVec, h: &BitMatrix) -> BitVec {
    let mut received_matrix = BitMatrix::zeros(7, 1);
    for i in 0..7 {
        received_matrix.set(i, 0, received.get(i));
    }

    let syndrome_matrix = h * &received_matrix;

    let mut s = BitVec::new();
    for i in 0..3 {
        s.push_bit(syndrome_matrix.get(i, 0));
    }

    s
}

fn decode(received: &BitVec, h: &BitMatrix) -> BitVec {
    let s = syndrome(received, h);

    let mut corrected = received.clone();
    if s.count_ones() > 0 {
        // A single error at position i gives the syndrome equal to column i of H.
        for i in 0..7 {
            let mut col = BitVec::new();
            for j in 0..3 {
                col.push_bit(h.get(j, i));
            }
            if col == s {
                corrected.set(i, !corrected.get(i));
                println!("  Error detected and corrected at position {}", i);
                break;
            }
        }
    } else {
        println!("  No errors detected");
    }

    let mut decoded = BitVec::new();
    for i in 0..4 {
        decoded.push_bit(corrected.get(i));
    }

    decoded
}

/// Flips each bit independently with probability `error_prob`; panics unless it lies in [0, 1].
fn binary_symmetric_channel(codeword: &BitVec, error_prob: f64) -> BitVec {
    assert!(
        (0.0..=1.0).contains(&error_prob),
        "error_prob must be between 0.0 and 1.0"
    );

    let mut rng = rand::thread_rng();
    let mut received = codeword.clone();

    for i in 0..received.len() {
        if rng.gen::<f64>() < error_prob {
            received.set(i, !received.get(i));
        }
    }

    received
}

fn main() {
    println!("=== Hamming (7,4) Error-Correcting Code Demo ===\n");

    let g = create_generator_matrix();
    let h = create_parity_check_matrix();

    println!("Generator Matrix G (4x7):");
    println!("{}\n", g);

    println!("Parity-Check Matrix H (3x7):");
    println!("{}\n", h);

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

    println!("--- Example 2: Single-bit error correction ---");
    let mut message2 = BitVec::new();
    message2.push_bit(true);
    message2.push_bit(true);
    message2.push_bit(false);
    message2.push_bit(true);
    println!("Message:  {}", message2);

    let mut codeword2 = encode(&message2, &g);
    println!("Encoded:  {}", codeword2);

    println!("Corrupting bit at position 2...");
    codeword2.set(2, !codeword2.get(2));
    println!("Received: {}", codeword2);

    let decoded2 = decode(&codeword2, &h);
    println!("Decoded:  {}", decoded2);
    assert_eq!(message2, decoded2);
    println!("✓ Error corrected successfully!\n");

    println!("--- Example 3: Another encoding/decoding example ---");
    let mut message3 = BitVec::new();
    message3.push_bit(false);
    message3.push_bit(false);
    message3.push_bit(false);
    message3.push_bit(true);
    println!("Message:  {}", message3);

    let codeword3 = encode(&message3, &g);
    println!("Encoded:  {}", codeword3);

    let decoded3 = decode(&codeword3, &h);
    println!("Decoded:  {}", decoded3);
    assert_eq!(message3, decoded3);
    println!("✓ Decoding successful!\n");

    println!("--- Example 4: Error correction at position 5 ---");
    let mut message4 = BitVec::new();
    message4.push_bit(true);
    message4.push_bit(true);
    message4.push_bit(true);
    message4.push_bit(true);
    println!("Message:  {}", message4);

    let mut codeword4 = encode(&message4, &g);
    println!("Encoded:  {}", codeword4);

    println!("Corrupting bit at position 5...");
    codeword4.set(5, !codeword4.get(5));
    println!("Received: {}", codeword4);

    let decoded4 = decode(&codeword4, &h);
    println!("Decoded:  {}", decoded4);
    assert_eq!(message4, decoded4);
    println!("✓ Error corrected successfully!\n");

    println!("--- Example 5: Binary Symmetric Channel with p=0.1 ---");
    let mut message5 = BitVec::new();
    message5.push_bit(true);
    message5.push_bit(false);
    message5.push_bit(true);
    message5.push_bit(true);
    println!("Message:         {}", message5);

    let codeword5 = encode(&message5, &g);
    println!("Encoded:         {}", codeword5);

    let received5 = binary_symmetric_channel(&codeword5, 0.1);
    println!("After Channel:   {}", received5);

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
    println!(
        "\nTransmitting {} times through BSC with p=0.15:",
        num_trials
    );
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
            print!(
                "{} errors at pos {:?}",
                error_positions.len(),
                error_positions
            );
        }

        if success {
            println!(" → ✓ Decoded successfully");
        } else {
            println!(" → ✗ Decoding failed");
        }
    }
    println!(
        "\nSuccess rate: {}/{} ({:.1}%)\n",
        successes,
        num_trials,
        (successes as f64 / num_trials as f64) * 100.0
    );

    println!("=== All examples completed successfully! ===");
}
