# D2 — Lean4 proof sketch: bipedal F_3 arithmetic correctness

**Issue:** `a0c0a45f`
**Epic:** `epic:gf2-algebra-permanent`
**Status:** sketch (pre-implementation)
**Format:** per CLAUDE.md §Verification work — lemma list + per-lemma tactic shape + production code path + Aeneas-generated def names. No proof bodies.

This sketch governs the V1 implementation (`epic:gf2-algebra-permanent` $W_6$). Per CLAUDE.md it must be approved before any Lean code is written.

---

## 0. Notation

Throughout, `P : Std.U64` is the U64-encoded modulus 3. The hypothesis `ValidPrime P` from `proofs/Gf2Core/Proofs/Defs.lean` gives `Nat.Prime P.val ∧ 1 < P.val ∧ P.val ≤ 2^63`. We will instantiate at `P = 3#u64` and discharge `ValidPrime` once via `native_decide`.

A bipedal pair is `(m, s) : Std.U64 × Std.U64`, interpreted lane-wise: bit `i` of the pair encodes one F_3 element. The 64 lanes are independent, so all per-op correctness theorems reduce to a single-bit truth-table check lifted to all 64 lanes.

`FpVal P` (the existing wrapper in `Defs.lean`) carries an Fp element together with the `mont.val < P.val` invariant. For `P.val = 3`, since `3 ≤ 2^63`, Montgomery and canonical representations coincide on `{0, 1, 2}` after `from_mont` (this is what V1 closes against).

## 1. Statement

V1 is the conjunction of four operation-correctness theorems plus one decoder lemma plus one bit-lifting lemma. Lean signatures (informal; namespace `gf2Algebra.packed.bipedal3` per D1a §2):

```
namespace Bipedal3Correctness

variable {P : Std.U64}

theorem bipedal3_add_correct (hP3 : P.val = 3) (m1 s1 m2 s2 : Std.U64) (i : Fin 64) :
    let r := gf2Algebra.packed.bipedal3.Bipedal3.add ⟨m1, s1⟩ ⟨m2, s2⟩
    psi (r.mag.bv.getLsbD i) (r.sgn.bv.getLsbD i)
      = (psi (m1.bv.getLsbD i) (s1.bv.getLsbD i)
         + psi (m2.bv.getLsbD i) (s2.bv.getLsbD i) : ZMod 3)

theorem bipedal3_sub_correct (hP3 : P.val = 3) (m1 s1 m2 s2 : Std.U64) (i : Fin 64) :
    let r := gf2Algebra.packed.bipedal3.Bipedal3.sub ⟨m1, s1⟩ ⟨m2, s2⟩
    psi (r.mag.bv.getLsbD i) (r.sgn.bv.getLsbD i)
      = (psi (m1.bv.getLsbD i) (s1.bv.getLsbD i)
         - psi (m2.bv.getLsbD i) (s2.bv.getLsbD i) : ZMod 3)

theorem bipedal3_mul_correct (hP3 : P.val = 3) (m1 s1 m2 s2 : Std.U64) (i : Fin 64) :
    let r := gf2Algebra.packed.bipedal3.Bipedal3.mul ⟨m1, s1⟩ ⟨m2, s2⟩
    psi (r.mag.bv.getLsbD i) (r.sgn.bv.getLsbD i)
      = (psi (m1.bv.getLsbD i) (s1.bv.getLsbD i)
         * psi (m2.bv.getLsbD i) (s2.bv.getLsbD i) : ZMod 3)

theorem bipedal3_div_correct (hP3 : P.val = 3) (m1 s1 m2 s2 : Std.U64) (i : Fin 64)
    (hb_nonzero : (m2.bv.getLsbD i) = true) :
    let r := gf2Algebra.packed.bipedal3.Bipedal3.div ⟨m1, s1⟩ ⟨m2, s2⟩
    psi (r.mag.bv.getLsbD i) (r.sgn.bv.getLsbD i)
      = (psi (m1.bv.getLsbD i) (s1.bv.getLsbD i)
         / psi (m2.bv.getLsbD i) (s2.bv.getLsbD i) : ZMod 3)

end Bipedal3Correctness
```

Mathlib `ZMod 3` carries the `Field` instance against which we compare. The lane-wise quantification over `i : Fin 64` is what makes the statement match a packed-vector implementation; the per-lane reduction is discharged once and reused for all four ops.

A second-level theorem (the headline V1 contract referenced from the epic doc §12 V1) combines all four into one statement:

```
theorem bipedal3_correct_vs_canonical_F3 (op : ArithOp) (a b : Bipedal3) (i : Fin 64) :
    psi_lane (Bipedal3.dispatch op a b) i
      = ZMod3.dispatch op (psi_lane a i) (psi_lane b i)
```

This is a corollary of the four lemmas above plus a finite case-split on `op : {add, sub, mul, div}` and is included for completeness; the substantive content is in the four per-op lemmas.

## 2. Decoder ψ

```
/-- Bipedal decoder. Maps both (0,0) and (0,1) to 0 (alternative-zero codeword). -/
def psi : Bool → Bool → ZMod 3
  | false, false => 0
  | false, true  => 0   -- alternative-zero codeword (paper §2.1)
  | true,  false => 1
  | true,  true  => 2   -- = -1 in ZMod 3
```

### Lemma list

| Name | Statement | Tactic |
|------|-----------|--------|
| `psi_zero_zero` | `psi false false = 0` | `rfl` |
| `psi_zero_one_alt` | `psi false true = 0` | `rfl` |
| `psi_one_zero` | `psi true false = 1` | `rfl` |
| `psi_one_one` | `psi true true = 2` | `rfl` |
| `psi_total` | `∀ m s, psi m s ∈ ({0, 1, 2} : Set (ZMod 3))` | `decide` after `Bool` exhaustion |
| `psi_neg` | `psi m (s != m && m) = - psi m s` (negation under `(m, s ⊕ m)` for nonzero `m`; for `m = 0` both sides are 0) | `decide` (8-element Bool×Bool case-split) |

`psi_neg` is needed only if the implementation chooses the prototype's `sub = add ∘ neg` factoring; the epic-§2.2 sub formula does not need it. We will pick the formula route for the V1 statement (cleaner per-op lemma) and prove `psi_neg` only as a sanity check.

## 3. Per-op lemma list

The bipedal formulas are bitwise identities on a single bit. Each per-op theorem follows from the per-lane truth-table check: enumerate the 16 cases of `(m1, s1, m2, s2) ∈ Bool^4` (or 8 for div given the nonzero hypothesis) and check that the bipedal output bit-pair decodes to the expected `ZMod 3` value. Mathlib's `decide` handles each row trivially because both sides reduce to closed `ZMod 3` values.

The lifting from "single lane" to "all 64 lanes simultaneously, given the implementation operates on `u64`" is the content of §4 below.

### 3.1 mul (paper §2.2, 2 ops): `m_× = m1 & m2`, `s_× = s1 ⊕ s2`

| Name | Statement (informal) | Tactic |
|------|----------------------|--------|
| `bipedal3_mul_lane` | `∀ (m1 s1 m2 s2 : Bool), psi (m1 ∧ m2) (s1 ⊕ s2) = psi m1 s1 * psi m2 s2 (in ZMod 3)` | `decide` (16-case truth table; closed `ZMod 3`) |
| `bipedal3_mul_word` | `∀ (m1 s1 m2 s2 : BitVec 64) (i : Fin 64), getLsbD (m1 &&& m2) i = (getLsbD m1 i ∧ getLsbD m2 i) ∧ getLsbD (s1 ^^^ s2) i = (getLsbD s1 i ⊕ getLsbD s2 i)` | `simp [BitVec.getLsbD_and, BitVec.getLsbD_xor]` |
| `bipedal3_mul_correct` | per-lane theorem from §1 | `apply bipedal3_mul_lane`; close getLsbD via `bipedal3_mul_word` |

### 3.2 add (paper §2.2, 6 ops): `t = m1 ⊕ s1 ⊕ s2; u = m2 ∧ t; m_+ = u | (m1 ⊕ m2); s_+ = u ⊕ s1`

| Name | Statement | Tactic |
|------|-----------|--------|
| `bipedal3_add_lane` | `∀ (m1 s1 m2 s2 : Bool), let t := m1 ⊕ s1 ⊕ s2; let u := m2 ∧ t; psi (u ∨ (m1 ⊕ m2)) (u ⊕ s1) = psi m1 s1 + psi m2 s2 (in ZMod 3)` | `decide` (16-case truth table) |
| `bipedal3_add_word` | as `bipedal3_mul_word` for `&&&`/`xor`/`|||` | `simp [BitVec.getLsbD_and, BitVec.getLsbD_xor, BitVec.getLsbD_or]` |
| `bipedal3_add_correct` | per-lane theorem from §1 | `apply bipedal3_add_lane`; close via `bipedal3_add_word` |

### 3.3 sub (paper §2.2, 6 ops): `t = s1 ⊕ s2; u = m1 ∧ t; m_- = u | (m1 ⊕ m2); s_- = u ⊕ (m2 ⊕ s2)`

| Name | Statement | Tactic |
|------|-----------|--------|
| `bipedal3_sub_lane` | `∀ (m1 s1 m2 s2 : Bool), let t := s1 ⊕ s2; let u := m1 ∧ t; psi (u ∨ (m1 ⊕ m2)) (u ⊕ (m2 ⊕ s2)) = psi m1 s1 - psi m2 s2 (in ZMod 3)` | `decide` (16-case truth table) |
| `bipedal3_sub_word` | as `bipedal3_mul_word` | `simp [BitVec.getLsbD_and, BitVec.getLsbD_xor, BitVec.getLsbD_or]` |
| `bipedal3_sub_correct` | per-lane theorem from §1 | `apply bipedal3_sub_lane`; close via `bipedal3_sub_word` |

Note on the `sub = add ∘ neg` alternative (used in `dev/research/f3_bipedal/src/bipedal.rs`): if D1a/D1b decide to express `sub` as `add` composed with a separate `neg`, the lemma list grows by one (`bipedal3_neg_lane` via the same `decide` pattern) and `bipedal3_sub_correct` is proved via `bipedal3_add_lane ∘ bipedal3_neg_lane` instead of a direct truth table. Both factorings work; the proof cost is identical.

### 3.4 div by nonzero (paper §2.2, 1 op given F_3* self-inverse: `m_÷ = m1`, `s_÷ = s1 ⊕ s2`)

| Name | Statement | Tactic |
|------|-----------|--------|
| `bipedal3_div_lane` | `∀ (m1 s1 m2 s2 : Bool), m2 = true → psi m1 (s1 ⊕ s2) = psi m1 s1 / psi m2 s2 (in ZMod 3)` | `decide` (8-case truth table given `m2 = true`) |
| `bipedal3_div_word` | as `bipedal3_mul_word` (only `xor` needed) | `simp [BitVec.getLsbD_xor]` |
| `bipedal3_div_correct` | per-lane theorem from §1 | `apply bipedal3_div_lane`; pull `m2_i = true` from `hb_nonzero`; close via `bipedal3_div_word` |

The nonzero hypothesis `m2.bv.getLsbD i = true` is the lane-level form of "divisor lane is nonzero." Note: the (0, 1) alternative-zero codeword is excluded by the implementation never producing it (Bipedal3 invariant: only the canonical encoding is constructed; the redundancy is for cheap arithmetic only). The `hb_nonzero` hypothesis pins the lane to `m2 = true`, which ensures `psi m2 s2 ∈ {1, 2}` and so is invertible in `ZMod 3`. No `m2 = false ∧ s2 = true` case is admitted.

### 3.5 Exhaustive low-level truth-table fallback

For each `*_lane` lemma above, an alternative tactic `bv_decide` would also work (it bit-blasts the entire 4-Bool / 64-lane statement to SAT). We prefer `decide` on the 16-case enumeration because (a) it is faster than `bv_decide` for tiny cases, (b) it does not require kernel-level `bv_decide` support which has historically been slower on this project, and (c) it produces stable proof terms that survive Mathlib version churn. `bv_decide` is held in reserve as a fallback if Mathlib changes the closed evaluation of `ZMod 3` arithmetic in a way that breaks `decide`.

## 4. Lane-vs-vector lifting

The implementation operates on `Std.U64` pairs; the spec lemma operates on `Bool` per lane. The bridge is `BitVec.getLsbD` and the standard `getLsbD_and / getLsbD_or / getLsbD_xor` simp lemmas.

### Lifting lemma

```
theorem getLsbD_bitwise_lift (op : BitVec 64 → BitVec 64 → BitVec 64)
    (op_lane : Bool → Bool → Bool)
    (h_lift : ∀ (x y : BitVec 64) (i : Fin 64),
      (op x y).getLsbD i = op_lane (x.getLsbD i) (y.getLsbD i))
    (m1 s1 m2 s2 : Std.U64) (i : Fin 64) :
    (gf2Algebra.packed.bipedal3.Bipedal3.add ⟨m1, s1⟩ ⟨m2, s2⟩ ).mag.bv.getLsbD i
      = -- the corresponding Bool expression on (m1.bv.getLsbD i, ...)
      ...
```

For the four bitwise ops we need, Mathlib provides the per-bit lifting directly:

- `BitVec.getLsbD_and : (x &&& y).getLsbD i = (x.getLsbD i && y.getLsbD i)`
- `BitVec.getLsbD_or  : (x ||| y).getLsbD i = (x.getLsbD i || y.getLsbD i)`
- `BitVec.getLsbD_xor : (x ^^^ y).getLsbD i = (x.getLsbD i ^^ y.getLsbD i)`

So the lift step is `simp [BitVec.getLsbD_and, BitVec.getLsbD_or, BitVec.getLsbD_xor]` (one tactic, three simp lemmas, no manual induction over the 64 lanes).

| Name | Statement | Tactic |
|------|-----------|--------|
| `getLsbD_bitwise_lift` | For any binary bitwise op `op : BitVec 64 → BitVec 64 → BitVec 64` formed from `&&&` / `\|\|\|` / `^^^`, with per-lane Boolean witness `op_lane : Bool → Bool → Bool` such that `(op x y).getLsbD i = op_lane (x.getLsbD i) (y.getLsbD i)`, the lift propagates through `Std.U64.bv` projections used by `Bipedal3::{add, sub, mul, div}`. | `simp [BitVec.getLsbD_and, BitVec.getLsbD_or, BitVec.getLsbD_xor]` |

The Aeneas-extracted Bipedal3 ops use `Std.U64` arithmetic. `Std.U64.bv` projects to `BitVec 64`; `UScalar.val_and / val_or / val_xor` (already used in `Gf2mAddition.lean`) bridge `.val` to the bitwise ops. The chain is:

```
Bipedal3.add ⟨m1, s1⟩ ⟨m2, s2⟩
  → Aeneas WP unfolding (no Result-monad branching: pure bitwise ops)
  → simp [UScalar.val_xor, UScalar.val_or, UScalar.val_and] reduces .val to BitVec ops
  → simp [BitVec.getLsbD_*] reduces per-lane to Bool ops
  → apply bipedal3_*_lane (closed `decide` truth table)
```

This matches the existing `gf2m_add_raw_correct` proof shape in `proofs/Gf2Core/Proofs/Gf2mAddition.lean` (which uses `simp [UScalar.val_xor]` then `rfl` on the spec). The bipedal proofs are 3–4 lines longer per op because of the additional `getLsbD` step and the `decide` evaluator on `ZMod 3`.

## 5. Production code path (Charon extraction target)

**Rust source:** `crates/gf2-algebra/src/packed/bipedal3.rs::Bipedal3::{add, sub, mul, div}`, per the D1a crate-boundary decision (`dev/plans/d1a_gf2_algebra_boundary.md` §2). The prototype at `dev/research/f3_bipedal/src/bipedal.rs` is a research artefact and is **not** the extraction target.

**Exact function signatures expected for V1 (Bipedal3 element-level, single `(u64, u64)` pair, 64 lanes):**

```rust
// crates/gf2-algebra/src/packed/bipedal3.rs

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Bipedal3 { mag: u64, sgn: u64 }

impl Bipedal3 {
    #[inline] pub const fn add(self, r: Self) -> Self { ... }    // paper §2.2 add formula
    #[inline] pub const fn sub(self, r: Self) -> Self { ... }    // paper §2.2 sub formula (NOT add∘neg)
    #[inline] pub const fn mul(self, r: Self) -> Self { ... }    // paper §2.2 mul formula
    #[inline] pub fn      div(self, r: Self) -> Self { ... }     // paper §2.2 div formula; r assumed lane-wise nonzero
    // neg is also extracted but is not part of the V1 contract; included for follow-up.
}
```

V1 explicitly targets the **element-level** ops (single `Bipedal3` pair, 64 lanes via the underlying U64 pair). It does **not** cover `Bipedal3Vec` or `Bipedal3Matrix` or any SIMD-batched dispatch — those are V1-out-of-scope (see §8 below). A follow-up sketch may extend to `Bipedal3Vec` once the single-pair correctness is in place.

**Why element-level is sufficient:** the multi-word `Bipedal3Vec` ops are the same per-`u64`-pair formulas applied in a `for w in 0..n_words` loop with a tail-mask. Once the single-pair theorem is proved, the vector form follows by `Vec` induction without any new bitwise reasoning. Splitting V1 into "element" and "vector" sub-proofs lets V1 land before D1a/T4 (Bipedal3Vec) is even started, which de-risks the epic schedule.

## 6. Expected Aeneas-generated def names

Following the existing extraction convention (e.g., `gf2_core::gfp::montgomery::compute_p_inv` becomes `gfp.montgomery.compute_p_inv` in `proofs/Gf2Core/Funs.lean`), the bipedal extraction lives in the `gf2-algebra` crate per D1a §2. The `verify-lean.sh` pipeline learns to extract this second crate; no `Bipedal3` mirror in `gf2-core` is created.

**Aeneas-generated def names (extraction from the `gf2-algebra` crate):**

```
gf2_algebra.packed.bipedal3.Bipedal3                       -- struct type
gf2_algebra.packed.bipedal3.Bipedal3.add                   -- (Bipedal3) (Bipedal3) → Result Bipedal3
gf2_algebra.packed.bipedal3.Bipedal3.sub                   -- (Bipedal3) (Bipedal3) → Result Bipedal3
gf2_algebra.packed.bipedal3.Bipedal3.mul                   -- (Bipedal3) (Bipedal3) → Result Bipedal3
gf2_algebra.packed.bipedal3.Bipedal3.div                   -- (Bipedal3) (Bipedal3) → Result Bipedal3
gf2_algebra.packed.bipedal3.Bipedal3.neg                   -- (Bipedal3) → Result Bipedal3 (V1-out-of-scope)
gf2_algebra.packed.bipedal3.Bipedal3.ZERO                  -- const (extracted as a function returning 0)
gf2_algebra.packed.bipedal3.Bipedal3.ONE                   -- const
```

The Lean type for the struct will be (matching the existing `Fp` extraction pattern in `proofs/Gf2Core/Types.lean`):

```
structure gf2_algebra.packed.bipedal3.Bipedal3 where
  mag : Std.U64
  sgn : Std.U64
```

V1 proofs live in a new file `proofs/Gf2Algebra/Proofs/Bipedal3Correctness.lean` (mirror of the existing `proofs/Gf2Core/Proofs/...` layout). The Lake project root will need a second module entry in `lakefile.lean` — this is a small infrastructure change tracked separately and is **not** part of the V1 implementation issue itself.

## 7. External-fns / axioms anticipated

The bipedal ops use only:

- `BitVec` `&&&`, `|||`, `^^^` (already supported natively by Aeneas via `UScalar.val_and / val_or / val_xor`).
- No `wrapping_*`, no `overflowing_*`, no `U128`, no `redc`, no Newton iteration. The proofs are categorically simpler than `MontgomeryRoundtrip.lean`.

**No new `FunsExternal.lean` definitions are required.** The four existing custom defs (`wrapping_neg`, `overflowing_sub`, `U128 add`, `U128 add_assign`) are not exercised by any of `Bipedal3::{add, sub, mul, div}`. This is a deliberate design property of bipedal F_3 arithmetic: it is purely bitwise on `u64` and never overflows.

The single concrete external dependency is the `core::ops::{BitAnd, BitOr, BitXor}` trait surface for `u64`, which Aeneas extracts into `core.num.U64.bitand / bitor / bitxor`. These are already exercised in `Gf2mAddition.lean` and need no additional axiomatic content.

If D1b's PackedField trait surface (epic §6) introduces any non-bitwise fallback path for `Bipedal3` (e.g., a `splat` that calls a `.fill()`-style loop, or a `lane` extractor that uses `Vec` indexing), those auxiliary functions are not part of the V1 contract — V1 proves only the four arithmetic ops on the `(u64, u64)` representation.

## 8. Risks and out-of-scope

### Risks

| # | Risk | Mitigation |
|---|------|------------|
| R1 | A future refactor moves `Bipedal3` out of `gf2-algebra` (e.g., consolidation with another packed-field crate) | D1a §2 fixes the home as `gf2-algebra::packed::bipedal3::Bipedal3`. Sketch is signature-driven; if a future epic re-homes the type, only the namespace prefix in §6 changes and the `open` directives in the proof file follow. |
| R2 | D1b introduces a `PackedField` trait that wraps `Bipedal3::{add,sub,mul,div}` in a generic-trait dispatch layer | Charon may extract trait dispatch into an indirection that breaks `decide`. Mitigation: prove against the inherent (non-trait) methods first; the trait-dispatch lemma is a one-line corollary. |
| R3 | `decide` evaluator on `ZMod 3` becomes slow if Mathlib refactors `ZMod` to a non-`Decidable` representation | Fallback: `bv_decide` over the same statement; or `fin_cases` on `(m1, s1, m2, s2) : Fin 2^4` followed by `rfl`. |
| R4 | The (0, 1) "alternative-zero" redundant codeword leaks into a real input via a buggy constructor | Out-of-scope for V1: V1 proves the formula correctness assuming the Bipedal3 invariant (no "alt-zero" produced by any constructor) holds. A separate "no-alt-zero" invariant lemma is a follow-up. The two cases `psi false false = 0` and `psi false true = 0` both coincidentally decode to 0 in ZMod 3, so the formulas are still correct on alt-zero inputs — verified by the truth table. |
| R5 | `Std.U64.bv.getLsbD` simp normal form changes between Mathlib versions | Pin via `lean-toolchain` (already done in `proofs/lean-toolchain`); test against current Mathlib in CI before V1 is closed. |
| R6 | Charon extraction warnings ("Type error after transformations") on the new crate | The existing 13 such warnings on `gf2-core` are benign per `MEMORY.md`. Apply the same `scripts/fix-aeneas-dupes.py` post-processing. If new failures surface, escalate to the user. |

### Out of scope for V1

1. **Bipedal3Vec correctness.** V1 covers single `(u64, u64)` pair only. Multi-word vectors are a follow-up sketch (D2-vector) once D1a is settled.
2. **Bipedal3Matrix correctness.** Trivially follows from Bipedal3Vec; not in V1.
3. **Mask-tail invariant.** The `mask_tail()` machinery from CLAUDE.md §Key design invariants is a Vec-level concern; V1 has no tail (single 64-lane pair fully populated).
4. **SIMD dispatch correctness.** Once `gf2-kernels-simd::bipedal3_kernel` is written (T12 in epic §13), proving the AVX2/AVX-512 path matches the scalar path is a separate verification track; the kernel crate is `unsafe` and outside the Aeneas-supported subset (per `dev/plans/formal_verification.md`).
5. **Permanent-formula correctness.** V2 (separate sketch D3, separate issue) covers Ryser's formula bounded to $n \le 63$. V1 and V2 are independent.
6. **Div-by-zero handling.** V1 takes a per-lane `m2 = true` hypothesis. Whole-vector "no zero divisor" is a Vec-level invariant proved at the call-site of `Bipedal3Vec::div`; not in V1.
7. **F_3* self-inverse algebraic justification.** The div formula `m_÷ = m1`, `s_÷ = s1 ⊕ s2` works because `2 · 2 ≡ 1 (mod 3)`, so every nonzero element of F_3 is its own inverse, and `a / b = a · b` for nonzero `b`. This algebraic fact is implicit in the `decide` truth table — we are not separately proving "F_3* is a self-inverse group" at the abstract level.

## 9. Reference: existing tactic patterns reused

| Pattern | Source proof | Where reused in V1 |
|---------|--------------|--------------------|
| `simp [UScalar.val_xor]` to bridge `.val` and `BitVec.xor` | `proofs/Gf2Core/Proofs/Gf2mAddition.lean:27` | All four per-op proofs |
| `simp [BitVec.getLsbD_and]` to lift bit-bv to bit-Bool | `proofs/Gf2Core/Proofs/Progress.lean:41` | All four per-op proofs (lift step) |
| `decide` truth-table on closed ZMod values | (new pattern; analogous to `native_decide` on `2 ^ 63 < U64.size`, e.g., `MontgomeryRoundtrip.lean:51`) | All four `*_lane` lemmas |
| `apply bipedal3_*_lane` after lift step | (new compositional pattern) | All four `*_correct` per-lane theorems |
| `progress as ⟨r, hr⟩` for Aeneas Result-monad elimination | `proofs/Gf2Core/Proofs/MontgomeryRoundtrip.lean:48` | Not needed — bipedal ops are `Result`-pure (no error path) |

The bipedal proofs are categorically simpler than the existing Montgomery and Gf2m proofs because there is no Result-monad branching, no Newton iteration, no shift-reduce loop, no overflow reasoning. The estimated proof file size is **~150 lines** (vs ~400 for `Gf2mAddition.lean` / `Gf2mMulRaw.lean` combined and ~700 for `MontgomeryRoundtrip.lean`).

## 10. Lemma-count summary

| Category | Count | Tactic distribution |
|----------|-------|---------------------|
| Decoder ψ | 6 | `rfl` × 4, `decide` × 2 |
| Per-op lane-level (`*_lane`) | 4 | `decide` × 4 |
| Per-op word-level (`*_word`) | 4 | `simp [BitVec.getLsbD_*]` × 4 |
| Per-op packed-correctness (`*_correct`) | 4 | `simp + apply *_lane` × 4 |
| Lifting helper (`getLsbD_bitwise_lift`) | 1 | `simp [BitVec.getLsbD_*]` |
| Headline corollary (`bipedal3_correct_vs_canonical_F3`) | 1 | `cases op <;> apply *_correct` |
| **Total** | **20** | All tactics from the existing project's stable set |

## 11. Approval block

This sketch must be approved by the user (per CLAUDE.md §Verification work) before V1 implementation is dispatched. The project lead handles the approval/escalation workflow (success-criterion 5 of the issue is deferred to the lead per dispatch instructions).

Once approved, the V1 implementation issue is dispatched as: "implement the 20 lemmas listed in §10 of `dev/plans/d2_lean_bipedal3_sketch.md`, in the file `proofs/Gf2Algebra/Proofs/Bipedal3Correctness.lean`, against Charon-extracted `gf2_algebra.packed.bipedal3.Bipedal3.{add,sub,mul,div}`." The implementation issue's success criteria mirror this sketch's lemma list verbatim.
