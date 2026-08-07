# D1b — PackedField + Permanent public API

**JIT issue:** `9fe275d3` (W0 / D1b)
**Epic:** `epic:gf2-algebra-permanent` (parent design: `dev/plans/gf2_algebra_permanent.md`)
**Predecessor decisions:**
- D1a (`6e20133d`, `dev/plans/d1a_gf2_algebra_boundary.md`) — fixes home crate `gf2-algebra` and the module paths `packed::PackedField`, `packed::PackedFieldVec`, `packed::bipedal3`, `permanent::Permanent`.
- D2 (`a0c0a45f`, `dev/plans/d2_lean_bipedal3_sketch.md`) — V1 Lean sketch; constrains the `Bipedal3` element-level extraction surface.
- D4 (`4c534d31`, `dev/plans/d4_intrinsic_feasibility.md`) — verifies the bitwise AVX2/AVX-512 intrinsics that an SIMD impl will dispatch through are stable on MSRV 1.95.
**Status:** decision (user approved 2026-05-09; recorded in JIT issue `9fe275d3` description `## Approval` section)
**Date:** 2026-05-09

## 1. Scope

This document fixes the public Rust trait surface for the
`gf2-algebra-permanent` epic's three core abstractions:

1. `PackedField<F: FiniteField>` — fixed-LANES lane-parallel arithmetic.
2. `PackedFieldVec<F: FiniteField>` — variable-length analogue.
3. `Permanent` — "this matrix-like value yields a permanent."

The surface is demonstrated against `Fp<3>` only — F_5 / F_7 follow the
R1 / R2 outcomes (`dev/plans/r1_f5_encoding_decision.md`,
`dev/plans/r2_f7_encoding_decision.md`,
`dev/plans/r2_packed_encoding_generalizations.md`). §6 of this document
shows the surface accommodates those outcomes without further redesign.

The decisions here are **frozen at the W6 `gate:api-freeze`** in the
epic's wave plan (parent §13 W6, parent §15 risk #7). Until then the
surface may be amended in-loop on review feedback; once the gate fires
the surface is locked because Charon extraction in V1 / V2 cannot
tolerate signature churn (parent §15 risk #8).

## 2. Trait signatures

The committed signatures, in the order the epic doc lists them. Final
home is `crates/gf2-algebra/src/packed/mod.rs` (`PackedField`,
`PackedFieldVec`) and `crates/gf2-algebra/src/permanent/mod.rs`
(`Permanent`). Every public item carries a doc comment per CLAUDE.md
§Documentation standards; the signatures below show the contract only.

### 2.1 `PackedField<F>`

```rust
pub trait PackedField<F: FiniteField>: Copy + Eq + core::fmt::Debug {
    /// Number of independent F-lanes packed into one Self.
    /// Must be positive. A power-of-two is preferred for SIMD-friendly
    /// mapping where feasible, but non-power-of-two values are permitted
    /// (e.g., the future `Bipedal5` encoding packs 21 lanes per `u64`).
    const LANES: usize;

    fn zero() -> Self;
    fn one() -> Self;
    fn splat(x: F) -> Self;

    fn add(self, rhs: Self) -> Self;
    fn sub(self, rhs: Self) -> Self;
    fn neg(self) -> Self;
    fn mul(self, rhs: Self) -> Self;

    fn lane(self, i: usize) -> F;
    fn with_lane(self, i: usize, x: F) -> Self;

    fn all_zero(self) -> bool;
}
```

### 2.2 `PackedFieldVec<F>`

```rust
pub trait PackedFieldVec<F: FiniteField>: Clone + Eq + core::fmt::Debug {
    type Element: PackedField<F>;

    fn zeros(len: usize) -> Self;
    fn from_field_slice(xs: &[F]) -> Self;

    fn len(&self) -> usize;
    fn is_empty(&self) -> bool { self.len() == 0 }

    fn get(&self, i: usize) -> F;

    fn add_assign(&mut self, rhs: &Self);
    fn sub_assign(&mut self, rhs: &Self);
    fn mul_assign(&mut self, rhs: &Self);

    fn all_zero(&self) -> bool;
}
```

`fold_mul` is **not** on this trait. See §3.3.

### 2.3 `Permanent`

```rust
pub trait Permanent {
    type Field: FiniteField;
    fn permanent(&self) -> Self::Field;
}
```

The epic strawman in parent §6 wrote `Permanent<F>` with an associated
`Matrix` type. The committed shape inverts that: an associated `Field`
type plus a self-referential method, so `permanent_ryser<F>` is the free
function and a concrete matrix type implements `Permanent` with
`Field = F`. See §5 for rationale.

## 3. Decision rationale

The five points below cover criterion 3 of the issue. Each subsection
records the chosen option, the alternative, and the deciding reason.

### 3.1 `LANES` — const, not runtime

**Decision:** `const LANES: usize`.

The alternative — a runtime `fn lanes(&self) -> usize` — was considered
because it admits one trait that abstracts both 64-lane scalar
`(u64, u64)` Bipedal3 and a hypothetical SIMD-batched
`Bipedal3x4 = (__m256i, __m256i)` (256 lanes) under a single concrete
type whose width depends on the active dispatch.

The const form wins on three grounds:

1. **Compile-time loop-bound constants.** The Gray-code Ryser inner
   loop reads `for i in 0..N::LANES { ... }` directly; a const lets the
   compiler fully unroll. A runtime lanes() forces an obscure indirect
   bound the unroller cannot prove.
2. **Type-level differentiation per dispatch.** The SIMD path is a
   different concrete type (e.g. `Bipedal3x4`) implementing the same
   trait with `LANES = 256`. Runtime dispatch picks the type at the
   `gf2-algebra::permanent::bipedal3` boundary; the trait is uniform
   below that boundary. This matches the existing
   `gf2-kernels-simd::LogicalFns` dispatch shape.
3. **Eliminates an asserts-in-hot-loop class of bug.** A const lane
   count makes `assert!(i < Self::LANES)` const-foldable and the panic
   branch eliminable for indices known at compile time.

The trade-off: a single-trait-multiple-widths setup in callers must
write `M: PackedField<F>` (generic over the lane count) instead of
naming a specific width. We accept this — the only callers that care
are inside `gf2-algebra::permanent`, and they are already generic.

### 3.2 `splat` — trait method, not inherent

**Decision:** `splat(x: F) -> Self` lives on the trait.

The alternative — keep it inherent on each concrete type — is cheaper
to specify but forces every generic Ryser caller to either (a) add an
extra trait or (b) require `F: ConstField` and call `Self::one()` then
do a generic `Self::mul` reduction to broadcast non-trivial scalars.
That is silly: every concrete `PackedField<F>` already needs to know
how to embed an `F` value into its lane representation (it has to do
that for `with_lane`), so making `splat` a trait method costs one
method and saves caller machinery.

`splat` does **not** require `F: ConstField` because the broadcast
is "produce any LANES value whose every lane decodes to `x`" — the
`x: F` argument carries the field witness for runtime-context fields.

### 3.3 `fold_mul` — inherent, not trait

**Decision:** `fold_mul(&self) -> F` lives as an **inherent method on
each concrete `PackedFieldVec` impl**, not on the trait.

The alternative — putting it on `PackedFieldVec<F>` — was tempting
because the Gray-code Ryser inner loop in parent §7.3 calls it once
per Gray step, and a trait method would let the loop be generic. The
deciding consideration is specialisation:

- The bipedal3 fold uses a `popcount`-driven trick that reduces the
  64-lane `(mag, sgn)` pair to a single F_3 result in `O(1)` after the
  per-word log-tree mul reduction.
- The F_5 D-bit-sliced fold uses a different log-tree (3 planes per
  word, log-tree of `mul` that respects the bit-sliced
  representation).
- The F_7 LUT-A fold reduces via byte-lane SIMD (or a scalar 4-bit
  table) — different again.

If `fold_mul` were on the trait, every `permanent_*` specialisation
would still be calling the concrete impl's specialised version anyway,
because the trait method body would just dispatch to it. The trait
would buy us nothing and would force the unstable details (popcount
shape, log-tree depth) into the trait surface where they are not
welcome.

We keep `fold_mul` as an inherent method on each concrete vec and let
each `permanent_*` reach for the type-specific version. The shape of
the Gray-code inner loop in parent §7.3 already names the concrete
type (`Bipedal3Vec`); the loop is not generic over `PackedFieldVec`.
Generic Ryser (`permanent_ryser<F>`) does not need `fold_mul` — it
walks `F` elements one at a time and uses `F`'s native `Mul`.

### 3.4 `Eq` — required, with canonical-decode semantics

**Decision:** `PackedField<F>: Copy + Eq + Debug` and
`PackedFieldVec<F>: Clone + Eq + Debug`. The `Eq` is **canonical-decode
equality**, not bit-pattern equality.

The alternative — only `PartialEq` — was rejected because no F_3 / F_5
/ F_7 packed encoding has NaN-like elements. Every bit-pattern that
the impl can produce decodes to a well-defined F element. Reducing the
contract to `PartialEq` would not buy us anything and would prevent
callers from using these types as keys / in hash sets / in
`assert_eq!` against canonical references.

The `Eq` contract is **canonical-decode**:

> `a == b` iff for all `i ∈ [0, LANES)`, `a.lane(i) == b.lane(i)`
> (equality in `F`).

This is materially different from raw bit-pattern equality on
`(mag, sgn)` because the bipedal F_3 encoding has a redundant
codeword: `(mag=0, sgn=0)` and `(mag=0, sgn=1)` both decode to F_3's
zero. Implementations MUST canonicalise before comparing. The
demonstration impl in `dev/research/packed_field_stub/src/lib.rs`
implements `PartialEq` for `Bipedal3` as
`(self.mag, self.sgn & self.mag) == (other.mag, other.sgn & other.mag)`
— this is the cheapest correct canonicalisation: clear the sign bits
that correspond to zero-magnitude lanes before comparing.

Concrete impls that have **no** redundant codewords (any `PackedField`
with a unique encoding per lane state) may implement `PartialEq` via a
direct field-by-field compare; they still satisfy the contract because
the canonical form is the only form. The trait does not prescribe an
implementation, only the equivalence relation.

`Hash` is intentionally **not** required. Adding it would force every
impl to canonicalise before hashing, which costs an extra
`sgn &= mag` per element on bipedal3 and is not justified by any
known caller. If a hash is needed later (HashMap keys, content-based
deduplication), it can be added by widening the trait bound; existing
impls would not need to change because they already canonicalise on
`==`.

### 3.5 Lane-extraction semantics on the alt-zero codeword

**Decision:** `lane(i)` **canonicalises**. For Bipedal3, both
`(mag=0, sgn=0)` and `(mag=0, sgn=1)` return `Fp<3>::ZERO`.

```rust
// Bipedal3::lane (concrete impl):
let m = (self.mag >> i) & 1 == 1;
let s = (self.sgn >> i) & 1 == 1;
if !m { Fp::<3>::new(0) }            // both alt-zero variants land here
else if !s { Fp::<3>::new(1) }
else { Fp::<3>::new(2) }
```

The alternative — exposing a raw `(bool, bool)` lane and forcing
callers to canonicalise — was rejected because:

- It leaks the encoding into every caller, including generic
  `permanent_ryser` which does not know `Bipedal3` exists.
- It contradicts the §3.4 `Eq` contract; if the trait says two equal
  values must agree on every `lane(i)`, lane extraction must already
  be canonicalising.
- It introduces a footgun where a buggy constructor produces an
  alt-zero and downstream code observes a "weird zero" without
  knowing what to do.

`with_lane(i, x)` similarly produces only the **canonical** encoding.
Setting `with_lane(i, Fp::<3>::ZERO)` writes `(mag=0, sgn=0)` at lane
`i`, never `(mag=0, sgn=1)`. The redundancy is preserved as an
implementation freedom for arithmetic kernels (the paper §2.2 add
formula may briefly produce `(mag=0, sgn=1)` as an intermediate; the
encoding allows it for cheap arithmetic) but it is **never observable
through the public API**.

`all_zero` similarly canonicalises:

```rust
// Bipedal3::all_zero
self.mag == 0   // both alt-zero variants have mag = 0
```

The bipedal3 stub at `dev/research/packed_field_stub/src/lib.rs`
exercises this contract end-to-end:

- `bipedal3_alt_zero_canonical_eq` constructs `(mag=0, sgn=!0)` (every
  lane is alt-zero) and asserts it compares `==` to canonical zero,
  passes `all_zero`, and decodes every `lane(i)` to `F_3::new(0)`.

## 4. Bipedal3 conformance walk-through

Every method of `PackedField<Fp<3>>` and `PackedFieldVec<Fp<3>>` is
exercised in `dev/research/packed_field_stub/src/lib.rs`. The mapping
to the stub is one-to-one:

| Trait item                                | Stub method                       | Notes |
|-------------------------------------------|-----------------------------------|-------|
| `PackedField::LANES`                      | `Bipedal3::LANES = 64`            | const |
| `PackedField::zero / one`                 | `Bipedal3::ZERO / ONE`            | const |
| `PackedField::splat(x)`                   | inherent match on `x.value()`     | 3-arm match over F_3 |
| `PackedField::add / sub / mul`            | paper §2.2 formulas               | `add_const` / `sub_const` / `mul_const` |
| `PackedField::neg`                        | `(mag, sgn ^ mag)`                | nonzero sign-flip; zero is self-inverse |
| `PackedField::lane(i)`                    | psi decoder                       | canonicalises alt-zero |
| `PackedField::with_lane(i, x)`            | bit set/clear on `(mag, sgn)`     | only canonical encoding written |
| `PackedField::all_zero`                   | `mag == 0`                        | covers alt-zero |
| `PackedFieldVec::Element`                 | `Bipedal3`                        | associated type |
| `PackedFieldVec::zeros / from_field_slice`| `Vec<u64>` allocation             | mask_tail invariant |
| `PackedFieldVec::len / is_empty / get`    | per-element decode                | canonicalises per lane |
| `PackedFieldVec::add_assign / sub_assign / mul_assign` | per-word loop of paper formulas | tail-mask after each |
| `PackedFieldVec::all_zero`                | `self.mag.iter().all(|&w| w == 0)`| covers alt-zero |
| inherent `Bipedal3Vec::fold_mul`          | stub returns `Fp<3>::ZERO`        | full impl is W2/T9 |

The stub crate's `_bound_checks` function is a static assertion that
the impl satisfies the trait against the **real**
`gf2_core::field::FiniteField` and `gf2_core::gfp::Fp<3>` types — not a
mock. Building the stub is therefore proof that the trait surface
admits a working `Bipedal3 : PackedField<Fp<3>>`.

Run-time confirmation (six tests pass):

- `bipedal3_packed_field_basic` — `zero / one / splat / lane / with_lane`
  round-trips for all 3 F_3 values.
- `bipedal3_alt_zero_canonical_eq` — alt-zero canonicalisation in `==`,
  `all_zero`, `lane`.
- `bipedal3_lane_arithmetic_matches_fp3` — `add / sub / mul` per-lane
  agree with `Fp<3>` for an 8-lane slice spanning all 9 / 9 / 9
  ordered pairs.
- `bipedal3_vec_round_trip` — `from_field_slice / get` round-trips a
  200-element vector (crosses 64- and 128-bit word boundaries).
- `bipedal3_vec_arithmetic` — `add_assign / sub_assign / mul_assign`
  on a 130-element vector (also crosses 128 boundary), per-element
  comparison against `Fp<3>` arithmetic.
- `permanent_stub_returns_zero` — `Bipedal3Matrix : Permanent` with
  `Field = Fp<3>` compiles and returns `Fp<3>::ZERO` from the stub
  body.

## 5. The `Permanent` trait shape

### 5.1 Final committed shape

```rust
pub trait Permanent {
    type Field: FiniteField;
    fn permanent(&self) -> Self::Field;
}
```

### 5.2 Why associated `Field`, not generic `<F>`

The strawman in parent §6 read `pub trait Permanent<F: FiniteField> {
type Matrix; fn permanent(&self) -> F; }` — a parameterised trait with
an associated matrix-storage type. The committed shape inverts that:
the matrix storage **is** the implementor (`Bipedal3Matrix`,
`Bipedal3Matrix5`, generic `FieldMatrix<F>`), and `F` is recovered as
the `Field` associated type.

Reasons:

1. **One impl per concrete matrix type, not per (matrix, field)
   pair.** `Bipedal3Matrix` is intrinsically over `Fp<3>` — it cannot
   be over any other field. With the strawman shape, somebody could
   type `impl Permanent<Fp<5>> for Bipedal3Matrix { ... }` (forbidden,
   but the type system would accept it). With `type Field`, the
   matrix's field is a property of the type, not a parameter, and the
   impl forces it to a specific value.
2. **Caller ergonomics.** `M::Field` is shorter than naming the
   parameter explicitly. For `permanent_ryser<F>` (the generic free
   function), `F` stays a type parameter; the trait is only what
   concrete matrix types implement.
3. **Matches the existing project pattern.** `FiniteField::Wide`,
   `PackedFieldVec::Element` — all "the type of X is determined by
   the type of self" relations are associated types in this codebase.

### 5.3 Where the generic Ryser lives

`permanent_ryser<F: FiniteField>(m: &FieldMatrix<F>) -> F` is a free
function in `gf2-algebra::permanent::ryser`, **not** a trait method.
The free-function shape is what the strawman called
"`Permanent<F>::permanent(&self) -> F`"; we move it out of the trait
because it does not need to dispatch on the matrix type — every
`FieldMatrix<F>` walks the same Gray-code subset enumeration. The
free function is the default; the trait `impl` for `FieldMatrix<F>`
is one line that calls it.

### 5.4 Where the bipedal-specialised Ryser lives

`permanent_bipedal3_single(m: &Bipedal3Matrix) -> Fp<3>` and
`permanent_bipedal3_multi(m: &Bipedal3Matrix) -> Fp<3>` are free
functions in `gf2-algebra::permanent::bipedal3`. The
`impl Permanent for Bipedal3Matrix` body picks single-vs-multi based
on `self.n() <= 64` and dispatches accordingly. Same shape for F_5 /
F_7.

## 6. Future-friendliness for F_5 / F_7

The R1 / R2 outcomes (`dev/plans/r1_f5_encoding_decision.md`,
`dev/plans/r2_f7_encoding_decision.md`,
`dev/plans/r2_packed_encoding_generalizations.md` §3) settle on:

- **F_5: D bit-sliced.** Three planes per word (`b0, b1, b2`), each
  `Vec<u64>`. Add and sub via three-bit ripple per element with no
  carry chain crossing element boundaries; mul via the same three
  planes with a small fixed-shape combinator.
- **F_7: A LUT.** 16-bit slot per element packed 4-per-`u64`, 16-bit
  multiply LUT, scalar add via LUT or popcount-driven Mersenne fold.

Neither encoding has a redundant codeword analogous to bipedal3's
alt-zero, so the §3.5 canonicalisation contract is trivially satisfied
(the canonical decode is the only decode).

The trait surface accommodates both:

- `Bipedal5 : PackedField<Fp<5>>` with `LANES = 21` (3 bits per
  element, 64 / 3 = 21 with one bit of headroom). The `splat / lane /
  with_lane` methods translate the 3-bit slot to / from `Fp<5>`. All
  three planes flow through the trait's `add / sub / mul` formulas
  unchanged.
- `Bipedal7Lut : PackedField<Fp<7>>` with `LANES = 4` (16 bits per
  element packed 4-per-`u64`). The lane methods read / write
  4-bit-aligned 16-bit slots; arithmetic dispatches to the 16-bit
  LUT. The trait does not see the LUT.

Both impls also yield `PackedFieldVec<F>` with the same word-loop
pattern and the same `all_zero` / `add_assign` / etc. shape. No
new trait methods are needed.

The `LANES` const value differs across the three primes (64 vs 21
vs 4 in the AVX2-256 case it would be 4× those, and AVX-512 would be
8×). The compile-time const continues to be correct and unsurprising.

## 7. Charon-extraction-friendliness (D2 constraint)

D2 §5 (`dev/plans/d2_lean_bipedal3_sketch.md`) names the V1 extraction
target as the **inherent** `Bipedal3::{add, sub, mul, div}` methods,
not the `PackedField<Fp<3>>` trait dispatch. The trait is a thin
forwarder over the inherent methods.

Concretely: the W1/T3 implementation issue produces

```rust
impl Bipedal3 {
    #[inline] pub const fn add(self, r: Self) -> Self { ... }
    #[inline] pub const fn sub(self, r: Self) -> Self { ... }
    #[inline] pub const fn mul(self, r: Self) -> Self { ... }
    #[inline] pub fn      div(self, r: Self) -> Self { ... }
}
impl PackedField<Fp<3>> for Bipedal3 {
    fn add(self, rhs: Self) -> Self { Bipedal3::add(self, rhs) }
    // …forwarders for sub / neg / mul …
}
```

The stub at `dev/research/packed_field_stub/src/lib.rs` already
follows this shape (the `add_const / sub_const / mul_const / neg_const`
inherent functions are the V1 extraction surface, and the
`impl PackedField<Fp<3>>` trait body simply forwards). This means:

1. **V1 proves the inherent methods.** Charon extracts
   `gf2_algebra.packed.bipedal3.Bipedal3.{add, sub, mul, div}` (per D2
   §6) — these are the inherent methods, with simple bitwise bodies.
   Aeneas does not need to handle trait dispatch.
2. **The trait dispatch lemma is a one-line corollary.** Mentioned in
   D2 §8 risk R2.
3. **No `dyn PackedField`** — the trait is statically dispatched at
   the `permanent_*` boundary. No virtual calls in any hot path.
4. **No SIMD trait methods.** SIMD-batched concrete types
   (`Bipedal3x4 = (__m256i, __m256i)` etc.) live in
   `gf2-kernels-simd` and are wired in via concrete-type dispatch in
   the `gf2-algebra::permanent::bipedal3` module. The `PackedField`
   trait's contract does not change with SIMD — the trait stays
   `#![deny(unsafe_code)]`-friendly.

## 8. API freeze contract for W6

The trait surface in §2 is **frozen at the W6 `gate:api-freeze`** in
the parent epic's wave plan (parent §13 W6, parent §15 risk #7). Until
the gate fires, the surface may be amended in-loop on review feedback
or on user direction. Once it fires, the surface is locked because
Charon extraction in V1 / V2 cannot tolerate signature churn (parent
§15 risk #8).

The api-freeze gate fires after W3 ($T_{13}$, $T_{15}$) closes, before
the V1 / V2 implementation issues are dispatched. Any post-freeze
amendment to the `PackedField` / `PackedFieldVec` / `Permanent`
signatures requires explicit user approval through the standard
escalation path (`.claude/skills/project-lead/references/escalation-policy.md`).

Items that are **not** frozen by this gate:

- Internal methods on concrete types (e.g. `Bipedal3::add_const`'s
  exact body, `Bipedal3Vec::fold_mul`'s reduction shape). Those may
  evolve as long as the public trait surface holds.
- The `permanent_bipedal3_single / _multi` free function names and
  bodies — they are not on any trait, only the `impl Permanent for
  Bipedal3Matrix` dispatcher is.
- SIMD kernel signatures in `gf2-kernels-simd`. Those are an internal
  contract between `gf2-algebra` and `gf2-kernels-simd`.

## 9. Stub crate location and verification

**Location:** `dev/research/packed_field_stub/`.

**Crate manifest:** `Cargo.toml` declares an empty `[workspace]`
table to detach from the parent workspace, `publish = false`,
`rust-version = "1.95"`. Path-deps `gf2-core` (no default features)
so the stub exercises the **real** `FiniteField` + `Fp<3>` types
this epic will bound `PackedField<F>` against, not a mock. This was
the preferred approach per the issue spec because it tests real
trait bounds; falling back to a mock would have weakened the
demonstration.

**Layout:**

```
packed_field_stub/
├── .gitignore   # target/ + Cargo.lock per project memory
├── Cargo.toml   # standalone, [workspace] empty, gf2-core path dep
└── src/
    └── lib.rs   # PackedField, PackedFieldVec, Permanent, Bipedal3*
```

**Verification commands:**

```sh
cd dev/research/packed_field_stub
cargo check --release    # PASS (zero stub-specific warnings)
cargo test  --release    # 6/6 tests pass
```

Both commands succeed on rustc 1.95.0 (the project MSRV per CLAUDE.md
§MSRV). The `cargo check --release` output reports only seven
`#[warn(dead_code)]` warnings from `gf2-core` itself (under
`default-features = false` some `pub(crate)` SIMD-dispatch helpers
become unused); these are pre-existing and unrelated to the stub.

## 10. Open questions deferred to W1-T2

The following are **out of scope for this trait-surface decision** and
land in the W1-T2 (`PackedField trait + scalar reference impl`) issue:

1. **Specific `fold_mul` reduction tree.** Each concrete vec picks
   its own log-tree shape; this doc only fixes that the method is
   inherent, not its body.
2. **`Hash` trait bound.** Not currently required (§3.4); revisit if
   a HashMap-keyed cache materialises in W3 / W5 simulation work.
3. **`Display` formatting.** Not currently required; concrete impls
   may add `Display` independently.
4. **Multi-word `Bipedal3Vec` SIMD chunking strategy.** Lives in T14
   per parent §13 and `dev/plans/r3_multi_word_streaming.md`; the
   trait surface does not constrain it.
5. **`PackedField` for SIMD-batched types.** A 256-lane
   `Bipedal3x4 = (__m256i, __m256i)` impl is a follow-up in T12 / T13
   (the SIMD kernel issue per parent §13 W3). The trait already
   accommodates it (`LANES = 256`); no surface change is needed at
   that point.
6. **Generic `BatchedBipedalLike<P, MagLanes, SgnLanes>` framework.**
   The R4 outcome (parent §10) decides whether to ship this. The
   trait surface accommodates it as an abstract concrete type that
   implements `PackedField<Fp<P>>` for each chosen prime; the trait
   does not need to grow.
7. **Alternative-zero policy in proofs.** D2 §3.4 + §8 R4 say the
   formula correctness already holds on alt-zero inputs (the truth
   table covers all 16 cases). V1 does not need a separate
   "no-alt-zero produced" invariant lemma — the canonicalisation
   only matters at the API boundary, not inside the formulas. This
   is consistent with the §3.5 decision.

## 11. Summary of decisions (criterion 3 explicit recap)

For the issue success-criterion 3 audit:

- **`LANES` const vs runtime** → const (§3.1).
- **`splat` placement** → trait method (§3.2).
- **`fold_mul` placement** → inherent on each concrete `PackedFieldVec`
  impl, **not** on the trait (§3.3).
- **`Eq` vs `PartialEq`** → `Eq` required, with **canonical-decode**
  semantics; `Hash` deferred (§3.4).
- **Lane-extraction semantics on alt-zero (0, 1)** → `lane(i)`
  canonicalises (returns `Fp<3>::ZERO`); `with_lane` writes only the
  canonical encoding; `all_zero` returns `true` for both alt-zero and
  canonical zero (§3.5).
- **`Permanent` shape** → associated `Field` type, not a trait
  parameter; matrix storage is the implementor (§5).

Criterion 4 (user approval) was recorded by the project lead in the
issue description's `## Approval` section on 2026-05-09; see
`jit issue show 9fe275d3` for the approval block.
