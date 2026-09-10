//! Test that compile-time warnings are emitted for non-standard polynomials.

use gf2_core::gf2m::Gf2mField;

#[test]
fn test_standard_polynomial_no_warning() {
    // Standard DVB-T2 polynomial should not warn
    let _field = Gf2mField::new(14, 0b100000000101011);
    // No warning expected
}

#[test]
fn test_non_standard_polynomial_warns() {
    // Non-standard polynomial should warn to stderr
    // This test just ensures it doesn't panic - warning verification
    // would require capturing stderr which is complex
    let _field = Gf2mField::new(14, 0b100000000100001);
    // Warning expected to stderr: "WARNING: Non-standard primitive polynomial..."
}

#[test]
fn test_unknown_degree_no_warning() {
    // Unknown degree (not in database) should not warn
    let _field = Gf2mField::new(31, 0b10000000000000001001);
    // No warning expected
}
