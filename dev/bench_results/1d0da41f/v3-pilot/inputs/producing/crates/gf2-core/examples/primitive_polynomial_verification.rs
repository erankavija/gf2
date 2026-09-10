//! Example demonstrating primitive polynomial verification.

use gf2_core::gf2m::Gf2mField;

fn main() {
    println!("=== Primitive Polynomial Verification Demo ===\n");

    // Example 1: DVB-T2 standard polynomial (correct)
    println!("1. DVB-T2 short frame polynomial (x^14 + x^5 + x^3 + x + 1):");
    let gf14_correct = Gf2mField::new(14, 0b100000000101011);
    println!("   Is primitive: {}", gf14_correct.verify_primitive());
    println!("   ✓ This is the CORRECT DVB-T2 polynomial\n");

    // Example 2: Wrong polynomial that caused the bug
    println!("2. Wrong polynomial (x^14 + x^5 + 1):");
    let gf14_wrong = Gf2mField::new(14, 0b100000000100001);
    println!("   Is primitive: {}", gf14_wrong.verify_primitive());
    println!("   ✗ This caused BCH decoding failures in DVB-T2\n");

    // Example 3: GF(256) primitive polynomial
    println!("3. GF(256) polynomial (x^8 + x^4 + x^3 + x^2 + 1):");
    let gf256 = Gf2mField::gf256();
    println!("   Is primitive: {}", gf256.verify_primitive());
    println!("   ✓ Standard primitive polynomial for GF(256)\n");

    // Example 4: Reducible polynomial
    println!("4. Reducible polynomial (x^2 + 1 = (x+1)^2):");
    let reducible = Gf2mField::new(2, 0b101);
    println!("   Is irreducible: {}", reducible.is_irreducible_rabin());
    println!("   Is primitive: {}", reducible.verify_primitive());
    println!("   ✗ Reducible polynomials cannot be primitive\n");

    println!("The verification prevents using wrong polynomials that would cause");
    println!("errors in error-correcting codes like BCH and Reed-Solomon.");
}
