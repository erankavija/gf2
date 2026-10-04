//! `Gf2mField::new` constructs degree-14 and degree-31 fields without
//! panicking.

use gf2_core::gf2m::Gf2mField;

#[test]
fn test_standard_polynomial_no_warning() {
    let _field = Gf2mField::new(14, 0b100000000101011);
}

#[test]
fn test_non_standard_polynomial_warns() {
    let _field = Gf2mField::new(14, 0b100000000100001);
}

#[test]
fn test_unknown_degree_no_warning() {
    let _field = Gf2mField::new(31, 0b10000000000000001001);
}
