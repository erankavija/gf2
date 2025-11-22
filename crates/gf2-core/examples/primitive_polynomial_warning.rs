//! Demonstrates compile-time warnings for non-standard primitive polynomials.

use gf2_core::gf2m::Gf2mField;

fn main() {
    println!("=== Testing Primitive Polynomial Warnings ===\n");
    
    println!("1. Creating field with STANDARD polynomial (no warning):");
    let _field1 = Gf2mField::new(14, 0b100000000101011);
    println!("   ✓ GF(2^14) created successfully\n");
    
    println!("2. Creating field with NON-STANDARD polynomial (warning expected):");
    let _field2 = Gf2mField::new(14, 0b100000000100001);
    println!("   ✓ GF(2^14) created successfully (but with warning above)\n");
    
    println!("3. Creating field with UNKNOWN degree (no warning):");
    let _field3 = Gf2mField::new(31, 0b10000000000000001001);
    println!("   ✓ GF(2^31) created successfully\n");
    
    println!("=== Summary ===");
    println!("Standard polynomials: no warning");
    println!("Non-standard polynomials: warning with standard reference");
    println!("Unknown degrees: no warning (not in database)");
}
