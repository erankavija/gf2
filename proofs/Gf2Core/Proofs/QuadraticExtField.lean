/-
  Gf2Core.Proofs.QuadraticExtField — CommRing/Field instances for the Aeneas QuadraticExt type

  Strategy: Build a bare Equiv between gfpn.quadratic.QuadraticExt and QExt BF β,
  then transfer the CommRing/Field instances from QExt (proven in ExtAlgebra.lean).
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

namespace QuadraticExtField

variable {C BF Char Wide : Type} [Field BF]
variable {inst : gfpn.ext_config.ExtConfig C BF Char Wide}

/-! ## Equivalence with pure QExt -/

/-- Bare bijection between Aeneas QuadraticExt and pure QExt -/
def qextEquiv (hv : ValidExtConfig inst) :
    gfpn.quadratic.QuadraticExt inst ≃ QExt BF hv.getNonResidue where
  toFun a := ⟨a.c0, a.c1⟩
  invFun a := ⟨a.c0, a.c1⟩
  left_inv a := by cases a; rfl
  right_inv a := by cases a; rfl

/-! ## Instances by transfer -/

/-- QuadraticExt forms a commutative ring -/
noncomputable def instCommRing (hv : ValidExtConfig inst) :
    CommRing (gfpn.quadratic.QuadraticExt inst) :=
  (qextEquiv hv).commRing

/-- QuadraticExt forms a field (given irreducibility of the non-residue) -/
noncomputable def instField (hv : ValidExtConfig inst) :
    Field (gfpn.quadratic.QuadraticExt inst) :=
  letI : Field (QExt BF hv.getNonResidue) := QExt.instField (hv.nr_irred _ hv.mul_nr_eq)
  (qextEquiv hv).field

/-! ## Headline theorems -/

/-- Karatsuba multiplication in the actual Rust code computes the same result
    as schoolbook extension field multiplication. -/
theorem karatsuba_correct (hv : ValidExtConfig inst)
    (a b : gfpn.quadratic.QuadraticExt inst) :
    ∃ r, ExtAbbrev.QMul inst a b = ok r ∧
      r.c0 = a.c0 * b.c0 + hv.getNonResidue * (a.c1 * b.c1) ∧
      r.c1 = a.c0 * b.c1 + a.c1 * b.c0 :=
  Aeneas.Std.WP.spec_imp_exists (QExtProgress.qmul_progress hv a b)

/-- Addition in the Rust code is component-wise. -/
theorem add_correct (hv : ValidExtConfig inst)
    (a b : gfpn.quadratic.QuadraticExt inst) :
    ∃ r, ExtAbbrev.QAdd inst a b = ok r ∧
      r.c0 = a.c0 + b.c0 ∧ r.c1 = a.c1 + b.c1 :=
  Aeneas.Std.WP.spec_imp_exists (QExtProgress.qadd_progress hv a b)

/-- Negation in the Rust code is component-wise. -/
theorem neg_correct (hv : ValidExtConfig inst)
    (a : gfpn.quadratic.QuadraticExt inst) :
    ∃ r, ExtAbbrev.QNeg inst a = ok r ∧
      r.c0 = -a.c0 ∧ r.c1 = -a.c1 :=
  Aeneas.Std.WP.spec_imp_exists (QExtProgress.qneg_progress hv a)

/-- Inversion in the Rust code computes conjugate/norm. -/
theorem inv_correct (hv : ValidExtConfig inst)
    (self : gfpn.quadratic.QuadraticExt inst) :
    ∃ o, ExtAbbrev.QInv inst self = ok o ∧
      (self.c0 = 0 ∧ self.c1 = 0 → o = none) ∧
      (¬(self.c0 = 0 ∧ self.c1 = 0) → ∃ r, o = some r ∧
        r.c0 = self.c0 * (self.c0 ^ 2 - hv.getNonResidue * self.c1 ^ 2)⁻¹ ∧
        r.c1 = -(self.c1 * (self.c0 ^ 2 - hv.getNonResidue * self.c1 ^ 2)⁻¹)) :=
  Aeneas.Std.WP.spec_imp_exists (QExtProgress.qinv_progress hv self)

/-- The order of QuadraticExt equals base_order² (assuming no U128 overflow). -/
theorem order_eq_base_squared (bo : Std.U128)
    (h_ord : inst.fieldtraitsConstFieldInst.order = ok bo)
    (h_max : bo.val * bo.val ≤ Std.U128.max) :
    ∃ r, ExtAbbrev.QOrder inst = ok r ∧ r.val = bo.val * bo.val :=
  Aeneas.Std.WP.spec_imp_exists (QExtProgress.qorder_progress bo h_ord h_max)

end QuadraticExtField

end
