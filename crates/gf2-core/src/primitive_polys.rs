//! Database of standard polynomials for GF(2^m), drawn from
//! `@/citation/LidlNiederreiter1996`, `@/citation/Menezes1997`,
//! `@/citation/Etsi2015`, `@/citation/ThreeGpp2017`,
//! `@/citation/Seroussi1998`, `@/citation/Zivkovic1994`,
//! `@/citation/Nist2013` Appendix D and `@/citation/Lubeck2024`. Entries for
//! `m <= 16` are verified primitive; entries for `m = 64..=127` are verified
//! irreducible only (see [`PrimitivePolynomialDatabase::standard_u128`]).

use crate::field::extension::FieldId;
use crate::field::modulus_select::{ModulusRegistry, RegistryEntry, RegistryProvenance};

/// Database of well-known polynomials for GF(2^m). Entries for
/// `m = 64..=127` are irreducible but not verified primitive; see
/// [`Self::standard_u128`].
pub struct PrimitivePolynomialDatabase;

impl ModulusRegistry for PrimitivePolynomialDatabase {
    fn conway(&self, base: &FieldId, degree: usize) -> Option<RegistryEntry> {
        if !is_binary_prime_base(base) || degree != 32 {
            return None;
        }
        Self::standard_u128(degree)
            .map(|polynomial| binary_registry_entry(polynomial, degree, RegistryProvenance::Conway))
    }

    fn verified(&self, base: &FieldId, degree: usize) -> Option<RegistryEntry> {
        if !is_binary_prime_base(base) || degree == 32 {
            return None;
        }
        Self::standard_u128(degree).map(|polynomial| {
            let provenance = if degree <= 16 {
                RegistryProvenance::VerifiedPrimitive
            } else {
                RegistryProvenance::VerifiedIrreducible
            };
            binary_registry_entry(polynomial, degree, provenance)
        })
    }
}

fn is_binary_prime_base(base: &FieldId) -> bool {
    match FieldId::prime(2) {
        Ok(binary) => base == &binary,
        Err(_) => false,
    }
}

fn binary_registry_entry(
    polynomial: u128,
    degree: usize,
    provenance: RegistryProvenance,
) -> RegistryEntry {
    let coefficients = (0..=degree)
        .map(|index| ((polynomial >> index) & 1) as u64)
        .collect();
    RegistryEntry::new(coefficients, provenance)
}

/// Result of verifying a polynomial against the database.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerificationResult {
    /// Polynomial matches the standard database entry
    Matches,
    /// Not in database but could be valid (needs verification)
    Unknown,
    /// Different from database entry - WARNING!
    Conflict,
}

impl PrimitivePolynomialDatabase {
    /// Returns the standard primitive polynomial for GF(2^m).
    ///
    /// Returns `Some(poly)` if a standard polynomial is known, `None` otherwise.
    pub fn standard(m: usize) -> Option<u64> {
        match m {
            2 => Some(0b111),                // x^2 + x + 1
            3 => Some(0b1011),               // x^3 + x + 1
            4 => Some(0b10011),              // x^4 + x + 1
            5 => Some(0b100101),             // x^5 + x^2 + 1
            6 => Some(0b1000011),            // x^6 + x + 1
            7 => Some(0b10000011),           // x^7 + x + 1
            8 => Some(0b100011101),          // x^8 + x^4 + x^3 + x^2 + 1 (primitive pentanomial)
            9 => Some(0b1000010001),         // x^9 + x^4 + 1
            10 => Some(0b10000001001),       // x^10 + x^3 + 1
            11 => Some(0b100000000101),      // x^11 + x^2 + 1
            12 => Some(0b1000001010011),     // x^12 + x^6 + x^4 + x + 1
            13 => Some(0b10000000011011),    // x^13 + x^4 + x^3 + x + 1
            14 => Some(0b100000000101011),   // x^14 + x^5 + x^3 + x + 1 (DVB-T2)
            15 => Some(0b1000000000000011),  // x^15 + x + 1
            16 => Some(0b10000000000101101), // x^16 + x^5 + x^3 + x^2 + 1 (DVB-T2)
            // Conway polynomial `f_{2,32}` of `@/citation/Lubeck2024`, hence
            // primitive; `test_standard_m32_is_irreducible` checks
            // irreducibility only.
            32 => Some(0x1_0000_8299u64), // x^32 + x^15 + x^9 + x^7 + x^4 + x^3 + 1
            _ => None,
        }
    }

    /// Returns the database's primitive trinomials of degree `m`, or an empty
    /// vector when it holds none.
    pub fn trinomials(m: usize) -> Vec<u64> {
        match m {
            2 => vec![0b111],                  // x^2 + x + 1
            3 => vec![0b1011],                 // x^3 + x + 1
            4 => vec![0b10011],                // x^4 + x + 1
            5 => vec![0b100101],               // x^5 + x^2 + 1
            6 => vec![0b1000011],              // x^6 + x + 1
            7 => vec![0b10000011, 0b10001001], // x^7 + x + 1, x^7 + x^3 + 1
            8 => vec![0b100010001],            // x^8 + x^4 + 1
            9 => vec![0b1000010001],           // x^9 + x^4 + 1
            10 => vec![0b10000001001],         // x^10 + x^3 + 1
            11 => vec![0b100000000101],        // x^11 + x^2 + 1
            15 => vec![0b1000000000000011],    // x^15 + x + 1
            _ => Vec::new(),
        }
    }

    /// Verifies a polynomial against the database.
    pub fn verify(m: usize, poly: u64) -> VerificationResult {
        match Self::standard(m) {
            Some(standard_poly) if standard_poly == poly => VerificationResult::Matches,
            Some(_) => VerificationResult::Conflict,
            None => VerificationResult::Unknown,
        }
    }

    /// Returns a standard polynomial for GF(2^m) as a `u128`, with its leading
    /// bit at position `m`.
    ///
    /// For `m <= 63` this is [`Self::standard`] widened, so degrees in
    /// `17..=63` other than 32 return `None`; for `m = 64..=127` the entry
    /// comes from `@/citation/Seroussi1998` or `@/citation/Nist2013`
    /// Appendix D.
    ///
    /// # Strength of the guarantee
    ///
    /// - For `m <= 16` the polynomial is **primitive**, checked by
    ///   `test_all_database_entries_are_primitive` in `gf2m/field.rs`.
    /// - For `m = 64..=127` the polynomial is verified only **irreducible
    ///   over GF(2)**, by `test_standard_u128_entries_are_irreducible`.
    ///   Callers that need a primitive polynomial verify it independently.
    pub fn standard_u128(m: usize) -> Option<u128> {
        if m <= 63 {
            return Self::standard(m).map(|p| p as u128);
        }
        Self::seroussi_u128(m)
    }

    /// Entries for `m = 64..=127` from `@/citation/Seroussi1998` and
    /// `@/citation/Nist2013`, each verified irreducible over GF(2) by
    /// `test_standard_u128_entries_are_irreducible`.
    fn seroussi_u128(m: usize) -> Option<u128> {
        let tri = |m: usize, k: usize| Some((1u128 << m) | (1u128 << k) | 1);
        let penta = |m: usize, a: usize, b: usize, c: usize| {
            Some((1u128 << m) | (1u128 << a) | (1u128 << b) | (1u128 << c) | 1)
        };
        match m {
            64 => penta(64, 4, 3, 1), // x^64 + x^4 + x^3 + x + 1
            65 => tri(65, 18),
            66 => penta(66, 9, 8, 6), // x^66 + x^9 + x^8 + x^6 + 1
            67 => penta(67, 5, 2, 1), // x^67 + x^5 + x^2 + x + 1
            68 => tri(68, 9),
            69 => penta(69, 6, 5, 2), // x^69 + x^6 + x^5 + x^2 + 1
            70 => penta(70, 5, 3, 1), // x^70 + x^5 + x^3 + x + 1
            71 => tri(71, 6),
            72 => penta(72, 10, 9, 3), // x^72 + x^10 + x^9 + x^3 + 1
            73 => tri(73, 25),
            74 => penta(74, 7, 4, 3), // x^74 + x^7 + x^4 + x^3 + 1
            75 => penta(75, 6, 3, 1), // x^75 + x^6 + x^3 + x + 1
            76 => penta(76, 5, 4, 2), // x^76 + x^5 + x^4 + x^2 + 1
            77 => penta(77, 6, 5, 2), // x^77 + x^6 + x^5 + x^2 + 1
            78 => penta(78, 7, 2, 1), // x^78 + x^7 + x^2 + x + 1
            79 => tri(79, 9),
            80 => penta(80, 9, 4, 2), // x^80 + x^9 + x^4 + x^2 + 1
            81 => tri(81, 4),
            82 => penta(82, 9, 6, 4), // x^82 + x^9 + x^6 + x^4 + 1
            83 => penta(83, 7, 4, 2), // x^83 + x^7 + x^4 + x^2 + 1
            84 => tri(84, 5),
            85 => penta(85, 8, 2, 1), // x^85 + x^8 + x^2 + x + 1
            86 => penta(86, 6, 5, 2), // x^86 + x^6 + x^5 + x^2 + 1
            87 => tri(87, 13),
            88 => penta(88, 7, 6, 2), // x^88 + x^7 + x^6 + x^2 + 1
            89 => tri(89, 38),
            90 => penta(90, 5, 3, 2), // x^90 + x^5 + x^3 + x^2 + 1
            91 => penta(91, 8, 5, 1), // x^91 + x^8 + x^5 + x + 1
            92 => penta(92, 6, 5, 2), // x^92 + x^6 + x^5 + x^2 + 1
            93 => tri(93, 2),
            94 => tri(94, 21),
            95 => tri(95, 11),
            96 => penta(96, 10, 9, 6), // x^96 + x^10 + x^9 + x^6 + 1
            97 => tri(97, 6),
            98 => tri(98, 11),
            99 => penta(99, 7, 5, 4), // x^99 + x^7 + x^5 + x^4 + 1
            100 => tri(100, 15),
            101 => penta(101, 7, 6, 1), // x^101 + x^7 + x^6 + x + 1
            102 => penta(102, 6, 5, 3), // x^102 + x^6 + x^5 + x^3 + 1
            103 => tri(103, 9),
            104 => penta(104, 11, 10, 1), // x^104 + x^11 + x^10 + x + 1
            105 => tri(105, 16),
            106 => tri(106, 15),
            107 => penta(107, 9, 7, 4), // x^107 + x^9 + x^7 + x^4 + 1
            108 => tri(108, 17),
            109 => penta(109, 5, 4, 2), // x^109 + x^5 + x^4 + x^2 + 1
            110 => tri(110, 33),
            111 => tri(111, 10),
            112 => penta(112, 5, 4, 3), // x^112 + x^5 + x^4 + x^3 + 1
            113 => tri(113, 9),
            114 => penta(114, 11, 2, 1), // x^114 + x^11 + x^2 + x + 1
            115 => penta(115, 8, 7, 5),  // x^115 + x^8 + x^7 + x^5 + 1
            116 => penta(116, 6, 5, 2),  // x^116 + x^6 + x^5 + x^2 + 1
            117 => penta(117, 5, 2, 1),  // x^117 + x^5 + x^2 + x + 1
            118 => tri(118, 33),
            119 => tri(119, 8),
            120 => penta(120, 9, 6, 2), // x^120 + x^9 + x^6 + x^2 + 1
            121 => tri(121, 18),
            122 => penta(122, 6, 2, 1), // x^122 + x^6 + x^2 + x + 1
            123 => tri(123, 2),
            124 => tri(124, 19),
            125 => penta(125, 7, 6, 5), // x^125 + x^7 + x^6 + x^5 + 1
            126 => penta(126, 7, 4, 2), // x^126 + x^7 + x^4 + x^2 + 1
            127 => tri(127, 1),
            _ => None,
        }
    }

    /// Verifies a `u128` polynomial against the extended database (`m` up to 127).
    ///
    /// See [`Self::verify`] for the `u64` variant.
    pub fn verify_u128(m: usize, poly: u128) -> VerificationResult {
        match Self::standard_u128(m) {
            Some(standard_poly) if standard_poly == poly => VerificationResult::Matches,
            Some(_) => VerificationResult::Conflict,
            None => VerificationResult::Unknown,
        }
    }

    /// Human-readable note clarifying the irreducibility-only guarantee for
    /// `m >= 64` entries returned by [`Self::standard_u128`].
    pub const fn standard_u128_irreducibility_note() -> &'static str {
        "PrimitivePolynomialDatabase::standard_u128 entries for m = 64..=127 \
         are verified irreducible over GF(2) but are NOT independently \
         verified primitive. Seroussi's table is a table of irreducible \
         (not necessarily primitive) polynomials. Callers requiring a \
         primitive polynomial (e.g. maximum-length LFSR, full-cycle PRNG) \
         must verify the multiplicative order independently."
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Polynomial multiplication modulo `p(x)` of degree `m`, over GF(2),
    /// with both operands represented as `u128`. Schoolbook shift-and-reduce.
    fn mul_mod_u128(a: u128, b: u128, m: u32, p: u128) -> u128 {
        assert!((1..=127).contains(&m));
        let mask = (1u128 << m) - 1;
        let mut acc: u128 = 0;
        let mut tmp = a & mask;
        for i in 0..m {
            if (b >> i) & 1 != 0 {
                acc ^= tmp;
            }
            let will_overflow = (tmp >> (m - 1)) & 1 != 0;
            tmp <<= 1;
            if will_overflow {
                tmp ^= p;
            }
            tmp &= mask;
        }
        acc & mask
    }

    /// Scalar GCD of two polynomials over GF(2), both represented as `u128`.
    fn gcd_poly_u128(mut a: u128, mut b: u128) -> u128 {
        loop {
            if b == 0 {
                return a;
            }
            if a == 0 {
                return b;
            }
            let da = 127 - a.leading_zeros();
            let db = 127 - b.leading_zeros();
            if da < db {
                std::mem::swap(&mut a, &mut b);
                continue;
            }
            a ^= b << (da - db);
        }
    }

    /// Rabin irreducibility test for `p(x)` of degree `m` over GF(2) using
    /// `u128` storage (supports `m` up to 127).
    fn is_irreducible_u128(p: u128, m: u32) -> bool {
        // Test 1: x^(2^m) ≡ x (mod p)
        let mut cur: u128 = 2;
        for _ in 0..m {
            cur = mul_mod_u128(cur, cur, m, p);
        }
        if cur != 2 {
            return false;
        }
        // Test 2: for each prime q | m, gcd(p, x^(2^(m/q)) - x) = 1
        let mut n = m;
        let mut q = 2u32;
        let mut primes_of_m: Vec<u32> = Vec::new();
        while q * q <= n {
            if n.is_multiple_of(q) {
                primes_of_m.push(q);
                while n.is_multiple_of(q) {
                    n /= q;
                }
            }
            q += 1;
        }
        if n > 1 {
            primes_of_m.push(n);
        }
        for q in primes_of_m {
            let exp = m / q;
            let mut cur: u128 = 2;
            for _ in 0..exp {
                cur = mul_mod_u128(cur, cur, m, p);
            }
            let diff = cur ^ 2;
            if diff == 0 {
                return false;
            }
            if gcd_poly_u128(p, diff) != 1 {
                return false;
            }
        }
        true
    }

    #[test]
    fn test_irreducible_helper_self_check_small_cases() {
        assert!(is_irreducible_u128(0b10011, 4));
        assert!(!is_irreducible_u128(0b101, 2));
        assert!(is_irreducible_u128(0b1011, 3));
    }

    #[test]
    fn test_standard_u128_entries_are_irreducible() {
        let mut failures: Vec<(usize, u128)> = Vec::new();
        for m in 2usize..=127 {
            if let Some(poly) = PrimitivePolynomialDatabase::standard_u128(m) {
                assert_eq!(
                    poly >> m,
                    1,
                    "m={}: leading bit not at position m, poly={:#x}",
                    m,
                    poly
                );
                if !is_irreducible_u128(poly, m as u32) {
                    failures.push((m, poly));
                }
            }
        }
        if !failures.is_empty() {
            let report: Vec<String> = failures
                .iter()
                .map(|(m, p)| format!("m={} poly={:#x}", m, p))
                .collect();
            panic!(
                "standard_u128 has {} reducible entries:\n  {}",
                failures.len(),
                report.join("\n  ")
            );
        }
    }

    #[test]
    fn test_standard_u128_covers_64_to_127() {
        for m in 64usize..=127 {
            assert!(
                PrimitivePolynomialDatabase::standard_u128(m).is_some(),
                "no u128 polynomial catalogued for m={}",
                m
            );
        }
    }

    #[test]
    fn test_database_has_common_fields() {
        assert!(PrimitivePolynomialDatabase::standard(2).is_some());
        assert!(PrimitivePolynomialDatabase::standard(3).is_some());
        assert!(PrimitivePolynomialDatabase::standard(4).is_some());
        assert!(PrimitivePolynomialDatabase::standard(8).is_some());
    }

    #[test]
    fn test_database_has_dvb_t2_fields() {
        assert_eq!(
            PrimitivePolynomialDatabase::standard(14),
            Some(0b100000000101011)
        );
        assert_eq!(
            PrimitivePolynomialDatabase::standard(16),
            Some(0b10000000000101101)
        );
    }

    #[test]
    fn test_database_gf256_standard() {
        assert_eq!(PrimitivePolynomialDatabase::standard(8), Some(0b100011101));
    }

    #[test]
    fn test_verify_matches_standard() {
        let result = PrimitivePolynomialDatabase::verify(8, 0b100011101);
        assert_eq!(result, VerificationResult::Matches);
    }

    #[test]
    fn test_verify_conflict_wrong_polynomial() {
        let result = PrimitivePolynomialDatabase::verify(14, 0b100000000100001);
        assert_eq!(result, VerificationResult::Conflict);
    }

    #[test]
    fn test_verify_unknown_not_in_database() {
        let result = PrimitivePolynomialDatabase::verify(31, 0b10000000000000001001);
        assert_eq!(result, VerificationResult::Unknown);
    }

    #[test]
    fn test_trinomials_gf8() {
        let trinomials = PrimitivePolynomialDatabase::trinomials(8);
        assert!(trinomials.contains(&0b100010001));
    }

    #[test]
    fn test_standard_m32_value() {
        // `f_{2,32}` of `@/citation/Lubeck2024`:
        // x^32 + x^15 + x^9 + x^7 + x^4 + x^3 + 1.
        let poly = PrimitivePolynomialDatabase::standard(32).expect("m=32 entry");
        assert_eq!(poly, 0x1_0000_8299u64);
        let expected_bits = [0u32, 3, 4, 7, 9, 15, 32];
        let mut got_bits: Vec<u32> = (0..64).filter(|i| (poly >> i) & 1 == 1).collect();
        got_bits.sort_unstable();
        assert_eq!(got_bits, expected_bits);
    }

    #[test]
    fn test_standard_m32_is_irreducible() {
        let poly = PrimitivePolynomialDatabase::standard(32).expect("m=32 entry");
        assert!(
            is_irreducible_u128(poly as u128, 32),
            "GF(2^32) Conway polynomial {:#x} is not irreducible — \
             database entry has drifted from the cited Lübeck table",
            poly
        );
    }
}
