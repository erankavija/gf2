/-
  Gf2Core.Proofs.Gf2mInverse — Correctness proofs for gf2m_pow_raw and gf2m_inverse_raw

  Proves:
  1. gf2m_pow_raw correctly computes pow_raw_spec (loop invariant proof)
  2. gf2m_pow_raw result is a valid field element (< 2^m)
  3. gf2m_inverse_raw correctly computes inverse_raw_spec
  4. inverse(a) * a = 1 for nonzero a (Fermat's little theorem — axiom)
-/
import Aeneas
import Gf2Core.Funs
import Gf2Core.Proofs.Gf2mDefs
import Gf2Core.Proofs.Gf2mProgress
import Gf2Core.Proofs.Gf2mMulRaw

open Aeneas Aeneas.Std Result ControlFlow Error Aeneas.Std.WP
open gf2_core Gf2mSpec

set_option maxHeartbeats 51200000

noncomputable section

namespace Gf2mInverse

/-! ## Helper lemmas -/

private lemma pow_loop_spec_step (base exp result m poly : ℕ) (hexp : 0 < exp) :
    pow_loop_spec base exp result m poly =
      let result' := if exp % 2 = 1 then mul_raw_spec result base m poly else result
      let exp' := exp / 2
      let base' := if exp' > 0 then mul_raw_spec base base m poly else base
      pow_loop_spec base' exp' result' m poly := by
  rw [pow_loop_spec]; simp [show ¬(exp = 0) from by omega]

private lemma pow_loop_spec_zero (base result m poly : ℕ) :
    pow_loop_spec base 0 result m poly = result := by
  rw [pow_loop_spec]; simp

/-! ## Pow loop correctness -/

/-- The pow loop invariant: values match the spec at every iteration. -/
private def PowLoopInv (init_base init_exp m_val poly_val : ℕ)
    (state : Std.U64 × Std.U64 × Std.U64) : Prop :=
  let (base, exp, result) := state
  result.val < 2 ^ m_val ∧
  base.val < 2 ^ m_val ∧
  pow_loop_spec base.val exp.val result.val m_val poly_val =
    pow_loop_spec init_base init_exp 1 m_val poly_val

/-- The pow_raw loop computes pow_loop_spec. -/
theorem pow_loop_correct
    (init_base : Std.U64) (base exp : Std.U64) (m : Std.Usize) (poly : Std.U64)
    (result : Std.U64)
    (hparams : ValidGf2mParams m poly)
    (hinv : PowLoopInv init_base.val exp.val m.val poly.val (base, exp, result)) :
    gf2m.mul_raw.gf2m_pow_raw_loop base exp m poly result
    ⦃ r => r.val = pow_loop_spec init_base.val exp.val 1 m.val poly.val ∧
            r.val < 2 ^ m.val ⦄ := by
  obtain ⟨hr_bound, hb_bound, hstate⟩ := hinv
  unfold gf2m.mul_raw.gf2m_pow_raw_loop
  apply loop.spec (γ := ℕ)
    (measure := fun ((_, exp1, _) : Std.U64 × Std.U64 × Std.U64) => exp1.val)
    (inv := PowLoopInv init_base.val exp.val m.val poly.val)
  · intro ⟨base1, exp1, result1⟩ ⟨hr1, hb1, hst1⟩
    dsimp only
    by_cases hgt : exp1 > 0#u64
    · simp only [hgt, ite_true, Std.lift, bind_tc_ok]
      have hexp1_pos : 0 < exp1.val := by scalar_tac
      -- Split on if (exp & 1) != 0: result conditional
      split
      · -- exp & 1 != 0 → multiply result by base
        rename_i h_bit; simp [bne_iff_ne] at h_bit
        have h_bit_mod : exp1.val % 2 = 1 := by
          have : (exp1 &&& 1#u64).val = exp1.val % 2 := by
            rw [UScalar.val_and]; simp [Nat.and_one_is_mod]
          omega
        have hmul := Gf2mMulRaw.gf2m_mul_raw_correct result1 base1 m poly hparams hr1 hb1
        obtain ⟨r, hr_eq, hr_val, hr_lt⟩ := hmul
        simp only [hr_eq, bind_tc_ok]
        progress as ⟨exp2, hexp2_val, _⟩
        have hexp2_eq : exp2.val = exp1.val / 2 := by
          simp [hexp2_val, Nat.shiftRight_eq_div_pow]
        have hexp2_lt : exp2.val < exp1.val := by
          rw [hexp2_eq]; exact Nat.div_lt_self hexp1_pos (by norm_num)
        split
        · -- exp2 > 0: square base
          have hsq := Gf2mMulRaw.gf2m_mul_raw_correct base1 base1 m poly hparams hb1 hb1
          obtain ⟨r2, hr2_eq, hr2_val, hr2_lt⟩ := hsq
          simp only [hr2_eq, bind_tc_ok]
          refine And.intro ⟨hr_lt, hr2_lt, ?_⟩ hexp2_lt
          rw [← hst1, pow_loop_spec_step _ _ _ _ _ hexp1_pos]
          simp only [h_bit_mod, ite_true, hexp2_eq, show exp1.val / 2 > 0 from by scalar_tac, ite_true]
          congr 1 <;> assumption
        · -- exp2 = 0
          refine And.intro ⟨hr_lt, hb1, ?_⟩ hexp2_lt
          rw [← hst1, pow_loop_spec_step _ _ _ _ _ hexp1_pos]
          simp only [h_bit_mod, ite_true, hexp2_eq, show ¬(exp1.val / 2 > 0) from by scalar_tac, ite_false, hr_val]
      · -- exp & 1 = 0 → result unchanged
        rename_i h_bit; simp [bne_iff_ne] at h_bit
        have h_bit_mod : ¬(exp1.val % 2 = 1) := by
          have : (exp1 &&& 1#u64).val = exp1.val % 2 := by
            rw [UScalar.val_and]; simp [Nat.and_one_is_mod]
          omega
        progress as ⟨exp2, hexp2_val, _⟩
        have hexp2_eq : exp2.val = exp1.val / 2 := by
          simp [hexp2_val, Nat.shiftRight_eq_div_pow]
        have hexp2_lt : exp2.val < exp1.val := by
          rw [hexp2_eq]; exact Nat.div_lt_self hexp1_pos (by norm_num)
        split
        · -- exp2 > 0: square base
          have hsq := Gf2mMulRaw.gf2m_mul_raw_correct base1 base1 m poly hparams hb1 hb1
          obtain ⟨r2, hr2_eq, hr2_val, hr2_lt⟩ := hsq
          simp only [hr2_eq, bind_tc_ok]
          refine And.intro ⟨hr1, hr2_lt, ?_⟩ hexp2_lt
          rw [← hst1, pow_loop_spec_step _ _ _ _ _ hexp1_pos]
          simp only [h_bit_mod, ite_false, hexp2_eq, show exp1.val / 2 > 0 from by scalar_tac, ite_true, hr2_val]
        · -- exp2 = 0
          refine And.intro ⟨hr1, hb1, ?_⟩ hexp2_lt
          rw [← hst1, pow_loop_spec_step _ _ _ _ _ hexp1_pos]
          simp only [h_bit_mod, ite_false, hexp2_eq, show ¬(exp1.val / 2 > 0) from by scalar_tac, ite_false]
    · -- exp = 0: done
      simp only [hgt, ite_false, spec, theta, wp_return]
      have hexp0 : exp1.val = 0 := by scalar_tac
      constructor
      · rw [← hst1, hexp0, pow_loop_spec_zero]
      · exact hr1
  · exact ⟨hr_bound, hb_bound, hstate⟩

/-! ## Top-level correctness theorems -/

/-- gf2m_pow_raw correctly computes pow_raw_spec and produces a valid field element. -/
theorem gf2m_pow_raw_correct
    (base exp : Std.U64) (m : Std.Usize) (poly : Std.U64)
    (hparams : ValidGf2mParams m poly)
    (hb : base.val < 2 ^ m.val) :
    ∃ r, gf2m.mul_raw.gf2m_pow_raw base exp m poly = ok r ∧
      r.val = pow_raw_spec base.val exp.val m.val poly.val ∧
      r.val < 2 ^ m.val := by
  unfold gf2m.mul_raw.gf2m_pow_raw
  have h1_lt : (1#u64 : Std.U64).val < 2 ^ m.val := by
    have := hparams.m_pos; exact Nat.one_lt_two_pow_iff.mpr (by omega)
  have hinv : PowLoopInv base.val exp.val m.val poly.val (base, exp, 1#u64) :=
    ⟨h1_lt, hb, rfl⟩
  have h := pow_loop_correct base base exp m poly 1#u64 hparams hinv
  obtain ⟨r, hr_eq, hr_val, hr_lt⟩ := spec_imp_exists h
  exact ⟨r, hr_eq, hr_val, hr_lt⟩

/-- gf2m_inverse_raw correctly computes inverse_raw_spec. -/
theorem gf2m_inverse_raw_correct
    (a : Std.U64) (m : Std.Usize) (poly : Std.U64)
    (hparams : ValidGf2mParams m poly)
    (ha : a.val < 2 ^ m.val) :
    ∃ r, gf2m.mul_raw.gf2m_inverse_raw a m poly = ok r ∧
      r.val = inverse_raw_spec a.val m.val poly.val ∧
      r.val < 2 ^ m.val := by
  unfold gf2m.mul_raw.gf2m_inverse_raw
  by_cases ha0 : a = 0#u64
  · subst ha0; refine ⟨_, rfl, ?_, by positivity⟩
    simp [inverse_raw_spec]
  · simp only [ha0, ite_false]
    have hm_lt_64 : m.val < 64 := by have := hparams.m_le; omega
    have hm_pos := hparams.m_pos
    -- 1 <<< m → 2^m
    have h_shl : (1#u64 <<< m) ⦃ r => r.val = 2 ^ m.val ⦄ := by
      progress as ⟨r, hr_val, _⟩
      simp only [Nat.shiftLeft_eq, one_mul] at hr_val
      have h_lt : 2 ^ m.val < Std.U64.size := by
        calc 2 ^ m.val ≤ 2 ^ 63 := Nat.pow_le_pow_right (by norm_num) hparams.m_le
          _ < Std.U64.size := by native_decide
      rw [Nat.mod_eq_of_lt h_lt] at hr_val; exact hr_val
    obtain ⟨shl_r, hshl_eq, hshl_val⟩ := spec_imp_exists h_shl
    simp only [hshl_eq, bind_tc_ok]
    -- shl_r - 2 → 2^m - 2
    have hshl_ge2 : (2#u64 : Std.U64).val ≤ shl_r.val := by
      rw [hshl_val]
      calc 2 = 2 ^ 1 := by norm_num
        _ ≤ 2 ^ m.val := Nat.pow_le_pow_right (by norm_num) hm_pos
    have h_sub : (shl_r - 2#u64) ⦃ r => r.val = 2 ^ m.val - 2 ⦄ := by
      progress as ⟨r, hr⟩
      rw [hshl_val] at hr; linarith
    obtain ⟨exp, hexp_eq, hexp_val⟩ := spec_imp_exists h_sub
    simp only [hexp_eq, bind_tc_ok]
    -- pow_raw
    have hpow := gf2m_pow_raw_correct a exp m poly hparams ha
    obtain ⟨r, hr_eq, hr_val, hr_lt⟩ := hpow
    refine ⟨r, hr_eq, ?_, hr_lt⟩
    unfold inverse_raw_spec
    have ha_ne : ¬(a.val = 0) := by scalar_tac
    simp only [ha_ne, ite_false]
    rw [hr_val]; unfold pow_raw_spec; congr 1

/-! ## Inverse algebraic identity -/

/-- Fermat's little theorem for GF(2^m) at the spec level.
    For valid parameters and nonzero a < 2^m:
      mul_raw_spec(pow_raw_spec(a, 2^m-2, m, poly), a, m, poly) = 1.

    This well-known algebraic identity states that every nonzero element
    of GF(2^m) has multiplicative order dividing 2^m - 1. A full proof
    requires formalizing the multiplicative group structure of GF(2^m). -/
axiom fermat_little_gf2m (a m poly : ℕ) (hm : 0 < m)
    (ha : 0 < a) (ha_lt : a < 2 ^ m)
    (hp_high : 2 ^ m ≤ poly) (hp_bound : poly < 2 ^ (m + 1)) :
    mul_raw_spec (pow_raw_spec a (2 ^ m - 2) m poly) a m poly = 1

/-- Inverse correctness: for nonzero a, mul_raw(inverse(a), a) = 1.
    Combines proven code-spec equivalence with Fermat's little theorem axiom. -/
theorem gf2m_inverse_mul_one
    (a : Std.U64) (m : Std.Usize) (poly : Std.U64)
    (hparams : ValidGf2mParams m poly)
    (ha : a.val ≠ 0) (ha_lt : a.val < 2 ^ m.val) :
    ∃ inv r, gf2m.mul_raw.gf2m_inverse_raw a m poly = ok inv ∧
      gf2m.mul_raw.gf2m_mul_raw inv a m poly = ok r ∧
      r.val = 1 := by
  have hinv := gf2m_inverse_raw_correct a m poly hparams ha_lt
  obtain ⟨inv, hinv_eq, hinv_spec, hinv_lt⟩ := hinv
  have hmul := Gf2mMulRaw.gf2m_mul_raw_correct inv a m poly hparams hinv_lt ha_lt
  obtain ⟨r, hr_eq, hr_val, _⟩ := hmul
  refine ⟨inv, r, hinv_eq, hr_eq, ?_⟩
  rw [hr_val]
  unfold inverse_raw_spec at hinv_spec
  simp only [ha, ite_false] at hinv_spec
  rw [hinv_spec]
  exact fermat_little_gf2m a.val m.val poly.val hparams.m_pos
    (by omega) ha_lt hparams.poly_high hparams.poly_bound

end Gf2mInverse
