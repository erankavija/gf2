//! Basic DVB-T2 LDPC code construction and validation.
//!
//! This example demonstrates creating DVB-T2 LDPC codes and verifying
//! their basic properties.

use gf2_coding::ldpc::LdpcCode;
use gf2_coding::CodeRate;
use gf2_core::BitVec;

fn main() {
    println!("DVB-T2 LDPC Code Examples\n");
    println!("=========================\n");

    // Normal frame rate 1/2
    println!("Normal Frame Rate 1/2:");
    let code = LdpcCode::dvb_t2_normal(CodeRate::Rate1_2);
    println!("  Codeword length n: {}", code.n());
    println!("  Information bits k: {}", code.k());
    println!("  Parity bits m: {}", code.m());
    println!("  Code rate: {:.3}", code.rate());

    // Verify zero codeword
    let zero_cw = BitVec::zeros(code.n());
    assert!(code.is_valid_codeword(&zero_cw));
    println!("  ✓ Zero codeword passes syndrome check\n");

    // Show other configurations (placeholders)
    println!("Other DVB-T2 Configurations:");
    println!("  Normal Rate 3/5: n=64800, k=38880 (placeholder)");
    println!("  Normal Rate 2/3: n=64800, k=43200 (placeholder)");
    println!("  Normal Rate 3/4: n=64800, k=48600 (placeholder)");
    println!("  Normal Rate 4/5: n=64800, k=51840 (placeholder)");
    println!("  Normal Rate 5/6: n=64800, k=54000 (placeholder)");
    println!();
    println!("  Short Rate 1/2:  n=16200, k=7200  (placeholder)");
    println!("  Short Rate 3/5:  n=16200, k=9720  (placeholder)");
    println!("  Short Rate 2/3:  n=16200, k=10800 (placeholder)");
    println!("  Short Rate 3/4:  n=16200, k=11880 (placeholder)");
    println!("  Short Rate 4/5:  n=16200, k=12600 (placeholder)");
    println!("  Short Rate 5/6:  n=16200, k=13320 (placeholder)");
    println!();
    println!("Note: Only Normal Rate 1/2 table is currently implemented.");
    println!("Other configurations will panic until tables are added.");
}
