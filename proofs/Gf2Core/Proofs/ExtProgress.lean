/-
  Gf2Core.Proofs.ExtProgress — @[step] lemmas linking Aeneas monadic code
  to pure algebraic specifications for QuadraticExt and CubicExt.
-/
import Aeneas
import Gf2Core.Types
import Gf2Core.FunsExternal
import Gf2Core.Funs
import Gf2Core.Proofs.ExtDefs
import Gf2Core.Proofs.ExtAlgebra

open Aeneas Aeneas.Std Result ControlFlow Error Aeneas.Std.WP
open gf2_core

set_option maxHeartbeats 4000000
set_option linter.unusedVariables false

/-! ## QuadraticExt step lemmas -/

namespace QExtProgress

variable {C BF Char Wide : Type} [Field BF]
variable {inst : gfpn.ext_config.ExtConfig C BF Char Wide}

omit [Field BF] in
/-- QuadraticExt.new always succeeds -/
@[step]
theorem qnew_progress (c0 c1 : BF) :
    gfpn.quadratic.QuadraticExt.new inst c0 c1 ⦃ fun r =>
      r.c0 = c0 ∧ r.c1 = c1 ⦄ := by
  simp [gfpn.quadratic.QuadraticExt.new, spec_ok]

/-- QuadraticExt.add: component-wise addition -/
@[step]
theorem qadd_progress (hv : ValidExtConfig inst)
    (a b : gfpn.quadratic.QuadraticExt C BF Char Wide) :
    ExtAbbrev.QAdd inst a b ⦃ fun r =>
      r.c0 = a.c0 + b.c0 ∧ r.c1 = a.c1 + b.c1 ⦄ := by
  simp only [ExtAbbrev.QAdd,
    gfpn.quadratic.QuadraticExt.Insts.CoreOpsArithAddQuadraticExtQuadraticExt.add,
    hv.add_ok, bind_ok, gfpn.quadratic.QuadraticExt.new]
  simp [spec_ok]

/-- QuadraticExt.sub: component-wise subtraction -/
@[step]
theorem qsub_progress (hv : ValidExtConfig inst)
    (a b : gfpn.quadratic.QuadraticExt C BF Char Wide) :
    ExtAbbrev.QSub inst a b ⦃ fun r =>
      r.c0 = a.c0 - b.c0 ∧ r.c1 = a.c1 - b.c1 ⦄ := by
  simp only [ExtAbbrev.QSub,
    gfpn.quadratic.QuadraticExt.Insts.CoreOpsArithSubQuadraticExtQuadraticExt.sub,
    hv.sub_ok, bind_ok, gfpn.quadratic.QuadraticExt.new]
  simp [spec_ok]

/-- QuadraticExt.neg: component-wise negation -/
@[step]
theorem qneg_progress (hv : ValidExtConfig inst)
    (a : gfpn.quadratic.QuadraticExt C BF Char Wide) :
    ExtAbbrev.QNeg inst a ⦃ fun r =>
      r.c0 = -a.c0 ∧ r.c1 = -a.c1 ⦄ := by
  simp only [ExtAbbrev.QNeg,
    gfpn.quadratic.QuadraticExt.Insts.CoreOpsArithNegQuadraticExt.neg,
    hv.neg_ok, bind_ok, gfpn.quadratic.QuadraticExt.new]
  simp [spec_ok]

/-- QuadraticExt.mul: Karatsuba multiplication equals schoolbook -/
@[step]
theorem qmul_progress (hv : ValidExtConfig inst)
    (a b : gfpn.quadratic.QuadraticExt C BF Char Wide) :
    ExtAbbrev.QMul inst a b ⦃ fun r =>
      r.c0 = a.c0 * b.c0 + hv.getNonResidue * (a.c1 * b.c1) ∧
      r.c1 = a.c0 * b.c1 + a.c1 * b.c0 ⦄ := by
  simp only [ExtAbbrev.QMul,
    gfpn.quadratic.QuadraticExt.Insts.CoreOpsArithMulQuadraticExtQuadraticExt.mul,
    hv.mul_ok, hv.add_ok, hv.sub_ok, hv.mul_nr_eq, bind_ok,
    gfpn.quadratic.QuadraticExt.new, spec_ok]
  exact ⟨trivial, by ring⟩

/-- QuadraticExt.norm: computes c0² - β·c1² -/
@[step]
theorem qnorm_progress (hv : ValidExtConfig inst)
    (a : gfpn.quadratic.QuadraticExt C BF Char Wide) :
    ExtAbbrev.QNorm inst a ⦃ fun r =>
      r = a.c0 ^ 2 - hv.getNonResidue * a.c1 ^ 2 ⦄ := by
  simp only [ExtAbbrev.QNorm, gfpn.quadratic.QuadraticExt.norm,
    hv.mul_ok, hv.sub_ok, hv.mul_nr_eq, bind_ok]
  simp only [spec_ok, sq]

/-- QuadraticExt.conjugate: (c0, -c1) -/
@[step]
theorem qconj_progress (hv : ValidExtConfig inst)
    (a : gfpn.quadratic.QuadraticExt C BF Char Wide) :
    ExtAbbrev.QConj inst a ⦃ fun r =>
      r.c0 = a.c0 ∧ r.c1 = -a.c1 ⦄ := by
  simp only [ExtAbbrev.QConj, gfpn.quadratic.QuadraticExt.conjugate,
    hv.neg_ok, bind_ok, gfpn.quadratic.QuadraticExt.new]
  simp [spec_ok]

/-- The closure for quadratic inv: given norm_inv, produces conjugate/norm -/
@[step]
theorem qinv_closure_progress (hv : ValidExtConfig inst)
    (self : gfpn.quadratic.QuadraticExt C BF Char Wide) (norm_inv : BF) :
    gfpn.quadratic.FiniteFieldQuadraticExtClause0_Clause0_Clause0_CharacteristicQuadraticExtWide.inv.closure.Insts.CoreOpsFunctionFnOnceTupleClause0_BaseFieldQuadraticExt.call_once
      inst self norm_inv ⦃ fun r =>
        r.c0 = self.c0 * norm_inv ∧ r.c1 = -(self.c1 * norm_inv) ⦄ := by
  simp only [
    gfpn.quadratic.FiniteFieldQuadraticExtClause0_Clause0_Clause0_CharacteristicQuadraticExtWide.inv.closure.Insts.CoreOpsFunctionFnOnceTupleClause0_BaseFieldQuadraticExt.call_once,
    hv.mul_ok, hv.neg_ok, bind_ok,
    gfpn.quadratic.QuadraticExt.new]
  simp [spec_ok]
/-- QuadraticExt.inv: computes norm, inverts, maps closure -/
@[step]
theorem qinv_progress (hv : ValidExtConfig inst)
    (self : gfpn.quadratic.QuadraticExt C BF Char Wide) :
    ExtAbbrev.QInv inst self ⦃ fun o =>
      (self.c0 = 0 ∧ self.c1 = 0 → o = none) ∧
      (¬(self.c0 = 0 ∧ self.c1 = 0) → ∃ r, o = some r ∧
        r.c0 = self.c0 * (self.c0 ^ 2 - hv.getNonResidue * self.c1 ^ 2)⁻¹ ∧
        r.c1 = -(self.c1 * (self.c0 ^ 2 - hv.getNonResidue * self.c1 ^ 2)⁻¹)) ⦄ := by
  simp only [ExtAbbrev.QInv,
    gfpn.quadratic.QuadraticExt.Insts.Gf2_coreFieldTraitsFiniteFieldClause0_Clause0_Clause0_CharacteristicQuadraticExtWide.inv,
    hv.mul_ok, hv.sub_ok, hv.mul_nr_eq, bind_ok]
  set β := hv.getNonResidue with hβ
  set norm_val := self.c0 ^ 2 - β * self.c1 ^ 2 with hnorm_def
  -- The monadic norm computation reduces to norm_val
  simp only [show self.c0 * self.c0 - β * (self.c1 * self.c1) = norm_val from by
    rw [hnorm_def]; ring]
  -- Get the inv of norm using hv.inv_ok
  obtain ⟨o, ho_eq, ho_ne, ho_zero⟩ := hv.inv_ok norm_val
  simp only [ho_eq, bind_ok]
  -- Case analysis on norm_val = 0
  by_cases hn0 : norm_val = 0
  · -- norm = 0
    have ho_none := ho_zero hn0
    simp only [ho_none, core.option.Option.map, bind_ok, spec_ok]
    constructor
    · -- if self = 0 then norm = 0 (trivially from above); return none OK
      intro _; trivial
    · -- if self ≠ 0, we still get none: this follows from nr_irred
      intro hne
      -- nr_irred says if norm = 0 and self ≠ 0, contradiction
      exfalso; apply hne
      have : self.c0 = 0 ∧ self.c1 = 0 :=
        hv.nr_irred β hv.mul_nr_ok.choose_spec self.c0 self.c1 (by
          rw [← hn0, hnorm_def])
      exact this
  · -- norm ≠ 0
    obtain ⟨r_inv, hr_inv_eq, hr_inv_mul⟩ := ho_ne hn0
    -- r_inv * norm_val = 1, so r_inv = norm_val⁻¹
    have hr_inv_val : r_inv = norm_val⁻¹ :=
      mul_right_cancel₀ hn0 (by rw [hr_inv_mul, inv_mul_cancel₀ hn0])
    simp only [hr_inv_eq, core.option.Option.map, bind_ok]
    -- Progress through closure call_once
    step as ⟨closure_r, hclosure_c0, hclosure_c1⟩
    -- Aeneas 5220259c normalises the postcondition's `c0 = 0 ∧ c1 = 0 → _`
    -- antecedent to its curried form `c0 = 0 → c1 = 0 → _`.
    exact ⟨fun h0 h1 => (hn0 (by rw [hnorm_def, h0, h1]; simp [sq])).elim,
      fun _ => ⟨closure_r, rfl,
        by rw [hclosure_c0, hr_inv_val],
        by rw [hclosure_c1, hr_inv_val]⟩⟩
/-- QuadraticExt.order = base_order² (given base order succeeds and no U128 overflow) -/
theorem qorder_progress (bo : Std.U128)
    (h_ord : inst.fieldtraitsConstFieldInst.order = ok bo)
    (h_max : bo.val * bo.val ≤ Std.U128.max) :
    ExtAbbrev.QOrder inst ⦃ fun r => r.val = bo.val * bo.val ⦄ := by
  simp only [ExtAbbrev.QOrder,
    gfpn.quadratic.QuadraticExt.Insts.Gf2_coreFieldTraitsConstFieldClause0_Clause0_Clause0_CharacteristicQuadraticExtWide.order,
    h_ord, bind_ok]
  exact Std.U128.mul_spec h_max

end QExtProgress

/-! ## CubicExt step lemmas -/

namespace CExtProgress

variable {C BF Char Wide : Type} [Field BF]
variable {inst : gfpn.ext_config.ExtConfig C BF Char Wide}

omit [Field BF] in
/-- CubicExt.new always succeeds -/
@[step]
theorem cnew_progress (c0 c1 c2 : BF) :
    gfpn.cubic.CubicExt.new inst c0 c1 c2 ⦃ fun r =>
      r.c0 = c0 ∧ r.c1 = c1 ∧ r.c2 = c2 ⦄ := by
  simp [gfpn.cubic.CubicExt.new, spec_ok]

/-- CubicExt.add: component-wise -/
@[step]
theorem cadd_progress (hv : ValidExtConfig inst)
    (a b : gfpn.cubic.CubicExt C BF Char Wide) :
    ExtAbbrev.CAdd inst a b ⦃ fun r =>
      r.c0 = a.c0 + b.c0 ∧ r.c1 = a.c1 + b.c1 ∧ r.c2 = a.c2 + b.c2 ⦄ := by
  simp only [ExtAbbrev.CAdd,
    gfpn.cubic.CubicExt.Insts.CoreOpsArithAddCubicExtCubicExt.add,
    hv.add_ok, bind_ok, gfpn.cubic.CubicExt.new]
  simp [spec_ok]

/-- CubicExt.sub: component-wise -/
@[step]
theorem csub_progress (hv : ValidExtConfig inst)
    (a b : gfpn.cubic.CubicExt C BF Char Wide) :
    ExtAbbrev.CSub inst a b ⦃ fun r =>
      r.c0 = a.c0 - b.c0 ∧ r.c1 = a.c1 - b.c1 ∧ r.c2 = a.c2 - b.c2 ⦄ := by
  simp only [ExtAbbrev.CSub,
    gfpn.cubic.CubicExt.Insts.CoreOpsArithSubCubicExtCubicExt.sub,
    hv.sub_ok, bind_ok, gfpn.cubic.CubicExt.new]
  simp [spec_ok]

/-- CubicExt.neg: component-wise -/
@[step]
theorem cneg_progress (hv : ValidExtConfig inst)
    (a : gfpn.cubic.CubicExt C BF Char Wide) :
    ExtAbbrev.CNeg inst a ⦃ fun r =>
      r.c0 = -a.c0 ∧ r.c1 = -a.c1 ∧ r.c2 = -a.c2 ⦄ := by
  simp only [ExtAbbrev.CNeg,
    gfpn.cubic.CubicExt.Insts.CoreOpsArithNegCubicExt.neg,
    hv.neg_ok, bind_ok, gfpn.cubic.CubicExt.new]
  simp [spec_ok]

/-- CubicExt.mul: Karatsuba 6-mul trick equals schoolbook -/
@[step]
theorem cmul_progress (hv : ValidExtConfig inst)
    (a b : gfpn.cubic.CubicExt C BF Char Wide) :
    let β := hv.getNonResidue
    ExtAbbrev.CMul inst a b ⦃ fun r =>
      r.c0 = a.c0 * b.c0 + β * (a.c1 * b.c2 + a.c2 * b.c1) ∧
      r.c1 = a.c0 * b.c1 + a.c1 * b.c0 + β * (a.c2 * b.c2) ∧
      r.c2 = a.c0 * b.c2 + a.c1 * b.c1 + a.c2 * b.c0 ⦄ := by
  simp only [ExtAbbrev.CMul,
    gfpn.cubic.CubicExt.Insts.CoreOpsArithMulCubicExtCubicExt.mul,
    hv.mul_ok, hv.add_ok, hv.sub_ok, hv.mul_nr_eq, bind_ok,
    gfpn.cubic.CubicExt.new, spec_ok]
  refine ⟨?_, ?_, ?_⟩ <;> ring

/-- CubicExt.norm: the full cubic norm computation -/
@[step]
theorem cnorm_progress (hv : ValidExtConfig inst)
    (a : gfpn.cubic.CubicExt C BF Char Wide) :
    let β := hv.getNonResidue
    ExtAbbrev.CNorm inst a ⦃ fun r =>
      r = a.c0 * (a.c0 ^ 2 - β * (a.c1 * a.c2)) +
          β * (a.c2 * (β * a.c2 ^ 2 - a.c0 * a.c1) +
               a.c1 * (a.c1 ^ 2 - a.c0 * a.c2)) ⦄ := by
  simp only [ExtAbbrev.CNorm, gfpn.cubic.CubicExt.norm,
    hv.mul_ok, hv.add_ok, hv.sub_ok, hv.mul_nr_eq, bind_ok]
  simp only [spec_ok, sq]

/-- Helper: closure for cubic inv — given (s0,s1,s2) and norm_inv, produces
    (s0·norm_inv, s1·norm_inv, s2·norm_inv) as a CubicExt. -/
@[step]
theorem cinv_closure_progress (hv : ValidExtConfig inst)
    (s0 s1 s2 : BF) (norm_inv : BF) :
    gfpn.cubic.FiniteFieldCubicExtClause0_Clause0_Clause0_CharacteristicCubicExtWide.inv.closure.Insts.CoreOpsFunctionFnOnceTupleClause0_BaseFieldCubicExt.call_once
      inst (s0, s1, s2) norm_inv ⦃ fun r =>
        r.c0 = s0 * norm_inv ∧ r.c1 = s1 * norm_inv ∧ r.c2 = s2 * norm_inv ⦄ := by
  simp only [
    gfpn.cubic.FiniteFieldCubicExtClause0_Clause0_Clause0_CharacteristicCubicExtWide.inv.closure.Insts.CoreOpsFunctionFnOnceTupleClause0_BaseFieldCubicExt.call_once,
    hv.mul_ok, bind_ok, gfpn.cubic.CubicExt.new]
  simp [spec_ok]

/-- CubicExt.inv: computes cofactors, norm, base inv, maps closure -/
@[step]
theorem cinv_progress (hv : ValidExtConfig inst)
    (self : gfpn.cubic.CubicExt C BF Char Wide) :
    let β := hv.getNonResidue
    let s0 := self.c0 ^ 2 - β * (self.c1 * self.c2)
    let s1 := β * self.c2 ^ 2 - self.c0 * self.c1
    let s2 := self.c1 ^ 2 - self.c0 * self.c2
    let norm_val := self.c0 * s0 + β * (self.c2 * s1 + self.c1 * s2)
    ExtAbbrev.CInv inst self ⦃ fun o =>
      (norm_val = 0 → o = none) ∧
      (norm_val ≠ 0 → ∃ r, o = some r ∧
        r.c0 = s0 * norm_val⁻¹ ∧
        r.c1 = s1 * norm_val⁻¹ ∧
        r.c2 = s2 * norm_val⁻¹) ⦄ := by
  simp only [ExtAbbrev.CInv,
    gfpn.cubic.CubicExt.Insts.Gf2_coreFieldTraitsFiniteFieldClause0_Clause0_Clause0_CharacteristicCubicExtWide.inv,
    hv.mul_ok, hv.sub_ok, hv.mul_nr_eq, hv.add_ok, bind_ok]
  set β := hv.getNonResidue with hβ
  set s0 := self.c0 ^ 2 - β * (self.c1 * self.c2) with hs0_def
  set s1 := β * self.c2 ^ 2 - self.c0 * self.c1 with hs1_def
  set s2 := self.c1 ^ 2 - self.c0 * self.c2 with hs2_def
  set norm_val := self.c0 * s0 + β * (self.c2 * s1 + self.c1 * s2) with hnorm_def
  -- Collapse intermediate arithmetic to s0, s1, s2, norm_val
  simp only [show self.c0 * self.c0 - β * (self.c1 * self.c2) = s0 from by
    rw [hs0_def]; ring]
  simp only [show β * (self.c2 * self.c2) - self.c0 * self.c1 = s1 from by
    rw [hs1_def]; ring]
  simp only [show self.c1 * self.c1 - self.c0 * self.c2 = s2 from by
    rw [hs2_def]; ring]
  simp only [show self.c0 * s0 + β * (self.c2 * s1 + self.c1 * s2) = norm_val from by
    rw [hnorm_def]]
  -- Get the base inv of norm_val
  obtain ⟨o, ho_eq, ho_ne, ho_zero⟩ := hv.inv_ok norm_val
  simp only [ho_eq, bind_ok]
  -- Case split on norm_val = 0
  by_cases hn0 : norm_val = 0
  · -- norm = 0: inv returns None
    have ho_none := ho_zero hn0
    simp only [ho_none, core.option.Option.map, spec_ok]
    exact ⟨fun _ => True.intro, fun hne => (hne hn0).elim⟩
  · -- norm ≠ 0: inv returns Some r_inv, r_inv = norm_val⁻¹
    obtain ⟨r_inv, hr_inv_eq, hr_inv_mul⟩ := ho_ne hn0
    have hr_inv_val : r_inv = norm_val⁻¹ :=
      mul_right_cancel₀ hn0 (by rw [hr_inv_mul, inv_mul_cancel₀ hn0])
    simp only [hr_inv_eq, core.option.Option.map]
    -- Progress through the cubic closure call (cinv_closure_progress is @[step])
    step as ⟨closure_r, hcr0, hcr1, hcr2⟩
    exact ⟨fun h => (hn0 h).elim,
      fun _ => ⟨closure_r, rfl,
        by rw [hcr0, hr_inv_val],
        by rw [hcr1, hr_inv_val],
        by rw [hcr2, hr_inv_val]⟩⟩
/-- CubicExt.order = base_order³ (given base order succeeds and no U128 overflow) -/
theorem corder_progress (bo : Std.U128)
    (h_ord : inst.fieldtraitsConstFieldInst.order = ok bo)
    (h_sq : bo.val * bo.val ≤ Std.U128.max)
    (h_cube : bo.val * bo.val * bo.val ≤ Std.U128.max) :
    ExtAbbrev.COrder inst ⦃ fun r => r.val = bo.val * bo.val * bo.val ⦄ := by
  simp only [ExtAbbrev.COrder,
    gfpn.cubic.CubicExt.Insts.Gf2_coreFieldTraitsConstFieldClause0_Clause0_Clause0_CharacteristicCubicExtWide.order,
    h_ord, bind_ok]
  -- Two multiplications: first bo*bo, then result*bo
  step as ⟨sq, hsq⟩
  have h2 : sq.val * bo.val ≤ Std.U128.max := by rw [hsq]; exact h_cube
  step as ⟨r, hr⟩
  simp only [hr, hsq]

end CExtProgress
