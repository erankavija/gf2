# CubicExt<C> Implementation Plan

> Design plan for jit issue 3f3d6c23
> Parent story: 3f4b946c (Tower extension field architecture for GF(p^n))

## Overview

Implement `CubicExt<C>`, the cubic extension type where elements are
`c0 + c1·v + c2·v²` with `v³ = β` (the non-residue from `C: ExtConfig`).

### Multiplication algorithm choice

The issue title says "Toom-Cook" but **Toom-Cook requires division by 2, 3, and 6**
in the base field, which fails for characteristic 2 and 3. Since we want generic
cubic extensions over any base field, we use the **Karatsuba-style 6-mul** formula
from Devegili et al. (ePrint 2006/471) instead. This is what arkworks, blstrs, and
constantine all use.

If a Toom-Cook specialization for large-characteristic base fields is desired later,
it can be added as an optional method on `ExtConfig`.

## Struct and ergonomics

```rust
/// Element of a cubic extension field: `c0 + c1·v + c2·v²` where `v³ = β`.
///
/// Parameterized by a config type `C: ExtConfig` that specifies the base field
/// and non-residue.
///
/// # Examples
///
/// ```
/// use gf2_core::gfp::Fp;
/// use gf2_core::gfpn::{ExtConfig, CubicExt};
/// use gf2_core::field::{FiniteField, ConstField};
///
/// struct Fq3Config;
/// impl ExtConfig for Fq3Config {
///     type BaseField = Fp<7>;
///     const NON_RESIDUE: Fp<7> = Fp::<7>::new(3); // β = 3 (non-cube mod 7)
/// }
/// type Fq3 = CubicExt<Fq3Config>;
///
/// let a = Fq3::new(Fp::new(1), Fp::new(2), Fp::new(3));
/// assert_eq!(a.c0().value(), 1);
/// assert!(Fq3::zero().is_zero());
/// assert!(Fq3::one().is_one());
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct CubicExt<C: ExtConfig> {
    c0: C::BaseField,
    c1: C::BaseField,
    c2: C::BaseField,
}
```

### Constructors and accessors

```rust
impl<C: ExtConfig> CubicExt<C> {
    /// Creates a new element c0 + c1·v + c2·v².
    pub const fn new(c0: C::BaseField, c1: C::BaseField, c2: C::BaseField) -> Self;

    /// Returns the constant component c0.
    pub const fn c0(&self) -> C::BaseField;

    /// Returns the linear component c1.
    pub const fn c1(&self) -> C::BaseField;

    /// Returns the quadratic component c2.
    pub const fn c2(&self) -> C::BaseField;

    /// Returns the field norm (a base field element).
    /// N(a) = a0³ + β·a1³ + β²·a2³ − 3β·a0·a1·a2
    pub fn norm(&self) -> C::BaseField;
}
```

### Embedding and Display

```rust
// Embed base field into extension: a ↦ a + 0·v + 0·v²
impl<C: ExtConfig> From<C::BaseField> for CubicExt<C>;

// Display: "1 + 2·v + 3·v²" (omit zero terms)
impl<C: ExtConfig> fmt::Display for CubicExt<C>;

// Debug: "CubicExt(1, 2, 3)"
impl<C: ExtConfig> fmt::Debug for CubicExt<C>;
```

## Arithmetic

### Addition / Subtraction / Negation (component-wise)

```rust
fn add(self, rhs: Self) -> Self {
    Self::new(self.c0 + rhs.c0, self.c1 + rhs.c1, self.c2 + rhs.c2)
}

fn sub(self, rhs: Self) -> Self {
    Self::new(self.c0 - rhs.c0, self.c1 - rhs.c1, self.c2 - rhs.c2)
}

fn neg(self) -> Self {
    Self::new(-self.c0, -self.c1, -self.c2)
}
```

### Multiplication (Karatsuba-style, 6M)

Reference: Devegili, O hEigeartaigh, Scott, Dahab (ePrint 2006/471).

Given `a = a0 + a1·v + a2·v²` and `b = b0 + b1·v + b2·v²` where `v³ = β`:

```rust
fn mul(self, rhs: Self) -> Self {
    let (a0, a1, a2) = (self.c0, self.c1, self.c2);
    let (b0, b1, b2) = (rhs.c0, rhs.c1, rhs.c2);

    // Three diagonal products
    let v0 = a0 * b0;     // a0·b0
    let v1 = a1 * b1;     // a1·b1
    let v2 = a2 * b2;     // a2·b2

    // Three cross products via Karatsuba identity
    // x = a1·b2 + a2·b1
    let x = (a1 + a2) * (b1 + b2) - v1 - v2;
    // y = a0·b1 + a1·b0
    let y = (a0 + a1) * (b0 + b1) - v0 - v1;
    // z = a0·b2 + a1·b1 + a2·b0  (note: v1 added back)
    let z = (a0 + a2) * (b0 + b2) - v0 + v1 - v2;

    // Assemble with reduction: v³ = β shifts degree-3+ terms down
    let c0 = v0 + C::mul_by_non_residue(x);       // v0 + β·x
    let c1 = y + C::mul_by_non_residue(v2);        // y + β·v2
    let c2 = z;

    Self::new(c0, c1, c2)
}
```

Operation count: 6M + 13A + 2B (M = base mul, A = add/sub, B = mul_by_non_residue).

### Inversion (adjugate/norm method)

Reference: Beuchat et al. (ePrint 2010/354); arkworks cubic_extension.rs.

```rust
fn inv(&self) -> Option<Self> {
    let (a0, a1, a2) = (self.c0, self.c1, self.c2);

    // Auxiliary values
    let a0_sq = a0 * a0;
    let a1_sq = a1 * a1;
    let a2_sq = a2 * a2;
    let a0a1 = a0 * a1;
    let a0a2 = a0 * a2;
    let a1a2 = a1 * a2;

    // Cofactors of the adjugate
    let s0 = a0_sq - C::mul_by_non_residue(a1a2);     // a0² − β·a1·a2
    let s1 = C::mul_by_non_residue(a2_sq) - a0a1;     // β·a2² − a0·a1
    let s2 = a1_sq - a0a2;                             // a1² − a0·a2

    // Norm = a0·s0 + β·(a2·s1 + a1·s2)
    let norm = a0 * s0
        + C::mul_by_non_residue(a2 * s1 + a1 * s2);

    norm.inv().map(|norm_inv| {
        Self::new(s0 * norm_inv, s1 * norm_inv, s2 * norm_inv)
    })
}
```

Operation count: 9M + 3S + 1I + 9A + 4B.

### Division

```rust
fn div(self, rhs: Self) -> Self {
    self * rhs.inv().expect("division by zero in CubicExt")
}
```

## Operator boilerplate

Same pattern as `QuadraticExt<C>` — ref-forwarding via dereference since the type
is `Copy`. Share the macro if one was created for QuadraticExt.

## FiniteField implementation

```rust
impl<C: ExtConfig> FiniteField for CubicExt<C>
where
    C::BaseField: FiniteField<Characteristic = u64>,
{
    type Characteristic = u64;
    type Wide = Self;  // identity, deferred to d11b769a

    fn characteristic(&self) -> u64 { self.c0.characteristic() }
    fn extension_degree(&self) -> usize { 3 * self.c0.extension_degree() }
    fn is_zero(&self) -> bool {
        self.c0.is_zero() && self.c1.is_zero() && self.c2.is_zero()
    }
    fn is_one(&self) -> bool {
        self.c0.is_one() && self.c1.is_zero() && self.c2.is_zero()
    }

    fn inv(&self) -> Option<Self> { /* adjugate/norm, as above */ }

    fn zero_like(&self) -> Self {
        let z = self.c0.zero_like();
        Self::new(z, z, z)
    }
    fn one_like(&self) -> Self {
        Self::new(self.c0.one_like(), self.c0.zero_like(), self.c0.zero_like())
    }

    fn to_wide(&self) -> Self { *self }
    fn mul_to_wide(&self, rhs: &Self) -> Self { *self * *rhs }
    fn reduce_wide(wide: &Self) -> Self { *wide }
    fn max_unreduced_additions() -> usize { usize::MAX }
}
```

## ConstField implementation

```rust
impl<C: ExtConfig> ConstField for CubicExt<C>
where
    C::BaseField: ConstField<Characteristic = u64>,
{
    fn zero() -> Self {
        Self::new(C::BaseField::zero(), C::BaseField::zero(), C::BaseField::zero())
    }
    fn one() -> Self {
        Self::new(C::BaseField::one(), C::BaseField::zero(), C::BaseField::zero())
    }
    fn order() -> u128 {
        let base_order = C::BaseField::order();
        base_order * base_order * base_order  // |F|³
    }
}
```

## Test plan

### Axiom test harness (required for success)

```rust
#[test]
fn test_cubic_ext_fp7_field_axioms() {
    // Strategy: generate random (c0, c1, c2) triples from [0, 7)³
    let strategy = (0..7u64, 0..7u64, 0..7u64)
        .prop_map(|(c0, c1, c2)| Fq3::new(Fp::new(c0), Fp::new(c1), Fp::new(c2)))
        .boxed();
    test_const_field_axioms(strategy, 7);
}
```

### Exhaustive multiplication cross-check

For a small field (e.g., GF(5³) with 125 elements), verify Karatsuba multiplication
matches naive schoolbook polynomial multiplication mod (v³ − β) for a representative
subset of element pairs.

### Inversion round-trip

For all non-zero elements of GF(5³), verify `a * a.inv() == one`.

### Known value tests

Hand-computed values for GF(7³) with β = 3:
- `v³ = 3` — verify `v * v * v == Fq3::from(Fp::new(3))`
- Specific multiplication results cross-checked against SageMath

### Extension degree and order

- `extension_degree() == 3`
- `order() == 343` for GF(7³)

## File layout

```
crates/gf2-core/src/gfpn/
├── mod.rs           — re-exports ExtConfig, QuadraticExt, CubicExt
├── ext_config.rs    — ExtConfig trait (done, 0203bebd)
├── quadratic.rs     — QuadraticExt<C> (8d2863e6)
└── cubic.rs         — CubicExt<C> (this issue)
```

## Dependencies

- 0203bebd (ExtConfig trait) — **done**
- 8d2863e6 (QuadraticExt) — not a hard dependency, but should land first since
  CubicExt follows the same patterns and QuadraticExt is simpler to verify.
  Also, nested towers (86b3dc7d) need both.

## Not in scope (deferred)

- Wide accumulator with actual lazy reduction → d11b769a
- Chung-Hasan SQR2 optimized squaring → future optimization (the blanket
  `FiniteFieldExt::square()` works correctly, just uses generic multiplication)
- Toom-Cook 5-mul specialization for large-characteristic fields → future optimization
