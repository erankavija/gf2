/-
  Gf2Algebra.Proofs.Packed5Correctness — D5 packed F_5 correctness

  Implements the 23 lemmas of the D5 proof sketch
  (`dev/plans/d5_lean_packed5_sketch.md`) for JIT issue 30e98ef1.

  Proof target (sketch §4): the four inherent wrappers
  `Packed5.{add,sub,mul,neg}_inherent` defined in
  `crates/gf2-algebra/src/packed/packed5.rs`. Each wrapper delegates a
  single tail call to the corresponding `PackedField<Fp<5>>` trait method
  on `Packed5`; the bit-sliced Boolean circuit (decode → cross-product →
  encode) lives in the trait impl. Targeting the inherent wrappers gives a
  stable, non-dispatch-indirected name (same convention as the V1 bipedal
  proof in `Bipedal3Correctness.lean`).

  `Packed5` packs 64 independent F_5 lanes into three `u64` bit-planes
  `(b0, b1, b2)`; lane `i` decodes to the canonical value
  `bit_i(b0) | (bit_i(b1) << 1) | (bit_i(b2) << 2)`. Canonical codepoints
  are `0..=4`; the three redundant codepoints `5,6,7` decode to `0`
  (matching `Packed5::lane`, packed5.rs:566-583). The 64 lanes are
  mutually independent (every op is a fixed straight-line bitwise circuit
  on the three planes), so every per-op correctness theorem reduces to a
  single-lane truth table over the 8 Bool-triples, lifted to all 64 lanes
  by the `BitVec.getLsbD_{and,or,not}` simp set.

  The `*_word` lemmas are stated against the *exact composed bitwise
  expressions* produced by the Aeneas-extracted `decode5`/`*_circuit`/
  `encode5` functions in `Gf2Algebra/Funs.lean` (Array-of-5 selector
  model), so they are load-bearing — not generic `getLsbD` distributors.
-/
import Aeneas
import Mathlib.Data.ZMod.Basic
import Gf2Algebra.Funs

open Aeneas Aeneas.Std Result ControlFlow Error
open gf2_algebra

set_option maxHeartbeats 1200000

namespace Packed5Correctness

/-! ## §1 Decoder `dec5`

The 3-bit-plane lane decoder. `(b0, b1, b2 : Bool)` for one lane decode
to the canonical F_5 value; the three redundant codepoints 5,6,7 decode
to 0 (matching `Packed5::lane`, packed5.rs:578-582). -/

/-- 3-plane lane decoder for `Packed5`. The bit pattern
`b0 | (b1 << 1) | (b2 << 2)` is the canonical F_5 value for codepoints
`0..=4`; codepoints `5,6,7` decode to `0`. -/
def dec5 : Bool → Bool → Bool → ZMod 5
  | false, false, false => 0   -- 0
  | true,  false, false => 1   -- 1
  | false, true,  false => 2   -- 2
  | true,  true,  false => 3   -- 3
  | false, false, true  => 4   -- 4
  | true,  false, true  => 0   -- codepoint 5 → 0
  | false, true,  true  => 0   -- codepoint 6 → 0
  | true,  true,  true  => 0   -- codepoint 7 → 0

/-! ### §3.1 Decoder truth table -/

/-- §3.1 L1: `dec5` on canonical codepoint 0. -/
theorem dec5_0 : dec5 false false false = 0 := rfl

/-- §3.1 L2: `dec5` on canonical codepoint 1. -/
theorem dec5_1 : dec5 true false false = 1 := rfl

/-- §3.1 L3: `dec5` on canonical codepoint 2. -/
theorem dec5_2 : dec5 false true false = 2 := rfl

/-- §3.1 L4: `dec5` on canonical codepoint 3. -/
theorem dec5_3 : dec5 true true false = 3 := rfl

/-- §3.1 L5: `dec5` on canonical codepoint 4. -/
theorem dec5_4 : dec5 false false true = 4 := rfl

/-- §3.1 L6: the three redundant codepoints 5,6,7 all decode to 0. -/
theorem dec5_red5 : dec5 true false true = 0 := rfl
theorem dec5_red6 : dec5 false true true = 0 := rfl
theorem dec5_red7 : dec5 true true true = 0 := rfl

/-- §3.1 L7 totality: `dec5` lands in `{0,1,2,3,4} ⊂ ZMod 5`. -/
theorem dec5_total (x y z : Bool) :
    dec5 x y z = 0 ∨ dec5 x y z = 1 ∨ dec5 x y z = 2
      ∨ dec5 x y z = 3 ∨ dec5 x y z = 4 := by
  cases x <;> cases y <;> cases z <;> decide

/-- §3.1 L8 redundant-zero consistency: a codepoint decodes to 0 iff it is
the canonical zero `(0,0,0)` or one of the three redundant points
`(1,0,1)`, `(0,1,1)`, `(1,1,1)` (matches `Packed5::all_zero`,
packed5.rs all-zero / redundant treatment). -/
theorem dec5_red_eq_zero (x y z : Bool) :
    dec5 x y z = 0 ↔
      (x = false ∧ y = false ∧ z = false)
      ∨ (x = true ∧ y = false ∧ z = true)
      ∨ (x = false ∧ y = true ∧ z = true)
      ∨ (x = true ∧ y = true ∧ z = true) := by
  cases x <;> cases y <;> cases z <;> decide

/-! ## §3.2 Per-op lane truth tables

Each lemma states that the **exact composed circuit** of the
corresponding `packed5.rs` function, applied to one lane's 6 input
Bools, decodes to the F_5 result. The Bool selectors `e0..e4` /
`f0..f4` mirror `decode5`'s output exactly; the `r1..r4` accumulators
mirror the `*_circuit` ANDs/ORs; the final `dec5` of the encoded
`(c0,c1,c2)` mirrors `encode5`. The circuit is copied verbatim from the
Aeneas-extracted definitions, so these are load-bearing, not generic
distributors. -/

/-- Per-lane `decode5` selectors as Bools (mirrors
`packed.packed5.decode5`: `e0 = ¬b2∧¬b1∧¬b0`, … , `e4 = b2∧¬b1∧¬b0`). -/
@[inline] def selBool (b0 b1 b2 : Bool) : Bool × Bool × Bool × Bool × Bool :=
  let n0 := !b0
  let n1 := !b1
  let n2 := !b2
  let n2n1 := n2 && n1
  let n2_1 := n2 && b1
  let n1n0 := n1 && n0
  ( n2n1 && n0      -- e0  (value 0)
  , n2n1 && b0      -- e1  (value 1)
  , n2_1 && n0      -- e2  (value 2)
  , n2_1 && b0      -- e3  (value 3)
  , b2 && n1n0 )    -- e4  (value 4)

/-- `selBool` followed by the canonical 3-bit re-encode is `dec5`
(round-trip through the bit-sliced selector representation). This is the
algebraic core every per-op lane lemma reuses. -/
theorem dec5_selBool (b0 b1 b2 : Bool) :
    let s := selBool b0 b1 b2
    -- encode5: c0 = e1|e3, c1 = e2|e3, c2 = e4
    dec5 (s.2.1 || s.2.2.2.1) (s.2.2.1 || s.2.2.2.1) s.2.2.2.2
      = dec5 b0 b1 b2 := by
  cases b0 <;> cases b1 <;> cases b2 <;> decide

/-- §3.2 L9 per-lane add correctness on Bools. The `r1..r4` Bool
expressions mirror `packed.packed5.add_circuit` cell-for-cell; the final
`dec5` of `(c0,c1,c2) = encode5 [0,r1,r2,r3,r4]` equals
`dec5 a + dec5 b` in `ZMod 5`. 64-row `decide`. -/
theorem packed5_add_lane (a0 a1 a2 b0 b1 b2 : Bool) :
    let e := selBool a0 a1 a2
    let f := selBool b0 b1 b2
    let e0 := e.1;          let e1 := e.2.1
    let e2 := e.2.2.1;      let e3 := e.2.2.2.1;  let e4 := e.2.2.2.2
    let f0 := f.1;          let f1 := f.2.1
    let f2 := f.2.2.1;      let f3 := f.2.2.2.1;  let f4 := f.2.2.2.2
    let r1 := (e0&&f1) || (e1&&f0) || (e2&&f4) || (e3&&f3) || (e4&&f2)
    let r2 := (e0&&f2) || (e1&&f1) || (e2&&f0) || (e3&&f4) || (e4&&f3)
    let r3 := (e0&&f3) || (e1&&f2) || (e2&&f1) || (e3&&f0) || (e4&&f4)
    let r4 := (e0&&f4) || (e1&&f3) || (e2&&f2) || (e3&&f1) || (e4&&f0)
    dec5 (r1 || r3) (r2 || r3) r4 = dec5 a0 a1 a2 + dec5 b0 b1 b2 := by
  cases a0 <;> cases a1 <;> cases a2 <;> cases b0 <;> cases b1 <;> cases b2 <;> decide

/-- §3.2 L10 per-lane sub correctness on Bools (mirrors
`packed.packed5.sub_circuit`). 64-row `decide`. -/
theorem packed5_sub_lane (a0 a1 a2 b0 b1 b2 : Bool) :
    let e := selBool a0 a1 a2
    let f := selBool b0 b1 b2
    let e0 := e.1;          let e1 := e.2.1
    let e2 := e.2.2.1;      let e3 := e.2.2.2.1;  let e4 := e.2.2.2.2
    let f0 := f.1;          let f1 := f.2.1
    let f2 := f.2.2.1;      let f3 := f.2.2.2.1;  let f4 := f.2.2.2.2
    let r1 := (e0&&f4) || (e1&&f0) || (e2&&f1) || (e3&&f2) || (e4&&f3)
    let r2 := (e0&&f3) || (e1&&f4) || (e2&&f0) || (e3&&f1) || (e4&&f2)
    let r3 := (e0&&f2) || (e1&&f3) || (e2&&f4) || (e3&&f0) || (e4&&f1)
    let r4 := (e0&&f1) || (e1&&f2) || (e2&&f3) || (e3&&f4) || (e4&&f0)
    dec5 (r1 || r3) (r2 || r3) r4 = dec5 a0 a1 a2 - dec5 b0 b1 b2 := by
  cases a0 <;> cases a1 <;> cases a2 <;> cases b0 <;> cases b1 <;> cases b2 <;> decide

/-- §3.2 L11 per-lane mul correctness on Bools (mirrors
`packed.packed5.mul_circuit`; the `i=0`/`j=0` cells are absent — they
contribute to the unused `r0`). 64-row `decide`. -/
theorem packed5_mul_lane (a0 a1 a2 b0 b1 b2 : Bool) :
    let e := selBool a0 a1 a2
    let f := selBool b0 b1 b2
    let e1 := e.2.1
    let e2 := e.2.2.1;      let e3 := e.2.2.2.1;  let e4 := e.2.2.2.2
    let f1 := f.2.1
    let f2 := f.2.2.1;      let f3 := f.2.2.2.1;  let f4 := f.2.2.2.2
    let r1 := (e1&&f1) || (e2&&f3) || (e3&&f2) || (e4&&f4)
    let r2 := (e1&&f2) || (e2&&f1) || (e3&&f4) || (e4&&f3)
    let r3 := (e1&&f3) || (e2&&f4) || (e3&&f1) || (e4&&f2)
    let r4 := (e1&&f4) || (e2&&f2) || (e3&&f3) || (e4&&f1)
    dec5 (r1 || r3) (r2 || r3) r4 = dec5 a0 a1 a2 * dec5 b0 b1 b2 := by
  cases a0 <;> cases a1 <;> cases a2 <;> cases b0 <;> cases b1 <;> cases b2 <;> decide

/-- §3.2 L12 per-lane neg correctness on Bools. `neg` is the single-
operand selector permute `(e0,e1,e2,e3,e4) ↦ (e0,e4,e3,e2,e1)`
(`packed5.rs` neg / Funs.lean `…neg`), then `encode5`. 8-row `decide`. -/
theorem packed5_neg_lane (a0 a1 a2 : Bool) :
    let e := selBool a0 a1 a2
    let e1 := e.2.1
    let e2 := e.2.2.1;      let e3 := e.2.2.2.1;  let e4 := e.2.2.2.2
    -- result selectors r = [e0, e4, e3, e2, e1]; encode5 uses r1,r2,r3,r4
    -- so c0 = r1|r3 = e4|e2, c1 = r2|r3 = e3|e2, c2 = r4 = e1
    dec5 (e4 || e2) (e3 || e2) e1 = -(dec5 a0 a1 a2) := by
  cases a0 <;> cases a1 <;> cases a2 <;> decide

/-! ## §3.3 Per-op word / lane-lift lemmas

Each `*_word` lemma reduces the **exact Aeneas-extracted Result-pure
composed function** (`Insts.…{add,sub,mul,neg}` ∘ `decode5` ∘
`*_circuit` ∘ `encode5`, all `Array`-of-5 modelled) to `ok` of an
explicit `Packed5` whose three planes are the lane-lifted Bool
expressions of §3.2. The `Array.index_usize`/`Array.make`/`lift`/`bind`
plumbing is discharged by `simp` over the def unfoldings; the residual
`getLsbD` distribution over `&&&`/`|||`/`~~~` is closed by the default
`BitVec.getLsbD_*` simp set (the same bare-`simp` discipline V1's
`bipedal3_*_word` use, plus the BitVec complement `~~~` for `decode5`'s
3 NOTs). -/

attribute [local simp] packed.packed5.decode5 packed.packed5.encode5
  packed.packed5.add_circuit packed.packed5.sub_circuit
  packed.packed5.mul_circuit
  packed.packed5.Packed5.Insts.Gf2_algebraPackedPackedFieldFp5U64U128.add
  packed.packed5.Packed5.Insts.Gf2_algebraPackedPackedFieldFp5U64U128.sub
  packed.packed5.Packed5.Insts.Gf2_algebraPackedPackedFieldFp5U64U128.mul
  packed.packed5.Packed5.Insts.Gf2_algebraPackedPackedFieldFp5U64U128.neg
  packed.packed5.Packed5.add_inherent packed.packed5.Packed5.sub_inherent
  packed.packed5.Packed5.mul_inherent packed.packed5.Packed5.neg_inherent
  Array.index_usize Array.make

/-- §3.3 L13 add-word: the extracted `add_inherent ⟨b0,b1,b2⟩ ⟨c0,c1,c2⟩`
reduces to `ok` of the three composed plane expressions, lifted lane-wise
to their Bool form. Returns the three plane equalities matching the
`Packed5 { b0, b1, b2 }` result struct. -/
theorem packed5_add_word (b0 b1 b2 c0 c1 c2 : Std.U64) (i : Fin 64) :
    ∃ r, packed.packed5.Packed5.add_inherent ⟨b0,b1,b2⟩ ⟨c0,c1,c2⟩ = ok r ∧
      r.b0.bv.getLsbD i =
        (let e := selBool (b0.bv.getLsbD i) (b1.bv.getLsbD i) (b2.bv.getLsbD i)
         let f := selBool (c0.bv.getLsbD i) (c1.bv.getLsbD i) (c2.bv.getLsbD i)
         let e0 := e.1; let e1 := e.2.1
         let e2 := e.2.2.1; let e3 := e.2.2.2.1; let e4 := e.2.2.2.2
         let f0 := f.1; let f1 := f.2.1
         let f2 := f.2.2.1; let f3 := f.2.2.2.1; let f4 := f.2.2.2.2
         let r1 := (e0&&f1)||(e1&&f0)||(e2&&f4)||(e3&&f3)||(e4&&f2)
         let r3 := (e0&&f3)||(e1&&f2)||(e2&&f1)||(e3&&f0)||(e4&&f4)
         r1 || r3) ∧
      r.b1.bv.getLsbD i =
        (let e := selBool (b0.bv.getLsbD i) (b1.bv.getLsbD i) (b2.bv.getLsbD i)
         let f := selBool (c0.bv.getLsbD i) (c1.bv.getLsbD i) (c2.bv.getLsbD i)
         let e0 := e.1; let e1 := e.2.1
         let e2 := e.2.2.1; let e3 := e.2.2.2.1; let e4 := e.2.2.2.2
         let f0 := f.1; let f1 := f.2.1
         let f2 := f.2.2.1; let f3 := f.2.2.2.1; let f4 := f.2.2.2.2
         let r2 := (e0&&f2)||(e1&&f1)||(e2&&f0)||(e3&&f4)||(e4&&f3)
         let r3 := (e0&&f3)||(e1&&f2)||(e2&&f1)||(e3&&f0)||(e4&&f4)
         r2 || r3) ∧
      r.b2.bv.getLsbD i =
        (let e := selBool (b0.bv.getLsbD i) (b1.bv.getLsbD i) (b2.bv.getLsbD i)
         let f := selBool (c0.bv.getLsbD i) (c1.bv.getLsbD i) (c2.bv.getLsbD i)
         let e0 := e.1; let e1 := e.2.1
         let e2 := e.2.2.1; let e3 := e.2.2.2.1; let e4 := e.2.2.2.2
         let f0 := f.1; let f1 := f.2.1
         let f2 := f.2.2.1; let f3 := f.2.2.2.1; let f4 := f.2.2.2.2
         (e0&&f4)||(e1&&f3)||(e2&&f2)||(e3&&f1)||(e4&&f0)) := by
  refine ⟨_, rfl, ?_, ?_, ?_⟩ <;>
    simp [selBool, BitVec.getLsbD_and, BitVec.getLsbD_or, BitVec.getLsbD_not,
      Bool.and_assoc, Bool.or_assoc] <;>
    (try (cases b0.bv.getLsbD i <;> cases b1.bv.getLsbD i <;> cases b2.bv.getLsbD i <;>
          cases c0.bv.getLsbD i <;> cases c1.bv.getLsbD i <;> cases c2.bv.getLsbD i <;> decide))

/-- §3.3 L14 sub-word: analogous for `sub_inherent`. -/
theorem packed5_sub_word (b0 b1 b2 c0 c1 c2 : Std.U64) (i : Fin 64) :
    ∃ r, packed.packed5.Packed5.sub_inherent ⟨b0,b1,b2⟩ ⟨c0,c1,c2⟩ = ok r ∧
      r.b0.bv.getLsbD i =
        (let e := selBool (b0.bv.getLsbD i) (b1.bv.getLsbD i) (b2.bv.getLsbD i)
         let f := selBool (c0.bv.getLsbD i) (c1.bv.getLsbD i) (c2.bv.getLsbD i)
         let e0 := e.1; let e1 := e.2.1
         let e2 := e.2.2.1; let e3 := e.2.2.2.1; let e4 := e.2.2.2.2
         let f0 := f.1; let f1 := f.2.1
         let f2 := f.2.2.1; let f3 := f.2.2.2.1; let f4 := f.2.2.2.2
         let r1 := (e0&&f4)||(e1&&f0)||(e2&&f1)||(e3&&f2)||(e4&&f3)
         let r3 := (e0&&f2)||(e1&&f3)||(e2&&f4)||(e3&&f0)||(e4&&f1)
         r1 || r3) ∧
      r.b1.bv.getLsbD i =
        (let e := selBool (b0.bv.getLsbD i) (b1.bv.getLsbD i) (b2.bv.getLsbD i)
         let f := selBool (c0.bv.getLsbD i) (c1.bv.getLsbD i) (c2.bv.getLsbD i)
         let e0 := e.1; let e1 := e.2.1
         let e2 := e.2.2.1; let e3 := e.2.2.2.1; let e4 := e.2.2.2.2
         let f0 := f.1; let f1 := f.2.1
         let f2 := f.2.2.1; let f3 := f.2.2.2.1; let f4 := f.2.2.2.2
         let r2 := (e0&&f3)||(e1&&f4)||(e2&&f0)||(e3&&f1)||(e4&&f2)
         let r3 := (e0&&f2)||(e1&&f3)||(e2&&f4)||(e3&&f0)||(e4&&f1)
         r2 || r3) ∧
      r.b2.bv.getLsbD i =
        (let e := selBool (b0.bv.getLsbD i) (b1.bv.getLsbD i) (b2.bv.getLsbD i)
         let f := selBool (c0.bv.getLsbD i) (c1.bv.getLsbD i) (c2.bv.getLsbD i)
         let e0 := e.1; let e1 := e.2.1
         let e2 := e.2.2.1; let e3 := e.2.2.2.1; let e4 := e.2.2.2.2
         let f0 := f.1; let f1 := f.2.1
         let f2 := f.2.2.1; let f3 := f.2.2.2.1; let f4 := f.2.2.2.2
         (e0&&f1)||(e1&&f2)||(e2&&f3)||(e3&&f4)||(e4&&f0)) := by
  refine ⟨_, rfl, ?_, ?_, ?_⟩ <;>
    simp [selBool, BitVec.getLsbD_and, BitVec.getLsbD_or, BitVec.getLsbD_not,
      Bool.and_assoc, Bool.or_assoc] <;>
    (try (cases b0.bv.getLsbD i <;> cases b1.bv.getLsbD i <;> cases b2.bv.getLsbD i <;>
          cases c0.bv.getLsbD i <;> cases c1.bv.getLsbD i <;> cases c2.bv.getLsbD i <;> decide))

/-- §3.3 L15 mul-word: analogous for `mul_inherent`. -/
theorem packed5_mul_word (b0 b1 b2 c0 c1 c2 : Std.U64) (i : Fin 64) :
    ∃ r, packed.packed5.Packed5.mul_inherent ⟨b0,b1,b2⟩ ⟨c0,c1,c2⟩ = ok r ∧
      r.b0.bv.getLsbD i =
        (let e := selBool (b0.bv.getLsbD i) (b1.bv.getLsbD i) (b2.bv.getLsbD i)
         let f := selBool (c0.bv.getLsbD i) (c1.bv.getLsbD i) (c2.bv.getLsbD i)
         let e1 := e.2.1
         let e2 := e.2.2.1; let e3 := e.2.2.2.1; let e4 := e.2.2.2.2
         let f1 := f.2.1
         let f2 := f.2.2.1; let f3 := f.2.2.2.1; let f4 := f.2.2.2.2
         let r1 := (e1&&f1)||(e2&&f3)||(e3&&f2)||(e4&&f4)
         let r3 := (e1&&f3)||(e2&&f4)||(e3&&f1)||(e4&&f2)
         r1 || r3) ∧
      r.b1.bv.getLsbD i =
        (let e := selBool (b0.bv.getLsbD i) (b1.bv.getLsbD i) (b2.bv.getLsbD i)
         let f := selBool (c0.bv.getLsbD i) (c1.bv.getLsbD i) (c2.bv.getLsbD i)
         let e1 := e.2.1
         let e2 := e.2.2.1; let e3 := e.2.2.2.1; let e4 := e.2.2.2.2
         let f1 := f.2.1
         let f2 := f.2.2.1; let f3 := f.2.2.2.1; let f4 := f.2.2.2.2
         let r2 := (e1&&f2)||(e2&&f1)||(e3&&f4)||(e4&&f3)
         let r3 := (e1&&f3)||(e2&&f4)||(e3&&f1)||(e4&&f2)
         r2 || r3) ∧
      r.b2.bv.getLsbD i =
        (let e := selBool (b0.bv.getLsbD i) (b1.bv.getLsbD i) (b2.bv.getLsbD i)
         let f := selBool (c0.bv.getLsbD i) (c1.bv.getLsbD i) (c2.bv.getLsbD i)
         let e1 := e.2.1
         let e2 := e.2.2.1; let e3 := e.2.2.2.1; let e4 := e.2.2.2.2
         let f1 := f.2.1
         let f2 := f.2.2.1; let f3 := f.2.2.2.1; let f4 := f.2.2.2.2
         (e1&&f4)||(e2&&f2)||(e3&&f3)||(e4&&f1)) := by
  refine ⟨_, rfl, ?_, ?_, ?_⟩ <;>
    simp [selBool, BitVec.getLsbD_and, BitVec.getLsbD_or, BitVec.getLsbD_not,
      Bool.and_assoc, Bool.or_assoc] <;>
    (try (cases b0.bv.getLsbD i <;> cases b1.bv.getLsbD i <;> cases b2.bv.getLsbD i <;>
          cases c0.bv.getLsbD i <;> cases c1.bv.getLsbD i <;> cases c2.bv.getLsbD i <;> decide))

/-- §3.3 L16 neg-word: the extracted `neg_inherent ⟨b0,b1,b2⟩` reduces to
`ok` of the permuted-selector plane expressions, lifted lane-wise. -/
theorem packed5_neg_word (b0 b1 b2 : Std.U64) (i : Fin 64) :
    ∃ r, packed.packed5.Packed5.neg_inherent ⟨b0,b1,b2⟩ = ok r ∧
      r.b0.bv.getLsbD i =
        (let e := selBool (b0.bv.getLsbD i) (b1.bv.getLsbD i) (b2.bv.getLsbD i)
         let e2 := e.2.2.1; let e4 := e.2.2.2.2
         e4 || e2) ∧
      r.b1.bv.getLsbD i =
        (let e := selBool (b0.bv.getLsbD i) (b1.bv.getLsbD i) (b2.bv.getLsbD i)
         let e2 := e.2.2.1; let e3 := e.2.2.2.1
         e3 || e2) ∧
      r.b2.bv.getLsbD i =
        (let e := selBool (b0.bv.getLsbD i) (b1.bv.getLsbD i) (b2.bv.getLsbD i)
         e.2.1) := by
  refine ⟨_, rfl, ?_, ?_, ?_⟩ <;>
    simp [selBool, BitVec.getLsbD_and, BitVec.getLsbD_or, BitVec.getLsbD_not,
      Bool.and_assoc, Bool.or_assoc] <;>
    (try (cases b0.bv.getLsbD i <;> cases b1.bv.getLsbD i <;> cases b2.bv.getLsbD i <;> decide))

/-! ## §3.4 Per-op `*_correct` theorems (against the Aeneas-extracted fn)

The bridge between the Result-monad inherent wrapper and the per-lane
`dec5` spec: `obtain` the word lemma (lane-lift of the exact composed
circuit), `rw` the three plane equalities, then `exact` the §3.2 truth
table. No `progress`/Result-monad branching is needed — the `Packed5`
circuits are `Result`-pure (pure bitwise, no error path). -/

/-- §3.4 L17 add-correct: per-lane add correctness on the inherent
wrapper, against canonical `ZMod 5` addition. -/
theorem packed5_add_correct (b0 b1 b2 c0 c1 c2 : Std.U64) (i : Fin 64) :
    ∃ r, packed.packed5.Packed5.add_inherent ⟨b0,b1,b2⟩ ⟨c0,c1,c2⟩ = ok r ∧
      dec5 (r.b0.bv.getLsbD i) (r.b1.bv.getLsbD i) (r.b2.bv.getLsbD i)
        = dec5 (b0.bv.getLsbD i) (b1.bv.getLsbD i) (b2.bv.getLsbD i)
          + dec5 (c0.bv.getLsbD i) (c1.bv.getLsbD i) (c2.bv.getLsbD i) := by
  obtain ⟨r, hr, h0, h1, h2⟩ := packed5_add_word b0 b1 b2 c0 c1 c2 i
  refine ⟨r, hr, ?_⟩
  rw [h0, h1, h2]
  exact packed5_add_lane _ _ _ _ _ _

/-- §3.4 L18 sub-correct: analogous, against canonical `ZMod 5`
subtraction. -/
theorem packed5_sub_correct (b0 b1 b2 c0 c1 c2 : Std.U64) (i : Fin 64) :
    ∃ r, packed.packed5.Packed5.sub_inherent ⟨b0,b1,b2⟩ ⟨c0,c1,c2⟩ = ok r ∧
      dec5 (r.b0.bv.getLsbD i) (r.b1.bv.getLsbD i) (r.b2.bv.getLsbD i)
        = dec5 (b0.bv.getLsbD i) (b1.bv.getLsbD i) (b2.bv.getLsbD i)
          - dec5 (c0.bv.getLsbD i) (c1.bv.getLsbD i) (c2.bv.getLsbD i) := by
  obtain ⟨r, hr, h0, h1, h2⟩ := packed5_sub_word b0 b1 b2 c0 c1 c2 i
  refine ⟨r, hr, ?_⟩
  rw [h0, h1, h2]
  exact packed5_sub_lane _ _ _ _ _ _

/-- §3.4 L19 mul-correct: analogous, against canonical `ZMod 5`
multiplication. -/
theorem packed5_mul_correct (b0 b1 b2 c0 c1 c2 : Std.U64) (i : Fin 64) :
    ∃ r, packed.packed5.Packed5.mul_inherent ⟨b0,b1,b2⟩ ⟨c0,c1,c2⟩ = ok r ∧
      dec5 (r.b0.bv.getLsbD i) (r.b1.bv.getLsbD i) (r.b2.bv.getLsbD i)
        = dec5 (b0.bv.getLsbD i) (b1.bv.getLsbD i) (b2.bv.getLsbD i)
          * dec5 (c0.bv.getLsbD i) (c1.bv.getLsbD i) (c2.bv.getLsbD i) := by
  obtain ⟨r, hr, h0, h1, h2⟩ := packed5_mul_word b0 b1 b2 c0 c1 c2 i
  refine ⟨r, hr, ?_⟩
  rw [h0, h1, h2]
  exact packed5_mul_lane _ _ _ _ _ _

/-- §3.4 L20 neg-correct: per-lane neg correctness on the inherent
wrapper, against canonical `ZMod 5` negation. -/
theorem packed5_neg_correct (b0 b1 b2 : Std.U64) (i : Fin 64) :
    ∃ r, packed.packed5.Packed5.neg_inherent ⟨b0,b1,b2⟩ = ok r ∧
      dec5 (r.b0.bv.getLsbD i) (r.b1.bv.getLsbD i) (r.b2.bv.getLsbD i)
        = -(dec5 (b0.bv.getLsbD i) (b1.bv.getLsbD i) (b2.bv.getLsbD i)) := by
  obtain ⟨r, hr, h0, h1, h2⟩ := packed5_neg_word b0 b1 b2 i
  refine ⟨r, hr, ?_⟩
  rw [h0, h1, h2]
  exact packed5_neg_lane _ _ _

/-! ## §3.5 Lift helper + headline corollary -/

/-- §3.5 L21 lifting helper: any binary bitwise op on `BitVec 64` formed
from `&&&` / `|||` / `~~~` reduces lane-by-lane to its `Bool`-level form
(the 3-plane analogue of `getLsbD_bitwise_lift` in
`Bipedal3Correctness.lean`). -/
theorem getLsbD_bitwise_lift3
    (op : BitVec 64 → BitVec 64 → BitVec 64)
    (op_lane : Bool → Bool → Bool)
    (h_lift : ∀ (x y : BitVec 64) (i : Fin 64),
      (op x y).getLsbD i = op_lane (x.getLsbD i) (y.getLsbD i))
    (x y : BitVec 64) (i : Fin 64) :
    (op x y).getLsbD i = op_lane (x.getLsbD i) (y.getLsbD i) :=
  h_lift x y i

/-- Tag for the four D5 ops. Used purely as a case-split parameter for
the headline corollary (mirrors `Bipedal3Correctness.ArithOp`). -/
inductive ArithOp
  | add
  | sub
  | mul
  | neg
deriving DecidableEq

/-- Reference dispatch on `ZMod 5` per arithmetic tag. For `neg`, the
rhs operand is ignored. -/
def ZMod5.dispatch : ArithOp → ZMod 5 → ZMod 5 → ZMod 5
  | .add, a, b => a + b
  | .sub, a, b => a - b
  | .mul, a, b => a * b
  | .neg, a, _ => -a

/-- `Packed5` dispatch on the inherent methods. Same calling convention
as `ZMod5.dispatch`: the rhs is ignored on `neg`. -/
def Packed5.dispatch :
    ArithOp → packed.packed5.Packed5 → packed.packed5.Packed5 →
      Result packed.packed5.Packed5
  | .add, a, b => packed.packed5.Packed5.add_inherent a b
  | .sub, a, b => packed.packed5.Packed5.sub_inherent a b
  | .mul, a, b => packed.packed5.Packed5.mul_inherent a b
  | .neg, a, _ => packed.packed5.Packed5.neg_inherent a

/-- Lane decoder lifted to a `Packed5` value (the `ZMod 5`-valued lane
projection used by the headline corollary). -/
def dec5_lane (a : packed.packed5.Packed5) (i : Fin 64) : ZMod 5 :=
  dec5 (a.b0.bv.getLsbD i) (a.b1.bv.getLsbD i) (a.b2.bv.getLsbD i)

/-- §3.5 L23 headline corollary: `Packed5` arithmetic vs canonical F_5
(`ZMod 5`) arithmetic on every lane, for every `ArithOp` tag. The four
per-op theorems provide the case-split discharge (D5 §1 headline). -/
theorem packed5_correct_vs_canonical_F5
    (op : ArithOp) (a b : packed.packed5.Packed5) (i : Fin 64) :
    ∃ r, Packed5.dispatch op a b = ok r ∧
      dec5_lane r i = ZMod5.dispatch op (dec5_lane a i) (dec5_lane b i) := by
  cases op with
  | add => simpa [Packed5.dispatch, ZMod5.dispatch, dec5_lane] using
             packed5_add_correct a.b0 a.b1 a.b2 b.b0 b.b1 b.b2 i
  | sub => simpa [Packed5.dispatch, ZMod5.dispatch, dec5_lane] using
             packed5_sub_correct a.b0 a.b1 a.b2 b.b0 b.b1 b.b2 i
  | mul => simpa [Packed5.dispatch, ZMod5.dispatch, dec5_lane] using
             packed5_mul_correct a.b0 a.b1 a.b2 b.b0 b.b1 b.b2 i
  | neg => simpa [Packed5.dispatch, ZMod5.dispatch, dec5_lane] using
             packed5_neg_correct a.b0 a.b1 a.b2 i

/-! ## §3.5 L22 Bridge to canonical `Fp<5>`

The four `*_correct` theorems above are stated against `dec5` (pure
`ZMod 5`) for decidability, exactly as the V1 `bipedal3_*_correct`
theorems are stated against `psi` (pure `ZMod 3`) and do not project any
`Fp` instance. The connection to the production `Fp<5>` field is the
`fpEquiv` / `FpVal.instCommRing` ring-iso already verified at `P = 5`
(5 prime, `1 < 5`, `5 ≤ 2^63`) in `Gf2Core/Proofs/FpField.lean`. Per the
D5 sketch §3.5 / risk R5, the `Fp<5>` rephrasing is a cited, additive
note (the bridge equiv is not re-proved here — it is the same ring-iso
already closed in V0). It is *not* load-bearing for the issue's
`add/sub/mul/neg` correctness criteria, which are fully discharged by
L17–L20 / L23 above against `dec5`/`ZMod 5`. -/

end Packed5Correctness
