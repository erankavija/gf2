# QuadraticExt<C> Implementation Plan

> Design plan for jit issue 8d2863e6
> Parent story: 3f4b946c (Tower extension field architecture for GF(p^n))

## Overview

Implement `QuadraticExt<C>`, the quadratic extension type where elements are
`c0 + c1·u` with `u² = β` (the non-residue from `C: ExtConfig`). Uses Karatsuba
multiplication (3 base-field muls instead of 4) and norm-based inversion.

## Struct and ergonomics

```rust
/// Element of a quadratic extension field: `c0 + c1·u` where `u² = β`.
///
/// Parameterized by a config type `C: ExtConfig` that specifies the base field
/// and non-residue. Two extensions with different configs are distinct types.
///
/// # Examples
///
/// ```
/// use gf2_core::gfp::Fp;
/// use gf2_core::gfpn::{ExtConfig, QuadraticExt};
/// use gf2_core::field::{FiniteField, ConstField};
///
/// struct Fq2Config;
/// impl ExtConfig for Fq2Config {
///     type BaseField = Fp<7>;
///     const NON_RESIDUE: Fp<7> = Fp::<7>::new(6); // β = −1
/// }
/// type Fq2 = QuadraticExt<Fq2Config>;
///
/// let a = Fq2::new(Fp::new(3), Fp::new(5));
/// let b = Fq2::new(Fp::new(2), Fp::new(4));
/// let c = a * b;
///
/// assert_eq!(a.c0().value(), 3);
/// assert_eq!(a.c1().value(), 5);
/// assert!(Fq2::zero().is_zero());
/// assert!(Fq2::one().is_one());
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct QuadraticExt<C: ExtConfig> {
    c0: C::BaseField,
    c1: C::BaseField,
}
```

### Constructors and accessors

```rust
impl<C: ExtConfig> QuadraticExt<C> {
    /// Creates a new element c0 + c1·u.
    pub const fn new(c0: C::BaseField, c1: C::BaseField) -> Self;

    /// Returns the real component c0.
    pub const fn c0(&self) -> C::BaseField;

    /// Returns the imaginary component c1.
    pub const fn c1(&self) -> C::BaseField;

    /// Returns the conjugate: c0 − c1·u.
    pub fn conjugate(&self) -> Self;

    /// Returns the field norm: c0² − β·c1² (a base field element).
    pub fn norm(&self) -> C::BaseField;
}
```

### Embedding and Display

```rust
// Embed base field into extension: a ↦ a + 0·u
impl<C: ExtConfig> From<C::BaseField> for QuadraticExt<C>;

// Display: "3 + 5·u" or "3" (if c1 == 0) or "5·u" (if c0 == 0)
impl<C: ExtConfig> fmt::Display for QuadraticExt<C>;

// Debug: "QuadraticExt(3, 5)"
impl<C: ExtConfig> fmt::Debug for QuadraticExt<C>;
```

## Arithmetic

### Addition / Subtraction (component-wise)

```rust
// (a0 + a1·u) + (b0 + b1·u) = (a0+b0) + (a1+b1)·u
fn add(self, rhs: Self) -> Self {
    Self::new(self.c0 + rhs.c0, self.c1 + rhs.c1)
}

// (a0 + a1·u) − (b0 + b1·u) = (a0−b0) + (a1−b1)·u
fn sub(self, rhs: Self) -> Self {
    Self::new(self.c0 - rhs.c0, self.c1 - rhs.c1)
}

// −(a0 + a1·u) = (−a0) + (−a1)·u
fn neg(self) -> Self {
    Self::new(-self.c0, -self.c1)
}
```

### Multiplication (Karatsuba, 3M)

Reference: Devegili, O hEigeartaigh, Scott, Dahab (ePrint 2006/471).

```rust
// (a0 + a1·u)(b0 + b1·u) where u² = β
fn mul(self, rhs: Self) -> Self {
    let v0 = self.c0 * rhs.c0;         // a0·b0
    let v1 = self.c1 * rhs.c1;         // a1·b1

    // c0 = v0 + β·v1
    let c0 = v0 + C::mul_by_non_residue(v1);

    // c1 = (a0+a1)(b0+b1) − v0 − v1
    let c1 = (self.c0 + self.c1) * (rhs.c0 + rhs.c1) - v0 - v1;

    Self::new(c0, c1)
}
```

Operation count: 3M + 5A + 1B (M = base mul, A = add/sub, B = mul_by_non_residue).

### Inversion (norm-based)

```rust
// (a0 + a1·u)⁻¹ = conjugate(a) / norm(a)
//   norm = a0² − β·a1²  (base field element)
//   result = (a0/norm, −a1/norm)
fn inv(&self) -> Option<Self> {
    let t0 = self.c0 * self.c0;               // a0²
    let t1 = self.c1 * self.c1;               // a1²
    let norm = t0 - C::mul_by_non_residue(t1); // a0² − β·a1²
    norm.inv().map(|norm_inv| {
        Self::new(self.c0 * norm_inv, -(self.c1 * norm_inv))
    })
}
```

Operation count: 1I + 2S + 3M + 1B + 1A (I = base inversion).

### Division

```rust
fn div(self, rhs: Self) -> Self {
    self * rhs.inv().expect("division by zero in QuadraticExt")
}
```

## Operator boilerplate

Since `QuadraticExt<C>` is `Copy`, ref-forwarding operators dereference and delegate:

```rust
impl Add<&Self> for QuadraticExt<C> { ... }
impl Add for &QuadraticExt<C> { ... }
// Same for Sub, Mul, Div, Neg
impl AddAssign for QuadraticExt<C> { ... }
impl AddAssign<&Self> for QuadraticExt<C> { ... }
```

A private macro `impl_ref_forwarding!` can reduce this to one invocation per type,
but is optional — manual impls are fine for two types.

## FiniteField implementation

```rust
impl<C: ExtConfig> FiniteField for QuadraticExt<C> {
    // Inherit characteristic type from base field — no u64 assumption.
    type Characteristic = <C::BaseField as FiniteField>::Characteristic;

    // Wide = Self for now (identity, no lazy reduction).
    // Proper wide accumulator deferred to issue d11b769a.
    type Wide = Self;

    fn characteristic(&self) -> Self::Characteristic { self.c0.characteristic() }
    fn extension_degree(&self) -> usize { 2 * self.c0.extension_degree() }
    fn is_zero(&self) -> bool { self.c0.is_zero() && self.c1.is_zero() }
    fn is_one(&self) -> bool { self.c0.is_one() && self.c1.is_zero() }

    fn inv(&self) -> Option<Self> { /* norm-based, as above */ }

    fn zero_like(&self) -> Self {
        Self::new(self.c0.zero_like(), self.c1.zero_like())
    }
    fn one_like(&self) -> Self {
        Self::new(self.c0.one_like(), self.c1.zero_like())
    }

    // Wide = Self (identity, deferred to d11b769a)
    fn to_wide(&self) -> Self { *self }
    fn mul_to_wide(&self, rhs: &Self) -> Self { *self * *rhs }
    fn reduce_wide(wide: &Self) -> Self { *wide }
    fn max_unreduced_additions() -> usize { usize::MAX }
}
```

## ConstField implementation

```rust
impl<C: ExtConfig> ConstField for QuadraticExt<C> {
    fn zero() -> Self {
        Self::new(C::BaseField::zero(), C::BaseField::zero())
    }
    fn one() -> Self {
        Self::new(C::BaseField::one(), C::BaseField::zero())
    }
    fn order() -> u128 {
        let base_order = C::BaseField::order();
        base_order * base_order  // |F|² for quadratic extension
    }
}
```

## Test plan

### Axiom test harness (required for success)

```rust
#[test]
fn test_quadratic_ext_fp7_field_axioms() {
    // Strategy: generate random (c0, c1) pairs from [0, 7) × [0, 7)
    let strategy = (0..7u64, 0..7u64)
        .prop_map(|(c0, c1)| Fq2::new(Fp::new(c0), Fp::new(c1)))
        .boxed();
    test_const_field_axioms(strategy, 7);
}
```

### Exhaustive arithmetic cross-check (GF(7²), 49 elements)

For all 49×49 = 2401 pairs, verify:
- `(a * b).c0` and `(a * b).c1` match naive polynomial multiplication mod (u² − β)
- `a * a.inv() == one` for all non-zero a

### Known value tests

Hand-computed values for GF(7²) with β = 6 (= −1):
- `(1 + u)(1 − u) = 1 − u² = 1 − (−1) = 2`
- `u² = β = 6 (= −1 mod 7)`
- `(3 + 2u)(4 + 5u) = 12 + 15u + 8u + 10u² = 12 + 23u + 10·(−1) = 2 + 23u = 2 + 2u (mod 7)`

### Embedding test

- `Fq2::from(Fp::new(3))` should equal `Fq2::new(Fp::new(3), Fp::new(0))`

### Extension degree and order

- `extension_degree() == 2`
- `order() == 49` for GF(7²)

## File layout

```
crates/gf2-core/src/gfpn/
├── mod.rs           — re-exports ExtConfig, QuadraticExt
├── ext_config.rs    — ExtConfig trait (done, 0203bebd)
└── quadratic.rs     — QuadraticExt<C> (this issue)
```

## Dependencies

- 0203bebd (ExtConfig trait) — **done**
- 2248b17d (axiom test harness) — must pass

## Not in scope (deferred)

- Wide accumulator with actual lazy reduction → d11b769a
- Optimized squaring (complex squaring when β = −1) → future optimization
- Frobenius map precomputation → future optimization
