//! Database of standard primitive polynomials for GF(2^m).
//!
//! This module provides a verified database of primitive polynomials from
//! authoritative sources including:
//! - Lidl & Niederreiter (1997). "Finite Fields", 2nd edition
//! - Menezes et al. (1996). "Handbook of Applied Cryptography"
//! - ETSI EN 302 755 (DVB-T2 standard)
//! - IEEE AES standard
//! - 3GPP TS 38.212 (5G NR standard)

/// Database of well-known primitive polynomials from authoritative sources.
pub struct PrimitivePolynomialDatabase;

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
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::primitive_polys::PrimitivePolynomialDatabase;
    ///
    /// // AES standard
    /// assert_eq!(PrimitivePolynomialDatabase::standard(8), Some(0b100011011));
    ///
    /// // DVB-T2 short frames
    /// assert_eq!(PrimitivePolynomialDatabase::standard(14), Some(0b100000000101011));
    ///
    /// // DVB-T2 normal frames
    /// assert_eq!(PrimitivePolynomialDatabase::standard(16), Some(0b10000000000101101));
    /// ```
    pub fn standard(m: usize) -> Option<u64> {
        match m {
            // Standard primitive polynomials from authoritative sources
            2 => Some(0b111),                    // x^2 + x + 1
            3 => Some(0b1011),                   // x^3 + x + 1
            4 => Some(0b10011),                  // x^4 + x + 1
            5 => Some(0b100101),                 // x^5 + x^2 + 1
            6 => Some(0b1000011),                // x^6 + x + 1
            7 => Some(0b10000011),               // x^7 + x + 1
            8 => Some(0b100011011),              // x^8 + x^4 + x^3 + x + 1 (AES)
            9 => Some(0b1000010001),             // x^9 + x^4 + 1
            10 => Some(0b10000001001),           // x^10 + x^3 + 1
            11 => Some(0b100000000101),          // x^11 + x^2 + 1
            12 => Some(0b1000001010011),         // x^12 + x^6 + x^4 + x + 1
            13 => Some(0b10000000011011),        // x^13 + x^4 + x^3 + x + 1
            14 => Some(0b100000000101011),       // x^14 + x^5 + x^3 + x + 1 (DVB-T2)
            15 => Some(0b1000000000000011),      // x^15 + x + 1
            16 => Some(0b10000000000101101),     // x^16 + x^5 + x^3 + x^2 + 1 (DVB-T2)
            _ => None,
        }
    }

    /// Returns all known primitive trinomials of degree m.
    ///
    /// Trinomials (x^m + x^k + 1) are preferred in hardware implementations
    /// because they minimize XOR gate count in LFSR circuits.
    ///
    /// Returns empty vector if no primitive trinomials exist for this degree,
    /// or if they are not in the database.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::primitive_polys::PrimitivePolynomialDatabase;
    ///
    /// let trinomials = PrimitivePolynomialDatabase::trinomials(8);
    /// assert!(!trinomials.is_empty());
    /// // x^8 + x^4 + 1 is a primitive trinomial
    /// assert!(trinomials.contains(&0b100010001));
    /// ```
    pub fn trinomials(m: usize) -> Vec<u64> {
        match m {
            // Known primitive trinomials (x^m + x^k + 1)
            2 => vec![0b111],                    // x^2 + x + 1
            3 => vec![0b1011],                   // x^3 + x + 1
            4 => vec![0b10011],                  // x^4 + x + 1
            5 => vec![0b100101],                 // x^5 + x^2 + 1
            6 => vec![0b1000011],                // x^6 + x + 1
            7 => vec![0b10000011, 0b10001001],   // x^7 + x + 1, x^7 + x^3 + 1
            8 => vec![0b100010001],              // x^8 + x^4 + 1
            9 => vec![0b1000010001],             // x^9 + x^4 + 1
            10 => vec![0b10000001001],           // x^10 + x^3 + 1
            11 => vec![0b100000000101],          // x^11 + x^2 + 1
            15 => vec![0b1000000000000011],      // x^15 + x + 1
            _ => Vec::new(),
        }
    }

    /// Verifies a polynomial against the database.
    ///
    /// Returns:
    /// - `Matches`: Polynomial matches the standard database entry
    /// - `Unknown`: Not in database but could be valid (needs verification)
    /// - `Conflict`: Different from database entry - WARNING!
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::primitive_polys::{PrimitivePolynomialDatabase, VerificationResult};
    ///
    /// // Correct DVB-T2 polynomial
    /// let result = PrimitivePolynomialDatabase::verify(14, 0b100000000101011);
    /// assert_eq!(result, VerificationResult::Matches);
    ///
    /// // Wrong polynomial that caused the bug
    /// let result = PrimitivePolynomialDatabase::verify(14, 0b100000000100001);
    /// assert_eq!(result, VerificationResult::Conflict);
    /// ```
    pub fn verify(m: usize, poly: u64) -> VerificationResult {
        match Self::standard(m) {
            Some(standard_poly) if standard_poly == poly => VerificationResult::Matches,
            Some(_) => VerificationResult::Conflict,
            None => VerificationResult::Unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_database_has_common_fields() {
        // Standard fields should be in database
        assert!(PrimitivePolynomialDatabase::standard(2).is_some());
        assert!(PrimitivePolynomialDatabase::standard(3).is_some());
        assert!(PrimitivePolynomialDatabase::standard(4).is_some());
        assert!(PrimitivePolynomialDatabase::standard(8).is_some());
    }

    #[test]
    fn test_database_has_dvb_t2_fields() {
        // DVB-T2 specific fields
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
    fn test_database_aes_standard() {
        // AES uses x^8 + x^4 + x^3 + x + 1
        assert_eq!(
            PrimitivePolynomialDatabase::standard(8),
            Some(0b100011011)
        );
    }

    #[test]
    fn test_verify_matches_standard() {
        let result = PrimitivePolynomialDatabase::verify(8, 0b100011011);
        assert_eq!(result, VerificationResult::Matches);
    }

    #[test]
    fn test_verify_conflict_wrong_polynomial() {
        // The DVB-T2 bug case
        let result = PrimitivePolynomialDatabase::verify(14, 0b100000000100001);
        assert_eq!(result, VerificationResult::Conflict);
    }

    #[test]
    fn test_verify_unknown_not_in_database() {
        // Some high degree not in database yet
        let result = PrimitivePolynomialDatabase::verify(31, 0b10000000000000001001);
        assert_eq!(result, VerificationResult::Unknown);
    }

    #[test]
    fn test_trinomials_gf8() {
        let trinomials = PrimitivePolynomialDatabase::trinomials(8);
        // x^8 + x^4 + 1 is a known primitive trinomial
        assert!(trinomials.contains(&0b100010001));
    }
}
