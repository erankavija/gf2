/-
  Gf2Algebra.Proofs.Bipedal3Correctness — V1 bipedal F_3 correctness

  Implements the 20 lemmas of the D2 proof sketch
  (`dev/plans/d2_lean_bipedal3_sketch.md`) for JIT issue f05ffbe1.

  Proof target (Option A in the dispatch prompt; sketch §5): the four
  inherent wrappers `Bipedal3.{add,sub,mul,neg}_inherent` defined in
  `crates/gf2-algebra/src/packed/bipedal3.rs`. Each wrapper delegates a
  single tail-call to the corresponding `PackedField<Fp<3>>` trait method
  on `Bipedal3`; the bipedal formula lives in the trait impl. Targeting
  the inherent wrappers gives us a stable, non-dispatch-indirected name
  that survives any future PackedField refactor.

  Divergence from the sketch §1: the sketch's V1 statement names the
  four operations as `{add, sub, mul, div}` (because in the abstract F_3,
  div by a nonzero element coincides with mul by the same — F_3* is
  self-inverse). The production code at `bipedal3.rs` exposes
  `{add, sub, mul, neg}` — `neg` instead of `div`, matching the
  PackedField trait surface (`packed/mod.rs:185–224`). Per the
  verification-work convention, the sketch's intent (one easy operation
  alongside the three main ones) is preserved by proving `neg`. The
  same per-lane truth-table tactic discharges the new lemma; no
  structural change to the sketch is needed.

  All 20 lemmas correspond verbatim to sketch §10 with the
  `div` ↦ `neg` substitution.
-/
import Aeneas
import Mathlib.Data.ZMod.Basic
import Gf2Algebra.Funs

open Aeneas Aeneas.Std Result ControlFlow Error
open gf2_algebra

set_option maxHeartbeats 1200000

namespace Bipedal3Correctness

/-! ## §2 Decoder ψ

The bipedal lane decoder. Bits `(m, s)` encode `ψ(m, s) : ZMod 3`:
  - `(false, false)` → `0` (canonical zero)
  - `(false, true)`  → `0` (alternative-zero codeword — paper §2.1)
  - `(true,  false)` → `1`
  - `(true,  true)`  → `2` (= −1 in ZMod 3)
-/

/-- Bipedal decoder. Both `(0,0)` and `(0,1)` map to `0` (alternative-zero
codeword); `(1, 0) ↦ 1`; `(1, 1) ↦ 2`. -/
def psi : Bool → Bool → ZMod 3
  | false, false => 0
  | false, true  => 0
  | true,  false => 1
  | true,  true  => 2

/-- §2 ψ truth table: `(0, 0) ↦ 0`. -/
theorem psi_zero_zero : psi false false = 0 := rfl

/-- §2 ψ truth table: `(0, 1) ↦ 0` (alternative-zero codeword). -/
theorem psi_zero_one_alt : psi false true = 0 := rfl

/-- §2 ψ truth table: `(1, 0) ↦ 1`. -/
theorem psi_one_zero : psi true false = 1 := rfl

/-- §2 ψ truth table: `(1, 1) ↦ 2`. -/
theorem psi_one_one : psi true true = 2 := rfl

/-- §2 totality: `ψ` lands in `{0, 1, 2} ⊂ ZMod 3`. -/
theorem psi_total (m s : Bool) : psi m s = 0 ∨ psi m s = 1 ∨ psi m s = 2 := by
  cases m <;> cases s <;> decide

/-- §2 negation lemma. For nonzero `m`, ψ-negation under the bipedal
`(m, s ⊕ m)` form coincides with additive negation in `ZMod 3`; for
`m = false` both sides decode to `0`. -/
theorem psi_neg (m s : Bool) :
    psi m (xor s m) = -(psi m s) := by
  cases m <;> cases s <;> decide

/-! ## §3.1 mul lane truth table

The bipedal mul formula `(m_×, s_×) = (m₁ ∧ m₂, s₁ ⊕ s₂)` realises
F_3 multiplication on each lane. The 16-case truth table on the four
Booleans closes by `decide`.
-/

/-- §3.1 per-lane mul correctness on Bools. -/
theorem bipedal3_mul_lane (m1 s1 m2 s2 : Bool) :
    psi (m1 && m2) (xor s1 s2) = psi m1 s1 * psi m2 s2 := by
  cases m1 <;> cases s1 <;> cases m2 <;> cases s2 <;> decide

/-! ## §3.2 add lane truth table

The bipedal add formula `(m_+, s_+) = (u ∨ (m₁ ⊕ m₂), u ⊕ s₁)` with
`u = m₂ ∧ (m₁ ⊕ s₁ ⊕ s₂)` realises F_3 addition. 16-case `decide`.
-/

/-- §3.2 per-lane add correctness on Bools. -/
theorem bipedal3_add_lane (m1 s1 m2 s2 : Bool) :
    let t := xor (xor m1 s1) s2
    let u := m2 && t
    psi (u || xor m1 m2) (xor u s1) = psi m1 s1 + psi m2 s2 := by
  cases m1 <;> cases s1 <;> cases m2 <;> cases s2 <;> decide

/-! ## §3.3 sub lane truth table

The bipedal sub formula `(m_-, s_-) = (u ∨ (m₁ ⊕ m₂), u ⊕ (m₂ ⊕ s₂))`
with `u = m₁ ∧ (s₁ ⊕ s₂)` realises F_3 subtraction. 16-case `decide`.
-/

/-- §3.3 per-lane sub correctness on Bools. -/
theorem bipedal3_sub_lane (m1 s1 m2 s2 : Bool) :
    let t := xor s1 s2
    let u := m1 && t
    psi (u || xor m1 m2) (xor u (xor m2 s2)) = psi m1 s1 - psi m2 s2 := by
  cases m1 <;> cases s1 <;> cases m2 <;> cases s2 <;> decide

/-! ## §3.4 neg lane truth table

Substituted for `div` per the divergence note in the file docstring.
The bipedal neg formula `(m_-, s_-) = (m, s ⊕ m)` realises F_3
additive negation. 4-case `decide`.
-/

/-- §3.4 (per-lane neg correctness on Bools, in place of the sketch's
div-by-nonzero lemma). -/
theorem bipedal3_neg_lane (m s : Bool) :
    psi m (xor s m) = -(psi m s) := psi_neg m s

/-! ## §3-word: per-lane lifting

For each binary bitwise op `&&&`, `|||`, `^^^` on `BitVec 64`, Mathlib
provides `getLsbD_and / getLsbD_or / getLsbD_xor`. We restate them for
proof-search clarity and to match the sketch's lemma list.
-/

/-- §3.1-word: `getLsbD` distributes through `&&&` and `^^^`. The proofs
follow directly from Mathlib's `BitVec.getLsbD_and` and
`BitVec.getLsbD_xor` simp lemmas. -/
theorem bipedal3_mul_word (m1 s1 m2 s2 : BitVec 64) (i : Fin 64) :
    (m1 &&& m2).getLsbD i = (m1.getLsbD i && m2.getLsbD i)
    ∧ (s1 ^^^ s2).getLsbD i = xor (s1.getLsbD i) (s2.getLsbD i) := by
  refine ⟨?_, ?_⟩ <;> simp

/-- §3.2-word: `getLsbD` distributes through `&&&`, `^^^`, `|||`. -/
theorem bipedal3_add_word (m1 s1 m2 s2 : BitVec 64) (i : Fin 64) :
    (m1 ^^^ s1).getLsbD i = xor (m1.getLsbD i) (s1.getLsbD i)
    ∧ (m1 &&& s2).getLsbD i = (m1.getLsbD i && s2.getLsbD i)
    ∧ (m1 ||| m2).getLsbD i = (m1.getLsbD i || m2.getLsbD i) := by
  refine ⟨?_, ?_, ?_⟩ <;> simp

/-- §3.3-word: same shape as §3.2-word; sub uses the same three bitwise ops. -/
theorem bipedal3_sub_word (m1 s1 m2 s2 : BitVec 64) (i : Fin 64) :
    (s1 ^^^ s2).getLsbD i = xor (s1.getLsbD i) (s2.getLsbD i)
    ∧ (m1 &&& s2).getLsbD i = (m1.getLsbD i && s2.getLsbD i)
    ∧ (m1 ||| m2).getLsbD i = (m1.getLsbD i || m2.getLsbD i) := by
  refine ⟨?_, ?_, ?_⟩ <;> simp

/-- §3.4-word: `getLsbD` distributes through `^^^` (only op needed for neg). -/
theorem bipedal3_neg_word (m s : BitVec 64) (i : Fin 64) :
    (s ^^^ m).getLsbD i = xor (s.getLsbD i) (m.getLsbD i) := by
  simp

/-! ## §4 Lifting helper

A single statement of the per-lane lifting principle for binary bitwise
ops. The four `*_correct` theorems below instantiate it implicitly via
`simp [BitVec.getLsbD_*]`.
-/

/-- §4 lifting: any binary bitwise op on `BitVec 64` formed from
`&&&` / `|||` / `^^^` reduces lane-by-lane to its `Bool`-level form. -/
theorem getLsbD_bitwise_lift
    (op : BitVec 64 → BitVec 64 → BitVec 64)
    (op_lane : Bool → Bool → Bool)
    (h_lift : ∀ (x y : BitVec 64) (i : Fin 64),
      (op x y).getLsbD i = op_lane (x.getLsbD i) (y.getLsbD i))
    (x y : BitVec 64) (i : Fin 64) :
    (op x y).getLsbD i = op_lane (x.getLsbD i) (y.getLsbD i) :=
  h_lift x y i

/-! ## §3.1-correct: packed mul against `Bipedal3.mul_inherent`

The bridge between the Aeneas-extracted Result-monad function and the
per-lane spec. The chain (sketch §4):

  `Bipedal3.mul_inherent ⟨m1, s1⟩ ⟨m2, s2⟩`
    → unfold `mul_inherent` and `Insts.PackedField….mul` (Result-pure)
    → `simp [UScalar.val_xor, UScalar.val_and]` to bridge `.val` ↔ `BitVec`
    → `simp [BitVec.getLsbD_*]` to drop to `Bool`
    → `apply bipedal3_mul_lane` (closed `decide`).
-/

/-- §3.1-correct: per-lane mul correctness on the inherent wrapper. -/
theorem bipedal3_mul_correct (m1 s1 m2 s2 : Std.U64) (i : Fin 64) :
    ∃ r, packed.bipedal3.Bipedal3.mul_inherent ⟨m1, s1⟩ ⟨m2, s2⟩ = ok r ∧
      psi (r.mag.bv.getLsbD i) (r.sgn.bv.getLsbD i)
        = psi (m1.bv.getLsbD i) (s1.bv.getLsbD i)
          * psi (m2.bv.getLsbD i) (s2.bv.getLsbD i) := by
  unfold packed.bipedal3.Bipedal3.mul_inherent
  unfold packed.bipedal3.Bipedal3.Insts.Gf2_algebraPackedPackedFieldFp3U64U128.mul
  refine ⟨_, rfl, ?_⟩
  -- Bring the Result-monad body into normal form (the result struct has
  -- `mag = m1 &&& m2` and `sgn = s1 ^^^ s2`).
  -- Drop `.bv` projections to BitVec ops.
  show psi (((m1.bv &&& m2.bv)).getLsbD i) (((s1.bv ^^^ s2.bv)).getLsbD i)
    = psi (m1.bv.getLsbD i) (s1.bv.getLsbD i)
      * psi (m2.bv.getLsbD i) (s2.bv.getLsbD i)
  simp only [BitVec.getLsbD_and, BitVec.getLsbD_xor]
  exact bipedal3_mul_lane _ _ _ _

/-- §3.2-correct: per-lane add correctness on the inherent wrapper. -/
theorem bipedal3_add_correct (m1 s1 m2 s2 : Std.U64) (i : Fin 64) :
    ∃ r, packed.bipedal3.Bipedal3.add_inherent ⟨m1, s1⟩ ⟨m2, s2⟩ = ok r ∧
      psi (r.mag.bv.getLsbD i) (r.sgn.bv.getLsbD i)
        = psi (m1.bv.getLsbD i) (s1.bv.getLsbD i)
          + psi (m2.bv.getLsbD i) (s2.bv.getLsbD i) := by
  unfold packed.bipedal3.Bipedal3.add_inherent
  unfold packed.bipedal3.Bipedal3.Insts.Gf2_algebraPackedPackedFieldFp3U64U128.add
  refine ⟨_, rfl, ?_⟩
  show psi
        (((m2.bv &&& (m1.bv ^^^ s1.bv ^^^ s2.bv)) ||| (m1.bv ^^^ m2.bv)).getLsbD i)
        (((m2.bv &&& (m1.bv ^^^ s1.bv ^^^ s2.bv)) ^^^ s1.bv).getLsbD i)
      = psi (m1.bv.getLsbD i) (s1.bv.getLsbD i)
        + psi (m2.bv.getLsbD i) (s2.bv.getLsbD i)
  simp only [BitVec.getLsbD_and, BitVec.getLsbD_xor, BitVec.getLsbD_or]
  exact bipedal3_add_lane _ _ _ _

/-- §3.3-correct: per-lane sub correctness on the inherent wrapper. -/
theorem bipedal3_sub_correct (m1 s1 m2 s2 : Std.U64) (i : Fin 64) :
    ∃ r, packed.bipedal3.Bipedal3.sub_inherent ⟨m1, s1⟩ ⟨m2, s2⟩ = ok r ∧
      psi (r.mag.bv.getLsbD i) (r.sgn.bv.getLsbD i)
        = psi (m1.bv.getLsbD i) (s1.bv.getLsbD i)
          - psi (m2.bv.getLsbD i) (s2.bv.getLsbD i) := by
  unfold packed.bipedal3.Bipedal3.sub_inherent
  unfold packed.bipedal3.Bipedal3.Insts.Gf2_algebraPackedPackedFieldFp3U64U128.sub
  refine ⟨_, rfl, ?_⟩
  show psi
        (((m1.bv &&& (s1.bv ^^^ s2.bv)) ||| (m1.bv ^^^ m2.bv)).getLsbD i)
        (((m1.bv &&& (s1.bv ^^^ s2.bv)) ^^^ (m2.bv ^^^ s2.bv)).getLsbD i)
      = psi (m1.bv.getLsbD i) (s1.bv.getLsbD i)
        - psi (m2.bv.getLsbD i) (s2.bv.getLsbD i)
  simp only [BitVec.getLsbD_and, BitVec.getLsbD_xor, BitVec.getLsbD_or]
  exact bipedal3_sub_lane _ _ _ _

/-- §3.4-correct: per-lane neg correctness on the inherent wrapper. -/
theorem bipedal3_neg_correct (m s : Std.U64) (i : Fin 64) :
    ∃ r, packed.bipedal3.Bipedal3.neg_inherent ⟨m, s⟩ = ok r ∧
      psi (r.mag.bv.getLsbD i) (r.sgn.bv.getLsbD i)
        = -(psi (m.bv.getLsbD i) (s.bv.getLsbD i)) := by
  unfold packed.bipedal3.Bipedal3.neg_inherent
  unfold packed.bipedal3.Bipedal3.Insts.Gf2_algebraPackedPackedFieldFp3U64U128.neg
  refine ⟨_, rfl, ?_⟩
  show psi (m.bv.getLsbD i) ((s.bv ^^^ m.bv).getLsbD i)
    = -(psi (m.bv.getLsbD i) (s.bv.getLsbD i))
  simp only [BitVec.getLsbD_xor]
  exact bipedal3_neg_lane _ _

/-! ## §1-headline: the V1 contract

A single corollary statement that combines the four per-op theorems
into one, parameterised by an `ArithOp` tag. This is the headline
referenced from the epic doc §12 V1.
-/

/-- Tag for the four V1 ops. Used purely as a case-split parameter for
the headline corollary. -/
inductive ArithOp
  | add
  | sub
  | mul
  | neg
deriving DecidableEq

/-- Reference dispatch on `ZMod 3` per arithmetic tag. For `neg`,
the rhs operand is ignored. -/
def ZMod3.dispatch : ArithOp → ZMod 3 → ZMod 3 → ZMod 3
  | .add, a, b => a + b
  | .sub, a, b => a - b
  | .mul, a, b => a * b
  | .neg, a, _ => -a

/-- Bipedal dispatch on the inherent methods. Same calling convention
as `ZMod3.dispatch`: the rhs is ignored on `neg`. -/
def Bipedal3.dispatch :
    ArithOp → packed.bipedal3.Bipedal3 → packed.bipedal3.Bipedal3 →
      Result packed.bipedal3.Bipedal3
  | .add, a, b => packed.bipedal3.Bipedal3.add_inherent a b
  | .sub, a, b => packed.bipedal3.Bipedal3.sub_inherent a b
  | .mul, a, b => packed.bipedal3.Bipedal3.mul_inherent a b
  | .neg, a, _ => packed.bipedal3.Bipedal3.neg_inherent a

/-- §1 headline corollary: bipedal arithmetic vs canonical F_3
arithmetic on every lane, for every `ArithOp` tag. The four per-op
lemmas above provide the case-split discharge. -/
theorem bipedal3_correct_vs_canonical_F3
    (op : ArithOp) (m1 s1 m2 s2 : Std.U64) (i : Fin 64) :
    ∃ r, Bipedal3.dispatch op ⟨m1, s1⟩ ⟨m2, s2⟩ = ok r ∧
      psi (r.mag.bv.getLsbD i) (r.sgn.bv.getLsbD i)
        = ZMod3.dispatch op
            (psi (m1.bv.getLsbD i) (s1.bv.getLsbD i))
            (psi (m2.bv.getLsbD i) (s2.bv.getLsbD i)) := by
  cases op with
  | add => simpa [Bipedal3.dispatch, ZMod3.dispatch] using
             bipedal3_add_correct m1 s1 m2 s2 i
  | sub => simpa [Bipedal3.dispatch, ZMod3.dispatch] using
             bipedal3_sub_correct m1 s1 m2 s2 i
  | mul => simpa [Bipedal3.dispatch, ZMod3.dispatch] using
             bipedal3_mul_correct m1 s1 m2 s2 i
  | neg => simpa [Bipedal3.dispatch, ZMod3.dispatch] using
             bipedal3_neg_correct m1 s1 i

end Bipedal3Correctness
