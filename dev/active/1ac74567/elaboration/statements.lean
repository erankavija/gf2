/-
  Statement-elaboration receipt for the `1ac74567` proof sketch.

  Every lemma the sketch states for `field.traits.FiniteFieldExt.square.default`
  and `field.traits.FiniteFieldExt.frobenius.default` appears here verbatim,
  with a `sorry` body. The bodies are out of scope for this issue: the receipt
  establishes that the *statements* type-check against the extracted
  definitions, nothing more.

  This file is an elaboration-harness input. It is not proof code, it lives
  outside `proofs/`, and it is not part of any lake target. It elaborates only
  against the harness tree that `elaborate.sh` stage 3 builds, which carries
  the `scripts/fix-aeneas-dupes.py` repair of the generated `Types.lean` and
  drops the `sorry`-poisoned `pow` chain.
-/
import Aeneas
import Mathlib.Algebra.CharP.Frobenius
import R8bGf2Core.Types
import R8bGf2Core.Funs

open Aeneas Aeneas.Std Result ControlFlow Error
open gf2_core

namespace ExtSketch

/-! ## Abbreviations for the extracted dictionary and its projections -/

abbrev FFExt (Self Char Wide : Type) := field.traits.FiniteFieldExt Self Char Wide

variable {Self Char Wide : Type}

abbrev dictClone (dict : FFExt Self Char Wide) :=
  dict.FiniteFieldInst.corecloneCloneInst.clone

abbrev dictMul (dict : FFExt Self Char Wide) :=
  dict.FiniteFieldInst.coreopsarithMulInst.mul

abbrev dictChar (dict : FFExt Self Char Wide) :=
  dict.FiniteFieldInst.characteristic

/-! ## Dictionary hypotheses -/

/-- The extracted `FiniteField` dictionary refines the abstract field `Self`:
    `Clone` is the identity and `Mul` is the field multiplication, both total. -/
structure LawfulDict [Field Self] (dict : FFExt Self Char Wide) : Prop where
  clone_ok : ∀ a : Self, dictClone dict a = ok a
  mul_ok : ∀ a b : Self, dictMul dict a b = ok (a * b)

/-- The dictionary's own `pow` method is the field power map. `frobenius.default`
    calls this field, never `pow.default`, so this hypothesis is what carries
    the `pow` obligation and it is discharged per backend. -/
structure LawfulPow [Field Self] (dict : FFExt Self Char Wide) : Prop where
  pow_ok : ∀ (a : Self) (e : Std.U64), dict.pow a e = ok (a ^ e.val)

/-- The dictionary reports characteristic `p`, and the `Into` dictionary the
    extraction demands converts it to the same `U64` for every element. -/
structure CharIs [Field Self] (dict : FFExt Self Char Wide)
    (intoU64 : core.convert.Into Char Std.U64) (p : Std.U64) : Prop where
  char_ok : ∀ a : Self, ∃ c, dictChar dict a = ok c ∧ intoU64.into c = ok p

/-! ## L1–L2 — trait-level laws for `square.default` -/

/-- L1. `square.default` is total and equals multiplication of the element by
    itself. -/
theorem square_default_eq_mul_self [Field Self]
    (dict : FFExt Self Char Wide) (hd : LawfulDict dict) (a : Self) :
    field.traits.FiniteFieldExt.square.default dict a = ok (a * a) := by
  sorry

/-- L2. `square.default` is multiplicative, stated conditionally on
    `Result`-monad success at all three arguments. -/
theorem square_default_mul_hom [Field Self]
    (dict : FFExt Self Char Wide) (hd : LawfulDict dict) (a b sa sb sab : Self)
    (ha : field.traits.FiniteFieldExt.square.default dict a = ok sa)
    (hb : field.traits.FiniteFieldExt.square.default dict b = ok sb)
    (hab : field.traits.FiniteFieldExt.square.default dict (a * b) = ok sab) :
    sab = sa * sb := by
  sorry

/-! ## L3–L4 — the extracted exponent loop -/

/-- L3. Below the `U64` ceiling the exponent loop is total and computes
    $p^k$. -/
theorem frobenius_default_loop_eq_pow
    (p : Std.U64) (k : Std.Usize) (hnof : p.val ^ k.val ≤ Std.U64.max) :
    ∃ e : Std.U64,
      field.traits.FiniteFieldExt.frobenius.default_loop
        { start := 0#usize, «end» := k } p 1#u64 = ok e ∧ e.val = p.val ^ k.val := by
  sorry

/-- L4. Above the ceiling the loop's `Option::expect` fires: the `checked_mul`
    at `A8b_lean/Funs.lean:114` returns `none` and the extraction fails rather
    than wrapping. This is the necessity direction of L3's hypothesis. -/
theorem frobenius_default_loop_overflow
    (p : Std.U64) (k : Std.Usize) (hov : Std.U64.max < p.val ^ k.val) :
    ∃ e, field.traits.FiniteFieldExt.frobenius.default_loop
      { start := 0#usize, «end» := k } p 1#u64 = fail e := by
  sorry

/-! ## L5–L8 — trait-level laws for `frobenius.default` -/

/-- L5. `frobenius.default` at $k$ is the $p^k$-power map. -/
theorem frobenius_default_eq_pow_char [Field Self]
    (p : Std.U64) (dict : FFExt Self Char Wide)
    (intoU64 : core.convert.Into Char Std.U64)
    (hp : LawfulPow dict) (hc : CharIs dict intoU64 p)
    (a : Self) (k : Std.Usize) (hnof : p.val ^ k.val ≤ Std.U64.max) :
    field.traits.FiniteFieldExt.frobenius.default dict intoU64 a k
      = ok (a ^ (p.val ^ k.val)) := by
  sorry

/-- L6. `frobenius.default` at $k$ is Mathlib's $k$-fold Frobenius, given that
    `Self` has exponential characteristic `p`. -/
theorem frobenius_default_eq_iterateFrobenius [Field Self]
    (p : Std.U64) [ExpChar Self p.val] (dict : FFExt Self Char Wide)
    (intoU64 : core.convert.Into Char Std.U64)
    (hp : LawfulPow dict) (hc : CharIs dict intoU64 p)
    (a : Self) (k : Std.Usize) (hnof : p.val ^ k.val ≤ Std.U64.max) :
    field.traits.FiniteFieldExt.frobenius.default dict intoU64 a k
      = ok (iterateFrobenius Self p.val k.val a) := by
  sorry

/-- L7. `frobenius.default` is additive. No overflow hypothesis is needed: the
    three calls share one $k$, so success at all three is what the law assumes. -/
theorem frobenius_default_add [Field Self]
    (p : Std.U64) [ExpChar Self p.val] (dict : FFExt Self Char Wide)
    (intoU64 : core.convert.Into Char Std.U64)
    (hp : LawfulPow dict) (hc : CharIs dict intoU64 p)
    (a b ra rb rab : Self) (k : Std.Usize)
    (ha : field.traits.FiniteFieldExt.frobenius.default dict intoU64 a k = ok ra)
    (hb : field.traits.FiniteFieldExt.frobenius.default dict intoU64 b k = ok rb)
    (hab : field.traits.FiniteFieldExt.frobenius.default dict intoU64 (a + b) k = ok rab) :
    rab = ra + rb := by
  sorry

/-- L8a. $k = 0$ is the identity. -/
theorem frobenius_default_zero [Field Self]
    (p : Std.U64) (dict : FFExt Self Char Wide)
    (intoU64 : core.convert.Into Char Std.U64)
    (hp : LawfulPow dict) (hc : CharIs dict intoU64 p) (a : Self) :
    field.traits.FiniteFieldExt.frobenius.default dict intoU64 a 0#usize = ok a := by
  sorry

/-- L8b. Composition: $\phi^{j} \circ \phi^{k} = \phi^{j+k}$, with the
    `Usize` addition discharged outside the statement so no `Usize` overflow
    hypothesis leaks into the law. -/
theorem frobenius_default_comp [Field Self]
    (p : Std.U64) [ExpChar Self p.val] (dict : FFExt Self Char Wide)
    (intoU64 : core.convert.Into Char Std.U64)
    (hp : LawfulPow dict) (hc : CharIs dict intoU64 p)
    (a x : Self) (j k jk : Std.Usize) (hjk : jk.val = j.val + k.val)
    (hnof : p.val ^ jk.val ≤ Std.U64.max)
    (hx : field.traits.FiniteFieldExt.frobenius.default dict intoU64 a k = ok x) :
    field.traits.FiniteFieldExt.frobenius.default dict intoU64 x j
      = field.traits.FiniteFieldExt.frobenius.default dict intoU64 a jk := by
  sorry

end ExtSketch
