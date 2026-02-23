# GF(p^n) Groundwork: Architectural Analysis (v2)

> Analysis for epic e095a100 — "Implement general Galois field GF(p^m) arithmetic"
>
> This document examines the current architecture, identifies the key decisions that must
> be made before implementation begins, and proposes a groundwork plan that preserves
> binary-field ergonomics while enabling extreme performance for general finite fields.
>
> v2: Revised after critical review. Addresses trait design issues for runtime-parameterized
> types, characteristic representation, missing `Fp<P>` groundwork, dependency graph
> corrections, and `Hash`/`Ord` considerations.

## 1. Scope and Constraints

The goal is to expand from GF(2)/GF(2^m) to a general-purpose finite field toolkit covering
GF(p), GF(2^n), and GF(p^n). Four non-negotiable constraints govern the design:

1. **The project name will evolve** to reflect the broader scope (e.g., `gfpn`). The
   architecture must support this transition without breaking existing users.
2. **Performance is non-negotiable.** Every abstraction must compile to zero overhead for
   known field types. We design for extreme performance from the start.
3. **No ergonomic degradation** for existing binary field calculations. `BitVec`, `BitMatrix`,
   and GF(2) operations must remain as direct and clean as they are today.
4. **Correctness is non-negotiable.** Property-based tests for field axioms gate every
   implementation.

## 2. Current Architecture

| Layer | Design | Limitation for GF(p^n) |
|-------|--------|------------------------|
| **Field types** | `Gf2mElement { value: u64, params: Arc<FieldParams> }` | Runtime-parameterized. Arc clone on every arithmetic result. No type-level encoding. |
| **Traits** | None for fields. Coding traits (`BlockEncoder` etc.) hardcoded to `BitVec`. | Cannot write generic algorithms over arbitrary fields. |
| **Kernels** | `Backend` trait = bitwise ops (AND/OR/XOR/NOT/popcount). | No modular arithmetic, no PCLMULQDQ dispatch, no integer SIMD. |
| **SIMD crate** | `gf2-kernels-simd` = logical ops + GF(2^m) carryless mul. | No GF(p) modular reduction kernels. |
| **Binary field API** | `BitVec`, `BitMatrix`, M4RM, Gauss-Jordan — mature, ergonomic. | Must remain untouched. |

### Current element creation pattern

Elements are always created through a field handle, never standalone:

```rust
let field = Gf2mField::gf256().with_tables();
let a = field.element(42);   // Arc::clone of field's params
let z = field.zero();        // Arc::clone → Gf2mElement { value: 0, params: ... }
let o = field.one();         // Arc::clone → Gf2mElement { value: 1, params: ... }
```

There is no `Gf2mElement::new()` — elements cannot exist without a field context. Every
arithmetic result (`a + b`, `a * b`) also clones the Arc. Field identity is checked by
`Arc::ptr_eq`, so two independently-constructed `Gf2mField::gf256()` instances produce
elements that cannot be mixed, even though mathematically they are the same field.

Current trait impls on `Gf2mElement`: `Clone`, `Debug`, `PartialEq`, `Eq`, `Add`, `Mul`,
`Div`, `Display`. **Not** implemented: `Sub`, `Neg`, `Hash`, `Ord`, `Copy`.

## 3. The Central Design Tension

There are two ways to represent "which field an element belongs to":

**Const-generic types** like `Fp<const P: u64>` encode the field in the type itself. The
compiler monomorphizes every operation — `zero()`, `one()`, `mul()` — into field-specific
machine code with zero runtime overhead. This is the path to extreme performance.

**Runtime-parameterized types** like the current `Gf2mElement` carry field parameters via
`Arc<FieldParams>`. This allows arbitrary `m` at runtime but pays an atomic refcount bump
on every arithmetic result. For table-based GF(2^m) multiplication (a few lookups), the Arc
overhead can dominate the actual math.

GF(p^n) needs both: const-generic for production fields with known parameters, runtime for
research exploration. **The trait hierarchy must make const-generic the primary design and
treat runtime parameterization as a compatibility layer, not the other way around.**

### The `zero()`/`one()` problem

This tension creates a fundamental API conflict. For const-generic types, `Fp::<7>::zero()`
is trivial — the type itself encodes the field, so `fn zero() -> Self` works as a plain
associated function. But for runtime-parameterized types like `Gf2mElement`, a zero element
requires field context (which `Arc<FieldParams>` to use? what is `m`?). The current API
creates zero/one through the field handle (`field.zero()`), not as a freestanding function.

A single trait cannot ergonomically serve both patterns with `fn zero() -> Self`. The
resolution is to split the concern.

## 4. Decisions

### 4.1 Trait Hierarchy: Two Tiers

The design uses two trait tiers: `FiniteField` for all field elements (including
runtime-parameterized), and `ConstField` for compile-time-known fields where zero-cost
identity construction is possible.

```rust
/// A finite field element. Covers both const-generic and runtime-parameterized fields.
///
/// The trait does NOT include `zero()` or `one()` as associated functions because
/// runtime-parameterized types cannot construct identity elements without field context.
/// Use `is_zero()` / `is_one()` for testing, and `ConstField::zero()` / `one()` for
/// construction when the field is known at compile time.
pub trait FiniteField:
    Sized + Clone + PartialEq + Eq + Hash + Debug
    + Add<Output = Self> + Sub<Output = Self>
    + Mul<Output = Self> + Div<Output = Self>
    + Neg<Output = Self>
    + for<'a> Add<&'a Self, Output = Self>
    + for<'a> Sub<&'a Self, Output = Self>
    + for<'a> Mul<&'a Self, Output = Self>
    + for<'a> Div<&'a Self, Output = Self>
{
    /// The type used to represent the field characteristic.
    /// `u64` for small fields, `[u64; N]` or a bigint for cryptographic fields.
    type Characteristic: Clone + Debug + PartialEq + Eq;

    /// Field characteristic (2 for binary fields, p for GF(p^n)).
    fn characteristic() -> Self::Characteristic;

    /// Extension degree (1 for prime fields, m for GF(p^m)).
    fn extension_degree() -> usize;

    /// Test for the additive identity.
    fn is_zero(&self) -> bool;

    /// Test for the multiplicative identity.
    fn is_one(&self) -> bool;

    /// Multiplicative inverse. Returns `None` for zero.
    fn inv(&self) -> Option<Self>;

    /// Additive identity, constructed relative to an existing element's field context.
    /// For const-generic types this ignores the argument; for runtime types it clones
    /// the field context.
    fn zero_like(&self) -> Self;

    /// Multiplicative identity, constructed relative to an existing element's field context.
    fn one_like(&self) -> Self;
}

/// A finite field where the field itself is fully determined at compile time.
/// Enables zero-cost identity construction and `Copy` semantics.
///
/// All const-generic field types (`Fp<P>`, `Gf2_8`, `Gf2_16`) implement this.
/// Runtime-parameterized types (`Gf2mElement`) do NOT.
pub trait ConstField: FiniteField + Copy {
    /// The additive identity. Zero-cost for const-generic types.
    fn zero() -> Self;

    /// The multiplicative identity. Zero-cost for const-generic types.
    fn one() -> Self;

    /// Field order (number of elements). `p^m`.
    fn order() -> u64;
}
```

**Design rationale**:

- **`zero_like(&self)` / `one_like(&self)`**: Every `FiniteField` element can produce
  identity elements by cloning its own field context. This works for both const-generic
  types (ignore `self`, return a literal) and runtime types (clone the Arc). Generic
  algorithms that need identities can call `some_element.zero_like()`.

- **`ConstField::zero()` / `one()`**: When the field is known at compile time, we want
  the clean `Fp::<7>::zero()` syntax. This subtrait provides it, but only for types that
  can support it. Algorithms that need freestanding identity construction use
  `T: ConstField` as their bound.

- **`Characteristic` as associated type**: `u64` for fields with small characteristic
  (all current use cases). A future `BigUint` or `[u64; 4]` for BLS12-381-class fields.
  This avoids baking a 64-bit limit into the core trait.

- **`Hash` in the supertraits**: Field elements in `HashSet`/`HashMap` is common in
  algebraic algorithms (syndrome tables, Groebner bases, polynomial GCD caching). The
  current `Gf2mElement` doesn't implement `Hash` — this must be added. For
  runtime-parameterized types, `Hash` hashes only the value, not the field context
  (elements from different fields should never be mixed, enforced by `Arc::ptr_eq` at
  arithmetic time).

- **`Div` in the supertraits**: Ergonomic `a / b` is expected for field arithmetic. The
  blanket impl is `self * rhs.inv().expect("division by zero")`. The current
  `Gf2mElement` already implements `Div`.

- **`Sub` in the supertraits**: In GF(2^m) sub = add, but in GF(p) subtraction is
  distinct. The trait requires both. For characteristic-2 types, `Sub` delegates to `Add`.

- **No `Ord`**: Field elements have no natural ordering. Algorithms that need sorting
  (e.g., for canonical forms) should sort by the internal representation, not by a
  mathematical ordering that doesn't exist. We do not include `Ord` in the trait, but
  individual types may implement it if useful.

### 4.1.1 Extension Trait for Convenience Methods

```rust
/// Convenience methods with default implementations. Auto-implemented for all FiniteField.
pub trait FiniteFieldExt: FiniteField {
    /// Square: `self * self`.
    fn square(&self) -> Self {
        self.clone() * self
    }

    /// Exponentiation by squaring.
    fn pow(&self, exp: u64) -> Self {
        // Standard square-and-multiply using one_like/zero_like
        let mut result = self.one_like();
        let mut base = self.clone();
        let mut e = exp;
        while e > 0 {
            if e & 1 == 1 {
                result = result * &base;
            }
            base = base.square();
            e >>= 1;
        }
        result
    }

    /// Compute the Frobenius endomorphism: `self^(p^k)`.
    fn frobenius(&self, k: usize) -> Self
    where
        Self::Characteristic: Into<u64>,
    {
        let p: u64 = Self::characteristic().into();
        let mut result = self.clone();
        for _ in 0..k {
            result = result.pow(p);
        }
        result
    }
}

// Blanket implementation
impl<T: FiniteField> FiniteFieldExt for T {}
```

### 4.1.2 How `Gf2mElement` Implements the Trait

The existing `Gf2mElement` implements `FiniteField` but **not** `ConstField`:

```rust
impl FiniteField for Gf2mElement {
    type Characteristic = u64;

    fn characteristic() -> u64 { 2 }
    fn extension_degree() -> usize {
        // Problem: m is not known at the type level.
        // This method cannot be implemented correctly as a static method
        // for runtime-parameterized types. See section 4.1.3.
        panic!("Gf2mElement::extension_degree() requires an instance; use instance methods")
    }

    fn is_zero(&self) -> bool { self.value == 0 }
    fn is_one(&self) -> bool { self.value == 1 }
    fn inv(&self) -> Option<Self> { self.inverse() }

    fn zero_like(&self) -> Self {
        Gf2mElement { value: 0, params: Arc::clone(&self.params) }
    }
    fn one_like(&self) -> Self {
        Gf2mElement { value: 1, params: Arc::clone(&self.params) }
    }
}
```

New trait impls that must be added to `Gf2mElement`:
- `Hash`: hash `self.value` only (field identity is a precondition, not part of the hash).
- `Sub`: delegate to `Add` (char 2: subtraction = addition).
- `Neg`: return `self.clone()` (char 2: negation is identity).
- `Div`: already implemented.

### 4.1.3 The `extension_degree()` Problem

`extension_degree()` is declared as `fn extension_degree() -> usize` (no `&self`). This
is correct for const-generic types where `m` is part of the type. But `Gf2mElement` has
a single Rust type for all values of `m` — there is no type-level distinction between
GF(2^8) and GF(2^16).

**Resolution**: `extension_degree()` is a *static* property of the field. For
runtime-parameterized types it returns a sentinel or panics. Runtime code that needs `m`
uses the existing `Gf2mField::degree()` method. Generic algorithms that need
`extension_degree()` should bound on `ConstField` where the value is meaningful.

This is acceptable because:
1. The primary path is const-generic types where `extension_degree()` is correct.
2. `Gf2mElement` is the compatibility layer, not the primary target.
3. Algorithms that truly need the degree at compile time (e.g., for fixed-size arrays)
   must use `ConstField` anyway.

### 4.2 Naming and Crate Structure: Plan for the Rename

The library will be renamed to reflect its broader scope. The target structure:

```
galois (or gfpn — name TBD)
├── crates/
│   ├── galois-core/          # Renamed from gf2-core
│   │   ├── src/
│   │   │   ├── field/        # NEW: FiniteField trait + field axiom tests
│   │   │   ├── bitvec.rs     # Unchanged — GF(2) bit packing
│   │   │   ├── matrix.rs     # Unchanged — GF(2) bit matrices
│   │   │   ├── gf2m/         # Unchanged — GF(2^m) extension fields
│   │   │   ├── gfp/          # NEW: GF(p) prime fields (Montgomery, etc.)
│   │   │   ├── gfpn/         # NEW: GF(p^n) tower extensions
│   │   │   ├── kernels/      # Existing bitwise Backend + new FieldKernel
│   │   │   └── ...
│   ├── galois-kernels-simd/  # Renamed from gf2-kernels-simd
│   ├── galois-coding/        # Renamed from gf2-coding
```

**Transition plan**: The rename is a single coordinated step (Cargo.toml changes, module
paths, re-exports). It does not need to happen before the trait work begins — the trait
design is name-independent. But the architecture should assume the broader scope from day one.

**Ergonomic preservation**: After rename, `use galois::BitVec` and `use galois::BitMatrix`
work exactly as `use gf2::BitVec` does today. The binary field API surface is unchanged.

### 4.3 Kernel Architecture: Parallel Paths, Not Generalization

The existing `Backend` trait serves GF(2) bitwise operations. Attempting to generalize it
for modular arithmetic would compromise both the existing binary performance and the new
field arithmetic performance. Instead, add parallel kernel traits:

```
kernels/
  backend.rs          — Existing GF(2) bitwise ops (AND/OR/XOR/NOT/popcount)
  field_backend.rs    — NEW: Field-element SIMD dispatch
```

The `FieldBackend` trait covers:
- **Carryless multiplication** (PCLMULQDQ/PMULL) — already partially in gf2m SIMD
- **Montgomery batch reduction** — for GF(p) arithmetic
- **IFMA (AVX-512)** — fused multiply-add for prime field batch ops
- **Branchless modular add/sub** — vectorized correction step

Both backend paths use the same runtime-detection mechanism (`OnceLock` + feature detection)
but dispatch to different instruction sets. The binary `Backend` is never touched.

### 4.4 The Arc<FieldParams> Problem: Performance-First Resolution

The current `Gf2mElement` clones `Arc<FieldParams>` on every arithmetic result. This is
unacceptable for inner-loop performance (matrix multiplication, polynomial evaluation).

**Resolution strategy (ordered by implementation priority):**

1. **Const-generic field types** (new code): `Gf2_8`, `Gf2_16`, `Fp<P>` — the field is the
   type. No Arc, no runtime overhead. Tables are `const` or `OnceLock`-initialized statics.
   These are the primary types for performance-critical code.

2. **Existing `Gf2mElement`** (compatibility): Remains as-is for runtime-parameterized use.
   The Arc overhead is the price of runtime flexibility — acceptable for research/exploration
   where field parameters are not known at compile time.

3. **The "field scope" pattern is explicitly deferred.** A borrowing-based scope pattern
   where elements borrow field context from a stack-allocated scope would require a
   fundamentally different element type (lifetime-annotated, like `Gf2mRef<'a>`). The
   current `FiniteField` trait, which requires `Sized + Clone`, is not compatible with
   borrowed elements. If profiling reveals that the Arc path is a bottleneck even for
   research use, the scope pattern can be designed as a separate trait/API outside the
   `FiniteField` hierarchy. We do not design for it now.

## 5. Build Order

The groundwork items, in dependency order:

### Phase 1: The Trait Foundation (immediate)

1. **`FiniteField` + `ConstField` + `FiniteFieldExt` trait definitions** — The `field/`
   module with the core traits as specified in section 4.1. No dependencies on benchmarks
   or SIMD — this is a pure algebraic abstraction.

2. **Generic property-based field axiom test harness** — A test harness parameterized over
   `T: FiniteField` that verifies:
   - Additive group: associativity, commutativity, identity (`zero_like`), inverse (`neg`)
   - Multiplicative group: associativity, commutativity, identity (`one_like`), inverse (`inv`)
   - Distributivity: `a * (b + c) = a * b + a * c`
   - Characteristic: `sum of p ones = zero`
   - Division: `a / b = a * b.inv()` for non-zero `b`
   - Subtraction: `a - b + b = a`
   - `Hash` consistency: `a == b → hash(a) == hash(b)`

   Every future field type plugs into this harness automatically. **This harness gates all
   subsequent field implementations** — no field type is considered correct without passing
   the axiom tests.

3. **`Gf2mElement` implements `FiniteField`** — Validates the trait against existing code.
   Adds `Hash`, `Sub`, `Neg` impls. No behavioral changes to `Gf2mElement` arithmetic.
   Runs the axiom test harness against GF(2^4), GF(2^8), GF(2^16).

4. **`Fp<const P: u64>` naive prime field implementation** — A minimal const-generic
   prime field type using simple `%` reduction. Implements both `FiniteField` and
   `ConstField`. This is the second concrete implementation of the trait and the first
   for characteristic != 2. It validates that:
   - The trait actually generalizes beyond GF(2^m)
   - `Sub` and `Neg` work correctly for odd characteristic
   - `zero()` / `one()` work as freestanding associated functions via `ConstField`
   - The axiom test harness works for prime fields

   The naive implementation is intentionally unoptimized — Montgomery multiplication
   replaces the internals later without changing the public API.

   ```rust
   /// A prime field element. The prime P is encoded in the type.
   #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
   pub struct Fp<const P: u64>(u64);

   impl<const P: u64> ConstField for Fp<P> {
       fn zero() -> Self { Fp(0) }
       fn one() -> Self { Fp(1) }
       fn order() -> u64 { P }
   }

   impl<const P: u64> FiniteField for Fp<P> {
       type Characteristic = u64;
       fn characteristic() -> u64 { P }
       fn extension_degree() -> usize { 1 }
       fn is_zero(&self) -> bool { self.0 == 0 }
       fn is_one(&self) -> bool { self.0 == 1 }
       fn inv(&self) -> Option<Self> {
           if self.0 == 0 { return None; }
           // Extended Euclidean algorithm or Fermat's little theorem
           Some(self.pow(P - 2))
       }
       fn zero_like(&self) -> Self { Fp(0) }
       fn one_like(&self) -> Self { Fp(1) }
   }
   ```

### Phase 2: Exercising the Trait (follows Phase 1)

5. **`FieldBackend` kernel trait** — Defines the SIMD dispatch surface for field-element
   operations. Initially with scalar fallbacks only.

6. **Generic `FieldVec<F: FiniteField>`** — The vector container for field elements. Basic
   operations: element access, dot product (scalar), map, fold. **No SIMD dependency** — the
   generic container is useful before SIMD is available. SIMD-accelerated specializations
   (delayed reduction, vectorized dot product) come later as optimizations.

7. **Batch polynomial operations over `FiniteField`** — Generic polynomial type
   `FieldPoly<F: FiniteField>` or generic batch operations on `Gf2mPoly`. Exercises the
   trait in an algebraically meaningful context.

### Phase 3: Deepening (follows Phase 2)

8. **Montgomery multiplication for `Fp<P>`** — Replace the naive `%` internals with
   Montgomery form. Precompute `R^2 mod P` and `mu = -P^{-1} mod 2^64` at compile time
   via `const fn`. The public API (`Fp<P>` + `FiniteField` + `ConstField`) is unchanged.

9. **GF(2^m) for m > 64** — Forces the element representation beyond u64. Stress-tests
   the trait with multi-word elements.

10. **SIMD-accelerated `FieldVec` operations** — Depends on `FieldBackend` + carry-less
    multiplication kernels. Vectorized dot product with delayed reduction.

Items 1–4 are the **immediate groundwork** that must be completed before any other GF(p^n)
work proceeds.

## 6. What NOT to Do Now

- **Don't impose traits on BitVec/BitMatrix.** The binary bit-packing layer is not a
  "field element" API — it's a storage and SIMD dispatch layer. Forcing it through
  `FiniteField` adds overhead and destroys ergonomics.
- **Don't build tower extensions yet.** Tower construction (GF(p^2) → GF(p^6) → GF(p^12))
  is the end goal, not the foundation. The trait must support it; we don't build it now.
- **Don't generalize coding traits yet.** `BlockEncoder<F: FiniteField>` is the future, but
  it depends on having field types to parameterize with. Premature generalization here would
  churn the coding crate for no immediate benefit.
- **Don't rename the project yet.** The rename is mechanical. Do it once the new modules
  exist and the scope expansion is tangible.
- **Don't build the "field scope" borrowing pattern.** It requires a different element
  representation incompatible with the `FiniteField` trait. Defer unless profiling
  demonstrates Arc overhead is a bottleneck for research use cases.
- **Don't block the trait design on benchmarks.** The trait is an algebraic abstraction.
  Benchmark results inform *implementation choices* within specific field types, not the
  trait signature itself.

## 7. Verification: Trait Against Future Types

Before implementing, verify the trait signatures on paper against these future types:

| Type | `FiniteField` | `ConstField` | Notes |
|------|:---:|:---:|-------|
| `Fp<const P: u64>` | yes | yes | Naive, then Montgomery internals |
| `Fp<const P: u64>` (Mersenne) | yes | yes | Specialized reduction, same trait surface |
| `Gf2_8` (compile-time GF(2^8)) | yes | yes | Const tables, Copy, zero-cost |
| `Gf2_16` (compile-time GF(2^16)) | yes | yes | Const tables, Copy, zero-cost |
| `Gf2mElement` (runtime GF(2^m)) | yes | **no** | Arc-based, no Copy, no freestanding zero/one |
| `QuadraticExt<F: ConstField>` | yes | yes | Tower: `(F, F)`, both components are Copy |
| `QuadraticExt<Gf2mElement>` | yes | **no** | Tower over runtime base: no Copy |
| `Fp<BigUint>` (large char) | yes | depends | `Characteristic = BigUint` |

The `zero_like`/`one_like` pattern works for all rows. `ConstField::zero()`/`one()`
works for all rows marked "yes" in the `ConstField` column. Generic algorithms choose
their bound based on what they need:

- `fn algorithm<F: FiniteField>(elements: &[F])` — works with everything, uses `zero_like`
- `fn algorithm<F: ConstField>()` — needs freestanding construction, uses `F::zero()`

## 8. Issue Structure Corrections

The following changes to the JIT issue graph are required to match this build order:

### Dependencies to remove

| From | To | Reason |
|------|----|--------|
| bfe0ba7b (FiniteField trait) | 9effa2e2 (GF(2^m) benchmarks) | Trait design is independent of benchmark results. Benchmarks inform implementation choices within field types, not the trait signature. |

### Dependencies to add

| From (blocked) | To (blocker) | Reason |
|----------------|--------------|--------|
| 8ce6f8aa (Montgomery GF(p)) | 2248b17d (axiom tests) | Montgomery impl must pass axiom tests before considered correct. |
| 3f4b946c (tower extensions) | 2248b17d (axiom tests) | Tower impl must pass axiom tests. |
| bdf95060 (batch poly ops) | bfe0ba7b (FiniteField trait) | Batch ops are generic over `FiniteField`; need the trait to exist. |
| 6fb4abad (GF(2^m) m>64) | 2248b17d (axiom tests) | Multi-word GF(2^m) must pass axiom tests. |

### Issues to create

| Title | Type | Priority | Depends on | Blocks |
|-------|------|----------|------------|--------|
| Implement naive `Fp<P>` prime field with `FiniteField` + `ConstField` | task | normal | bfe0ba7b (trait), 2248b17d (axiom tests) | 8ce6f8aa (Montgomery), 1f2f8371 (specialized primes) |
| Add `Hash`, `Sub`, `Neg` impls to `Gf2mElement` | task | normal | — | bfe0ba7b (trait impl for Gf2mElement) |
| Implement generic `FieldVec<F: FiniteField>` container (scalar) | task | normal | bfe0ba7b (trait) | 0fb99491 (SIMD FieldVec) |

### Issues to modify

| Issue | Change |
|-------|--------|
| 0fb99491 (FieldVec with SIMD) | Rename to "SIMD-accelerated FieldVec operations". Add dependency on new generic FieldVec task. This issue becomes the SIMD *optimization* of the generic container, not the container itself. |
| bfe0ba7b (FiniteField trait) | Remove dependency on 9effa2e2. Update description to reflect the two-tier trait design (`FiniteField` + `ConstField`). Move to `ready` state. |

## 9. Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Two-tier trait hierarchy adds complexity | Moderate | `FiniteFieldExt` blanket impl means users rarely interact with the tiers directly. Most generic code needs only `F: FiniteField`. |
| `zero_like(&self)` less ergonomic than `F::zero()` | Low | Algorithms always have at least one element available. `ConstField` provides the clean syntax for const-generic types, which are the primary path. |
| `Hash` for `Gf2mElement` hashes only `value`, not field context | Correctness risk | Document that elements from different fields must never be mixed in the same collection. The `Arc::ptr_eq` check at arithmetic time catches violations. |
| Trait too abstract → performance loss | Critical | Design for const-generic monomorphization. Benchmark trait-dispatched vs. direct calls early. |
| Trait too narrow → doesn't accommodate GF(p^n) towers | Blocks future work | Verified in section 7 against concrete future types including `QuadraticExt<F>`. |
| `Characteristic` as associated type adds generics complexity | Low | Default to `u64` for all current types. The associated type only matters when someone implements a large-characteristic field. |
| Arc overhead in existing Gf2mElement | Moderate | Const-generic types are the primary path. Arc stays only for runtime flexibility. |
| Rename disrupts downstream users | Moderate | Provide `gf2` re-export crate as compatibility shim. |
| Binary field ergonomics degrade | Critical | Binary path is never touched. Trait applies only to element-level field types. |
| Naive `Fp<P>` with `%` is slow | None (intentional) | It exists for correctness validation and as a reference for Montgomery. Performance is not a goal for the naive impl. |
