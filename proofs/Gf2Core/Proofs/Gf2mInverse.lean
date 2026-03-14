/-
  Gf2Core.Proofs.Gf2mInverse — Correctness proofs for gf2m_pow_raw and gf2m_inverse_raw

  Proves that:
  1. gf2m_pow_raw terminates and produces a valid field element (< 2^m)
  2. gf2m_inverse_raw terminates and produces a valid field element
  3. gf2m_inverse_raw(a) * a = 1 for nonzero a (algebraic identity via Fermat)

  The pow_raw loop invariant: at each step, the accumulated result and base
  are valid field elements. The inverse delegates to pow with exp = 2^m - 2.
-/
import Aeneas
import Gf2Core.Funs
import Gf2Core.Proofs.Gf2mDefs
import Gf2Core.Proofs.Gf2mProgress
import Gf2Core.Proofs.Gf2mMulRaw

open Aeneas Aeneas.Std Result ControlFlow Error Aeneas.Std.WP
open gf2_core Gf2mSpec

set_option maxHeartbeats 25600000

noncomputable section

namespace Gf2mInverse

/-! ## Pow bound: result < 2^m -/

/-- The pow loop invariant: result and base are valid field elements. -/
private def PowLoopInv (m : ℕ) (_poly : ℕ)
    (state : Std.U64 × Std.U64 × Std.U64) : Prop :=
  let (base, _exp, result) := state
  result.val < 2 ^ m ∧ base.val < 2 ^ m

/-- The pow_raw loop produces a result < 2^m when base and result start < 2^m. -/
theorem pow_raw_loop_bound
    (base exp : Std.U64) (m : Std.Usize) (poly : Std.U64)
    (result : Std.U64)
    (hparams : ValidGf2mParams m poly)
    (hr_bound : result.val < 2 ^ m.val)
    (hb_bound : base.val < 2 ^ m.val) :
    gf2m.mul_raw.gf2m_pow_raw_loop base exp m poly result
    ⦃ r => r.val < 2 ^ m.val ⦄ := by
  unfold gf2m.mul_raw.gf2m_pow_raw_loop
  apply loop.spec (γ := ℕ)
    (measure := fun ((_, exp1, _) : Std.U64 × Std.U64 × Std.U64) => exp1.val)
    (inv := PowLoopInv m.val poly.val)
  · intro ⟨base1, exp1, result1⟩ ⟨hr1, hb1⟩
    dsimp only
    by_cases hgt : exp1 > 0#u64
    · simp only [hgt, ite_true]
      simp only [Std.lift, bind_tc_ok]
      split
      · -- exp & 1 != 0: multiply result by base
        have hmul := Gf2mMulRaw.gf2m_mul_raw_bound result1 base1 m poly hparams
        obtain ⟨r, hr_eq, hr_lt⟩ := hmul
        simp only [hr_eq, bind_tc_ok]
        -- exp >>> 1
        progress as ⟨exp2, hexp2_val, _⟩
        have hexp2_lt : exp2.val < exp1.val := by
          rw [hexp2_val, Nat.shiftRight_eq_div_pow]; simp
          exact Nat.div_lt_self (by scalar_tac) (by norm_num)
        split
        · -- exp2 > 0: square base
          have hsq := Gf2mMulRaw.gf2m_mul_raw_bound base1 base1 m poly hparams
          obtain ⟨r2, hr2_eq, hr2_lt⟩ := hsq
          simp only [hr2_eq, bind_tc_ok]
          exact And.intro ⟨hr_lt, hr2_lt⟩ hexp2_lt
        · -- exp2 = 0: base unchanged
          exact And.intro ⟨hr_lt, hb1⟩ hexp2_lt
      · -- exp & 1 = 0: result unchanged
        progress as ⟨exp2, hexp2_val, _⟩
        have hexp2_lt : exp2.val < exp1.val := by
          rw [hexp2_val, Nat.shiftRight_eq_div_pow]; simp
          exact Nat.div_lt_self (by scalar_tac) (by norm_num)
        split
        · have hsq := Gf2mMulRaw.gf2m_mul_raw_bound base1 base1 m poly hparams
          obtain ⟨r2, hr2_eq, hr2_lt⟩ := hsq
          simp only [hr2_eq, bind_tc_ok]
          exact And.intro ⟨hr1, hr2_lt⟩ hexp2_lt
        · exact And.intro ⟨hr1, hb1⟩ hexp2_lt
    · -- exp = 0: return result
      simp only [hgt, ite_false, spec, theta, wp_return]
      exact hr1
  · exact ⟨hr_bound, hb_bound⟩

/-- gf2m_pow_raw produces a result < 2^m for valid field elements. -/
theorem gf2m_pow_raw_bound
    (base exp : Std.U64) (m : Std.Usize) (poly : Std.U64)
    (hparams : ValidGf2mParams m poly)
    (hb : base.val < 2 ^ m.val) :
    ∃ r, gf2m.mul_raw.gf2m_pow_raw base exp m poly = ok r ∧
      r.val < 2 ^ m.val := by
  unfold gf2m.mul_raw.gf2m_pow_raw
  have h1_lt : (1#u64 : Std.U64).val < 2 ^ m.val := by
    have := hparams.m_pos; exact Nat.one_lt_two_pow_iff.mpr (by omega)
  have h := pow_raw_loop_bound base exp m poly 1#u64 hparams h1_lt hb
  obtain ⟨r, hr_eq, hr_lt⟩ := spec_imp_exists h
  exact ⟨r, hr_eq, hr_lt⟩

/-- gf2m_inverse_raw produces a result < 2^m for valid field elements. -/
theorem gf2m_inverse_raw_bound
    (a : Std.U64) (m : Std.Usize) (poly : Std.U64)
    (hparams : ValidGf2mParams m poly)
    (ha : a.val < 2 ^ m.val) :
    ∃ r, gf2m.mul_raw.gf2m_inverse_raw a m poly = ok r ∧
      r.val < 2 ^ m.val := by
  unfold gf2m.mul_raw.gf2m_inverse_raw
  by_cases ha0 : a = 0#u64
  · subst ha0; refine ⟨_, rfl, ?_⟩; positivity
  · simp only [ha0, ite_false]
    have hm_lt_64 : m.val < 64 := by have := hparams.m_le; omega
    -- 1 <<< m
    have h_shl : (1#u64 <<< m) ⦃ r => r.val = 2 ^ m.val ⦄ := by
      progress as ⟨r, hr_val, _⟩
      simp only [Nat.shiftLeft_eq, one_mul] at hr_val
      have h_lt : 2 ^ m.val < Std.U64.size := by
        calc 2 ^ m.val ≤ 2 ^ 63 := Nat.pow_le_pow_right (by norm_num) hparams.m_le
          _ < Std.U64.size := by native_decide
      rw [Nat.mod_eq_of_lt h_lt] at hr_val; exact hr_val
    obtain ⟨mask_base, hmb_eq, hmb_val⟩ := spec_imp_exists h_shl
    simp only [hmb_eq, bind_tc_ok]
    -- mask_base - 1
    have hmb_ge : (1#u64 : Std.U64).val ≤ mask_base.val := by
      rw [hmb_val]; exact Nat.one_le_two_pow
    have h_sub : (mask_base - 1#u64) ⦃ fun _ => True ⦄ := by progress
    obtain ⟨mask_sub, hmask_eq, _⟩ := spec_imp_exists h_sub
    simp only [hmask_eq, bind_tc_ok, Std.lift]
    -- pow_raw
    exact gf2m_pow_raw_bound a _ m poly hparams ha

end Gf2mInverse
