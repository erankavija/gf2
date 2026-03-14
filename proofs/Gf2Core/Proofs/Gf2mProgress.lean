/-
  Gf2Core.Proofs.Gf2mProgress — Progress lemmas for gf2m_mul_raw

  Proves that the Aeneas-generated gf2m_mul_raw and its loop terminate
  (return `ok`, not `fail`) when inputs satisfy ValidGf2mParams.
-/
import Aeneas
import Gf2Core.Funs
import Gf2Core.Proofs.Gf2mDefs

open Aeneas Aeneas.Std Result ControlFlow Error Aeneas.Std.WP
open gf2_core

set_option maxHeartbeats 6400000

noncomputable section

namespace Gf2mProgress

/-- The loop terminates for valid parameters. -/
@[progress]
theorem gf2m_mul_raw_loop_progress
    (b : Std.U64) (m : Std.Usize) (primitive_poly : Std.U64)
    (result : Std.U64) (temp : Std.U64) (i : Std.Usize)
    (hparams : ValidGf2mParams m primitive_poly)
    (hi : i.val ≤ m.val) :
    gf2m.mul_raw.gf2m_mul_raw_loop b m primitive_poly result temp i
    ⦃ fun _ => True ⦄ := by
  unfold gf2m.mul_raw.gf2m_mul_raw_loop
  apply loop.spec (γ := ℕ)
    (measure := fun ((_, _, i1) : Std.U64 × Std.U64 × Std.Usize) => m.val - i1.val)
    (inv := fun ((_, _, i1) : Std.U64 × Std.U64 × Std.Usize) => i1.val ≤ m.val)
  · intro ⟨result1, temp1, i1⟩ hinv
    simp only at hinv
    dsimp only
    by_cases hlt : i1 < m
    · simp only [hlt, ite_true]
      progress as ⟨shifted_b, _, _⟩
      · have := hparams.m_le; scalar_tac
      simp only [Std.lift, bind_tc_ok]
      -- result conditional
      split
      -- Both branches share the same tail proof
      -- Bring ValidGf2mParams fields into scope for scalar_tac
      <;> (
        have hm_pos := hparams.m_pos
        have hm_le := hparams.m_le
        -- m - 1 (auto-discharged: 1 ≤ m from hm_pos + scalar_tac)
        progress as ⟨m1, hm1_val, _⟩
        -- temp >>> m1 (m1.val = m.val - 1 < 64)
        have hm1_lt : m1.val < 64 := by omega
        progress as ⟨shifted_temp, _, _⟩
        -- temp <<< 1 (lift was consumed by progress)
        progress as ⟨temp_shifted, _, _⟩
        -- temp conditional + i+1
        split <;> (
          progress as ⟨i_next, hi_next⟩
          refine ⟨by scalar_tac, ?_⟩
          -- WellFoundedRelation.rel on ℕ is (<): m - (i1+1) < m - i1
          show m.val - i_next.val < m.val - i1.val
          have : i_next.val = i1.val + 1 := by scalar_tac
          have : i1.val < m.val := by scalar_tac
          omega))
    · simp only [hlt, ite_false, spec, theta, wp_return]
  · exact hi

/-- The top-level gf2m_mul_raw terminates for valid parameters. -/
theorem gf2m_mul_raw_progress
    (a b : Std.U64) (m : Std.Usize) (primitive_poly : Std.U64)
    (hparams : ValidGf2mParams m primitive_poly) :
    ∃ r, gf2m.mul_raw.gf2m_mul_raw a b m primitive_poly = ok r := by
  unfold gf2m.mul_raw.gf2m_mul_raw
  by_cases ha0 : a = 0#u64
  · subst ha0; exact ⟨_, rfl⟩
  · simp only [ha0, ite_false]
    by_cases hb0 : b = 0#u64
    · subst hb0; simp only [ite_true]; exact ⟨_, rfl⟩
    · simp only [hb0, ite_false]
      -- Loop
      have hloop_wp := gf2m_mul_raw_loop_progress b m primitive_poly 0#u64 a 0#usize hparams (Nat.zero_le _)
      obtain ⟨loop_result, hloop_eq, _⟩ := spec_imp_exists hloop_wp
      simp only [hloop_eq, bind_tc_ok]
      -- 1 <<< m: produces mask_base ≥ 1
      have hm_lt_64 : m.val < 64 := by have := hparams.m_le; omega
      have h_shl_spec : (1#u64 <<< m) ⦃ r => r.val ≥ 1 ⦄ := by
        progress as ⟨r, hr_val, _⟩
        simp only [Nat.shiftLeft_eq, one_mul] at hr_val
        -- hr_val : r.val = 2^m.val % U64.size
        have h2pm_lt : 2 ^ m.val < Std.U64.size := by
          calc 2 ^ m.val ≤ 2 ^ 63 := Nat.pow_le_pow_right (by norm_num) hparams.m_le
            _ < Std.U64.size := by native_decide
        rw [Nat.mod_eq_of_lt h2pm_lt] at hr_val
        rw [hr_val]; exact Nat.one_le_two_pow
      obtain ⟨mask_base, hmb_eq, hmb_ge⟩ := spec_imp_exists h_shl_spec
      simp only [hmb_eq, bind_tc_ok]
      -- mask_base - 1
      have h_sub_spec : (mask_base - 1#u64) ⦃ fun _ => True ⦄ := by
        progress
      obtain ⟨mask, hmask_eq, _⟩ := spec_imp_exists h_sub_spec
      simp only [hmask_eq, bind_tc_ok]
      exact ⟨_, rfl⟩

end Gf2mProgress
