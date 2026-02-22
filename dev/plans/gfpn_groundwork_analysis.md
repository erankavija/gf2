# GF(p^n) Groundwork: Architectural Analysis

> Analysis for epic e095a100 — "Implement general Galois field GF(p^m) arithmetic"
>
> This document examines the current architecture, identifies the key decisions that must
> be made before implementation begins, and proposes a groundwork plan that preserves
> binary-field ergonomics while enabling extreme performance for general finite fields.

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

## 4. Decisions

### 4.1 Trait Hierarchy: Zero-Cost by Design

The `FiniteField` trait must enable the compiler to eliminate all abstraction cost when the
field is known at compile time. Operator overloading (`Add`, `Mul`, etc.) ensures ergonomic
arithmetic. The trait provides the algebraic interface; implementations choose the representation.

```rust
/// A finite field element. The type itself identifies the field.
///
/// For const-generic fields (e.g., `Fp<P>`), all methods monomorphize
/// to field-specific machine code with zero dispatch overhead.
///
/// For runtime-parameterized fields (e.g., `Gf2mElement`), the field
/// context is carried internally.
pub trait FiniteField:
    Sized + Clone + PartialEq + Eq + Debug
    + Add<Output = Self> + Sub<Output = Self>
    + Mul<Output = Self> + Neg<Output = Self>
    + for<'a> Add<&'a Self, Output = Self>
    + for<'a> Mul<&'a Self, Output = Self>
{
    /// The additive identity.
    fn zero() -> Self;

    /// The multiplicative identity.
    fn one() -> Self;

    fn is_zero(&self) -> bool;

    /// Multiplicative inverse. Returns `None` for zero.
    fn inv(&self) -> Option<Self>;

    /// Field characteristic (2 for binary fields, p for GF(p^n)).
    fn characteristic() -> u64;
}
```

**Critical**: This trait is for *element-level field arithmetic* (GF(2^m), GF(p), GF(p^n)).
It is **not** imposed on the binary bit-packing layer (`BitVec`, `BitMatrix`). Those types
operate at the GF(2) word level where XOR *is* addition and the abstraction would add no
value — only overhead. The binary path stays exactly as it is.

**Implementing for the existing `Gf2mElement`**: The current Arc-based design implements
`FiniteField` as a compatibility bridge. This validates the trait against real code. Future
const-generic binary extension types (e.g., `Gf2_8`, `Gf2_16` with compile-time tables)
will provide the zero-overhead path. The existing `Gf2mElement` remains available for
runtime-parameterized research use.

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

3. **Future optimization** (if needed): A "field scope" pattern where elements borrow field
   context from a stack-allocated scope, eliminating Arc entirely for batch operations:
   ```rust
   field.scope(|ctx| {
       let a = ctx.element(42);
       let b = ctx.element(17);
       let c = &a * &b;  // No Arc clone — borrows ctx
   });
   ```

This gives us three performance tiers: zero-cost (const-generic), amortized (scope), and
flexible (Arc). The trait accommodates all three.

## 5. Build Order

The groundwork items, in dependency order:

1. **`FiniteField` trait definition** — The trait module with the core trait, plus
   `FiniteFieldExt` for convenience methods (square, pow, batch operations).

2. **Generic property-based field axiom tests** — A test harness parameterized over
   `T: FiniteField` that verifies associativity, commutativity, distributivity, inverse,
   identity, and characteristic. Every future field type plugs into this automatically.

3. **`Gf2mElement` implements `FiniteField`** — Validates the trait against existing code.
   No behavioral changes to `Gf2mElement` itself.

4. **`FieldBackend` kernel trait** — Defines the SIMD dispatch surface for field-element
   operations. Initially with scalar fallbacks only.

5. **Batch polynomial operations over `FiniteField`** — Generic `FieldVec<F>` with dot
   product and delayed reduction. Exercises the trait in a performance-sensitive context.

6. **GF(2^m) for m > 64** — Forces the element representation beyond u64. Stress-tests
   the trait with multi-word elements.

Items 1–3 are the immediate groundwork. Items 4–6 follow naturally and are already tracked
as child issues of the epic.

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

## 7. Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Trait too abstract → performance loss | Critical | Design for const-generic monomorphization. Benchmark trait-dispatched vs. direct calls early. |
| Trait too narrow → doesn't accommodate GF(p^n) towers | Blocks future work | Verify trait design against tower extension signatures on paper before implementing. |
| Arc overhead in existing Gf2mElement | Moderate | Const-generic types are the primary path. Arc stays only for runtime flexibility. |
| Rename disrupts downstream users | Moderate | Provide `gf2` re-export crate as compatibility shim. |
| Binary field ergonomics degrade | Critical | Binary path is never touched. Trait applies only to element-level field types. |
