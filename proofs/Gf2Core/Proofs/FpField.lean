/-
  Gf2Core.Proofs.FpField — Ring isomorphism FpVal P ≃+* ZMod P and Field instance

  Strategy: Build a bare Equiv between FpVal P and ZMod P.val using the Montgomery
  representation value, then transfer CommRing and Field via Mathlib's Equiv.commRing
  and Equiv.field from ZMod.
-/
import Mathlib.Data.ZMod.Basic
import Mathlib.Algebra.Ring.TransferInstance
import Mathlib.Algebra.Field.TransferInstance
import Mathlib.Algebra.Field.ZMod
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

/-! ## Equivalence with ZMod -/

/-- Two FpVal values with equal mont.val are equal. -/
private lemma fpVal_ext_val {P : Std.U64} {a b : FpVal P}
    (h : a.mont.val = b.mont.val) : a = b := by
  cases a with | mk ma ha =>
  cases b with | mk mb hb =>
  simp only [FpVal.mk.injEq]
  cases ma with | mk bva =>
  cases mb with | mk bvb =>
  simp only [UScalar.mk.injEq]
  apply BitVec.eq_of_toNat_eq
  exact h

/-- Bare bijection between FpVal P and ZMod P.val.
    Uses the Montgomery representation value directly as a ZMod element.
    This is purely a set-level bijection; algebraic structure is transferred
    from ZMod via Equiv.commRing / Equiv.field. -/
private noncomputable def fpEquiv {P : Std.U64} (hP : ValidPrime P) :
    FpVal P ≃ ZMod P.val where
  toFun a := (a.mont.val : ZMod P.val)
  invFun x :=
    haveI : NeZero P.val := ⟨by have := hP.2.1; omega⟩
    have hlt : ZMod.val x < P.val := ZMod.val_lt x
    have hlt64 : ZMod.val x < 2 ^ 64 := by have := hP.2.2; omega
    ⟨⟨BitVec.ofNat 64 (ZMod.val x)⟩, by
      show (BitVec.ofNat 64 (ZMod.val x)).toNat < P.val
      rw [BitVec.toNat_ofNat, Nat.mod_eq_of_lt hlt64]
      exact hlt⟩
  left_inv a := by
    haveI : NeZero P.val := ⟨by have := hP.2.1; omega⟩
    apply fpVal_ext_val
    show (BitVec.ofNat 64 (ZMod.val ((a.mont.val : ℕ) : ZMod P.val))).toNat = a.mont.val
    rw [BitVec.toNat_ofNat, ZMod.val_natCast, Nat.mod_eq_of_lt a.h_lt,
        Nat.mod_eq_of_lt (by have := hP.2.2; have := a.h_lt; omega)]
  right_inv x := by
    haveI : NeZero P.val := ⟨by have := hP.2.1; omega⟩
    show ((BitVec.ofNat 64 (ZMod.val x)).toNat : ZMod P.val) = x
    rw [BitVec.toNat_ofNat, Nat.mod_eq_of_lt (by have := ZMod.val_lt x; have := hP.2.2; omega)]
    exact ZMod.natCast_zmod_val x

/-! ## CommRing instance -/

/-- FpVal P forms a commutative ring (for valid odd primes P).
    Transferred from ZMod P.val via the bare equivalence. -/
noncomputable instance FpVal.instCommRing {P : Std.U64}
    (hP : ValidPrime P) (hP2 : P.val ≠ 2) : CommRing (FpVal P) :=
  (fpEquiv hP).commRing

/-! ## Map to ZMod -/

/-- Map from FpVal to ZMod: extract canonical value via from_mont -/
noncomputable def toZMod {P : Std.U64} (hP : ValidPrime P) (hP2 : P.val ≠ 2)
    (a : FpVal P) : ZMod P.val :=
  let h := Aeneas.Std.WP.spec_imp_exists (FpProgress.from_mont_progress hP a.h_lt)
  (h.choose.val : ZMod P.val)

/-- Map from ZMod to FpVal: use the bare equivalence inverse -/
noncomputable def fromZMod {P : Std.U64} (hP : ValidPrime P) (hP2 : P.val ≠ 2)
    (x : ZMod P.val) : FpVal P :=
  (fpEquiv hP).symm x

/-! ## Ring isomorphism -/

/-- The fundamental isomorphism: FpVal P ≃+* ZMod P.val.
    Constructed from the bare equivalence; the ring operations on FpVal P
    are defined by transfer, so the equiv is a ring homomorphism by construction. -/
noncomputable def fpValRingEquiv {P : Std.U64} (hP : ValidPrime P)
    (hP2 : P.val ≠ 2) :
    @RingEquiv (FpVal P) (ZMod P.val)
      (FpVal.instCommRing hP hP2).toMul _
      (FpVal.instCommRing hP hP2).toAdd _ := by
  letI := FpVal.instCommRing hP hP2
  exact { fpEquiv hP with
    map_mul' := fun a b => (fpEquiv hP).apply_symm_apply _
    map_add' := fun a b => (fpEquiv hP).apply_symm_apply _ }

/-! ## Field instance -/

/-- FpVal P is a field when P is a valid odd prime.
    Derived from ZMod.instField via the bare equivalence. -/
noncomputable def FpVal.instField {P : Std.U64}
    (hP : ValidPrime P) (hP2 : P.val ≠ 2) : Field (FpVal P) :=
  letI : Fact (Nat.Prime P.val) := ⟨hP.1⟩
  (fpEquiv hP).field

end FpField

end
