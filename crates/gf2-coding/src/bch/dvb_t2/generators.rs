//! DVB-T2 BCH generator polynomials from ETSI EN 302 755.
//!
//! These are the explicit generator polynomials g_1(x) through g_12(x) from the standard.
//! The actual generator polynomial for a t-error correcting code is:
//!
//! g(x) = g_1(x) × g_2(x) × ... × g_t(x)

use gf2_core::gf2m::{Gf2mField, Gf2mPoly};

/// Short frame generator polynomials over GF(2^14).
///
/// From ETSI EN 302 755, these are g_1(x) through g_12(x).
/// The generator for a t-error correcting code is the product of the first t polynomials.
pub const SHORT_GENERATORS: &[&[usize]] = &[
    &[0, 1, 3, 5, 14],
    &[0, 6, 8, 11, 14],
    &[0, 1, 2, 6, 9, 10, 14],
    &[0, 4, 7, 8, 10, 12, 14],
    &[0, 2, 4, 6, 8, 9, 11, 13, 14],
    &[0, 3, 7, 8, 9, 13, 14],
    &[0, 2, 5, 6, 7, 10, 11, 13, 14],
    &[0, 5, 8, 9, 10, 11, 14],
    &[0, 1, 2, 3, 9, 10, 14],
    &[0, 3, 6, 9, 11, 12, 14],
    &[0, 4, 11, 12, 14],
    &[0, 1, 2, 3, 5, 6, 7, 8, 10, 13, 14],
];

/// Normal frame generator polynomials over GF(2^16).
///
/// From ETSI EN 302 755, these are g_1(x) through g_12(x).
/// The generator for a t-error correcting code is the product of the first t polynomials.
pub const NORMAL_GENERATORS: &[&[usize]] = &[
    &[0, 2, 3, 5, 16],
    &[0, 1, 4, 5, 6, 8, 16],
    &[0, 2, 3, 4, 5, 7, 8, 9, 10, 11, 16],
    &[0, 2, 4, 6, 9, 11, 12, 14, 16],
    &[0, 1, 2, 3, 5, 8, 9, 10, 11, 12, 16],
    &[0, 2, 4, 5, 7, 8, 9, 10, 12, 13, 14, 15, 16],
    &[0, 2, 5, 6, 8, 9, 10, 11, 13, 15, 16],
    &[0, 1, 2, 5, 6, 8, 9, 12, 13, 14, 16],
    &[0, 5, 7, 9, 10, 11, 16],
    &[0, 1, 2, 5, 7, 8, 10, 12, 13, 14, 16],
    &[0, 2, 3, 5, 9, 11, 12, 13, 16],
    &[0, 1, 5, 6, 7, 9, 11, 12, 16],
];



/// Computes the product of the first t DVB-T2 generator polynomials.
///
/// # Arguments
///
/// * `field` - The extension field GF(2^m)
/// * `generators` - Array of generator polynomial exponent lists
/// * `t` - Number of polynomials to multiply (error correction capability)
///
/// # Examples
///
/// ```
/// use gf2_core::gf2m::Gf2mField;
/// use gf2_coding::bch::dvb_t2::generators::{product_of_generators, SHORT_GENERATORS};
///
/// let field = Gf2mField::new(14, 0b100000000100001);
/// let g = product_of_generators(&field, SHORT_GENERATORS, 2);
/// // g = g_1 × g_2
/// ```
pub fn product_of_generators(field: &Gf2mField, generators: &[&[usize]], t: usize) -> Gf2mPoly {
    assert!(t > 0, "t must be positive");
    assert!(t <= generators.len(), "t exceeds available generators");

    let mut g = Gf2mPoly::from_exponents(field, generators[0]);

    for gen in generators.iter().take(t).skip(1) {
        let g_i = Gf2mPoly::from_exponents(field, gen);
        g = &g * &g_i;
    }

    g
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_poly_from_exponents_simple() {
        let field = Gf2mField::new(4, 0b10011);
        let poly = Gf2mPoly::from_exponents(&field, &[0, 1, 4]); // 1 + x + x^4

        assert_eq!(poly.degree(), Some(4));
        assert_eq!(poly.coeff(0), field.one());
        assert_eq!(poly.coeff(1), field.one());
        assert_eq!(poly.coeff(2), field.zero());
        assert_eq!(poly.coeff(3), field.zero());
        assert_eq!(poly.coeff(4), field.one());
    }

    #[test]
    fn test_poly_from_exponents_short_g1() {
        let field = Gf2mField::new(14, 0b100000000100001);
        let poly = Gf2mPoly::from_exponents(&field, SHORT_GENERATORS[0]);

        // g_1(x) = 1 + x + x^3 + x^5 + x^14
        assert_eq!(poly.degree(), Some(14));
        assert_eq!(poly.coeff(0), field.one());
        assert_eq!(poly.coeff(1), field.one());
        assert_eq!(poly.coeff(2), field.zero());
        assert_eq!(poly.coeff(3), field.one());
        assert_eq!(poly.coeff(4), field.zero());
        assert_eq!(poly.coeff(5), field.one());
        assert_eq!(poly.coeff(14), field.one());
    }

    #[test]
    fn test_product_single_polynomial() {
        let field = Gf2mField::new(14, 0b100000000100001);
        let g = product_of_generators(&field, SHORT_GENERATORS, 1);
        let g1 = Gf2mPoly::from_exponents(&field, SHORT_GENERATORS[0]);

        assert_eq!(g, g1);
    }

    #[test]
    fn test_product_two_polynomials() {
        let field = Gf2mField::new(14, 0b100000000100001);
        let g = product_of_generators(&field, SHORT_GENERATORS, 2);

        // Should equal g_1 × g_2
        let g1 = Gf2mPoly::from_exponents(&field, SHORT_GENERATORS[0]);
        let g2 = Gf2mPoly::from_exponents(&field, SHORT_GENERATORS[1]);
        let expected = &g1 * &g2;

        assert_eq!(g, expected);
    }

    #[test]
    fn test_product_all_twelve_polynomials() {
        let field = Gf2mField::new(14, 0b100000000100001);
        let g = product_of_generators(&field, SHORT_GENERATORS, 12);

        // Generator for t=12 should be product of all 12 polynomials
        // Degree should be sum of individual degrees
        let total_degree: usize = SHORT_GENERATORS[..12]
            .iter()
            .map(|g| g.iter().max().unwrap())
            .sum();

        assert_eq!(g.degree(), Some(total_degree));
    }

    #[test]
    fn test_short_generators_count() {
        assert_eq!(SHORT_GENERATORS.len(), 12);
    }

    #[test]
    fn test_normal_generators_count() {
        assert_eq!(NORMAL_GENERATORS.len(), 12);
    }

    #[test]
    fn test_all_short_generators_degree_14() {
        for (i, gen) in SHORT_GENERATORS.iter().enumerate() {
            let max_exp = gen.iter().max().unwrap();
            assert_eq!(*max_exp, 14, "g_{} should have degree 14", i + 1);
        }
    }

    #[test]
    fn test_all_normal_generators_degree_16() {
        for (i, gen) in NORMAL_GENERATORS.iter().enumerate() {
            let max_exp = gen.iter().max().unwrap();
            assert_eq!(*max_exp, 16, "g_{} should have degree 16", i + 1);
        }
    }

    #[test]
    #[should_panic(expected = "t must be positive")]
    fn test_product_zero_t_panics() {
        let field = Gf2mField::new(14, 0b100000000100001);
        product_of_generators(&field, SHORT_GENERATORS, 0);
    }

    #[test]
    #[should_panic(expected = "t exceeds available generators")]
    fn test_product_exceeds_generators_panics() {
        let field = Gf2mField::new(14, 0b100000000100001);
        product_of_generators(&field, SHORT_GENERATORS, 13);
    }
}
