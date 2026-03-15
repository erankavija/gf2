/-
  Gf2Core.Proofs.Gf2mProgress — Progress lemmas for GF(2^m) operations

  Proves that the Aeneas-generated gf2m_mul_raw, gf2m_add_raw, gf2m_pow_raw,
  and gf2m_inverse_raw terminate (return `ok`, not `fail`) when inputs
  satisfy ValidGf2mParams.
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
  -- Discharge assert! guards: m >= 1 and m <= 63
  have hm_ge1 : m ≥ 1#usize := by have := hparams.m_pos; scalar_tac
  have hm_le63 : m ≤ 63#usize := by have := hparams.m_le; scalar_tac
  simp only [hm_ge1, ite_true, hm_le63]
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

/-- gf2m_add_raw always terminates (pure XOR). -/
theorem gf2m_add_raw_progress
    (a b : Std.U64) :
    ∃ r, gf2m.mul_raw.gf2m_add_raw a b = ok r := by
  unfold gf2m.mul_raw.gf2m_add_raw
  exact ⟨_, rfl⟩

/-- The pow_raw loop terminates for valid parameters.
    Measure: exp strictly decreases via right-shift. -/
@[progress]
theorem gf2m_pow_raw_loop_progress
    (base exp : Std.U64) (m : Std.Usize) (primitive_poly : Std.U64)
    (result : Std.U64)
    (hparams : ValidGf2mParams m primitive_poly) :
    gf2m.mul_raw.gf2m_pow_raw_loop base exp m primitive_poly result
    ⦃ fun _ => True ⦄ := by
  unfold gf2m.mul_raw.gf2m_pow_raw_loop
  apply loop.spec (γ := ℕ)
    (measure := fun ((_, exp1, _) : Std.U64 × Std.U64 × Std.U64) => exp1.val)
    (inv := fun _ => True)
  · intro ⟨base1, exp1, result1⟩ _
    dsimp only
    by_cases hgt : exp1 > 0#u64
    · simp only [hgt, ite_true]
      simp only [Std.lift, bind_tc_ok]
      -- Split on if (exp & 1) != 0: result conditional
      -- Both branches share the same tail proof.
      split <;> (
        -- Handle the result mul_raw call if present (true branch), skip if not
        try (
          have hmul := gf2m_mul_raw_progress result1 base1 m primitive_poly hparams
          obtain ⟨r, hr⟩ := hmul
          simp only [hr, bind_tc_ok])
        -- exp >>> 1
        progress as ⟨exp2, hexp2_val, _⟩
        have hexp2_lt : exp2.val < exp1.val := by
          rw [hexp2_val, Nat.shiftRight_eq_div_pow]; simp
          exact Nat.div_lt_self (by scalar_tac) (by norm_num)
        -- Split on exp2 > 0 (square base or not)
        split <;> (
          -- Handle the base mul_raw call if present (true branch)
          try (
            have hsq := gf2m_mul_raw_progress base1 base1 m primitive_poly hparams
            obtain ⟨r2, hr2⟩ := hsq
            simp only [hr2, bind_tc_ok])
          -- Goal is now exp2.val < exp1.val (True ∧ was simplified away)
          show exp2.val < exp1.val
          exact hexp2_lt))
    · simp only [hgt, ite_false, spec, theta, wp_return]
  · trivial

/-- The top-level gf2m_pow_raw terminates for valid parameters. -/
theorem gf2m_pow_raw_progress
    (base exp : Std.U64) (m : Std.Usize) (primitive_poly : Std.U64)
    (hparams : ValidGf2mParams m primitive_poly) :
    ∃ r, gf2m.mul_raw.gf2m_pow_raw base exp m primitive_poly = ok r := by
  unfold gf2m.mul_raw.gf2m_pow_raw
  have h := gf2m_pow_raw_loop_progress base exp m primitive_poly 1#u64 hparams
  obtain ⟨r, hr, _⟩ := spec_imp_exists h
  exact ⟨r, hr⟩

/-- gf2m_inverse_raw terminates for valid parameters. -/
theorem gf2m_inverse_raw_progress
    (a : Std.U64) (m : Std.Usize) (primitive_poly : Std.U64)
    (hparams : ValidGf2mParams m primitive_poly) :
    ∃ r, gf2m.mul_raw.gf2m_inverse_raw a m primitive_poly = ok r := by
  unfold gf2m.mul_raw.gf2m_inverse_raw
  by_cases ha0 : a = 0#u64
  · subst ha0; exact ⟨_, rfl⟩
  · simp only [ha0, ite_false]
    have hm_lt_64 : m.val < 64 := by have := hparams.m_le; omega
    -- 1 <<< m
    have h_shl : (1#u64 <<< m) ⦃ r => r.val ≥ 2 ⦄ := by
      progress as ⟨r, hr_val, _⟩
      simp only [Nat.shiftLeft_eq, one_mul] at hr_val
      have h2pm_lt : 2 ^ m.val < Std.U64.size := by
        calc 2 ^ m.val ≤ 2 ^ 63 := Nat.pow_le_pow_right (by norm_num) hparams.m_le
          _ < Std.U64.size := by native_decide
      rw [Nat.mod_eq_of_lt h2pm_lt] at hr_val
      rw [hr_val]
      calc 2 = 2 ^ 1 := by norm_num
        _ ≤ 2 ^ m.val := Nat.pow_le_pow_right (by norm_num) hparams.m_pos
    obtain ⟨shl_r, hshl_eq, hshl_ge⟩ := spec_imp_exists h_shl
    simp only [hshl_eq, bind_tc_ok]
    -- shl_r - 2
    have h_sub : (shl_r - 2#u64) ⦃ fun _ => True ⦄ := by progress
    obtain ⟨exp, hexp_eq, _⟩ := spec_imp_exists h_sub
    simp only [hexp_eq, bind_tc_ok]
    -- pow_raw
    exact gf2m_pow_raw_progress a _ m primitive_poly hparams

end Gf2mProgress
