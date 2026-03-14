/-
  Gf2Core.Proofs.Gf2mAddition — Correctness proof for gf2m_add_raw

  Proves that the Aeneas-generated gf2m_add_raw computes bitwise XOR,
  which is GF(2^m) addition (characteristic-2 field addition = XOR).
-/
import Aeneas
import Gf2Core.Funs
import Gf2Core.Proofs.Gf2mDefs

open Aeneas Aeneas.Std Result ControlFlow Error
open gf2_core Gf2mSpec

set_option maxHeartbeats 800000

noncomputable section

namespace Gf2mAddition

/-- gf2m_add_raw computes bitwise XOR, matching add_raw_spec. -/
theorem gf2m_add_raw_correct (a b : Std.U64) :
    ∃ r, gf2m.mul_raw.gf2m_add_raw a b = ok r ∧
      r.val = add_raw_spec a.val b.val := by
  unfold gf2m.mul_raw.gf2m_add_raw
  refine ⟨_, rfl, ?_⟩
  simp [add_raw_spec, UScalar.val_xor]

/-- gf2m_add_raw preserves field element bounds. -/
theorem gf2m_add_raw_bound (a b : Std.U64) (m : ℕ)
    (ha : a.val < 2 ^ m) (hb : b.val < 2 ^ m) :
    ∃ r, gf2m.mul_raw.gf2m_add_raw a b = ok r ∧
      r.val < 2 ^ m := by
  unfold gf2m.mul_raw.gf2m_add_raw
  refine ⟨_, rfl, ?_⟩
  rw [UScalar.val_xor]
  exact Nat.xor_lt_two_pow ha hb

end Gf2mAddition
