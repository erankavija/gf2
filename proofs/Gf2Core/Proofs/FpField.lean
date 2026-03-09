/-
  Gf2Core.Proofs.FpField — Ring isomorphism FpVal P ≃+* ZMod P and Field instance

  Strategy: prove FpVal P ≃+* ZMod P.val, then derive Field from Mathlib's
  ZMod.instField (when P is prime).
-/
import Mathlib.Data.ZMod.Basic
import Aeneas
import Gf2Core.Funs
import Gf2Core.Proofs.Defs
import Gf2Core.Proofs.Progress
import Gf2Core.Proofs.ModArith

open Aeneas Aeneas.Std Result ControlFlow Error
open gf2_core

set_option maxHeartbeats 800000

noncomputable section

namespace FpField

/-! ## Pure operations on FpVal, extracted from progress lemmas -/

/-- Zero element: Montgomery form of 0 is 0 -/
def FpVal.zero {P : Std.U64} (hP : ValidPrime P) : FpVal P where
  mont := ⟨0#64⟩
  h_lt := by
    have h1 := hP.2.1
    have h0 : (⟨0#64⟩ : Std.U64).val = 0 := by decide
    omega

/-- One element: Montgomery form of 1 is R mod P -/
noncomputable def FpVal.one {P : Std.U64} (hP : ValidPrime P)
    (hP2 : P.val ≠ 2) : FpVal P :=
  have h := Aeneas.Std.WP.spec_imp_exists (FpProgress.to_mont_progress (a := ⟨1#64⟩) hP (by
    have h0 : (⟨1#64⟩ : Std.U64).val = 1 := by decide
    have h1 := hP.2.1
    omega))
  ⟨h.choose, h.choose_spec.2⟩

/-- Addition: extract from mont_add progress -/
noncomputable def FpVal.add' {P : Std.U64} (hP : ValidPrime P)
    (a b : FpVal P) : FpVal P :=
  have h := FpProgress.mont_add_progress hP a.h_lt b.h_lt
  ⟨h.choose, h.choose_spec.2⟩

/-- Multiplication: extract from mul progress -/
noncomputable def FpVal.mul' {P : Std.U64} (hP : ValidPrime P) (hP2 : P.val ≠ 2)
    (a b : FpVal P) : FpVal P :=
  have h := FpProgress.mul_progress hP a.h_lt b.h_lt hP2
  ⟨h.choose, h.choose_spec.2⟩

/-- Negation: extract from neg progress -/
noncomputable def FpVal.neg' {P : Std.U64} (hP : ValidPrime P)
    (a : FpVal P) : FpVal P :=
  have h := FpProgress.neg_progress hP a.h_lt
  ⟨h.choose, h.choose_spec.2⟩

/-- Subtraction: defined via add and neg -/
noncomputable def FpVal.sub' {P : Std.U64} (hP : ValidPrime P)
    (a b : FpVal P) : FpVal P :=
  FpVal.add' hP a (FpVal.neg' hP b)

/-! ## CommRing instance -/

/-- FpVal P forms a commutative ring (for valid odd primes P).
    All ring axioms follow from the Montgomery arithmetic being isomorphic
    to ZMod P.val arithmetic. -/
noncomputable instance FpVal.instCommRing {P : Std.U64}
    (hP : ValidPrime P) (hP2 : P.val ≠ 2) : CommRing (FpVal P) := by
  exact sorry

/-! ## Map to ZMod -/

/-- Map from FpVal to ZMod: extract canonical value via from_mont -/
noncomputable def toZMod {P : Std.U64} (hP : ValidPrime P) (hP2 : P.val ≠ 2)
    (a : FpVal P) : ZMod P.val :=
  let h := Aeneas.Std.WP.spec_imp_exists (FpProgress.from_mont_progress hP a.h_lt)
  (h.choose.val : ZMod P.val)

/-- Map from ZMod to FpVal: convert to Montgomery form via to_mont -/
noncomputable def fromZMod {P : Std.U64} (hP : ValidPrime P) (hP2 : P.val ≠ 2)
    (x : ZMod P.val) : FpVal P :=
  sorry

/-! ## Ring isomorphism -/

/-- The fundamental isomorphism: FpVal P ≃+* ZMod P.val.
    Once proven, this lets us derive all field properties from Mathlib's
    ZMod.instField. -/
noncomputable def fpValRingEquiv {P : Std.U64} (hP : ValidPrime P)
    (hP2 : P.val ≠ 2) :
    @RingEquiv (FpVal P) (ZMod P.val)
      (FpVal.instCommRing hP hP2).toMul _
      (FpVal.instCommRing hP hP2).toAdd _ := by
  exact sorry

/-! ## Field instance -/

/-- FpVal P is a field when P is a valid odd prime.
    Derived from ZMod.instField via the ring isomorphism. -/
noncomputable def FpVal.instField {P : Std.U64}
    (hP : ValidPrime P) (hP2 : P.val ≠ 2) : Field (FpVal P) := by
  exact sorry

end FpField

end
