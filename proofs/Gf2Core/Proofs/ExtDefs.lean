/-
  Gf2Core.Proofs.ExtDefs — Foundation definitions for extension field proofs

  Defines ValidExtConfig / ValidCubicExtConfig predicates and short abbreviations
  for the extremely long Aeneas-generated names.
-/
import Aeneas
import Gf2Core.Types
import Gf2Core.FunsExternal
import Gf2Core.Funs

open Aeneas Aeneas.Std Result ControlFlow Error
open gf2_core

set_option maxHeartbeats 400000

/-! ## Name abbreviations -/

namespace ExtAbbrev

variable {C BF Char Wide : Type}
variable (inst : gfpn.ext_config.ExtConfig C BF Char Wide)

-- Base field operations (via ExtConfig's ConstField → FiniteField)
abbrev BAdd := inst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithAddInst.add
abbrev BSub := inst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithSubInst.sub
abbrev BMul := inst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithMulInst.mul
abbrev BNeg := inst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithNegInst.neg
abbrev BInv := inst.fieldtraitsConstFieldInst.FiniteFieldInst.inv
abbrev BZero := inst.fieldtraitsConstFieldInst.zero
abbrev BOne := inst.fieldtraitsConstFieldInst.one
abbrev MulNR := inst.mul_by_non_residue

-- QuadraticExt operations
abbrev QAdd := gfpn.quadratic.QuadraticExt.Insts.CoreOpsArithAddQuadraticExtQuadraticExt.add inst
abbrev QSub := gfpn.quadratic.QuadraticExt.Insts.CoreOpsArithSubQuadraticExtQuadraticExt.sub inst
abbrev QMul := gfpn.quadratic.QuadraticExt.Insts.CoreOpsArithMulQuadraticExtQuadraticExt.mul inst
abbrev QNeg := gfpn.quadratic.QuadraticExt.Insts.CoreOpsArithNegQuadraticExt.neg inst
abbrev QInv := gfpn.quadratic.QuadraticExt.Insts.Gf2_coreFieldTraitsFiniteFieldClause0_Clause0_Clause0_CharacteristicQuadraticExt.inv inst
abbrev QNorm := gfpn.quadratic.QuadraticExt.norm inst
abbrev QNew := gfpn.quadratic.QuadraticExt.new inst
abbrev QConj := gfpn.quadratic.QuadraticExt.conjugate inst

-- QuadraticExt order
abbrev QOrder := gfpn.quadratic.QuadraticExt.Insts.Gf2_coreFieldTraitsConstFieldClause0_Clause0_Clause0_CharacteristicQuadraticExt.order inst

-- CubicExt operations
abbrev CAdd := gfpn.cubic.CubicExt.Insts.CoreOpsArithAddCubicExtCubicExt.add inst
abbrev CSub := gfpn.cubic.CubicExt.Insts.CoreOpsArithSubCubicExtCubicExt.sub inst
abbrev CMul := gfpn.cubic.CubicExt.Insts.CoreOpsArithMulCubicExtCubicExt.mul inst
abbrev CNeg := gfpn.cubic.CubicExt.Insts.CoreOpsArithNegCubicExt.neg inst
abbrev CInv := gfpn.cubic.CubicExt.Insts.Gf2_coreFieldTraitsFiniteFieldClause0_Clause0_Clause0_CharacteristicCubicExt.inv inst
abbrev CNorm := gfpn.cubic.CubicExt.norm inst
abbrev CNew := gfpn.cubic.CubicExt.new inst

-- CubicExt order
abbrev COrder := gfpn.cubic.CubicExt.Insts.Gf2_coreFieldTraitsConstFieldClause0_Clause0_Clause0_CharacteristicCubicExt.order inst

end ExtAbbrev

/-! ## ValidExtConfig — assumptions on the base field for quadratic extensions -/

/-- A valid extension configuration: base field ops are total and satisfy field axioms.
    The non-residue β is extracted from `mul_by_non_residue`. -/
structure ValidExtConfig {C BF Char Wide : Type}
    (inst : gfpn.ext_config.ExtConfig C BF Char Wide)
    [Field BF] where
  /-- Base addition is total and agrees with the Field instance -/
  add_ok : ∀ a b : BF, ExtAbbrev.BAdd inst a b = ok (a + b)
  /-- Base subtraction is total -/
  sub_ok : ∀ a b : BF, ExtAbbrev.BSub inst a b = ok (a - b)
  /-- Base multiplication is total -/
  mul_ok : ∀ a b : BF, ExtAbbrev.BMul inst a b = ok (a * b)
  /-- Base negation is total -/
  neg_ok : ∀ a : BF, ExtAbbrev.BNeg inst a = ok (-a)
  /-- mul_by_non_residue is multiplication by some fixed β -/
  mul_nr_ok : ∃ β : BF, ∀ x, inst.mul_by_non_residue x = ok (β * x)
  /-- zero returns the field zero -/
  zero_ok : inst.fieldtraitsConstFieldInst.zero = ok (0 : BF)
  /-- one returns the field one -/
  one_ok : inst.fieldtraitsConstFieldInst.one = ok (1 : BF)
  /-- Base field inv is total and correct -/
  inv_ok : ∀ a : BF, ∃ o, ExtAbbrev.BInv inst a = ok o ∧
    (a ≠ 0 → ∃ r, o = some r ∧ r * a = 1) ∧ (a = 0 → o = none)
  /-- Non-residue irreducibility: a² - β·b² = 0 implies a = 0 ∧ b = 0 -/
  nr_irred : ∀ β : BF, (∀ x, inst.mul_by_non_residue x = ok (β * x)) →
    ∀ a b : BF, a ^ 2 - β * b ^ 2 = 0 → a = 0 ∧ b = 0

namespace ValidExtConfig

variable {C BF Char Wide : Type} [Field BF] {inst : gfpn.ext_config.ExtConfig C BF Char Wide}

/-- Extract the non-residue β from a valid config -/
noncomputable def getNonResidue (hv : ValidExtConfig inst) : BF :=
  hv.mul_nr_ok.choose

theorem mul_nr_eq (hv : ValidExtConfig inst) (x : BF) :
    inst.mul_by_non_residue x = ok (hv.getNonResidue * x) :=
  hv.mul_nr_ok.choose_spec x

end ValidExtConfig

/-! ## ValidCubicExtConfig — for cubic extensions -/

/-- Valid extension configuration for cubic extensions F[v]/(v³=β) -/
structure ValidCubicExtConfig {C BF Char Wide : Type} [Field BF]
    (inst : gfpn.ext_config.ExtConfig C BF Char Wide) extends
    ValidExtConfig inst where
  /-- Cubic norm anisotropy: the cubic norm form is anisotropic -/
  cubic_nr_irred : ∀ β : BF, (∀ x, inst.mul_by_non_residue x = ok (β * x)) →
    ∀ a b c : BF,
      a ^ 3 + β * b ^ 3 + β ^ 2 * c ^ 3 - 3 * β * a * b * c = 0 →
      a = 0 ∧ b = 0 ∧ c = 0
