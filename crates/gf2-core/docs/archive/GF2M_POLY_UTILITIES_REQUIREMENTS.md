# GF(2^m) Polynomial Utility Functions - Requirements

**Date**: 2025-11-30  
**Status**: Requirements Definition  
**Target**: gf2-core v0.2.0

## Overview

This document defines requirements for polynomial construction convenience methods that should be added to `Gf2mPoly` in gf2-core. Currently, these utilities exist in application code (gf2-coding) but belong in the core library for reusability.

## Motivation

### Current Problem

The `poly_from_exponents` function currently lives in `gf2-coding/src/bch/dvb_t2/generators.rs`:

```rust
pub fn poly_from_exponents(field: &Gf2mField, exponents: &[usize]) -> Gf2mPoly {
    let max_exp = exponents.iter().copied().max().unwrap_or(0);
    let mut coeffs = vec![field.zero(); max_exp + 1];
    
    for &exp in exponents {
        coeffs[exp] = field.one();
    }
    
    Gf2mPoly::new(coeffs)
}
```

**Issues**:
- ❌ Located in application code (BCH-specific module)
- ❌ Not reusable by other codes (Reed-Solomon, Goppa, etc.)
- ❌ Inconsistent API: `Gf2mPoly::new()` exists, but not `from_exponents()`
- ❌ Duplicated across projects that use GF(2^m) polynomials

**Benefits of moving to gf2-core**:
- ✅ Single source of truth
- ✅ Consistent API surface
- ✅ Testable in isolation
- ✅ Reusable across all error-correcting codes
- ✅ Natural extension of existing `Gf2mPoly` API

## Requirements

### REQ-1: `Gf2mPoly::from_exponents()` Constructor

**Priority**: HIGH  
**Effort**: Low (2-4 hours)

Add a static constructor method to create polynomials from exponent lists.

#### Specification

```rust
impl Gf2mPoly {
    /// Creates a polynomial from a list of exponents.
    ///
    /// Each exponent in the list corresponds to a term with coefficient 1.
    /// For example, `[0, 2, 5]` represents `1 + x² + x⁵`.
    ///
    /// This is particularly useful for constructing generator polynomials
    /// from standard tables (e.g., BCH, Goppa codes).
    ///
    /// # Arguments
    ///
    /// * `field` - The field over which the polynomial is defined
    /// * `exponents` - Slice of exponents where coefficients are 1
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gf2m::{Gf2mField, Gf2mPoly};
    ///
    /// let field = Gf2mField::new(4, 0b10011);
    /// 
    /// // Create polynomial: 1 + x + x^4
    /// let poly = Gf2mPoly::from_exponents(&field, &[0, 1, 4]);
    /// 
    /// assert_eq!(poly.degree(), Some(4));
    /// assert_eq!(poly.coeff(0), field.one());
    /// assert_eq!(poly.coeff(1), field.one());
    /// assert_eq!(poly.coeff(2), field.zero());
    /// assert_eq!(poly.coeff(3), field.zero());
    /// assert_eq!(poly.coeff(4), field.one());
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if `exponents` is empty.
    pub fn from_exponents(field: &Gf2mField, exponents: &[usize]) -> Self;
}
```

#### Implementation Notes

- **Empty list**: Should panic (no meaningful zero polynomial representation)
- **Duplicate exponents**: Allowed (x^2 + x^2 = 0 in GF(2), cancels out)
- **Unsorted exponents**: Allowed (internally handled by polynomial normalization)
- **Performance**: O(n) where n is max exponent

#### Test Coverage

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn from_exponents_simple() {
        // Test basic construction
    }
    
    #[test]
    fn from_exponents_single() {
        // Test monomial: [5] → x^5
    }
    
    #[test]
    fn from_exponents_duplicates() {
        // Test cancellation: [2, 2] → 0 (x^2 + x^2 = 0 in GF(2))
    }
    
    #[test]
    fn from_exponents_unsorted() {
        // Test [5, 1, 3] produces correct polynomial
    }
    
    #[test]
    #[should_panic]
    fn from_exponents_empty() {
        // Empty list should panic
    }
    
    #[test]
    fn from_exponents_dvb_t2_example() {
        // Real-world: DVB-T2 generator polynomial g_1
        let field = Gf2mField::new(14, 0b100000000100001);
        let g1 = Gf2mPoly::from_exponents(&field, &[0, 1, 3, 5, 14]);
        assert_eq!(g1.degree(), Some(14));
    }
}
```

### REQ-2: `Gf2mPoly::from_roots()` Constructor

**Priority**: MEDIUM  
**Effort**: Medium (4-6 hours)

Construct polynomial from its roots: `(x - r₁)(x - r₂)...(x - rₙ)`.

#### Specification

```rust
impl Gf2mPoly {
    /// Creates a polynomial from its roots.
    ///
    /// Constructs the polynomial `(x - r₁)(x - r₂)...(x - rₙ)` where
    /// `rᵢ` are the roots.
    ///
    /// This is fundamental for BCH and Reed-Solomon code construction,
    /// where generator polynomials are defined by consecutive roots.
    ///
    /// # Arguments
    ///
    /// * `roots` - Slice of field elements that are roots of the polynomial
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gf2m::{Gf2mField, Gf2mPoly};
    ///
    /// let field = Gf2mField::new(4, 0b10011).with_tables();
    /// let alpha = field.primitive_element().unwrap();
    ///
    /// // BCH generator: g(x) = (x - α)(x - α²)
    /// let g = Gf2mPoly::from_roots(&[
    ///     alpha.clone(),
    ///     &alpha * &alpha,
    /// ]);
    ///
    /// // Verify roots
    /// assert!(g.eval(&alpha).is_zero());
    /// assert!(g.eval(&(&alpha * &alpha)).is_zero());
    /// ```
    ///
    /// # Complexity
    ///
    /// O(n²) where n is the number of roots (sequential multiplication).
    ///
    /// # Panics
    ///
    /// Panics if `roots` is empty.
    pub fn from_roots(roots: &[Gf2mElement]) -> Self;
}
```

#### Implementation Notes

- **Algorithm**: Sequential multiplication `p(x) = p(x) * (x - rᵢ)`
- **Empty roots**: Should panic
- **Duplicate roots**: Allowed (creates polynomial with repeated roots)
- **Optimization opportunity**: Could use FFT-based multiplication for large n
- **Field consistency**: All roots must be from the same field (enforce via type system)

#### Test Coverage

```rust
#[test]
fn from_roots_single() {
    // (x - α) should have degree 1
}

#[test]
fn from_roots_bch() {
    // BCH generator with consecutive powers of α
}

#[test]
fn from_roots_verify_roots() {
    // Verify p(rᵢ) = 0 for all roots
}

#[test]
fn from_roots_duplicates() {
    // (x - α)² should work correctly
}

#[test]
#[should_panic]
fn from_roots_empty() {
    // Empty roots should panic
}
```

### REQ-3: `Gf2mPoly::monomial()` Constructor

**Priority**: MEDIUM  
**Effort**: Low (1-2 hours)

Create monomial `c·xⁿ` (single term polynomial).

#### Specification

```rust
impl Gf2mPoly {
    /// Creates a monomial: `c·xⁿ`.
    ///
    /// A monomial is a polynomial with a single term.
    ///
    /// # Arguments
    ///
    /// * `coeff` - The coefficient (may be any field element)
    /// * `degree` - The exponent of x
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gf2m::{Gf2mField, Gf2mPoly};
    ///
    /// let field = Gf2mField::new(4, 0b10011);
    /// let alpha = field.element(0b0010); // α
    ///
    /// // Create α·x³
    /// let poly = Gf2mPoly::monomial(alpha.clone(), 3);
    ///
    /// assert_eq!(poly.degree(), Some(3));
    /// assert_eq!(poly.coeff(0), field.zero());
    /// assert_eq!(poly.coeff(3), alpha);
    /// ```
    ///
    /// # Special Cases
    ///
    /// - `monomial(c, 0)` returns constant polynomial `c`
    /// - `monomial(0, n)` returns zero polynomial regardless of n
    pub fn monomial(coeff: Gf2mElement, degree: usize) -> Self;
}
```

#### Implementation Notes

- **Zero coefficient**: Returns zero polynomial (degree = None)
- **Performance**: O(n) to allocate coefficient vector
- **Common use**: Shifting polynomials (multiply by x^n)

#### Test Coverage

```rust
#[test]
fn monomial_zero_degree() {
    // c·x⁰ = c (constant)
}

#[test]
fn monomial_zero_coeff() {
    // 0·x^5 = 0
}

#[test]
fn monomial_general() {
    // α·x³ has correct degree and coefficient
}
```

### REQ-4: `Gf2mPoly::product()` Static Method

**Priority**: LOW  
**Effort**: Low (1-2 hours)

Compute product of multiple polynomials.

#### Specification

```rust
impl Gf2mPoly {
    /// Computes the product of multiple polynomials.
    ///
    /// Returns `p₁(x) · p₂(x) · ... · pₙ(x)`.
    ///
    /// # Arguments
    ///
    /// * `polys` - Slice of polynomials to multiply
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gf2m::{Gf2mField, Gf2mPoly};
    ///
    /// let field = Gf2mField::new(4, 0b10011);
    /// let p1 = Gf2mPoly::from_exponents(&field, &[0, 1]);    // 1 + x
    /// let p2 = Gf2mPoly::from_exponents(&field, &[0, 2]);    // 1 + x²
    /// let p3 = Gf2mPoly::from_exponents(&field, &[0, 1, 2]); // 1 + x + x²
    ///
    /// let product = Gf2mPoly::product(&[p1, p2, p3]);
    /// // (1 + x)(1 + x²)(1 + x + x²) = ...
    /// ```
    ///
    /// # Complexity
    ///
    /// O(n · d²) where n is number of polynomials and d is average degree.
    ///
    /// # Panics
    ///
    /// Panics if `polys` is empty.
    pub fn product(polys: &[Gf2mPoly]) -> Self;
}
```

#### Implementation Notes

- **Empty list**: Panic (no identity element defined)
- **Single element**: Return clone
- **Algorithm**: Left-to-right sequential multiplication
- **Optimization**: Could sort by degree (multiply smaller polynomials first)

### REQ-5: `Gf2mPoly::x()` Static Method

**Priority**: LOW  
**Effort**: Trivial (30 minutes)

Create the polynomial `x` (the indeterminate).

#### Specification

```rust
impl Gf2mPoly {
    /// Creates the polynomial `x` (the indeterminate).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gf2m::{Gf2mField, Gf2mPoly};
    ///
    /// let field = Gf2mField::new(4, 0b10011);
    /// let x = Gf2mPoly::x(&field);
    ///
    /// assert_eq!(x.degree(), Some(1));
    /// assert_eq!(x.coeff(0), field.zero());
    /// assert_eq!(x.coeff(1), field.one());
    /// ```
    pub fn x(field: &Gf2mField) -> Self;
}
```

#### Implementation Notes

- **Implementation**: `Gf2mPoly::monomial(field.one(), 1)`
- **Use case**: Building polynomials programmatically

## Implementation Plan

### Phase 1: Core Utilities (Week 1)

1. **REQ-1: `from_exponents()`** - HIGH priority
   - Implement function
   - Add comprehensive tests
   - Add documentation with examples
   - Migrate gf2-coding BCH code to use it

2. **REQ-5: `x()`** - Trivial helper
   - Implement as one-liner
   - Add basic test

3. **REQ-3: `monomial()`** - Medium priority
   - Implement with proper zero handling
   - Add tests for edge cases

### Phase 2: Advanced Constructors (Week 2)

4. **REQ-2: `from_roots()`** - Medium priority
   - Implement sequential multiplication
   - Add comprehensive tests
   - Consider future optimization with FFT

5. **REQ-4: `product()`** - Low priority
   - Implement if time permits
   - Useful for code construction

## Migration Path

### Step 1: Add to gf2-core

```rust
// gf2-core/src/gf2m/field.rs
impl Gf2mPoly {
    pub fn from_exponents(field: &Gf2mField, exponents: &[usize]) -> Self {
        // Implementation from gf2-coding
    }
}
```

### Step 2: Update gf2-coding

```rust
// gf2-coding/src/bch/dvb_t2/generators.rs

// BEFORE:
pub fn poly_from_exponents(field: &Gf2mField, exponents: &[usize]) -> Gf2mPoly {
    // ...
}

// AFTER:
#[deprecated(since = "0.2.0", note = "Use Gf2mPoly::from_exponents instead")]
pub fn poly_from_exponents(field: &Gf2mField, exponents: &[usize]) -> Gf2mPoly {
    Gf2mPoly::from_exponents(field, exponents)
}
```

### Step 3: Update all call sites

```rust
// OLD:
let g1 = poly_from_exponents(&field, &[0, 1, 3, 5, 14]);

// NEW:
let g1 = Gf2mPoly::from_exponents(&field, &[0, 1, 3, 5, 14]);
```

### Step 4: Remove deprecated function (v0.3.0)

After one release cycle with deprecation warning.

## Related Work

### Similar APIs in Other Libraries

**Python (SageMath)**:
```python
# Polynomial from coefficients
R.<x> = PolynomialRing(GF(2^8))
p = R([1, 0, 1, 0, 1])  # 1 + x^2 + x^4

# Polynomial from roots
roots = [alpha, alpha^2]
p = prod(x - r for r in roots)
```

**NTL (C++)**:
```cpp
GF2E::init(primitive_poly);
GF2EX p;
// Build from roots
BuildFromRoots(p, roots);
```

### Consistency with gf2-core API

Current `Gf2mPoly` constructors:
- ✅ `new(coeffs)` - from coefficient vector
- ✅ `zero(field)` - zero polynomial
- ✅ `constant(value)` - constant polynomial
- ✅ `from_bitvec()` - from bit vector representation

Proposed additions align with existing patterns.

## Success Criteria

- ✅ `Gf2mPoly::from_exponents()` implemented and tested
- ✅ All gf2-coding BCH code migrated to use new API
- ✅ No breaking changes to existing gf2-core API
- ✅ Comprehensive test coverage (>95%)
- ✅ Documentation with real-world examples
- ✅ Performance benchmarks show no regression

## Open Questions

1. **Field inference**: Should `from_roots()` infer field from roots, or require explicit field parameter?
   - **Recommendation**: Infer from roots (all roots must be from same field)

2. **Empty inputs**: Panic or return error type?
   - **Recommendation**: Panic (consistent with `Vec::new()`, `slice.first()` patterns)

3. **Optimization**: Should `from_roots()` use FFT for large n?
   - **Recommendation**: Start simple, optimize if benchmarks show need

4. **Naming**: `from_exponents()` vs `from_sparse()` vs `sparse()`?
   - **Recommendation**: `from_exponents()` is clearest (matches mathematical terminology)

## References

- BCH code implementation: `gf2-coding/src/bch/`
- DVB-T2 generator polynomials: `gf2-coding/src/bch/dvb_t2/generators.rs`
- Existing `Gf2mPoly` API: `gf2-core/src/gf2m/field.rs`
- ETSI EN 302 755 (DVB-T2 standard)
