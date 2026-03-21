/-
  Gf2Core.Proofs.Progress — Result elimination (progress) lemmas

  For each Aeneas-generated gfp function, proves it returns `ok` (not `fail`)
  when inputs satisfy ValidPrime and elements are in range [0, P).
  These lemmas let us extract pure functions from the Result monad.
-/
import Aeneas
import Gf2Core.Funs
import Gf2Core.Proofs.Defs
import Gf2Core.Proofs.ModArith

open Aeneas Aeneas.Std Result ControlFlow Error Aeneas.Std.WP
open gf2_core

set_option maxHeartbeats 1600000

noncomputable section

namespace FpProgress

/-! ## @[progress] lemmas for FunsExternal definitions

These let the `progress` tactic reason through our custom defs. -/

@[progress]
theorem overflowing_sub_spec (x y : Std.U64) :
    core.num.U64.overflowing_sub x y ⦃ r =>
      r.1.bv = x.bv - y.bv ∧ r.2 = decide (x.val < y.val) ⦄ := by
  simp only [core.num.U64.overflowing_sub, spec, theta, wp_return]; trivial

@[progress]
theorem wrapping_neg_spec (x : Std.U64) :
    core.num.U64.wrapping_neg x ⦃ r =>
      r = UScalar.wrapping_sub (⟨0#64⟩ : Std.U64) x ⦄ := by
  simp only [core.num.U64.wrapping_neg, spec, theta, wp_return]

/-! ## BitVec helpers for conditional subtraction pattern -/

theorem bv_allOnes_and {n : ℕ} (x : BitVec n) : BitVec.allOnes n &&& x = x := by
  ext i; simp [BitVec.getLsbD_and, BitVec.getLsbD_allOnes]

theorem bv_zero_and {n : ℕ} (x : BitVec n) : (0 : BitVec n) &&& x = 0 := by
  ext i; simp [BitVec.getLsbD_and]

/-- wrapping_add with zero is identity on val -/
theorem wrapping_add_zero_val (a : Std.U64) :
    (UScalar.wrapping_add a (⟨0#64⟩ : Std.U64)).val = a.val := by
  rw [UScalar.wrapping_add_val_eq]
  have h0 : (⟨0#64⟩ : Std.U64).val = 0 := by native_decide
  rw [h0, Nat.add_zero]
  exact Nat.mod_eq_of_lt a.hSize

/-- BitVec sub to Nat sub when x >= P -/
theorem bv_sub_toNat (x P : Std.U64) (h : P.val ≤ x.val) :
    (⟨x.bv - P.bv⟩ : Std.U64).val = x.val - P.val := by
  show (x.bv - P.bv).toNat = x.bv.toNat - P.bv.toNat
  exact BitVec.toNat_sub_of_le (show P.bv.toNat ≤ x.bv.toNat from h)

/-- The branchless conditional subtraction pattern used by mont_add, mont_sub, and redc.
    Given x < 2P, produces a value < P (either x or x - P). -/
theorem cond_sub_val (x P : Std.U64) (hx : x.val < 2 * P.val) :
    let result : Std.U64 := ⟨x.bv - P.bv⟩
    let borrow : Bool := decide (x.val < P.val)
    let i : Std.U64 := UScalar.cast_fromBool .U64 borrow
    let neg_i : Std.U64 := UScalar.wrapping_sub ⟨0#64⟩ i
    let correction : Std.U64 := neg_i &&& P
    (UScalar.wrapping_add result correction).val < P.val := by
  simp only
  by_cases h : x.val < P.val
  · simp only [h, decide_true]
    have : UScalar.cast_fromBool .U64 true = (⟨1#64⟩ : Std.U64) := by native_decide
    rw [this]
    have : UScalar.wrapping_sub (⟨0#64⟩ : Std.U64) (⟨1#64⟩ : Std.U64) =
           ⟨BitVec.allOnes 64⟩ := by native_decide
    rw [this]
    have : ((⟨BitVec.allOnes 64⟩ : Std.U64) &&& P) = P := by
      show (⟨BitVec.allOnes 64 &&& P.bv⟩ : UScalar .U64) = P
      congr 1; exact bv_allOnes_and P.bv
    rw [this]
    suffices (UScalar.wrapping_add (⟨x.bv - P.bv⟩ : Std.U64) P).val = x.val by rw [this]; exact h
    show (UScalar.wrapping_add (⟨x.bv - P.bv⟩ : Std.U64) P).bv.toNat = x.bv.toNat
    congr 1; change (x.bv - P.bv) + P.bv = x.bv; bv_omega
  · push_neg at h
    simp only [show ¬(x.val < P.val) from not_lt.mpr h, decide_false]
    have : UScalar.cast_fromBool .U64 false = (⟨0#64⟩ : Std.U64) := by native_decide
    rw [this]
    have : UScalar.wrapping_sub (⟨0#64⟩ : Std.U64) (⟨0#64⟩ : Std.U64) = (⟨0#64⟩ : Std.U64) := by
      native_decide
    rw [this]
    have : ((⟨0#64⟩ : Std.U64) &&& P) = (⟨0#64⟩ : Std.U64) := by
      show (⟨(0 : BitVec 64) &&& P.bv⟩ : UScalar .U64) = ⟨0#64⟩
      congr 1; exact bv_zero_and P.bv
    rw [this, wrapping_add_zero_val, bv_sub_toNat x P h]; omega

/-- The branchless conditional add-back pattern used by mont_sub.
    Given a, b < P and P ≤ 2^63, computes a - b mod P via wrapping subtraction
    and conditional add-back of P. -/
theorem cond_add_back_val (a b P : Std.U64)
    (ha : a.val < P.val) (hb : b.val < P.val) (hP : P.val ≤ 2 ^ 63) :
    let result : Std.U64 := ⟨a.bv - b.bv⟩
    let borrow : Bool := decide (a.val < b.val)
    let i : Std.U64 := UScalar.cast_fromBool .U64 borrow
    let neg_i : Std.U64 := UScalar.wrapping_sub ⟨0#64⟩ i
    let correction : Std.U64 := neg_i &&& P
    (UScalar.wrapping_add result correction).val < P.val := by
  simp only
  by_cases h : a.val < b.val
  · -- borrow = true, correction = P, result = wrapping(a - b)
    -- Final: wrapping_add (a wrapping- b) P
    -- At bv level: (a.bv - b.bv) + P.bv, and .toNat = P - (b - a) < P
    simp only [h, decide_true]
    have : UScalar.cast_fromBool .U64 true = (⟨1#64⟩ : Std.U64) := by native_decide
    rw [this]
    have : UScalar.wrapping_sub (⟨0#64⟩ : Std.U64) (⟨1#64⟩ : Std.U64) =
           ⟨BitVec.allOnes 64⟩ := by native_decide
    rw [this]
    have : ((⟨BitVec.allOnes 64⟩ : Std.U64) &&& P) = P := by
      show (⟨BitVec.allOnes 64 &&& P.bv⟩ : UScalar .U64) = P
      congr 1; exact bv_allOnes_and P.bv
    rw [this]
    -- Goal: (UScalar.wrapping_add ⟨a.bv - b.bv⟩ P).val < P.val
    -- bv_omega needs to know that .bv.toNat = .val for UScalar
    show (UScalar.wrapping_add (⟨a.bv - b.bv⟩ : Std.U64) P).bv.toNat < P.bv.toNat
    change ((a.bv - b.bv) + P.bv).toNat < P.bv.toNat
    have : a.bv.toNat = a.val := rfl
    have : b.bv.toNat = b.val := rfl
    have : P.bv.toNat = P.val := rfl
    bv_omega
  · -- borrow = false, correction = 0, result = a - b
    push_neg at h
    simp only [show ¬(a.val < b.val) from not_lt.mpr h, decide_false]
    have : UScalar.cast_fromBool .U64 false = (⟨0#64⟩ : Std.U64) := by native_decide
    rw [this]
    have : UScalar.wrapping_sub (⟨0#64⟩ : Std.U64) (⟨0#64⟩ : Std.U64) = (⟨0#64⟩ : Std.U64) := by
      native_decide
    rw [this]
    have : ((⟨0#64⟩ : Std.U64) &&& P) = (⟨0#64⟩ : Std.U64) := by
      show (⟨(0 : BitVec 64) &&& P.bv⟩ : UScalar .U64) = ⟨0#64⟩
      congr 1; exact bv_zero_and P.bv
    rw [this, wrapping_add_zero_val, bv_sub_toNat a b h]; omega

/-! ## Existential form helpers (for theorems not using progress tactic) -/

theorem wrapping_neg_ok (x : Std.U64) :
    ∃ r, core.num.U64.wrapping_neg x = ok r :=
  ⟨_, rfl⟩

theorem overflowing_sub_ok (x y : Std.U64) :
    ∃ r, core.num.U64.overflowing_sub x y = ok r :=
  ⟨_, rfl⟩

theorem wrapping_neg_val (x : Std.U64) :
    core.num.U64.wrapping_neg x = ok (UScalar.wrapping_sub (⟨0#64⟩ : Std.U64) x) := rfl

theorem overflowing_sub_val (x y : Std.U64) :
    core.num.U64.overflowing_sub x y =
      ok (⟨x.bv - y.bv⟩, decide (x.val < y.val)) := rfl

/-! ## compute_p_inv progress -/

/-- The P_INV computation terminates and produces a result for any input. -/
theorem compute_p_inv_loop_progress (p inv : Std.U64) (i : Std.I32)
    (hi : i.val ≤ 6) :
    ∃ r, gfp.montgomery.compute_p_inv_loop p inv i = ok r := by
  have hspec : gfp.montgomery.compute_p_inv_loop p inv i ⦃ fun _ => True ⦄ := by
    unfold gfp.montgomery.compute_p_inv_loop
    apply loop.spec
      (measure := fun ((_, i1) : Std.U64 × Std.I32) => (6 - i1.val).toNat)
      (inv := fun ((_, i1) : Std.U64 × Std.I32) => i1.val ≤ 6)
    · intro ⟨inv1, i1⟩ hinv
      simp only at hinv
      dsimp only
      by_cases hlt : i1 < 6#i32
      · simp only [hlt, ite_true]
        progress as ⟨i2, hi2⟩
        progress as ⟨i3, hi3⟩
        progress as ⟨inv2, hinv2⟩
        progress as ⟨i4, hi4⟩
        have hi1_lt : i1.val < 6 := by scalar_tac
        constructor
        · omega
        · show (6 - i4.val).toNat < (6 - i1.val).toNat; omega
      · simp only [hlt, ite_false, spec, theta, wp_return]
    · exact hi
  obtain ⟨r, hr, _⟩ := spec_imp_exists hspec
  exact ⟨r, hr⟩

theorem compute_p_inv_progress (p : Std.U64) :
    ∃ r, gfp.montgomery.compute_p_inv p = ok r := by
  unfold gfp.montgomery.compute_p_inv
  obtain ⟨inv, hinv⟩ := compute_p_inv_loop_progress p 1#u64 0#i32 (by decide)
  simp only [hinv, Bind.bind]
  exact wrapping_neg_ok inv

@[progress]
theorem p_inv_progress {P : Std.U64} :
    gfp.montgomery.MontConsts.P_INV P ⦃ fun _ => True ⦄ := by
  simp only [gfp.montgomery.MontConsts.P_INV]
  obtain ⟨r, hr⟩ := compute_p_inv_progress P
  rw [show spec = theta from rfl, hr]; simp [theta, wp_return]

/-! ## REDC progress -/

/-- REDC returns ok when inputs satisfy the Montgomery precondition.
    Precondition: t < P * 2^64, which ensures no U128 overflow in t + m*P. -/
@[progress]
theorem redc_progress {P : Std.U64} {t : Std.U128}
    (hP : ValidPrime P) (ht : t.val < P.val * 2 ^ 64) :
    gfp.montgomery.redc P t ⦃ r => r.val < P.val ⦄ := by
  unfold gfp.montgomery.redc
  progress as ⟨t_lo, ht_lo⟩
  progress as ⟨p_inv, _⟩
  progress as ⟨m, hm⟩
  progress as ⟨i1, hi1⟩
  progress as ⟨i2, hi2⟩
  progress as ⟨mp, hmp⟩
  progress as ⟨i3, hi3⟩
  · -- U128 add overflow: t + mp ≤ U128.max
    have : U128.max = 2^128 - 1 := by native_decide
    rw [this]
    have h_i1_val : i1.val = m.val := by rw [hi1]; exact U64.cast_U128_val_eq m
    have h_i2_val : i2.val = P.val := by rw [hi2]; exact U64.cast_U128_val_eq P
    have : m.val < 2^64 := m.hBounds
    have : mp.val ≤ (2^64 - 1) * 2^63 := by
      rw [hmp, h_i1_val, h_i2_val]
      exact Nat.mul_le_mul (by omega) hP.2.2
    have : t.val < 2^63 * 2^64 := by
      calc t.val < P.val * 2^64 := ht
        _ ≤ 2^63 * 2^64 := Nat.mul_le_mul_right _ hP.2.2
    have : (2^64 - 1) * (2:ℕ)^63 + 2^63 * 2^64 ≤ 2^128 := by norm_num
    omega
  progress as ⟨i4, hi4⟩
  progress as ⟨u, hu⟩
  progress as ⟨discr, hdiscr1, hdiscr2⟩
  progress as ⟨i5, hi5⟩
  progress as ⟨neg_i, hneg_i⟩
  progress as ⟨correction, hcorrection⟩
  -- Rewrite to match cond_sub_val pattern
  have h_result_eq : discr.1 = (⟨u.bv - P.bv⟩ : Std.U64) := by
    apply UScalar.val_eq_imp
    show discr.1.bv.toNat = (u.bv - P.bv).toNat; rw [hdiscr1]
  have h_corr_struct : correction = neg_i &&& P := by
    apply UScalar.val_eq_imp; exact hcorrection
  have cast_fromBool_val : ∀ (b : Bool),
      (UScalar.cast_fromBool .U64 b).val = b.toNat := by
    intro b; cases b <;> native_decide
  have h_i5_eq : i5 = UScalar.cast_fromBool .U64 discr.2 := by
    apply UScalar.val_eq_imp
    rw [hi5, cast_fromBool_val]
  rw [h_result_eq, h_corr_struct, hneg_i, h_i5_eq, hdiscr2]
  apply cond_sub_val
  -- Prove u.val < 2 * P.val via REDC bound chain
  have h_i1_val : i1.val = m.val := by rw [hi1]; exact U64.cast_U128_val_eq m
  have h_i2_val : i2.val = P.val := by rw [hi2]; exact U64.cast_U128_val_eq P
  have hmp_val : mp.val = m.val * P.val := by rw [hmp, h_i1_val, h_i2_val]
  have hP_pos : 0 < P.val := by omega
  have hmp_lt : mp.val < 2^64 * P.val := by
    rw [hmp_val]; exact Nat.mul_lt_mul_of_pos_right m.hBounds hP_pos
  have hi3_lt : i3.val < 2 * P.val * 2^64 := by rw [hi3]; omega
  have hi4_lt : i4.val < 2 * P.val := by
    rw [hi4, Nat.shiftRight_eq_div_pow]
    exact Nat.div_lt_of_lt_mul (by ring_nf; linarith)
  have : u.val = i4.val := by
    rw [hu]; exact UScalar.cast_val_mod_pow_of_inBounds_eq .U64 i4 (by
      have := hP.2.2; scalar_tac)
  rw [this]; exact hi4_lt

/-! ## to_mont / from_mont progress -/

@[progress]
theorem compute_r_mod_p_progress {P : Std.U64} (hP : ValidPrime P) :
    gfp.montgomery.compute_r_mod_p P ⦃ r => r.val < P.val ⦄ := by
  unfold gfp.montgomery.compute_r_mod_p
  progress as ⟨i, hi⟩         -- 1 <<< 64
  progress as ⟨i1, hi1⟩       -- cast U128 P
  progress as ⟨i2, hi2⟩       -- i % i1 (checked rem)
  · -- divisor nonzero: i1.val ≠ 0
    have : i1.val = P.val := by rw [hi1]; exact U64.cast_U128_val_eq P
    have : 1 < P.val := hP.2.1
    omega
  have hi_val : i.val = 2^64 := by rw [hi]; native_decide
  have hi1_val : i1.val = P.val := by rw [hi1]; exact U64.cast_U128_val_eq P
  have hi2_val : i2.val = 2^64 % P.val := by rw [hi2, hi_val, hi1_val]
  have hlt : i2.val < P.val := by
    rw [hi2_val]; exact Nat.mod_lt _ (by have := hP.2.1; omega)
  have : P.val ≤ 2^63 := hP.2.2
  have : (UScalar.cast .U64 i2).val = i2.val :=
    UScalar.cast_val_mod_pow_of_inBounds_eq .U64 i2 (by scalar_tac)
  rw [this]; exact hlt

@[progress]
theorem compute_r2_mod_p_progress {P : Std.U64} (hP : ValidPrime P) :
    gfp.montgomery.compute_r2_mod_p P ⦃ r => r.val < P.val ⦄ := by
  unfold gfp.montgomery.compute_r2_mod_p
  progress as ⟨i, hi⟩         -- compute_r_mod_p P: i.val < P.val
  progress as ⟨r, hr⟩         -- cast U128 i
  progress as ⟨i1, hi1⟩       -- r * r (checked U128 mul, auto-resolved)
  progress as ⟨i2, hi2⟩       -- cast U128 P
  progress as ⟨i3, hi3⟩       -- i1 % i2 (divisor nonzero auto-resolved)
  have hi2_val : i2.val = P.val := by rw [hi2]; exact U64.cast_U128_val_eq P
  have hi3_lt : i3.val < P.val := by
    rw [hi3, hi2_val]; exact Nat.mod_lt _ (by have := hP.2.1; omega)
  have : P.val ≤ 2^63 := hP.2.2
  have : (UScalar.cast .U64 i3).val = i3.val :=
    UScalar.cast_val_mod_pow_of_inBounds_eq .U64 i3 (by scalar_tac)
  rw [this]; exact hi3_lt

@[progress]
theorem r2_mod_p_progress {P : Std.U64} (hP : ValidPrime P) :
    gfp.montgomery.MontConsts.R2_MOD_P P ⦃ r => r.val < P.val ⦄ := by
  simp only [gfp.montgomery.MontConsts.R2_MOD_P]
  exact compute_r2_mod_p_progress hP

@[progress]
theorem from_mont_progress {P : Std.U64} {a : Std.U64}
    (hP : ValidPrime P) (ha : a.val < P.val) :
    gfp.montgomery.from_mont P a ⦃ r => r.val < P.val ⦄ := by
  unfold gfp.montgomery.from_mont
  progress as ⟨i, hi⟩         -- cast U128 a
  progress                     -- redc P i
  · -- redc precondition: i.val < P.val * 2^64
    have : i.val = a.val := by rw [hi]; exact U64.cast_U128_val_eq a
    have : 1 < P.val := hP.2.1
    nlinarith

@[progress]
theorem to_mont_progress {P : Std.U64} {a : Std.U64}
    (hP : ValidPrime P) (ha : a.val < P.val) :
    gfp.montgomery.to_mont P a ⦃ r => r.val < P.val ⦄ := by
  unfold gfp.montgomery.to_mont
  progress as ⟨i, hi⟩         -- cast U128 a
  progress as ⟨i1, hi1⟩       -- R2_MOD_P P: i1.val < P.val
  progress as ⟨i2, hi2⟩       -- cast U128 i1
  progress as ⟨i3, hi3⟩       -- i * i2 (checked U128 mul)
  progress                     -- redc P i3
  · -- redc precondition: i3.val < P.val * 2^64
    have hi_val : i.val = a.val := by rw [hi]; exact U64.cast_U128_val_eq a
    have hi2_val : i2.val = i1.val := by rw [hi2]; exact U64.cast_U128_val_eq i1
    rw [hi3, hi_val, hi2_val]
    calc a.val * i1.val < P.val * P.val :=
            Nat.mul_lt_mul_of_lt_of_lt ha hi1
      _ ≤ P.val * 2^63 := Nat.mul_le_mul_left _ hP.2.2
      _ < P.val * 2^64 := by have := hP.2.1; omega
  assumption

/-! ## Arithmetic operation progress -/

theorem mont_add_progress {P : Std.U64} {a b : Std.U64}
    (hP : ValidPrime P) (ha : a.val < P.val) (hb : b.val < P.val) :
    ∃ r, gfp.montgomery.mont_add P a b = ok r ∧ r.val < P.val := by
  have hspec : gfp.montgomery.mont_add P a b ⦃ r => r.val < P.val ⦄ := by
    unfold gfp.montgomery.mont_add
    progress as ⟨sum, hsum⟩
    · have := hP.2.2; scalar_tac
    progress as ⟨discr, hdiscr1, hdiscr2⟩
    progress as ⟨i, hi⟩
    progress as ⟨neg_i, hneg_i⟩
    progress as ⟨correction, hcorrection⟩
    have hsum_lt : sum.val < 2 * P.val := by omega
    have h_result_eq : discr.1 = (⟨sum.bv - P.bv⟩ : Std.U64) := by
      apply UScalar.val_eq_imp
      show discr.1.bv.toNat = (sum.bv - P.bv).toNat; rw [hdiscr1]
    have h_corr_struct : correction = neg_i &&& P := by
      apply UScalar.val_eq_imp; exact hcorrection
    have cast_fromBool_val : ∀ (b : Bool),
        (UScalar.cast_fromBool .U64 b).val = b.toNat := by
      intro b; cases b <;> native_decide
    have h_i_eq : i = UScalar.cast_fromBool .U64 discr.2 := by
      apply UScalar.val_eq_imp
      rw [hi, cast_fromBool_val]
    rw [h_result_eq, h_corr_struct, hneg_i, h_i_eq, hdiscr2]
    exact cond_sub_val sum P hsum_lt
  exact spec_imp_exists hspec

theorem mont_sub_progress {P : Std.U64} {a b : Std.U64}
    (hP : ValidPrime P) (ha : a.val < P.val) (hb : b.val < P.val) :
    ∃ r, gfp.montgomery.mont_sub P a b = ok r ∧ r.val < P.val := by
  have hspec : gfp.montgomery.mont_sub P a b ⦃ r => r.val < P.val ⦄ := by
    unfold gfp.montgomery.mont_sub
    progress as ⟨discr, hdiscr1, hdiscr2⟩
    progress as ⟨i, hi⟩
    progress as ⟨neg_i, hneg_i⟩
    progress as ⟨correction, hcorrection⟩
    have h_result_eq : discr.1 = (⟨a.bv - b.bv⟩ : Std.U64) := by
      apply UScalar.val_eq_imp
      show discr.1.bv.toNat = (a.bv - b.bv).toNat; rw [hdiscr1]
    have h_corr_struct : correction = neg_i &&& P := by
      apply UScalar.val_eq_imp; exact hcorrection
    have cast_fromBool_val : ∀ (bl : Bool),
        (UScalar.cast_fromBool .U64 bl).val = bl.toNat := by
      intro bl; cases bl <;> native_decide
    have h_i_eq : i = UScalar.cast_fromBool .U64 discr.2 := by
      apply UScalar.val_eq_imp
      rw [hi, cast_fromBool_val]
    rw [h_result_eq, h_corr_struct, hneg_i, h_i_eq, hdiscr2]
    exact cond_add_back_val a b P ha hb hP.2.2
  exact spec_imp_exists hspec

theorem mul_progress {P : Std.U64} {a b : Std.U64}
    (hP : ValidPrime P) (ha : a.val < P.val) (hb : b.val < P.val)
    (hP2 : P.val ≠ 2) :
    ∃ r, gfp.Fp.Insts.CoreOpsArithMulFpFp.mul (P := P) a b = ok r ∧
      r.val < P.val := by
  have hspec : gfp.Fp.Insts.CoreOpsArithMulFpFp.mul (P := P) a b
      ⦃ r => r.val < P.val ⦄ := by
    unfold gfp.Fp.Insts.CoreOpsArithMulFpFp.mul
    split
    case isTrue h => exfalso; exact hP2 (by subst h; native_decide)
    case isFalse h =>
      progress as ⟨i, hi⟩       -- cast U128 a
      progress as ⟨i1, hi1⟩     -- cast U128 b
      progress as ⟨i2, hi2⟩     -- i * i1 (checked U128 mul)
      progress as ⟨i3, hi3⟩     -- redc P i2
      · -- redc precondition: i2.val < P.val * 2^64
        have hi_val : i.val = a.val := by rw [hi]; exact U64.cast_U128_val_eq a
        have hi1_val : i1.val = b.val := by rw [hi1]; exact U64.cast_U128_val_eq b
        rw [hi2, hi_val, hi1_val]
        calc a.val * b.val < P.val * P.val :=
                Nat.mul_lt_mul_of_lt_of_lt ha hb
          _ ≤ P.val * 2^63 := Nat.mul_le_mul_left _ hP.2.2
          _ < P.val * 2^64 := by have := hP.2.1; omega
      exact hi3
  exact spec_imp_exists hspec

theorem neg_progress {P : Std.U64} {a : Std.U64}
    (hP : ValidPrime P) (ha : a.val < P.val) :
    ∃ r, gfp.Fp.Insts.CoreOpsArithNegFp.neg (P := P) a = ok r ∧
      r.val < P.val := by
  unfold gfp.Fp.Insts.CoreOpsArithNegFp.neg
  split
  case isTrue h => exact ⟨a, rfl, ha⟩
  case isFalse h =>
    have h0val : (⟨0#64⟩ : U64).val = 0 := by decide
    have hane : a.val ≠ 0 := by
      intro heq; apply h
      exact UScalar.val_eq_imp a ⟨0#64⟩ (by rw [h0val]; exact heq)
    simp only [HSub.hSub, Sub.sub, UScalar.sub,
               show ¬ (P.val < a.val) from by omega, ite_false]
    refine ⟨_, rfl, ?_⟩
    simp only [UScalar.val, BitVec.toNat_ofNat, Nat.sub_eq]
    change (P.val - a.val) % 2 ^ UScalarTy.U64.numBits < P.val
    have hbits : UScalarTy.U64.numBits = 64 := by decide
    rw [hbits]
    have : P.val - a.val < 2 ^ 64 := by have := hP.2.2; omega
    rw [Nat.mod_eq_of_lt this]
    omega

/-! ## Fp.new and Fp.value progress -/

theorem fp_new_progress {P : Std.U64}
    (hP : ValidPrime P) (v : Std.U64) (hP2 : P.val ≠ 2) :
    ∃ r, gfp.Fp.new P v = ok r ∧ r.val < P.val := by
  have hspec : gfp.Fp.new P v ⦃ r => r.val < P.val ⦄ := by
    unfold gfp.Fp.new
    progress   -- VALIDATED P
    progress as ⟨reduced, hreduced⟩  -- v % P
    · have := hP.2.1; omega
    have hne : ¬(P = 2#u64) := by
      intro h; exact hP2 (by subst h; native_decide)
    simp only [hne, ite_false]
    progress  -- to_mont P reduced
    · rw [hreduced]; exact Nat.mod_lt _ (by have := hP.2.1; omega)
    assumption
  exact spec_imp_exists hspec

theorem fp_value_progress {P : Std.U64} {a : Std.U64}
    (hP : ValidPrime P) (ha : a.val < P.val) (hP2 : P.val ≠ 2) :
    ∃ r, gfp.Fp.value (P := P) a = ok r ∧ r.val < P.val := by
  have hspec : gfp.Fp.value (P := P) a ⦃ r => r.val < P.val ⦄ := by
    unfold gfp.Fp.value
    split
    case isTrue h => exfalso; exact hP2 (by subst h; native_decide)
    case isFalse h => exact from_mont_progress hP ha
  exact spec_imp_exists hspec

/-! ## max_unreduced_additions progress -/

@[progress]
theorem max_unreduced_additions_progress {P : Std.U64} (hP : ValidPrime P) :
    gfp.Fp.Insts.Gf2_coreFieldTraitsFiniteFieldU64U128.max_unreduced_additions P
    ⦃ fun _ => True ⦄ := by
  unfold gfp.Fp.Insts.Gf2_coreFieldTraitsFiniteFieldU64U128.max_unreduced_additions
  progress as ⟨i, hi⟩          -- cast U128 P
  have hi_val : i.val = P.val := by rw [hi]; exact U64.cast_U128_val_eq P
  progress as ⟨i1, hi1, _⟩     -- i - 1#u128
  · have := hP.2.1; have := hi_val; scalar_tac
  progress as ⟨i2, hi2⟩        -- cast U128 P
  have hi2_val : i2.val = P.val := by rw [hi2]; exact U64.cast_U128_val_eq P
  progress as ⟨i3, hi3, _⟩     -- i2 - 1#u128; auto-closed
  progress as ⟨max_product, hmp⟩  -- i1 * i3
  · -- (P-1) * (P-1) ≤ U128.max
    have h1val : (1#u128 : Std.U128).val = 1 := by native_decide
    have hi1_val : i1.val = P.val - 1 := by omega
    have hi3_val : i3.val = P.val - 1 := by omega
    rw [hi1_val, hi3_val]
    exact MontArith.p_minus_one_sq_le_u128_max hP
  by_cases hmp0 : max_product = 0#u128
  · simp [hmp0, spec, theta, wp_return]
  · simp only [show ¬(max_product = 0#u128) from hmp0, ite_false]
    progress as ⟨k, hk⟩       -- U128.MAX / max_product
    progress as ⟨i4, hi4⟩     -- cast U128 Usize.MAX
    by_cases hgt : k > i4
    · simp [hgt, spec, theta, wp_return]
    · simp [hgt, spec, theta, wp_return]

/-! ## mod_pow_mont progress -/

/-- Helper: the redc precondition holds for products of values less than P. -/
lemma redc_precond {P : Std.U64} {i3 : Std.U128} {a b : Std.U64}
    (hP : ValidPrime P) (ha : a.val < P.val) (hb : b.val < P.val)
    (hi3 : i3.val = a.val * b.val) :
    i3.val < P.val * 2 ^ 64 := by
  rw [hi3]
  calc a.val * b.val < P.val * P.val :=
          Nat.mul_lt_mul_of_lt_of_lt ha hb
    _ ≤ P.val * 2 ^ 63 := Nat.mul_le_mul_left _ hP.2.2
    _ < P.val * 2 ^ 64 := by have := hP.2.1; omega

@[progress]
theorem mod_pow_mont_loop_progress {P : Std.U64}
    (hP : ValidPrime P)
    (base exp result : Std.U64)
    (hb : base.val < P.val) (hr : result.val < P.val) :
    gfp.montgomery.mod_pow_mont_loop P base exp result
    ⦃ r => r.val < P.val ⦄ := by
  unfold gfp.montgomery.mod_pow_mont_loop
  apply loop.spec (γ := ℕ)
    (measure := fun ((_, exp1, _) : Std.U64 × Std.U64 × Std.U64) => exp1.val)
    (inv := fun ((base1, _, result1) : Std.U64 × Std.U64 × Std.U64) =>
      base1.val < P.val ∧ result1.val < P.val)
  · intro ⟨base1, exp1, result1⟩ ⟨hb1, hr1⟩
    dsimp only
    by_cases hgt : exp1 > 0#u64
    · simp only [hgt, ite_true, Std.lift, bind_tc_ok]
      have hexp1_pos : 0 < exp1.val := by scalar_tac
      -- Split on if (exp1 &&& 1) = 1
      split
      · -- Odd: multiply result by base
        progress as ⟨prod, hprod⟩  -- checked U128 mul
        progress as ⟨result2, hresult2⟩  -- redc P prod
        · -- redc precondition: prod.val < P.val * 2^64
          have h1 := U64.cast_U128_val_eq result1
          have h2 := U64.cast_U128_val_eq base1
          exact redc_precond hP hr1 hb1 (by simp only [hprod, h1, h2])
        progress as ⟨exp2, hexp2_val, _⟩  -- shift
        have hexp2_lt : exp2.val < exp1.val := by
          simp [hexp2_val, Nat.shiftRight_eq_div_pow]
          exact Nat.div_lt_self hexp1_pos (by norm_num)
        split
        · -- exp2 > 0: square base
          progress as ⟨sq, hsq⟩
          progress as ⟨base2, hbase2⟩
          · have hv := U64.cast_U128_val_eq base1
            exact redc_precond hP hb1 hb1 (by simp only [hsq, hv])
          exact ⟨hbase2, hresult2, hexp2_lt⟩
        · -- exp2 = 0: base unchanged
          exact ⟨hb1, hresult2, hexp2_lt⟩
      · -- Even: result unchanged
        progress as ⟨exp2, hexp2_val, _⟩  -- shift
        have hexp2_lt : exp2.val < exp1.val := by
          simp [hexp2_val, Nat.shiftRight_eq_div_pow]
          exact Nat.div_lt_self hexp1_pos (by norm_num)
        split
        · -- exp2 > 0: square base
          progress as ⟨sq, hsq⟩
          progress as ⟨base2, hbase2⟩
          · have hv := U64.cast_U128_val_eq base1
            exact redc_precond hP hb1 hb1 (by simp only [hsq, hv])
          exact ⟨hbase2, hr1, hexp2_lt⟩
        · -- exp2 = 0: base unchanged
          exact ⟨hb1, hr1, hexp2_lt⟩
    · -- exp = 0: done
      simp only [hgt, ite_false, spec, theta, wp_return]
      exact hr1
  · exact ⟨hb, hr⟩

theorem mod_pow_mont_progress {P : Std.U64}
    (hP : ValidPrime P)
    (base exp : Std.U64) (hb : base.val < P.val) :
    ∃ r, gfp.montgomery.mod_pow_mont P base exp = ok r ∧ r.val < P.val := by
  have hspec : gfp.montgomery.mod_pow_mont P base exp ⦃ r => r.val < P.val ⦄ := by
    unfold gfp.montgomery.mod_pow_mont
    simp only [gfp.montgomery.MontConsts.R_MOD_P]
    progress as ⟨rmod, hrmod⟩  -- compute_r_mod_p
    exact mod_pow_mont_loop_progress hP base exp rmod hb hrmod
  exact spec_imp_exists hspec

/-! ## Option.expect progress -/

@[progress]
theorem option_expect_some_progress {T : Type} (x : T) (msg : Str) :
    core.option.Option.expect (some x) msg ⦃ r => r = x ⦄ := by
  simp only [core.option.Option.expect, spec, theta, wp_return]

/-! ## inv / div progress -/

theorem inv_progress {P : Std.U64} {self : Std.U64}
    (hP : ValidPrime P) (hP2 : P.val ≠ 2) (hself : self.val < P.val) (hne : self.val ≠ 0) :
    ∃ r, gfp.Fp.Insts.Gf2_coreFieldTraitsFiniteFieldU64U128.inv (P := P) self
      = ok (some r) ∧ r.val < P.val := by
  unfold gfp.Fp.Insts.Gf2_coreFieldTraitsFiniteFieldU64U128.inv
  have h0 : ¬(self = 0#u64) := by
    intro h; apply hne; have := congrArg UScalar.val h; simpa using this
  have hP2u : ¬(P = 2#u64) := by
    intro h; exact hP2 (by subst h; native_decide)
  simp only [h0, ite_false, hP2u]
  -- P - 2 succeeds (P ≥ 3)
  have hP_ge3 : (2#u64 : Std.U64).val ≤ P.val := by
    have h2val : (2#u64 : Std.U64).val = 2 := by native_decide
    rcases Nat.Prime.eq_two_or_odd hP.1 with h2 | hodd
    · exact absurd h2 hP2
    · rw [h2val]; omega
  have hsub : ∃ e, P - (2#u64 : Std.U64) = ok e := by
    simp only [HSub.hSub, Sub.sub, UScalar.sub, show ¬(P.val < (2#u64 : Std.U64).val) from by omega, ite_false]
    exact ⟨_, rfl⟩
  obtain ⟨e, he_eq⟩ := hsub
  simp only [he_eq, bind_tc_ok]
  -- mod_pow_mont returns ok with result < P
  obtain ⟨r, hr_eq, hr_lt⟩ := mod_pow_mont_progress hP self e hself
  simp only [hr_eq, bind_tc_ok]
  exact ⟨r, rfl, hr_lt⟩

theorem div_progress {P : Std.U64} {self rhs : Std.U64}
    (hP : ValidPrime P) (hP2 : P.val ≠ 2)
    (hself : self.val < P.val) (hrhs : rhs.val < P.val) (hne : rhs.val ≠ 0) :
    ∃ r, gfp.Fp.Insts.CoreOpsArithDivFpFp.div (P := P) self rhs = ok r ∧ r.val < P.val := by
  unfold gfp.Fp.Insts.CoreOpsArithDivFpFp.div
  obtain ⟨inv_r, hinv_eq, hinv_lt⟩ := inv_progress hP hP2 hrhs hne
  simp only [hinv_eq, bind_tc_ok]
  -- Option.expect (some inv_r)
  simp only [core.option.Option.expect, bind_tc_ok]
  -- mul
  exact mul_progress hP hself hinv_lt hP2

end FpProgress

end
