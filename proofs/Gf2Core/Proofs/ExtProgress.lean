/-
  Gf2Core.Proofs.ExtProgress — @[progress] lemmas linking Aeneas monadic code
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

/-! ## QuadraticExt progress lemmas -/

namespace QExtProgress

variable {C BF Char Wide : Type} [Field BF]
variable {inst : gfpn.ext_config.ExtConfig C BF Char Wide}

omit [Field BF] in
/-- QuadraticExt.new always succeeds -/
@[progress]
theorem qnew_progress (c0 c1 : BF) :
    gfpn.quadratic.QuadraticExt.new inst c0 c1 ⦃ fun r =>
      r.c0 = c0 ∧ r.c1 = c1 ⦄ := by
  simp [gfpn.quadratic.QuadraticExt.new, spec, theta, wp_return]

/-- QuadraticExt.add: component-wise addition -/
@[progress]
theorem qadd_progress (hv : ValidExtConfig inst)
    (a b : gfpn.quadratic.QuadraticExt inst) :
    ExtAbbrev.QAdd inst a b ⦃ fun r =>
      r.c0 = a.c0 + b.c0 ∧ r.c1 = a.c1 + b.c1 ⦄ := by
  simp only [ExtAbbrev.QAdd,
    gfpn.quadratic.QuadraticExt.Insts.CoreOpsArithAddQuadraticExtQuadraticExt.add,
    hv.add_ok, bind_tc_ok, gfpn.quadratic.QuadraticExt.new]
  simp [spec, theta, wp_return]

/-- QuadraticExt.sub: component-wise subtraction -/
@[progress]
theorem qsub_progress (hv : ValidExtConfig inst)
    (a b : gfpn.quadratic.QuadraticExt inst) :
    ExtAbbrev.QSub inst a b ⦃ fun r =>
      r.c0 = a.c0 - b.c0 ∧ r.c1 = a.c1 - b.c1 ⦄ := by
  simp only [ExtAbbrev.QSub,
    gfpn.quadratic.QuadraticExt.Insts.CoreOpsArithSubQuadraticExtQuadraticExt.sub,
    hv.sub_ok, bind_tc_ok, gfpn.quadratic.QuadraticExt.new]
  simp [spec, theta, wp_return]

/-- QuadraticExt.neg: component-wise negation -/
@[progress]
theorem qneg_progress (hv : ValidExtConfig inst)
    (a : gfpn.quadratic.QuadraticExt inst) :
    ExtAbbrev.QNeg inst a ⦃ fun r =>
      r.c0 = -a.c0 ∧ r.c1 = -a.c1 ⦄ := by
  simp only [ExtAbbrev.QNeg,
    gfpn.quadratic.QuadraticExt.Insts.CoreOpsArithNegQuadraticExt.neg,
    hv.neg_ok, bind_tc_ok, gfpn.quadratic.QuadraticExt.new]
  simp [spec, theta, wp_return]

/-- QuadraticExt.mul: Karatsuba multiplication equals schoolbook -/
@[progress]
theorem qmul_progress (hv : ValidExtConfig inst)
    (a b : gfpn.quadratic.QuadraticExt inst) :
    ExtAbbrev.QMul inst a b ⦃ fun r =>
      r.c0 = a.c0 * b.c0 + hv.getNonResidue * (a.c1 * b.c1) ∧
      r.c1 = a.c0 * b.c1 + a.c1 * b.c0 ⦄ := by
  simp only [ExtAbbrev.QMul,
    gfpn.quadratic.QuadraticExt.Insts.CoreOpsArithMulQuadraticExtQuadraticExt.mul,
    hv.mul_ok, hv.add_ok, hv.sub_ok, hv.mul_nr_eq, bind_tc_ok,
    gfpn.quadratic.QuadraticExt.new, spec, theta, wp_return]
  exact ⟨trivial, by ring⟩

/-- QuadraticExt.norm: computes c0² - β·c1² -/
@[progress]
theorem qnorm_progress (hv : ValidExtConfig inst)
    (a : gfpn.quadratic.QuadraticExt inst) :
    ExtAbbrev.QNorm inst a ⦃ fun r =>
      r = a.c0 ^ 2 - hv.getNonResidue * a.c1 ^ 2 ⦄ := by
  simp only [ExtAbbrev.QNorm, gfpn.quadratic.QuadraticExt.norm,
    hv.mul_ok, hv.sub_ok, hv.mul_nr_eq, bind_tc_ok]
  simp only [spec, theta, wp_return, sq]

/-- QuadraticExt.conjugate: (c0, -c1) -/
@[progress]
theorem qconj_progress (hv : ValidExtConfig inst)
    (a : gfpn.quadratic.QuadraticExt inst) :
    ExtAbbrev.QConj inst a ⦃ fun r =>
      r.c0 = a.c0 ∧ r.c1 = -a.c1 ⦄ := by
  simp only [ExtAbbrev.QConj, gfpn.quadratic.QuadraticExt.conjugate,
    hv.neg_ok, bind_tc_ok, gfpn.quadratic.QuadraticExt.new]
  simp [spec, theta, wp_return]

/-- The closure for quadratic inv: given norm_inv, produces conjugate/norm -/
theorem qinv_closure_progress (hv : ValidExtConfig inst)
    (self : gfpn.quadratic.QuadraticExt inst) (norm_inv : BF) :
    gfpn.quadratic.FiniteFieldQuadraticExtClause0_Clause0_Clause0_CharacteristicQuadraticExt.inv.closure.Insts.CoreOpsFunctionFnOnceTupleClause0_BaseFieldQuadraticExt.call_once
      inst self norm_inv ⦃ fun r =>
        r.c0 = self.c0 * norm_inv ∧ r.c1 = -(self.c1 * norm_inv) ⦄ := by
  simp only [
    gfpn.quadratic.FiniteFieldQuadraticExtClause0_Clause0_Clause0_CharacteristicQuadraticExt.inv.closure.Insts.CoreOpsFunctionFnOnceTupleClause0_BaseFieldQuadraticExt.call_once,
    hv.mul_ok, hv.neg_ok, bind_tc_ok, gfpn.quadratic.QuadraticExt.new]
  simp [spec, theta, wp_return]

/-- QuadraticExt.inv: computes norm, inverts, maps closure -/
@[progress]
theorem qinv_progress (hv : ValidExtConfig inst)
    (self : gfpn.quadratic.QuadraticExt inst) :
    ExtAbbrev.QInv inst self ⦃ fun o =>
      (self.c0 = 0 ∧ self.c1 = 0 → o = none) ∧
      (¬(self.c0 = 0 ∧ self.c1 = 0) → ∃ r, o = some r ∧
        r.c0 = self.c0 * (self.c0 ^ 2 - hv.getNonResidue * self.c1 ^ 2)⁻¹ ∧
        r.c1 = -(self.c1 * (self.c0 ^ 2 - hv.getNonResidue * self.c1 ^ 2)⁻¹)) ⦄ := by
  simp only [ExtAbbrev.QInv,
    gfpn.quadratic.QuadraticExt.Insts.Gf2_coreFieldTraitsFiniteFieldClause0_Clause0_Clause0_CharacteristicQuadraticExt.inv,
    hv.mul_ok, hv.sub_ok, hv.mul_nr_eq, bind_tc_ok]
  set norm_val := self.c0 * self.c0 - hv.getNonResidue * (self.c1 * self.c1) with hnorm_def
  obtain ⟨o, ho_eq, ho_nz, ho_z⟩ := hv.inv_ok norm_val
  simp only [ho_eq, bind_tc_ok]
  have hnorm_sq : norm_val = self.c0 ^ 2 - hv.getNonResidue * self.c1 ^ 2 := by
    rw [hnorm_def]; ring
  -- Case split on o
  rcases o with _ | r
  · -- o = none: norm_val = 0, so self = 0 by irreducibility
    simp only [core.option.Option.map, spec, theta, wp_return]
    refine ⟨fun _ => trivial, fun hne => ?_⟩
    exfalso; apply hne
    have hn : norm_val = 0 := by
      by_contra h; obtain ⟨r, hr, _⟩ := ho_nz h; exact absurd hr (by simp)
    exact hv.nr_irred hv.getNonResidue hv.mul_nr_eq self.c0 self.c1 (by rw [sq, sq]; exact hn)
  · -- o = some r: norm_val ≠ 0, r = norm_val⁻¹
    simp only [core.option.Option.map,
      gfpn.quadratic.FiniteFieldQuadraticExtClause0_Clause0_Clause0_CharacteristicQuadraticExt.inv.closure.Insts.CoreOpsFunctionFnOnceTupleClause0_BaseFieldQuadraticExt.call_once,
      hv.mul_ok, hv.neg_ok, bind_tc_ok, gfpn.quadratic.QuadraticExt.new,
      spec, theta, wp_return]
    have hn : norm_val ≠ 0 := fun h => absurd (ho_z h) (by simp)
    have hr_inv : r * norm_val = 1 := by
      obtain ⟨r', hr_eq, hr_inv⟩ := ho_nz hn
      have : r' = r := by simpa using hr_eq.symm
      subst this; exact hr_inv
    have hr_val : r = norm_val⁻¹ := by
      rw [eq_comm]; exact inv_eq_of_mul_eq_one_right (by rwa [mul_comm])
    constructor
    · intro ⟨hc0, hc1⟩; exfalso; apply hn; rw [hnorm_def, hc0, hc1]; ring
    · intro _; exact ⟨_, rfl, by rw [hr_val, hnorm_sq], by rw [hr_val, hnorm_sq]⟩

end QExtProgress

/-! ## CubicExt progress lemmas -/

namespace CExtProgress

variable {C BF Char Wide : Type} [Field BF]
variable {inst : gfpn.ext_config.ExtConfig C BF Char Wide}

omit [Field BF] in
/-- CubicExt.new always succeeds -/
@[progress]
theorem cnew_progress (c0 c1 c2 : BF) :
    gfpn.cubic.CubicExt.new inst c0 c1 c2 ⦃ fun r =>
      r.c0 = c0 ∧ r.c1 = c1 ∧ r.c2 = c2 ⦄ := by
  simp [gfpn.cubic.CubicExt.new, spec, theta, wp_return]

/-- CubicExt.add: component-wise -/
@[progress]
theorem cadd_progress (hv : ValidExtConfig inst)
    (a b : gfpn.cubic.CubicExt inst) :
    ExtAbbrev.CAdd inst a b ⦃ fun r =>
      r.c0 = a.c0 + b.c0 ∧ r.c1 = a.c1 + b.c1 ∧ r.c2 = a.c2 + b.c2 ⦄ := by
  simp only [ExtAbbrev.CAdd,
    gfpn.cubic.CubicExt.Insts.CoreOpsArithAddCubicExtCubicExt.add,
    hv.add_ok, bind_tc_ok, gfpn.cubic.CubicExt.new]
  simp [spec, theta, wp_return]

/-- CubicExt.sub: component-wise -/
@[progress]
theorem csub_progress (hv : ValidExtConfig inst)
    (a b : gfpn.cubic.CubicExt inst) :
    ExtAbbrev.CSub inst a b ⦃ fun r =>
      r.c0 = a.c0 - b.c0 ∧ r.c1 = a.c1 - b.c1 ∧ r.c2 = a.c2 - b.c2 ⦄ := by
  simp only [ExtAbbrev.CSub,
    gfpn.cubic.CubicExt.Insts.CoreOpsArithSubCubicExtCubicExt.sub,
    hv.sub_ok, bind_tc_ok, gfpn.cubic.CubicExt.new]
  simp [spec, theta, wp_return]

/-- CubicExt.neg: component-wise -/
@[progress]
theorem cneg_progress (hv : ValidExtConfig inst)
    (a : gfpn.cubic.CubicExt inst) :
    ExtAbbrev.CNeg inst a ⦃ fun r =>
      r.c0 = -a.c0 ∧ r.c1 = -a.c1 ∧ r.c2 = -a.c2 ⦄ := by
  simp only [ExtAbbrev.CNeg,
    gfpn.cubic.CubicExt.Insts.CoreOpsArithNegCubicExt.neg,
    hv.neg_ok, bind_tc_ok, gfpn.cubic.CubicExt.new]
  simp [spec, theta, wp_return]

/-- CubicExt.mul: Karatsuba 6-mul trick equals schoolbook -/
@[progress]
theorem cmul_progress (hv : ValidExtConfig inst)
    (a b : gfpn.cubic.CubicExt inst) :
    let β := hv.getNonResidue
    ExtAbbrev.CMul inst a b ⦃ fun r =>
      r.c0 = a.c0 * b.c0 + β * (a.c1 * b.c2 + a.c2 * b.c1) ∧
      r.c1 = a.c0 * b.c1 + a.c1 * b.c0 + β * (a.c2 * b.c2) ∧
      r.c2 = a.c0 * b.c2 + a.c1 * b.c1 + a.c2 * b.c0 ⦄ := by
  simp only [ExtAbbrev.CMul,
    gfpn.cubic.CubicExt.Insts.CoreOpsArithMulCubicExtCubicExt.mul,
    hv.mul_ok, hv.add_ok, hv.sub_ok, hv.mul_nr_eq, bind_tc_ok,
    gfpn.cubic.CubicExt.new, spec, theta, wp_return]
  refine ⟨?_, ?_, ?_⟩ <;> ring

/-- CubicExt.norm: the full cubic norm computation -/
@[progress]
theorem cnorm_progress (hv : ValidExtConfig inst)
    (a : gfpn.cubic.CubicExt inst) :
    let β := hv.getNonResidue
    ExtAbbrev.CNorm inst a ⦃ fun r =>
      r = a.c0 * (a.c0 ^ 2 - β * (a.c1 * a.c2)) +
          β * (a.c2 * (β * a.c2 ^ 2 - a.c0 * a.c1) +
               a.c1 * (a.c1 ^ 2 - a.c0 * a.c2)) ⦄ := by
  simp only [ExtAbbrev.CNorm, gfpn.cubic.CubicExt.norm,
    hv.mul_ok, hv.add_ok, hv.sub_ok, hv.mul_nr_eq, bind_tc_ok]
  simp only [spec, theta, wp_return, sq]

/-- CubicExt.inv: computes cofactors, norm, base inv, maps closure -/
@[progress]
theorem cinv_progress (hv : ValidExtConfig inst)
    (self : gfpn.cubic.CubicExt inst) :
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
    gfpn.cubic.CubicExt.Insts.Gf2_coreFieldTraitsFiniteFieldClause0_Clause0_Clause0_CharacteristicCubicExt.inv,
    hv.mul_ok, hv.add_ok, hv.sub_ok, hv.mul_nr_eq, bind_tc_ok]
  -- Set up the cofactors and norm
  set s0 := self.c0 * self.c0 - hv.getNonResidue * (self.c1 * self.c2)
  set s1 := hv.getNonResidue * (self.c2 * self.c2) - self.c0 * self.c1
  set s2 := self.c1 * self.c1 - self.c0 * self.c2
  set norm_val := self.c0 * s0 + hv.getNonResidue * (self.c2 * s1 + self.c1 * s2)
  -- Get base field inv
  obtain ⟨o, ho_eq, ho_nz, ho_z⟩ := hv.inv_ok norm_val
  simp only [ho_eq, bind_tc_ok]
  -- Case split on o
  rcases o with _ | r
  · -- o = none: norm_val = 0
    simp only [core.option.Option.map, spec, theta, wp_return, sq]
    refine ⟨fun _ => trivial, fun hn => ?_⟩
    obtain ⟨r, hr, _⟩ := ho_nz hn; exact absurd hr (by simp)
  · -- o = some r: norm_val ≠ 0, r = norm_val⁻¹
    simp only [core.option.Option.map,
      gfpn.cubic.FiniteFieldCubicExtClause0_Clause0_Clause0_CharacteristicCubicExt.inv.closure.Insts.CoreOpsFunctionFnOnceTupleClause0_BaseFieldCubicExt.call_once,
      hv.mul_ok, bind_tc_ok, gfpn.cubic.CubicExt.new,
      spec, theta, wp_return, sq]
    have hn : norm_val ≠ 0 := fun h => absurd (ho_z h) (by simp)
    have hr_inv : r * norm_val = 1 := by
      obtain ⟨r', hr_eq, hr_inv⟩ := ho_nz hn
      have : r' = r := by simpa using hr_eq.symm
      subst this; exact hr_inv
    have hr_val : r = norm_val⁻¹ := by
      rw [eq_comm]; exact inv_eq_of_mul_eq_one_right (by rwa [mul_comm])
    exact ⟨fun hn' => absurd hn' hn,
      fun _ => ⟨_, rfl, by rw [hr_val], by rw [hr_val], by rw [hr_val]⟩⟩

end CExtProgress
