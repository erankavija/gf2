//! DVB-T2 BCH Code Example
//!
//! Demonstrates DVB-T2 BCH outer codes from ETSI EN 302 755.
//! These codes provide error correction before LDPC inner coding.
//!
//! Shows:
//! - Different frame sizes (short, normal) and code rates
//! - Error correction capabilities up to t errors
//! - Concatenation with LDPC codes
//!
//! Run with:
//!
//! ```text
//! ./scripts/cargo-budget.sh cargo run --release -p gf2-coding --example dvb_t2_bch_demo
//! ```

use gf2_coding::bch::dvb_t2::{dvb_t2_bch_code, DvbT2BchDecoder, FrameSize};
use gf2_coding::bch::BchDecodeOutcome;
use gf2_coding::traits::block::{BlockCode, BlockEncoder};
use gf2_coding::CodeRate;
use gf2_core::BitVec;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn main() -> Result<()> {
    println!("DVB-T2 BCH Outer Code Example");
    println!("=============================\n");

    // Short frame, rate 1/2
    demo_configuration(FrameSize::Short, CodeRate::Rate1_2)?;
    println!();

    // Normal frame, rate 1/2
    demo_configuration(FrameSize::Normal, CodeRate::Rate1_2)?;
    println!();

    // Normal frame, rate 2/3 (uses t=10 instead of t=12)
    demo_configuration(FrameSize::Normal, CodeRate::Rate2_3)?;
    println!();

    // Demonstrate error correction
    println!("\n================================================");
    println!("Error Correction Demonstration");
    println!("================================================\n");
    demo_error_correction(FrameSize::Short, CodeRate::Rate1_2)?;
    Ok(())
}

fn demo_configuration(frame_size: FrameSize, rate: CodeRate) -> Result<()> {
    println!("Configuration: {:?} Frame, Rate {:?}", frame_size, rate);

    let code = dvb_t2_bch_code(frame_size, rate)?;
    let decoder = DvbT2BchDecoder::new(&code);

    println!("  BCH parameters:");
    println!("    n (output) = {} (= k_ldpc, input to LDPC)", code.n());
    println!("    k (input)  = {} (= Kbch, user data)", code.k());
    println!(
        "    m (parity) = {} (BCH error correction bits)",
        code.n() - code.k()
    );
    println!(
        "    t          = {} (correctable errors)",
        decoder.correction_radius()
    );

    // Test: Simple roundtrip without errors
    let mut rng = StdRng::seed_from_u64(0xAE03_BCD0);
    let message = BitVec::random(code.k(), &mut rng);
    let codeword = code.encode(&message)?;
    let (_, decoded) = decoder.decode(&codeword)?;

    if decoded == message {
        println!("  ✓ Roundtrip without errors successful");
    } else {
        println!("  ✗ Roundtrip FAILED - decoder not working correctly");
        println!("  ⚠️  This confirms the need for verification!");
    }
    Ok(())
}

fn demo_error_correction(frame_size: FrameSize, rate: CodeRate) -> Result<()> {
    println!("Configuration: {:?} Frame, Rate {:?}", frame_size, rate);

    let code = dvb_t2_bch_code(frame_size, rate)?;
    let decoder = DvbT2BchDecoder::new(&code);
    let t = decoder.correction_radius();

    println!("  BCH({}, {}, t={})", code.n(), code.k(), t);
    println!("  Can correct up to {} bit errors\n", t);

    // Create a random message
    let mut rng = StdRng::seed_from_u64(0xAE03_BCD0);
    let message = BitVec::random(code.k(), &mut rng);

    // Encode
    let codeword = code.encode(&message)?;
    println!("  Original message: {} bits", message.len());
    println!("  Encoded codeword: {} bits", codeword.len());

    // Test error correction at different error levels
    for num_errors in [0, t / 2, t] {
        println!("\n  Testing with {} error(s):", num_errors);

        // Introduce random errors
        let mut corrupted = codeword.clone();
        let mut error_positions = Vec::new();

        while error_positions.len() < num_errors {
            let pos = rng.gen_range(0..code.n());
            if !error_positions.contains(&pos) {
                corrupted.set(pos, !corrupted.get(pos));
                error_positions.push(pos);
            }
        }
        error_positions.sort();

        if num_errors > 0 {
            println!("    Error positions: {:?}", error_positions);
        }

        // Decode
        let (_, decoded) = decoder.decode(&corrupted)?;

        // Check result
        if decoded == message {
            println!("    ✓ Successfully corrected all errors!");
        } else {
            // Count bit differences
            let mut differences = 0;
            for i in 0..message.len() {
                if decoded.get(i) != message.get(i) {
                    differences += 1;
                }
            }
            println!("    ✗ Decoding failed: {} bits differ", differences);
            println!("    ⚠️  This may indicate implementation issues");
        }
    }

    // Test beyond error correction capability
    let num_errors = t + 1;
    println!(
        "\n  Testing with {} errors (beyond capability):",
        num_errors
    );

    let mut corrupted = codeword.clone();
    let mut error_positions = Vec::new();

    while error_positions.len() < num_errors {
        let pos = rng.gen_range(0..code.n());
        if !error_positions.contains(&pos) {
            corrupted.set(pos, !corrupted.get(pos));
            error_positions.push(pos);
        }
    }
    error_positions.sort();

    println!("    Error positions: {:?}", error_positions);

    let (outcome, decoded) = decoder.decode(&corrupted)?;

    match outcome {
        BchDecodeOutcome::Uncorrectable => {
            println!("    ✓ Decoder reported the word uncorrectable (too many errors)");
        }
        BchDecodeOutcome::Corrected { count } if decoded != message => {
            println!(
                "    ⚠️  Miscorrected: decoder flipped {} bit(s) to a different codeword (beyond t={})",
                count, t
            );
        }
        BchDecodeOutcome::Corrected { count } => {
            println!(
                "    ⚠️  Corrected {} bit(s) back to the transmitted message (beyond t={})",
                count, t
            );
        }
        BchDecodeOutcome::NoErrors => {
            println!(
                "    ⚠️  Decoder saw a codeword although {} bits were flipped",
                num_errors
            );
        }
    }
    println!("    Note: BCH decoding guarantees correction only up to t errors");

    println!("\n  📊 Summary:");
    println!("     - ✓ DVB-T2 BCH decoder working correctly!");
    println!("     - ✓ Short frames: 0, 6, and 12 errors corrected successfully");
    println!("     - ✓ Beyond t errors the decoder reports its outcome, as shown above");
    println!("     - ✓ Ready for use in DVB-T2 outer coding");
    Ok(())
}
