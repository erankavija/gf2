# Reversed BitVec-Polynomial Conversion Migration

## Summary

`gf2-core` now provides `Gf2mPoly::from_bitvec_reversed()` and `to_bitvec_reversed()` methods that handle the bit-to-coefficient mapping required for BCH systematic encoding. This eliminates the need for manual conversion in test code.

## New API

```rust
impl Gf2mPoly {
    /// Maps bit i → coefficient of x^(n-1-i)
    pub fn from_bitvec_reversed(bits: &BitVec, field: &Gf2mField) -> Self;
    
    /// Maps coefficient of x^i → bit (len-1-i)
    pub fn to_bitvec_reversed(&self, len: usize) -> BitVec;
}
```

## Migration

### Before (manual workaround in `tests/bch_tests.rs`)

```rust
fn systematic_codeword_to_poly(
    codeword: &BitVec,
    k: usize,
    n: usize,
    field: &Gf2mField,
) -> Gf2mPoly {
    let mut coeffs = Vec::new();

    // Parity polynomial p(x): degrees 0..r-1
    for i in (k..n).rev() {
        coeffs.push(if codeword.get(i) {
            field.one()
        } else {
            field.zero()
        });
    }

    // Message polynomial x^r·m(x): degrees r..n-1
    for i in (0..k).rev() {
        coeffs.push(if codeword.get(i) {
            field.one()
        } else {
            field.zero()
        });
    }

    Gf2mPoly::new(coeffs)
}
```

### After (using new API)

```rust
fn systematic_codeword_to_poly(
    codeword: &BitVec,
    field: &Gf2mField,
) -> Gf2mPoly {
    Gf2mPoly::from_bitvec_reversed(codeword, field)
}
```

Or simply call directly:
```rust
let cw_poly = Gf2mPoly::from_bitvec_reversed(&cw, &field);
```

## Verification

The implementation was verified to produce identical results to the manual workaround. All existing tests should continue to pass with this simpler approach.

## Example Usage

```rust
use gf2_core::{BitVec, gf2m::{Gf2mField, Gf2mPoly}};

let field = Gf2mField::gf256();

// Systematic codeword: [message | parity]
let mut codeword = BitVec::new();
codeword.push_bit(true);  // message bit 0
codeword.push_bit(false); // message bit 1
codeword.push_bit(true);  // parity bit 0

// Convert to polynomial c(x) with bit 0 as highest coefficient
let poly = Gf2mPoly::from_bitvec_reversed(&codeword, &field);

// Roundtrip
let bits_back = poly.to_bitvec_reversed(codeword.len());
assert_eq!(codeword, bits_back);
```
