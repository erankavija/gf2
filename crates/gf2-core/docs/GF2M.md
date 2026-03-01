# GF(2^m) Extension Field Arithmetic

**API**: `gf2m::Gf2mField`, `gf2m::Gf2mElement`, `gf2m::Gf2mPoly`

For full API details, run `cargo doc -p gf2-core --no-deps --open` and see the `gf2m` module.

## Overview

Binary extension fields GF(2^m) are fundamental for algebraic error-correcting codes (BCH, Reed-Solomon) used in DVB-T2, DVB-S2, and similar standards.

- Field element arithmetic (add, multiply, divide, inversion, exponentiation)
- Table-based multiplication for m ≤ 16 (10× faster than schoolbook)
- Karatsuba and SIMD-accelerated multiplication for larger fields
- Polynomial operations (add, multiply, divide, GCD, evaluation)
- Thread-safe field sharing (`Arc`-based)
- Implements the generic `FiniteField` trait (see `field::FiniteField`)

## Architecture

All core types are generic over `V: UintExt`, a sealed trait implemented for
`u8`, `u16`, `u32`, `u64`, and `u128`. Type aliases default to `u64`:

```rust
pub struct Gf2mField_<V: UintExt = u64> { params: Arc<FieldParams_<V>> }
pub struct Gf2mElement_<V: UintExt = u64> { value: V, params: Arc<FieldParams_<V>> }
pub struct Gf2mPoly_<V: UintExt = u64> { coeffs: Vec<Gf2mElement_<V>> }

pub type Gf2mField = Gf2mField_<u64>;
pub type Gf2mElement = Gf2mElement_<u64>;
pub type Gf2mPoly = Gf2mPoly_<u64>;
```

`Gf2mElement_<V>` implements `FiniteField` for all `V: UintExt`, with `Wide = Self`
(XOR addition never overflows).

### Multiplication priority

1. **Table-based** (m ≤ 16): Log/antilog lookup — O(1)
2. **SIMD** (u64 + `simd` feature): Hardware PCLMULQDQ acceleration
3. **Schoolbook** (fallback): Polynomial multiplication with reduction

## Quick start

```rust
use gf2_core::gf2m::Gf2mField;

let field = Gf2mField::new(8, 0b100011101).with_tables();
let a = field.element(0x53);
let b = field.element(0xCA);
let product = &a * &b;

// Division, exponentiation
let quotient = &a / &b;
let power = a.pow(10);
```

## Performance

| Field | Multiply | vs NTL | vs SageMath |
|-------|----------|--------|-------------|
| GF(2^8) | 2.70 ns | 17× faster | 114,000× faster |
| GF(2^16) | 4.41 ns | 13× faster | 70,000× faster |
| GF(2^32) | 52.2 ns | 1.9× faster | 6,500× faster |

## References

- **Rustdocs**: `cargo doc -p gf2-core --no-deps --open`
- **Implementation**: `src/gf2m/`
- **Theory**: *Error Control Coding* by Lin & Costello
- **Standards**: ETSI EN 302 755 (DVB-T2), ETSI EN 302 307 (DVB-S2)
