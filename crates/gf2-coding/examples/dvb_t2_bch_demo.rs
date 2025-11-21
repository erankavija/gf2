//! DVB-T2 BCH Code Example
//!
//! Demonstrates DVB-T2 BCH outer codes from ETSI EN 302 755.
//! These codes provide error correction before LDPC inner coding.
//!
//! ⚠️  WARNING: This implementation requires verification against
//! reference test vectors from the DVB-T2 standard or an independent
//! implementation. Mathematical correctness is verified, but standard
//! compliance is not yet confirmed.

use gf2_coding::bch::dvb_t2::FrameSize;
use gf2_coding::bch::{BchCode, BchDecoder, BchEncoder};
use gf2_coding::traits::{BlockEncoder, HardDecisionDecoder};
use gf2_coding::CodeRate;
use gf2_core::BitVec;

fn main() {
    println!("DVB-T2 BCH Outer Code Example");
    println!("=============================\n");

    // Short frame, rate 1/2
    demo_configuration(FrameSize::Short, CodeRate::Rate1_2);

    println!();

    // Normal frame, rate 1/2
    demo_configuration(FrameSize::Normal, CodeRate::Rate1_2);

    println!();

    // Normal frame, rate 2/3 (uses t=10 instead of t=12)
    demo_configuration(FrameSize::Normal, CodeRate::Rate2_3);
}

fn demo_configuration(frame_size: FrameSize, rate: CodeRate) {
    println!("Configuration: {:?} Frame, Rate {:?}", frame_size, rate);

    // Create BCH code
    let code = BchCode::dvb_t2(frame_size, rate);

    println!("  BCH parameters:");
    println!("    n (output) = {} (= k_ldpc, input to LDPC)", code.n());
    println!("    k (input)  = {} (= Kbch, user data)", code.k());
    println!(
        "    m (parity) = {} (BCH error correction bits)",
        code.n() - code.k()
    );
    println!("    t          = {} (correctable errors)", code.t());

    // Create encoder/decoder
    let encoder = BchEncoder::new(code.clone());
    let decoder = BchDecoder::new(code.clone());

    // Test: Simple roundtrip without errors
    let mut rng = rand::thread_rng();
    let message = BitVec::random(code.k(), &mut rng);
    let codeword = encoder.encode(&message);
    let decoded = decoder.decode(&codeword);

    if decoded == message {
        println!("  ✓ Roundtrip without errors successful");
    } else {
        println!("  ✗ Roundtrip FAILED - decoder not working correctly");
        println!("  ⚠️  This confirms the need for verification!");
        return;
    }

    println!();
    println!("  ⚠️  VERIFICATION REQUIRED:");
    println!("     - Generator polynomials match ETSI EN 302 755 standard");
    println!("     - Encoder/decoder algorithms are standard BCH");
    println!("     - But: No reference test vectors available for validation");
    println!("     - Decoding with errors may fail due to implementation issues");
    println!();
    println!("  Recommended: Verify against commercial DVB-T2 implementation");
}
