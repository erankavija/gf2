# Wide Accumulator Integration for Tower Types

**Issue**: d11b769a — Wide accumulator integration for tower types
**Parent**: 3f4b946c — Tower extension field architecture for GF(p^n)

## Problem

Currently `QuadraticExt<C>` (and the planned `CubicExt<C>`) use `type Wide = Self` as a placeholder. This means:

- `mul_to_wide` just calls regular `mul` (does full reduction at every tower level)
- `to_wide`/`reduce_wide` are identity operations
- `max_unreduced_additions()` returns `usize::MAX` (lie — no delayed reduction)

This defeats the purpose of the Wide accumulator protocol: enabling dot-product-style loops that accumulate multiple products before a single expensive reduction.

## Background: How Wide works for Fp

For `Fp<P>` (prime field with 64-bit elements):
- `type Wide = u128` — double-width, holds unreduced products
- `mul_to_wide`: multiplies values (not Montgomery form), returns `u128`
- `reduce_wide`: `(*wide % P) as u64`, then converts back to Montgomery
- `max_unreduced_additions()`: `u128::MAX / ((P-1)*(P-1))` — how many `u128` products can be summed before overflow

A dot product loop looks like:
```rust
let mut acc: u128 = 0;
for (a, b) in pairs {
    acc += a.mul_to_wide(&b);  // no reduction per iteration
}
let result = Fp::reduce_wide(&acc);  // single reduction at end
```

## Design: Tower-level Wide types

### QuadraticExt wide type

For `QuadraticExt<C>` where `C::BaseField` has `Wide = W`, the wide representation stores component-wise wide values:

```rust
/// Wide accumulator for QuadraticExt: two base-field wide components.
///
/// Stores unreduced c0 and c1 components. Reduction is deferred to
/// the base field level.
#[derive(Clone)]
pub struct QuadraticExtWide<W: Clone + Add<Output = W> + AddAssign> {
    c0: W,
    c1: W,
}
```

For `QuadraticExt<Fp7Config>` where `Fp<7>::Wide = u128`, this becomes `QuadraticExtWide<u128>` = `(u128, u128)` — exactly as described in the parent issue.

### CubicExt wide type

```rust
/// Wide accumulator for CubicExt: three base-field wide components.
#[derive(Clone)]
pub struct CubicExtWide<W: Clone + Add<Output = W> + AddAssign> {
    c0: W,
    c1: W,
    c2: W,
}
```

### FiniteField impl changes

For `QuadraticExt<C>`:

```rust
impl<C: ExtConfig> FiniteField for QuadraticExt<C> {
    type Wide = QuadraticExtWide<<C::BaseField as FiniteField>::Wide>;

    fn to_wide(&self) -> Self::Wide {
        QuadraticExtWide {
            c0: self.c0.to_wide(),
            c1: self.c1.to_wide(),
        }
    }

    fn mul_to_wide(&self, rhs: &Self) -> Self::Wide {
        // Karatsuba in wide arithmetic:
        // v0 = a0 * b0            (wide)
        // v1 = a1 * b1            (wide)
        // v2 = (a0+a1) * (b0+b1)  (wide)
        // c0_wide = v0 + β·v1     (wide, but β·v1 needs thought)
        // c1_wide = v2 - v0 - v1  (wide)
        //
        // Problem: β·v1 is a multiplication of a field element by a wide
        // value — this doesn't fit the Wide protocol. See "Key challenge" below.
        //
        // Practical approach: just do the full multiplication and convert result.
        let result = *self * *rhs;
        result.to_wide()
    }

    fn reduce_wide(wide: &Self::Wide) -> Self {
        Self::new(
            C::BaseField::reduce_wide(&wide.c0),
            C::BaseField::reduce_wide(&wide.c1),
        )
    }

    fn max_unreduced_additions() -> usize {
        // Both components accumulate independently, so the limit
        // is the base field's limit.
        C::BaseField::max_unreduced_additions()
    }
}
```

### Key challenge: mul_to_wide for tower types

True lazy tower multiplication (Karatsuba entirely in wide arithmetic) requires multiplying a wide value by the non-residue β, which is a *field element × wide value* operation — not supported by the current `Wide` trait bounds (`Clone + Add + AddAssign`).

**Options**:

1. **Practical approach (recommended)**: `mul_to_wide` performs full Karatsuba reduction at the tower level, then converts the result to wide via `to_wide()`. The Wide type still enables accumulation of *sums of products* without reduction — the value comes from the `+=` in dot-product loops, not from delaying within a single multiplication.

2. **Full lazy approach**: Add a `MulByScalar` bound to `Wide` that allows `β * wide_value`. This adds complexity to the trait hierarchy and only helps when the base field's reduction is the bottleneck (not the case for small primes where Montgomery reduction is cheap).

3. **Specialized approach**: Only provide true lazy reduction for specific concrete types (e.g., `QuadraticExt<Fp<P>>` where we know `Wide = u128` and can manually compute `β_raw * wide_value`). This breaks generality.

**Decision**: Option 1. The primary use case for Wide is accumulating dot products (`sum += a_i * b_i`). The per-element `mul_to_wide` does a full multiplication, but the accumulated sum stays unreduced across iterations. This gives the same asymptotic benefit (O(1) reductions per dot product instead of O(n)) without complicating the trait hierarchy.

### Add and AddAssign for wide types

These are straightforward component-wise operations:

```rust
impl<W: Clone + Add<Output = W> + AddAssign> Add for QuadraticExtWide<W> {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self { c0: self.c0 + rhs.c0, c1: self.c1 + rhs.c1 }
    }
}

impl<W: Clone + Add<Output = W> + AddAssign> AddAssign for QuadraticExtWide<W> {
    fn add_assign(&mut self, rhs: Self) {
        self.c0 += rhs.c0;
        self.c1 += rhs.c1;
    }
}
```

Same pattern for `CubicExtWide` with three components.

## File changes

| File | Change |
|------|--------|
| `gfpn/quadratic.rs` | Add `QuadraticExtWide<W>` struct, impl `Add`/`AddAssign`/`Clone`. Change `type Wide` from `Self` to `QuadraticExtWide<...>`. Update `to_wide`, `mul_to_wide`, `reduce_wide`, `max_unreduced_additions`. |
| `gfpn/cubic.rs` | Same pattern: `CubicExtWide<W>` with three components. (Blocked on 3f3d6c23 completing first.) |
| `gfpn/mod.rs` | Re-export `QuadraticExtWide`, `CubicExtWide`. |

## Ordering constraint

CubicExt (3f3d6c23) must be implemented first — this issue depends on both QuadraticExt and CubicExt. However, the QuadraticExt wide integration can be developed and tested immediately since QuadraticExt already exists. The CubicExt wide integration is additive.

Suggested approach: implement and test QuadraticExtWide first, then add CubicExtWide once 3f3d6c23 lands.

## Test plan

1. **Wide roundtrip**: `reduce_wide(to_wide(a)) == a` for all 49 elements of GF(7²). (Already tested by axiom harness — will start passing with real Wide type.)

2. **mul_to_wide consistency**: `reduce_wide(mul_to_wide(a, b)) == a * b` for proptest-generated pairs. (Already in axiom harness.)

3. **Accumulation correctness**: Manual dot-product test — accumulate N products in wide, reduce once, compare to element-wise multiply-and-add.

4. **max_unreduced_additions correctness**: Verify it equals `Fp::<P>::max_unreduced_additions()` for `QuadraticExt<FpConfig>`.

5. **Overflow safety**: Accumulate exactly `max_unreduced_additions()` products and verify no panic/overflow; verify that `max_unreduced_additions() + 1` could theoretically overflow (test with known worst-case values).

6. **Nested tower**: Once GF(p^4) = `QuadraticExt<QuadraticExt<Fp<P>>>` exists (86b3dc7d), verify that `Wide` propagates correctly: the wide type should be `QuadraticExtWide<QuadraticExtWide<u128>>`, and `max_unreduced_additions` should still equal the base `Fp`'s bound.

## Success criteria

- `QuadraticExt` and `CubicExt` pass the existing axiom test harness (which tests Wide roundtrip and mul_to_wide consistency).
- `type Wide` is no longer `Self` — it's a proper accumulator that separates multiplication from reduction.
- `max_unreduced_additions()` returns the base field's bound (not `usize::MAX`).
- A dot-product loop over `QuadraticExt` elements can accumulate up to `max_unreduced_additions()` products without overflow.
