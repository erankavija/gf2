/-
  Gf2Core.Proofs.CubicExtField — CommRing/Field instances for the Aeneas CubicExt type

  Strategy: Build a bare Equiv between gfpn.cubic.CubicExt and CExt BF β,
  then transfer the CommRing/Field instances from CExt (proven in ExtAlgebra.lean).
-/
import Mathlib.Algebra.Ring.TransferInstance
import Mathlib.Algebra.Field.TransferInstance
import Aeneas
import Gf2Core.Types
import Gf2Core.FunsExternal
import Gf2Core.Funs
import Gf2Core.Proofs.ExtDefs
import Gf2Core.Proofs.ExtAlgebra
import Gf2Core.Proofs.ExtProgress

open Aeneas Aeneas.Std Result ControlFlow Error
open gf2_core

set_option maxHeartbeats 1600000
set_option linter.unusedVariables false

noncomputable section

namespace CubicExtField

variable {C BF Char Wide : Type} [Field BF]
variable {inst : gfpn.ext_config.ExtConfig C BF Char Wide}

/-! ## Equivalence with pure CExt -/

/-- Bare bijection between Aeneas CubicExt and pure CExt -/
def cextEquiv (hv : ValidCubicExtConfig inst) :
    gfpn.cubic.CubicExt inst ≃ CExt BF hv.toValidExtConfig.getNonResidue where
  toFun a := ⟨a.c0, a.c1, a.c2⟩
  invFun a := ⟨a.c0, a.c1, a.c2⟩
  left_inv a := by cases a; rfl
  right_inv a := by cases a; rfl

/-! ## Instances by transfer -/

/-- CubicExt forms a commutative ring -/
noncomputable def instCommRing (hv : ValidCubicExtConfig inst) :
    CommRing (gfpn.cubic.CubicExt inst) :=
  (cextEquiv hv).commRing

/-- CubicExt forms a field (given irreducibility of the cubic norm) -/
noncomputable def instField (hv : ValidCubicExtConfig inst) :
    Field (gfpn.cubic.CubicExt inst) :=
  letI : Field (CExt BF hv.toValidExtConfig.getNonResidue) :=
    CExt.instField (hv.cubic_nr_irred _ hv.toValidExtConfig.mul_nr_eq)
  (cextEquiv hv).field

/-! ## Headline theorems -/

/-- Karatsuba multiplication in the actual Rust code computes the same result
    as schoolbook cubic extension multiplication. -/
theorem karatsuba_correct (hv : ValidCubicExtConfig inst)
    (a b : gfpn.cubic.CubicExt inst) :
    let β := hv.toValidExtConfig.getNonResidue
    ∃ r, ExtAbbrev.CMul inst a b = ok r ∧
      r.c0 = a.c0 * b.c0 + β * (a.c1 * b.c2 + a.c2 * b.c1) ∧
      r.c1 = a.c0 * b.c1 + a.c1 * b.c0 + β * (a.c2 * b.c2) ∧
      r.c2 = a.c0 * b.c2 + a.c1 * b.c1 + a.c2 * b.c0 :=
  Aeneas.Std.WP.spec_imp_exists (CExtProgress.cmul_progress hv.toValidExtConfig a b)

/-- Addition in the Rust code is component-wise. -/
theorem add_correct (hv : ValidCubicExtConfig inst)
    (a b : gfpn.cubic.CubicExt inst) :
    ∃ r, ExtAbbrev.CAdd inst a b = ok r ∧
      r.c0 = a.c0 + b.c0 ∧ r.c1 = a.c1 + b.c1 ∧ r.c2 = a.c2 + b.c2 :=
  Aeneas.Std.WP.spec_imp_exists (CExtProgress.cadd_progress hv.toValidExtConfig a b)

/-- Negation in the Rust code is component-wise. -/
theorem neg_correct (hv : ValidCubicExtConfig inst)
    (a : gfpn.cubic.CubicExt inst) :
    ∃ r, ExtAbbrev.CNeg inst a = ok r ∧
      r.c0 = -a.c0 ∧ r.c1 = -a.c1 ∧ r.c2 = -a.c2 :=
  Aeneas.Std.WP.spec_imp_exists (CExtProgress.cneg_progress hv.toValidExtConfig a)

/-- Inversion in the Rust code computes cofactors/norm correctly. -/
theorem inv_correct (hv : ValidCubicExtConfig inst)
    (self : gfpn.cubic.CubicExt inst) :
    let β := hv.toValidExtConfig.getNonResidue
    let s0 := self.c0 ^ 2 - β * (self.c1 * self.c2)
    let s1 := β * self.c2 ^ 2 - self.c0 * self.c1
    let s2 := self.c1 ^ 2 - self.c0 * self.c2
    let norm_val := self.c0 * s0 + β * (self.c2 * s1 + self.c1 * s2)
    ∃ o, ExtAbbrev.CInv inst self = ok o ∧
      (norm_val = 0 → o = none) ∧
      (norm_val ≠ 0 → ∃ r, o = some r ∧
        r.c0 = s0 * norm_val⁻¹ ∧
        r.c1 = s1 * norm_val⁻¹ ∧
        r.c2 = s2 * norm_val⁻¹) :=
  Aeneas.Std.WP.spec_imp_exists (CExtProgress.cinv_progress hv.toValidExtConfig self)

end CubicExtField

end
