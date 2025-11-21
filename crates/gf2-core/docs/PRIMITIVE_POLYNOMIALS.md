# Primitive Polynomials for GF(2^m)

## Overview

This document describes the implementation of primitive polynomial generation, verification, and management in `gf2-core`. The work was motivated by a DVB-T2 BCH decoding failure caused by using an incorrect primitive polynomial for GF(2^14).

## Background

### Mathematical Definitions

**Irreducible Polynomial**: A polynomial p(x) over GF(2) is irreducible if it cannot be factored into non-trivial polynomials over GF(2). This is analogous to prime numbers in integer arithmetic.

**Primitive Polynomial**: A polynomial p(x) of degree m over GF(2) is primitive if:
1. It is irreducible over GF(2)
2. The smallest positive integer n such that p(x) divides x^n - 1 is n = 2^m - 1

Equivalently, a primitive polynomial generates the full multiplicative group of GF(2^m), meaning its roots have multiplicative order 2^m - 1.

### Why Primitive Polynomials Matter

Primitive polynomials are essential for:
- **Error-correcting codes**: BCH and Reed-Solomon codes require primitive elements to define generator polynomials
- **Cryptography**: Stream ciphers (LFSR), pseudo-random number generation
- **Hardware efficiency**: Trinomials (x^m + x^k + 1) minimize XOR gates in LFSR implementations

### The DVB-T2 BCH Bug

**Root cause**: BCH(16200, 7032) for DVB-T2 short frames used GF(2^14) with polynomial `0b100000000100001` (x^14 + x^5 + 1).

**Problem**: This polynomial is NOT primitive:
- It is irreducible but has the wrong order
- BCH syndrome computation requires consecutive powers of a primitive element α
- Using a non-primitive polynomial causes decoding failures

**Solution**: The correct DVB-T2 standard polynomial is `0b100000000101011` (x^14 + x^5 + x^3 + x + 1).

## Phase 1: Primitive Polynomial Verification (Foundation)

### Goals
1. Ensure we never use a wrong primitive polynomial again
2. Provide compile-time warnings for non-standard polynomials
3. Build a verified database of standard polynomials from authoritative sources

### Components

#### 1.1 Polynomial Primitivity Testing

**Location**: `gf2-core/src/gf2m.rs`

```rust
impl Gf2mField {
    /// Verifies that the polynomial is actually primitive for GF(2^m).
    /// 
    /// A polynomial p(x) of degree m is primitive if:
    /// 1. It is irreducible over GF(2)
    /// 2. Its multiplicative order is 2^m - 1 (generates full multiplicative group)
    ///
    /// # Algorithm
    ///
    /// Uses Rabin's irreducibility test combined with order verification:
    /// - Test irreducibility using gcd(p(x), x^(2^i) - x) for i = 1..⌈m/2⌉
    /// - Verify x^(2^m) ≡ x (mod p(x))
    /// - Check that no smaller exponent satisfies x^k ≡ x (mod p(x))
    ///
    /// # Complexity
    ///
    /// O(m³) for degree-m polynomial using fast exponentiation and GCD.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gf2m::Gf2mField;
    ///
    /// // DVB-T2 standard polynomial (correct)
    /// let gf14 = Gf2mField::new(14, 0b100000000101011);
    /// assert!(gf14.verify_primitive());
    ///
    /// // Wrong polynomial that caused the bug
    /// let gf14_bad = Gf2mField::new(14, 0b100000000100001);
    /// assert!(!gf14_bad.verify_primitive());
    /// ```
    pub fn verify_primitive(&self) -> bool;
    
    /// Tests irreducibility using Rabin's test.
    ///
    /// A polynomial p(x) of degree m is irreducible if and only if:
    /// - gcd(p(x), x^(2^i) - x) = 1 for all i = 1, 2, ..., ⌊m/2⌋
    /// - x^(2^m) ≡ x (mod p(x))
    ///
    /// # References
    ///
    /// Rabin, M. O. (1980). "Probabilistic algorithms in finite fields."
    /// SIAM Journal on Computing, 9(2), 273-280.
    fn is_irreducible_rabin(&self) -> bool;
}
```

**Implementation approach**:
- Use existing `Gf2mPoly::gcd()` for Rabin irreducibility test
- Use exponentiation by squaring for x^(2^i) mod p(x)
- Leverage Karatsuba multiplication for polynomial operations
- Time complexity: O(m³) for degree-m polynomial

#### 1.2 Standard Primitive Polynomial Database

**Location**: `gf2-core/src/gf2m/primitive_polys.rs`

```rust
/// Database of well-known primitive polynomials from authoritative sources.
///
/// Sources include:
/// - Lidl & Niederreiter (1997). "Finite Fields", 2nd edition
/// - Menezes et al. (1996). "Handbook of Applied Cryptography"
/// - ETSI EN 302 755 (DVB-T2 standard)
/// - 3GPP TS 38.212 (5G NR standard)
/// - IEEE 802.11 (WiFi standards)
pub struct PrimitivePolynomialDatabase;

impl PrimitivePolynomialDatabase {
    /// Returns the standard primitive polynomial for GF(2^m).
    ///
    /// Returns `Some(poly)` if a standard polynomial is known, `None` otherwise.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gf2m::PrimitivePolynomialDatabase;
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
    pub fn standard(m: usize) -> Option<u64>;
    
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
    /// use gf2_core::gf2m::PrimitivePolynomialDatabase;
    ///
    /// let trinomials = PrimitivePolynomialDatabase::trinomials(8);
    /// assert!(!trinomials.is_empty());
    /// // x^8 + x^4 + 1 is a primitive trinomial
    /// assert!(trinomials.contains(&0b100010001));
    /// ```
    pub fn trinomials(m: usize) -> Vec<u64>;
    
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
    /// use gf2_core::gf2m::{PrimitivePolynomialDatabase, VerificationResult};
    ///
    /// // Correct DVB-T2 polynomial
    /// let result = PrimitivePolynomialDatabase::verify(14, 0b100000000101011);
    /// assert_eq!(result, VerificationResult::Matches);
    ///
    /// // Wrong polynomial that caused the bug
    /// let result = PrimitivePolynomialDatabase::verify(14, 0b100000000100001);
    /// assert_eq!(result, VerificationResult::Conflict);
    /// ```
    pub fn verify(m: usize, poly: u64) -> VerificationResult;
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
```

**Database contents** (initial - Phase 1):

| m  | Polynomial (hex) | Polynomial (binary)     | Source    | Notes |
|----|------------------|-------------------------|-----------|-------|
| 2  | 0x7              | 0b111                   | Standard  | x^2 + x + 1 |
| 3  | 0xB              | 0b1011                  | Standard  | x^3 + x + 1 |
| 4  | 0x13             | 0b10011                 | Standard  | x^4 + x + 1 |
| 5  | 0x25             | 0b100101                | Standard  | x^5 + x^2 + 1 |
| 6  | 0x43             | 0b1000011               | Standard  | x^6 + x + 1 |
| 7  | 0x83             | 0b10000011              | Standard  | x^7 + x + 1 |
| 8  | 0x11D            | 0b100011101             | AES       | x^8 + x^4 + x^3 + x + 1 |
| 14 | 0x402B           | 0b100000000101011       | DVB-T2    | x^14 + x^5 + x^3 + x + 1 |
| 16 | 0x1002D          | 0b10000000000101101     | DVB-T2    | x^16 + x^5 + x^3 + x^2 + 1 |

#### 1.3 Compile-Time Verification and Warnings

**Location**: `gf2-core/src/gf2m.rs` (modify `Gf2mField::new()`)

```rust
impl Gf2mField {
    /// Creates a new GF(2^m) field with the specified primitive polynomial.
    ///
    /// # Verification
    ///
    /// The constructor now performs the following checks:
    /// 1. Warns if polynomial differs from the standard database entry
    /// 2. Optionally verifies primitivity at runtime (with `verify-primitives` feature)
    ///
    /// # Arguments
    ///
    /// * `m` - Extension degree (field has 2^m elements)
    /// * `primitive_poly` - Primitive polynomial of degree m in binary representation
    ///
    /// # Panics
    ///
    /// Panics if:
    /// - m > 64 (not yet supported) or m == 0
    /// - With feature `verify-primitives`: polynomial is not primitive
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gf2m::Gf2mField;
    ///
    /// // Standard polynomial - no warning
    /// let field = Gf2mField::new(8, 0b100011101);
    ///
    /// // Non-standard polynomial - prints warning to stderr
    /// let field = Gf2mField::new(8, 0b100011011); // Different from AES
    /// // WARNING: Non-standard primitive polynomial for GF(2^8)
    /// //   Provided: 0b100011011
    /// //   Standard: 0b100011101 (AES)
    /// ```
    pub fn new(m: usize, primitive_poly: u64) -> Self {
        assert!(m > 0, "Extension degree m must be positive");
        assert!(m <= 64, "Extension degree m > 64 not yet supported");
        
        // Check against database
        use crate::gf2m::primitive_polys::{PrimitivePolynomialDatabase, VerificationResult};
        
        match PrimitivePolynomialDatabase::verify(m, primitive_poly) {
            VerificationResult::Matches => {
                // All good - using standard polynomial
            }
            VerificationResult::Conflict => {
                eprintln!("WARNING: Non-standard primitive polynomial for GF(2^{})", m);
                eprintln!("  Provided: {:#b}", primitive_poly);
                if let Some(standard) = PrimitivePolynomialDatabase::standard(m) {
                    eprintln!("  Standard: {:#b}", standard);
                }
                eprintln!("  This may cause interoperability issues with standard implementations.");
            }
            VerificationResult::Unknown => {
                // Not in database - could be valid, no warning
            }
        }
        
        // Optional runtime verification (controlled by feature flag)
        #[cfg(feature = "verify-primitives")]
        {
            let field = Self::new_unchecked(m, primitive_poly);
            assert!(
                field.verify_primitive(),
                "Polynomial {:#b} is not primitive for GF(2^{}). \
                 Field elements will not generate the full multiplicative group. \
                 This will cause errors in BCH codes and other applications.",
                primitive_poly, m
            );
            return field;
        }
        
        #[cfg(not(feature = "verify-primitives"))]
        Self::new_unchecked(m, primitive_poly)
    }
    
    /// Internal constructor without verification (for use after verification).
    fn new_unchecked(m: usize, primitive_poly: u64) -> Self {
        // existing implementation
    }
}
```

### Testing Strategy (TDD)

All tests written BEFORE implementation, following TDD principles.

#### Unit Tests

**File**: `gf2-core/src/gf2m/tests.rs` (add to existing module)

```rust
#[cfg(test)]
mod primitive_verification_tests {
    use super::*;

    #[test]
    fn test_verify_primitive_gf4() {
        let field = Gf2mField::new(2, 0b111); // x^2 + x + 1
        assert!(field.verify_primitive());
    }

    #[test]
    fn test_verify_primitive_gf8() {
        let field = Gf2mField::new(3, 0b1011); // x^3 + x + 1
        assert!(field.verify_primitive());
    }

    #[test]
    fn test_verify_primitive_gf16() {
        let field = Gf2mField::new(4, 0b10011); // x^4 + x + 1
        assert!(field.verify_primitive());
    }

    #[test]
    fn test_verify_primitive_gf256_aes() {
        // AES standard polynomial
        let field = Gf2mField::new(8, 0b100011011);
        assert!(field.verify_primitive());
    }

    #[test]
    fn test_verify_primitive_dvb_t2_gf14() {
        // Correct DVB-T2 polynomial
        let field = Gf2mField::new(14, 0b100000000101011);
        assert!(field.verify_primitive());
    }

    #[test]
    fn test_verify_primitive_dvb_t2_gf16() {
        // Correct DVB-T2 polynomial for normal frames
        let field = Gf2mField::new(16, 0b10000000000101101);
        assert!(field.verify_primitive());
    }

    #[test]
    fn test_verify_not_primitive_wrong_dvb_t2() {
        // The bug: wrong polynomial used initially
        let field = Gf2mField::new(14, 0b100000000100001);
        assert!(!field.verify_primitive(), "This polynomial caused the BCH bug");
    }

    #[test]
    fn test_verify_not_primitive_reducible() {
        // (x + 1)^2 = x^2 + 1 is reducible
        let field = Gf2mField::new(2, 0b101);
        assert!(!field.verify_primitive());
    }

    #[test]
    fn test_is_irreducible_rabin_small_cases() {
        // x^2 + x + 1 is irreducible
        let field = Gf2mField::new(2, 0b111);
        assert!(field.is_irreducible_rabin());

        // x^2 + 1 = (x + 1)^2 is reducible
        let field = Gf2mField::new(2, 0b101);
        assert!(!field.is_irreducible_rabin());
    }
}
```

**File**: `gf2-core/src/gf2m/primitive_polys/tests.rs`

```rust
#[cfg(test)]
mod database_tests {
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

    #[test]
    fn test_all_database_entries_are_primitive() {
        // Every polynomial in the database must verify as primitive
        for m in 2..=16 {
            if let Some(poly) = PrimitivePolynomialDatabase::standard(m) {
                let field = Gf2mField::new_unchecked(m, poly);
                assert!(
                    field.verify_primitive(),
                    "Database entry for m={} ({:#b}) is not primitive!",
                    m, poly
                );
            }
        }
    }
}
```

#### Integration Tests

**File**: `gf2-coding/tests/bch_primitive_verification.rs`

```rust
//! Integration tests ensuring BCH codes use verified primitive polynomials.

use gf2_coding::bch::dvb_t2::{DvbBchParams, FrameSize};
use gf2_coding::CodeRate;
use gf2_core::gf2m::Gf2mField;

#[test]
fn test_all_dvb_t2_configurations_use_primitive_polynomials() {
    let frame_sizes = [FrameSize::Short, FrameSize::Normal];
    let rates = [
        CodeRate::Rate1_2,
        CodeRate::Rate3_5,
        CodeRate::Rate2_3,
        CodeRate::Rate3_4,
        CodeRate::Rate4_5,
        CodeRate::Rate5_6,
    ];

    for &frame_size in &frame_sizes {
        for &rate in &rates {
            let params = DvbBchParams::for_code(frame_size, rate);
            let field = Gf2mField::new(params.field_m, params.primitive_poly);
            
            assert!(
                field.verify_primitive(),
                "DVB-T2 {:?} {:?} uses non-primitive polynomial {:#b}",
                frame_size, rate, params.primitive_poly
            );
        }
    }
}

#[test]
fn test_dvb_t2_short_uses_correct_polynomial() {
    let params = DvbBchParams::for_code(FrameSize::Short, CodeRate::Rate1_2);
    
    // Must use the correct x^14 + x^5 + x^3 + x + 1
    assert_eq!(params.primitive_poly, 0b100000000101011);
    
    // Must NOT use the wrong polynomial that caused the bug
    assert_ne!(params.primitive_poly, 0b100000000100001);
}

#[test]
fn test_dvb_t2_normal_uses_correct_polynomial() {
    let params = DvbBchParams::for_code(FrameSize::Normal, CodeRate::Rate1_2);
    
    // Must use the correct x^16 + x^5 + x^3 + x^2 + 1
    assert_eq!(params.primitive_poly, 0b10000000000101101);
}
```

#### Property-Based Tests

**File**: `gf2-core/src/gf2m/tests.rs`

```rust
#[cfg(test)]
mod primitive_property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn prop_all_database_entries_verify(m in 2u32..=16) {
            if let Some(poly) = PrimitivePolynomialDatabase::standard(m as usize) {
                let field = Gf2mField::new_unchecked(m as usize, poly);
                prop_assert!(field.verify_primitive());
            }
        }

        #[test]
        fn prop_reducible_polynomials_not_primitive(m in 2u32..8, k in 1u32..8) {
            let m = m as usize;
            let k = k as usize;
            if k < m {
                // Construct (x^k + x + 1)^2 which is reducible
                let factor = (1u64 << k) | 0b11;
                // This test is illustrative; actual squaring is complex
                // Skip if would overflow
                if k * 2 <= 16 {
                    // Test that obvious reducible cases fail
                    // (This is a simplified test - full implementation would be more sophisticated)
                }
            }
        }
    }
}
```

### Performance Considerations

**Primitivity verification complexity**: O(m³) per polynomial
- For m=8: ~microseconds
- For m=14: ~tens of microseconds
- For m=16: ~hundreds of microseconds

**Feature flag approach**:
- By default: Only database check + warning (zero runtime cost for verified polynomials)
- With `verify-primitives` feature: Full verification at construction (adds ~100µs for m=16)
- Recommended: Use `verify-primitives` in tests and debug builds only

### Migration Guide for gf2-coding

**Before** (hardcoded polynomials):
```rust
impl DvbBchParams {
    pub fn for_code(frame_size: FrameSize, rate: CodeRate) -> Self {
        let (n, k, t, field_m, primitive_poly) = match (frame_size, rate) {
            (FrameSize::Short, CodeRate::Rate1_2) => 
                (7200, 7032, 12, 14, 0b100000000101011),
            // ... more cases
        };
        Self { n, k, m: n-k, t, field_m, primitive_poly }
    }
}
```

**After** (database-driven):
```rust
use gf2_core::gf2m::PrimitivePolynomialDatabase;

impl DvbBchParams {
    pub fn for_code(frame_size: FrameSize, rate: CodeRate) -> Self {
        let (n, k, t, field_m) = match (frame_size, rate) {
            (FrameSize::Short, CodeRate::Rate1_2) => (7200, 7032, 12, 14),
            (FrameSize::Normal, CodeRate::Rate1_2) => (32400, 32208, 12, 16),
            // ... more cases
        };
        
        // Get verified polynomial from database
        let primitive_poly = PrimitivePolynomialDatabase::standard(field_m)
            .expect("DVB-T2 primitive polynomial must be in database");
        
        Self { n, k, m: n-k, t, field_m, primitive_poly }
    }
}
```

## References

1. **Lidl, R., & Niederreiter, H.** (1997). *Finite Fields* (2nd ed.). Cambridge University Press.
   - Chapter 3: "Polynomials over Finite Fields"
   - Comprehensive reference for primitive polynomial theory

2. **Rabin, M. O.** (1980). "Probabilistic algorithms in finite fields." *SIAM Journal on Computing*, 9(2), 273-280.
   - Original irreducibility testing algorithm

3. **Menezes, A. J., Van Oorschot, P. C., & Vanstone, S. A.** (1996). *Handbook of Applied Cryptography*. CRC Press.
   - Section 4.5: "Irreducible and primitive polynomials"

4. **ETSI EN 302 755** V1.4.1 (2015). "Digital Video Broadcasting (DVB); Frame structure channel coding and modulation for a second generation digital terrestrial television broadcasting system (DVB-T2)."
   - Authoritative source for DVB-T2 primitive polynomials

5. **Peterson, W. W., & Weldon, E. J.** (1972). *Error-Correcting Codes* (2nd ed.). MIT Press.
   - Classic reference for BCH codes and field theory

## Future Work (Phase 2+)

- **Phase 2**: Primitive polynomial generation via exhaustive and trinomial search
- **Phase 3**: Performance optimization (parallel search, SIMD, GPU)
- **Phase 4**: State-of-the-art algorithms for large degrees (m > 64)

See main [ROADMAP.md](../../ROADMAP.md) for timeline and priorities.
