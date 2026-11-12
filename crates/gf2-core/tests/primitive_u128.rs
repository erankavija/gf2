use gf2_core::gf2m::Gf2mField;
use gf2_core::primitive_polys::{is_primitive_u128, PrimitivePolynomialDatabase};

#[test]
fn primitive_verifier_handles_the_u128_degree_limit() {
    let polynomial = PrimitivePolynomialDatabase::standard_u128(127).unwrap();
    assert!(is_primitive_u128(polynomial));
    assert!(!is_primitive_u128(polynomial ^ 1));
}

#[test]
fn primitive_verifier_agrees_with_u64_database_verification() {
    for m in 1..=63 {
        if let Some(polynomial) = PrimitivePolynomialDatabase::standard(m) {
            assert_eq!(
                is_primitive_u128(polynomial as u128),
                Gf2mField::new(m, polynomial).verify_primitive(),
                "degree {m}"
            );
        }
    }
}

#[test]
fn standard_u128_entries_primitivity() {
    let nonprimitive: Vec<_> = (64..=127)
        .filter(|&m| {
            let polynomial = PrimitivePolynomialDatabase::standard_u128(m).unwrap();
            assert_eq!(127 - polynomial.leading_zeros(), m as u32);
            !is_primitive_u128(polynomial)
        })
        .collect();
    assert!(
        nonprimitive.is_empty(),
        "non-primitive degrees: {nonprimitive:?}"
    );
}

#[test]
fn primitive_verifier_matches_exhaustive_small_order_oracle() {
    assert!(!is_primitive_u128(0));
    assert!(!is_primitive_u128(1));
    for m in 1..=8 {
        for polynomial in (1u128 << m)..(1u128 << (m + 1)) {
            let order = (1u128 << m) - 1;
            let mut current = 1;
            let mut first_return = None;
            for exponent in 1..=order {
                current <<= 1;
                if current & (1u128 << m) != 0 {
                    current ^= polynomial;
                }
                if current == 1 {
                    first_return = Some(exponent);
                    break;
                }
            }
            assert_eq!(
                is_primitive_u128(polynomial),
                first_return == Some(order),
                "polynomial {polynomial:#x} of degree {m}"
            );
        }
    }
}
