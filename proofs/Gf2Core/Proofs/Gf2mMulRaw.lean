/-
  Gf2Core.Proofs.Gf2mMulRaw — Correctness proof for gf2m_mul_raw

  Proves that the Aeneas-generated gf2m_mul_raw produces a valid field
  element (< 2^m) and matches the mathematical specification mul_raw_spec.
-/
import Aeneas
import Gf2Core.Funs
import Gf2Core.Proofs.Gf2mDefs
import Gf2Core.Proofs.Gf2mProgress
import Mathlib.Data.Nat.Bitwise

open Aeneas Aeneas.Std Result ControlFlow Error Aeneas.Std.WP
open gf2_core Gf2mSpec

set_option maxHeartbeats 25600000

noncomputable section

namespace Gf2mMulRaw

/-! ## Helper lemmas -/

private lemma nat_xor_eq (a b : ℕ) : a.xor b = a ^^^ b := rfl

private lemma testBit_eq_div_mod (n i : ℕ) :
    n.testBit i = decide (n / 2 ^ i % 2 = 1) := by
  simp only [Nat.testBit, Nat.shiftRight_eq_div_pow]
  rw [Nat.and_comm, Nat.and_one_is_mod]
  cases h : n / 2 ^ i % 2 with
  | zero => simp
  | succ k =>
    have : k = 0 := by omega
    subst this; simp

private lemma two_pow_pred_mul_two (m : ℕ) (hm : 0 < m) : 2 ^ (m - 1) * 2 = 2 ^ m := by
  rw [← Nat.pow_succ]; congr 1; omega

/-- shift_reduce preserves the < 2^m bound. -/
theorem shift_reduce_lt (temp m poly : ℕ) (hm : 0 < m)
    (ht : temp < 2 ^ m) (hp_high : 2 ^ m ≤ poly) (hp_bound : poly < 2 ^ (m + 1)) :
    shift_reduce temp m poly < 2 ^ m := by
  unfold shift_reduce; simp only []
  split
  · rename_i h_overflow
    simp only [nat_xor_eq]
    apply Nat.lt_of_testBit (i := m)
    · rw [Nat.testBit_xor]
      have h_shiftl : (temp * 2).testBit m = temp.testBit (m - 1) := by
        rw [show temp * 2 = temp <<< 1 from by simp [Nat.shiftLeft_eq]]
        rw [Nat.testBit_shiftLeft]; simp [show m ≥ 1 from hm]
      have h_temp_bit : temp.testBit (m - 1) = true := by
        rw [testBit_eq_div_mod]; simp [h_overflow]
      rw [h_shiftl, h_temp_bit]
      have h_poly_bit : poly.testBit m = true := by
        rw [testBit_eq_div_mod]
        have h1 : poly / 2 ^ m < 2 := by
          apply Nat.div_lt_of_lt_mul
          calc poly < 2 ^ (m + 1) := hp_bound
            _ = 2 ^ m * 2 := by ring
        have h2 : 1 ≤ poly / 2 ^ m := by
          rw [Nat.le_div_iff_mul_le (by positivity)]; linarith
        simp; omega
      rw [h_poly_bit]; rfl
    · exact Nat.testBit_two_pow_self
    · intro j hj
      rw [Nat.testBit_xor]
      have h1 : (temp * 2).testBit j = false := by
        rw [show temp * 2 = temp <<< 1 from by simp [Nat.shiftLeft_eq]]
        rw [Nat.testBit_shiftLeft]
        simp only [show j ≥ 1 from by omega, decide_true, Bool.true_and]
        exact Nat.testBit_lt_two_pow (calc temp < 2 ^ m := ht
          _ ≤ 2 ^ (j - 1) := Nat.pow_le_pow_right (by norm_num) (by omega))
      have h2 : poly.testBit j = false :=
        Nat.testBit_lt_two_pow (calc poly < 2 ^ (m + 1) := hp_bound
          _ ≤ 2 ^ j := Nat.pow_le_pow_right (by norm_num) hj)
      rw [h1, h2]; simp; rw [Nat.testBit_two_pow]; simp [show m ≠ j from by omega]
  · rename_i h_no_overflow
    have h_div_lt : temp / 2 ^ (m - 1) < 2 := by
      apply Nat.div_lt_of_lt_mul; linarith [two_pow_pred_mul_two m hm]
    have h_div_zero : temp / 2 ^ (m - 1) = 0 := by
      have := Nat.eq_zero_or_pos (temp / 2 ^ (m - 1))
      rcases this with h | h
      · exact h
      · exfalso; have : temp / 2 ^ (m - 1) = 1 := by omega
        rw [this] at h_no_overflow; simp at h_no_overflow
    have h_lt_half : temp < 2 ^ (m - 1) := by
      rw [Nat.div_eq_zero_iff] at h_div_zero
      rcases h_div_zero with h | h
      · exact absurd h (by positivity)
      · exact h
    linarith [two_pow_pred_mul_two m hm]

/-- AND with (2^m - 1) is identity for values < 2^m. -/
private lemma and_mask_identity (x m : ℕ) (hx : x < 2 ^ m) :
    x &&& (2 ^ m - 1) = x := by
  apply Nat.eq_of_testBit_eq
  intro j
  simp only [Nat.testBit_and, Nat.testBit_two_pow_sub_one m j]
  by_cases hj : j < m
  · simp [hj]
  · have hx_bit : x.testBit j = false :=
      Nat.testBit_lt_two_pow (calc x < 2 ^ m := hx
        _ ≤ 2 ^ j := Nat.pow_le_pow_right (by norm_num) (by omega))
    simp [hj, hx_bit]

private theorem shift_reduce_zero (m poly : ℕ) : shift_reduce 0 m poly = 0 := by
  unfold shift_reduce; simp

private theorem specLoop_b_zero (m poly i result temp : ℕ) :
    (specLoop 0 m poly i result temp).1 = result := by
  unfold specLoop; split
  · rfl
  · simp only [show ¬(0 / 2 ^ i % 2 = 1) from by simp, ite_false]
    exact specLoop_b_zero m poly (i + 1) result _
termination_by m - i

private theorem specLoop_a_zero (b m poly i : ℕ) :
    specLoop b m poly i 0 0 = (0, 0) := by
  unfold specLoop; split
  · rfl
  · simp only [show Nat.xor 0 0 = 0 from rfl, ite_self, shift_reduce_zero]; exact specLoop_a_zero b m poly (i + 1)
termination_by m - i

/-! ## Bound proof: result < 2^m -/

theorem gf2m_mul_raw_bound (a b : Std.U64) (m : Std.Usize) (poly : Std.U64)
    (hparams : ValidGf2mParams m poly) :
    ∃ r, gf2m.mul_raw.gf2m_mul_raw a b m poly = ok r ∧
      r.val < 2 ^ m.val := by
  unfold gf2m.mul_raw.gf2m_mul_raw
  by_cases ha0 : a = 0#u64
  · subst ha0; refine ⟨_, rfl, ?_⟩; positivity
  · simp only [ha0, ite_false]
    by_cases hb0 : b = 0#u64
    · subst hb0; simp only [ite_true]; refine ⟨_, rfl, ?_⟩; positivity
    · simp only [hb0, ite_false]
      have hloop_wp := Gf2mProgress.gf2m_mul_raw_loop_progress
        b m poly 0#u64 a 0#usize hparams (Nat.zero_le _)
      obtain ⟨loop_result, hloop_eq, _⟩ := spec_imp_exists hloop_wp
      simp only [hloop_eq, bind_tc_ok]
      have hm_lt_64 : m.val < 64 := by have := hparams.m_le; omega
      have h_shl : (1#u64 <<< m) ⦃ r => r.val = 2 ^ m.val ⦄ := by
        progress as ⟨r, hr_val, _⟩
        simp only [Nat.shiftLeft_eq, one_mul] at hr_val
        have h_lt : 2 ^ m.val < Std.U64.size := by
          calc 2 ^ m.val ≤ 2 ^ 63 := Nat.pow_le_pow_right (by norm_num) hparams.m_le
            _ < Std.U64.size := by native_decide
        rw [Nat.mod_eq_of_lt h_lt] at hr_val; exact hr_val
      obtain ⟨mask_base, hmb_eq, hmb_val⟩ := spec_imp_exists h_shl
      simp only [hmb_eq, bind_tc_ok]
      have hmb_ge : (1#u64 : Std.U64).val ≤ mask_base.val := by
        rw [hmb_val]; exact Nat.one_le_two_pow
      have h_sub : (mask_base - 1#u64) ⦃ r => r.val = 2 ^ m.val - 1 ⦄ := by
        progress as ⟨r, hr⟩
        rw [hmb_val] at hr; linarith
      obtain ⟨mask, hmask_eq, hmask_val⟩ := spec_imp_exists h_sub
      simp only [hmask_eq, bind_tc_ok]
      refine ⟨_, rfl, ?_⟩
      rw [UScalar.val_and]
      apply Nat.and_lt_two_pow
      rw [hmask_val]
      exact Nat.sub_lt (by positivity) (by norm_num)

/-! ## Loop correctness: strengthened loop theorem -/

/-- The loop invariant predicate. -/
private def LoopInv (b m poly a_val : ℕ)
    (state : Std.U64 × Std.U64 × Std.Usize) : Prop :=
  let (result, temp, i) := state
  i.val ≤ m ∧
  result.val < 2 ^ m ∧
  temp.val < 2 ^ m ∧
  specLoop b m poly i.val result.val temp.val = specLoop b m poly 0 0 a_val

/-- specLoop unfolds one step when i < m. -/
private lemma specLoop_step (b m poly i result temp : ℕ) (hi : i < m) :
    specLoop b m poly i result temp =
      let bit_set := b / 2 ^ i % 2 = 1
      let result' := if bit_set then Nat.xor result temp else result
      let temp' := shift_reduce temp m poly
      specLoop b m poly (i + 1) result' temp' := by
  rw [specLoop]
  simp [show ¬(i ≥ m) from by omega]

/-- specLoop returns (result, temp) when i ≥ m. -/
private lemma specLoop_done (b m poly i result temp : ℕ) (hi : i ≥ m) :
    specLoop b m poly i result temp = (result, temp) := by
  rw [specLoop]; simp [hi]

/-- The loop computes specLoop, preserving bounds. -/
theorem loop_correct
    (a b : Std.U64) (m : Std.Usize) (poly : Std.U64)
    (result temp : Std.U64) (i : Std.Usize)
    (hparams : ValidGf2mParams m poly)
    (ha : a.val < 2 ^ m.val)
    (hinv : LoopInv b.val m.val poly.val a.val (result, temp, i)) :
    gf2m.mul_raw.gf2m_mul_raw_loop b m poly result temp i
    ⦃ r =>
      let s := specLoop b.val m.val poly.val 0 0 a.val
      r.val = s.1 ∧ r.val < 2 ^ m.val ⦄ := by
  obtain ⟨hi, hr_bound, ht_bound, hstate⟩ := hinv
  unfold gf2m.mul_raw.gf2m_mul_raw_loop
  apply loop.spec (γ := ℕ)
    (measure := fun ((_, _, i1) : Std.U64 × Std.U64 × Std.Usize) => m.val - i1.val)
    (inv := LoopInv b.val m.val poly.val a.val)
  · -- Body preserves invariant
    intro ⟨result1, temp1, i1⟩ ⟨hinv_i, hinv_r, hinv_t, hinv_state⟩
    dsimp only
    by_cases hlt : i1 < m
    · simp only [hlt, ite_true]
      have hm_pos := hparams.m_pos
      have hm_le := hparams.m_le
      have hi1_lt_64 : i1.val < 64 := by scalar_tac
      -- b >>> i1
      progress as ⟨shifted_b, hshb_val, _⟩
      simp only [Std.lift, bind_tc_ok]
      -- Get val
      have hshb : shifted_b.val = b.val / 2 ^ i1.val := by
        rw [hshb_val, Nat.shiftRight_eq_div_pow]
      have hi1_lt_m : i1.val < m.val := by scalar_tac
      -- Result conditional
      split
      <;> (
        rename_i h_bit
        -- m - 1
        progress as ⟨m1, hm1_val, _⟩
        -- temp1 >>> m1
        have hm1_lt : m1.val < 64 := by omega
        progress as ⟨shifted_temp, hst_val, _⟩
        have hm1_eq : m1.val = m.val - 1 := by scalar_tac
        have hst : shifted_temp.val = temp1.val / 2 ^ (m.val - 1) := by
          rw [hst_val, Nat.shiftRight_eq_div_pow, hm1_eq]
        -- temp1 <<< 1
        progress as ⟨temp_shifted, hts_val, _⟩
        have hts : temp_shifted.val = temp1.val * 2 := by
          rw [hts_val]; simp [Nat.shiftLeft_eq]
          rw [Nat.mod_eq_of_lt]
          calc temp1.val * 2 < 2 ^ m.val * 2 := by linarith [hinv_t]
            _ = 2 ^ (m.val + 1) := by ring
            _ ≤ 2 ^ 64 := Nat.pow_le_pow_right (by norm_num) (by omega)
            _ = Std.U64.size := by native_decide
        -- Temp conditional + i+1
        split <;> (
          rename_i h_temp
          progress as ⟨i_next, hi_next⟩
          -- Extract bit test results from Aeneas conditions
          simp [bne_iff_ne] at h_bit h_temp
          -- h_bit: either shifted_b.val % 2 = 1 or = 0 (result branch)
          -- h_temp: either shifted_temp.val % 2 = 1 or = 0 (temp branch)
          -- Connect to spec via hshb and hst
          have h_bit_spec_val : shifted_b.val % 2 = b.val / 2 ^ i1.val % 2 := by
            rw [hshb]
          have h_temp_spec_val : shifted_temp.val % 2 = temp1.val / 2 ^ (m.val - 1) % 2 := by
            rw [hst]
          -- Compute result' and temp' values
          -- result' value: either result1 ^^^ temp1 or result1
          -- temp' value: either temp_shifted ^^^ poly or temp_shifted (= temp1 * 2 or temp1 * 2 ^^^ poly)
          refine ⟨⟨by scalar_tac, ?_, ?_, ?_⟩, ?_⟩
          -- result' < 2^m
          · -- In the result ^^^ temp case: use Nat.xor_lt_two_pow
            -- In the result unchanged case: use hinv_r
            try (rw [UScalar.val_xor]; exact Nat.xor_lt_two_pow hinv_r hinv_t)
            try exact hinv_r
          -- temp' < 2^m = shift_reduce temp1 m poly < 2^m
          · -- temp' is either temp_shifted ^^^ poly or temp_shifted
            -- In either case, this is shift_reduce temp1.val m.val poly.val
            -- Need: value matches shift_reduce, and shift_reduce < 2^m
            have h_sr := shift_reduce_lt temp1.val m.val poly.val hm_pos hinv_t
              hparams.poly_high hparams.poly_bound
            unfold shift_reduce at h_sr
            simp only [] at h_sr
            -- h_temp tells us which branch of shift_reduce we're in
            -- The Aeneas branch (XOR with poly or not) matches the spec branch
            have h_temp_match : temp1.val / 2 ^ (m.val - 1) % 2 = 1 ∨
              ¬(temp1.val / 2 ^ (m.val - 1) % 2 = 1) := by tauto
            -- The Aeneas value matches
            try (-- XOR branch: temp' = temp_shifted ^^^ poly
              rw [UScalar.val_xor, hts]
              rw [h_temp_spec_val] at h_temp
              simp only [nat_xor_eq] at h_sr
              split at h_sr
              · exact h_sr
              · rename_i h_contra; omega)
            try (-- No-XOR branch: temp' = temp_shifted
              rw [hts]
              rw [h_temp_spec_val] at h_temp
              split at h_sr
              · rename_i h_contra; omega
              · exact h_sr)
          -- specLoop (i+1) result' temp' = specLoop 0 0 a
          · rw [← hinv_state, specLoop_step _ _ _ _ _ _ hi1_lt_m]
            simp only []
            rw [h_bit_spec_val] at h_bit
            rw [h_temp_spec_val] at h_temp
            -- The if-then-else in specLoop_step matches the Aeneas branch
            -- because h_bit/h_temp determine which branch both take.
            -- Rewrite the spec's bit_set condition to use h_bit
            -- and will_overflow condition to use h_temp
            have h_bit_eq : (b.val / 2 ^ i1.val % 2 = 1) = (shifted_b.val % 2 = 1) := by
              rw [hshb]
            have h_temp_eq : (temp1.val / 2 ^ (m.val - 1) % 2 = 1) = (shifted_temp.val % 2 = 1) := by
              rw [hst]
            -- The specLoop step's if-then-else agrees with the Aeneas branches.
            -- h_bit and h_temp tell us which branches were taken.
            -- We need result' and temp' values to match the spec.
            -- Use congr to split into result and temp goals, then show each matches.
            have h_bit_iff : (b.val / 2 ^ i1.val % 2 = 1) ↔
              (shifted_b.val % 2 = 1) := by constructor <;> (intro; omega)
            have h_temp_iff : (temp1.val / 2 ^ (m.val - 1) % 2 = 1) ↔
              (shifted_temp.val % 2 = 1) := by constructor <;> (intro; omega)
            -- For the result: need to show the if-then-else evaluates correctly
            -- For the temp: need to show shift_reduce evaluates correctly
            -- Since both sides of the = are specLoop applied to the same args
            -- (just different expressions for result' and temp'), show args equal.
            congr 1
            · -- Result value
              try simp only [h_bit, ite_true, nat_xor_eq, UScalar.val_xor]
              try simp only [show ¬(b.val / 2 ^ i1.val % 2 = 1) from by omega, ite_false]
              try simp  -- catch degenerate cases like `if 0 = 1`
            · -- Temp value = shift_reduce
              unfold shift_reduce; simp only []
              try simp only [h_temp, ite_true, nat_xor_eq, UScalar.val_xor, hts]
              try simp only [show ¬(temp1.val / 2 ^ (m.val - 1) % 2 = 1) from by omega, ite_false, hts]
              try simp  -- catch degenerate cases
          -- measure decreases
          · show m.val - i_next.val < m.val - i1.val
            have : i_next.val = i1.val + 1 := by scalar_tac
            have : i1.val < m.val := by scalar_tac
            omega))
    · -- Loop exits: i1 ≥ m
      simp only [hlt, ite_false, spec, theta, wp_return]
      have hi1_ge : i1.val ≥ m.val := by scalar_tac
      -- specLoop at i1 ≥ m returns (result1, temp1)
      rw [specLoop_done _ _ _ _ _ _ (by omega)] at hinv_state
      -- hinv_state : (result1.val, temp1.val) = specLoop ... 0 0 a.val
      exact ⟨by exact hinv_state ▸ rfl, hinv_r⟩
  · -- Initial invariant: specLoop i 0 0 a = specLoop 0 0 a when (result, temp, i) = (0, a, 0)
    -- Wait, we need specLoop i.val result.val temp.val = specLoop 0 0 a.val
    -- This is provided by hstate directly
    exact ⟨hi, hr_bound, ht_bound, hstate⟩

/-! ## Top-level correctness -/

theorem gf2m_mul_raw_correct (a b : Std.U64) (m : Std.Usize) (poly : Std.U64)
    (hparams : ValidGf2mParams m poly)
    (ha : a.val < 2 ^ m.val) (hb : b.val < 2 ^ m.val) :
    ∃ r, gf2m.mul_raw.gf2m_mul_raw a b m poly = ok r ∧
      r.val = mul_raw_spec a.val b.val m.val poly.val ∧
      r.val < 2 ^ m.val := by
  unfold gf2m.mul_raw.gf2m_mul_raw
  by_cases ha0 : a = 0#u64
  · subst ha0; refine ⟨_, rfl, ?_, by positivity⟩
    show (0#u64 : Std.U64).val = mul_raw_spec (0#u64 : Std.U64).val b.val m.val poly.val
    unfold mul_raw_spec
    have h0 : (0#u64 : Std.U64).val = 0 := by native_decide
    simp only [h0, specLoop_a_zero, Nat.zero_mod]
  · simp only [ha0, ite_false]
    by_cases hb0 : b = 0#u64
    · subst hb0; simp only [ite_true]; refine ⟨_, rfl, ?_, by positivity⟩
      -- b = 0: mul_raw_spec a 0 ... = 0
      -- specLoop 0 m poly 0 0 a.val: since all bits of b=0 are clear,
      -- result stays 0 throughout. Use native_decide for small cases
      -- or prove by induction. For now, unfold directly.
      show (0#u64 : Std.U64).val = mul_raw_spec a.val (0#u64 : Std.U64).val m.val poly.val
      unfold mul_raw_spec
      have h0 : (0#u64 : Std.U64).val = 0 := by native_decide
      simp only [h0, specLoop_b_zero, Nat.zero_mod]
    · simp only [hb0, ite_false]
      -- Non-zero case: use loop_correct
      have hm_pos := hparams.m_pos
      have h_loop_inv : LoopInv b.val m.val poly.val a.val (0#u64, a, 0#usize) := by
        refine ⟨by scalar_tac, by positivity, ha, ?_⟩
        -- specLoop b m poly 0 0 a = specLoop b m poly 0 0 a (reflexivity!)
        rfl
      have h_loop := loop_correct a b m poly 0#u64 a 0#usize hparams ha h_loop_inv
      -- Get existential
      obtain ⟨loop_result, hloop_eq, hloop_val, hloop_bound⟩ := spec_imp_exists h_loop
      simp only [hloop_eq, bind_tc_ok]
      -- 1 <<< m
      have hm_lt_64 : m.val < 64 := by have := hparams.m_le; omega
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
      have h_sub : (mask_base - 1#u64) ⦃ r => r.val = 2 ^ m.val - 1 ⦄ := by
        progress as ⟨r, hr⟩
        rw [hmb_val] at hr; linarith
      obtain ⟨mask, hmask_eq, hmask_val⟩ := spec_imp_exists h_sub
      simp only [hmask_eq, bind_tc_ok]
      -- Final: loop_result &&& mask
      refine ⟨_, rfl, ?_, ?_⟩
      · -- Correctness: (loop_result &&& mask).val = mul_raw_spec
        rw [UScalar.val_and, hmask_val, and_mask_identity _ _ hloop_bound]
        unfold mul_raw_spec
        -- hloop_val : loop_result.val = (specLoop ...).1
        -- hloop_bound : loop_result.val < 2^m
        -- Goal: loop_result.val = (specLoop ...).1 % 2^m
        rw [hloop_val, Nat.mod_eq_of_lt (hloop_val ▸ hloop_bound)]
      · -- Bound
        rw [UScalar.val_and]; apply Nat.and_lt_two_pow
        rw [hmask_val]; exact Nat.sub_lt (by positivity) (by norm_num)

end Gf2mMulRaw
