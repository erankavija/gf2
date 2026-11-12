use super::*;
use std::collections::BTreeSet;

// Overflow-free integer arithmetic for the Lucas certificates, including
// moduli near u128::MAX. Polynomial multiplication uses Gf2mField_::mul_raw.
fn add_mod(a: u128, b: u128, modulus: u128) -> u128 {
    if a >= modulus - b {
        a - (modulus - b)
    } else {
        a + b
    }
}

fn mul_mod(mut a: u128, mut b: u128, modulus: u128) -> u128 {
    let mut result = 0;
    while b != 0 {
        if b & 1 != 0 {
            result = add_mod(result, a, modulus);
        }
        b >>= 1;
        a = add_mod(a, a, modulus);
    }
    result
}

fn pow_mod(mut a: u128, mut exponent: u128, modulus: u128) -> u128 {
    let mut result = 1;
    while exponent != 0 {
        if exponent & 1 != 0 {
            result = mul_mod(result, a, modulus);
        }
        exponent >>= 1;
        a = mul_mod(a, a, modulus);
    }
    result
}

// Lucas's n-1 criterion, @/citation/CrandallPomerance2005, §4.1:
// a^(n-1) = 1 and a^((n-1)/q) != 1 for every prime q | n-1 prove n prime.
// Complete factorizations below reduce recursively to the existing exact
// u64 primality test used by factor_order; no probable-prime assumption.
fn has_lucas_certificate(
    n: u128,
    witness: u128,
    factors: &[u128],
    proven: &mut BTreeSet<u128>,
) -> bool {
    n > 2
        && (2..n).contains(&witness)
        && factors.iter().try_fold(1u128, |a, &b| a.checked_mul(b)) == Some(n - 1)
        && factors.iter().all(|&q| q < n && prove_prime(q, proven))
        && pow_mod(witness, n - 1, n) == 1
        && factors
            .iter()
            .all(|&q| pow_mod(witness, (n - 1) / q, n) != 1)
}

fn prove_prime(n: u128, proven: &mut BTreeSet<u128>) -> bool {
    if proven.contains(&n) {
        return true;
    }
    let prime = if let Ok(small) = u64::try_from(n) {
        crate::field::extension::factor_order(small).as_deref() == Some(&[small])
    } else {
        let Some((witness, factors)) = lucas_certificate(n) else {
            return false;
        };
        has_lucas_certificate(n, witness, factors, proven)
    };
    if prime {
        proven.insert(n);
    }
    prime
}

#[test]
fn order_factor_tables_are_complete_and_prime() {
    let mut proven = BTreeSet::new();
    for (m, factors) in ORDER_FACTORS.iter().enumerate().skip(1) {
        assert_eq!(
            factors.iter().try_fold(1u128, |a, &b| a.checked_mul(b)),
            Some((1u128 << m) - 1),
            "incomplete factorization for degree {m}"
        );
        assert!(factors.windows(2).all(|w| w[0] <= w[1]));
        for &q in *factors {
            assert!(prove_prime(q, &mut proven), "degree {m}: {q} is not prime");
        }
    }
}

#[test]
fn lucas_certificates_reject_composites_and_incomplete_factorizations() {
    let mut proven = BTreeSet::new();
    assert!(has_lucas_certificate(17, 3, &[2, 2, 2, 2], &mut proven));
    assert!(!has_lucas_certificate(17, 3, &[2], &mut proven));
    assert!(!has_lucas_certificate(341, 2, &[2, 2, 5, 17], &mut proven));
    assert!(!has_lucas_certificate(
        (1u128 << 64) + 1,
        3,
        &[2; 64],
        &mut proven
    ));
    for n in [0, 1, 4, 9, 25, 341, 561] {
        assert!(!prove_prime(n, &mut proven));
    }
}

#[test]
fn certificate_mul_mod_covers_the_full_u128_range() {
    for modulus in [(1u128 << 127) - 1, u128::MAX - 1, u128::MAX] {
        for a in [1, 2, 1u128 << 63, 1u128 << 64, 1u128 << 126, modulus - 1] {
            assert_eq!(mul_mod(a, modulus - 1, modulus), modulus - a);
            assert_eq!(mul_mod(modulus - 1, a, modulus), modulus - a);
        }
    }
    for a in 0..37 {
        for b in 0..37 {
            assert_eq!(mul_mod(a, b, 37), a * b % 37);
        }
    }
}

#[test]
fn rejects_reducible_and_irreducible_nonprimitive_across_64() {
    for (m, polynomial, order_divisor) in [
        (8, 0x11b, 5),
        (63, 0x8000_0000_0000_02d3, 7),
        (64, 0x1_0000_0000_0000_008d, 3),
        (65, 0x2_0000_0000_0000_0535, 31),
    ] {
        assert!(is_irreducible_u128(polynomial, m), "degree {m}");
        assert_eq!(
            x_power_mod(((1u128 << m) - 1) / order_divisor, m, polynomial),
            1,
            "degree {m}: x has proper order"
        );
        assert!(!is_primitive_u128(polynomial), "degree {m}");

        // (x^(m-1) + 1)(x + 1), with nonzero constant coefficient.
        let reducible = (1u128 << m) | (1u128 << (m - 1)) | 3;
        assert!(!is_irreducible_u128(reducible, m));
        assert!(!is_primitive_u128(reducible));
    }
}

// Each row is (n, witness, prime factors of n-1 with multiplicity).
fn lucas_certificate(n: u128) -> Option<(u128, &'static [u128])> {
    match n {
        3976656429941438590393 => Some((5, &[2, 2, 2, 3, 103, 149, 4657, 71429, 32456563])),
        57912614113275649087721 => Some((6, &[2, 2, 2, 5, 83, 383, 4049, 11248342573613])),
        870035986098720987332873 => Some((3, &[2, 2, 2, 59, 109, 16910977804748891839])),
        13842607235828485645766393 => Some((3, &[2, 2, 2, 97, 1297, 13753593975618284111])),
        273399736511044418098908817 => Some((
            10,
            &[2, 2, 2, 2, 3, 3, 47, 79, 103, 211, 75983, 309652459427],
        )),
        618970019642690137449562111 => {
            Some((3, &[2, 3, 5, 17, 23, 89, 353, 397, 683, 2113, 2931542417]))
        }
        1786393878363164227858270210279 => {
            Some((3, &[2, 3, 3, 3, 11, 11, 273399736511044418098908817]))
        }
        162259276829213363391578010288127 => {
            Some((3, &[2, 3, 107, 6361, 69431, 20394401, 28059810762433]))
        }
        170141183460469231731687303715884105727 => Some((
            43,
            &[
                2,
                3,
                3,
                3,
                7,
                7,
                19,
                43,
                73,
                127,
                337,
                5419,
                92737,
                649657,
                77158673929,
            ],
        )),
        _ => None,
    }
}
