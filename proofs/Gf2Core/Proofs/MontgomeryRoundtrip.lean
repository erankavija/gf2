/-
  Gf2Core.Proofs.MontgomeryRoundtrip — Flagship roundtrip theorem

  Proves that the *actual Rust code* (via Aeneas extraction) correctly
  roundtrips through Montgomery form: from_mont(to_mont(a)) = a.

  Architecture:
  - `p_inv_value_spec` is the single key lemma — it states that the
    Newton iteration computes pinv * P ≡ -1 (mod 2^64).
  - `redc_value_spec` is fully proved from `p_inv_value_spec`, establishing
    that REDC satisfies r · R ≡ t (mod P).
  - `compute_r_mod_p_value`, `compute_r2_mod_p_value`, `r2_mod_p_value` track
    the R² computation values through the monadic chain.
  - `from_mont_value`, `to_mont_value` are fully proved from `redc_value_spec`.
  - `montgomery_roundtrip` chains the above and cancels R² via coprimality.
-/
import Aeneas
import Gf2Core.Funs
import Gf2Core.Proofs.Defs
import Gf2Core.Proofs.Progress
import Gf2Core.Proofs.ModArith

open Aeneas Aeneas.Std Result ControlFlow Error Aeneas.Std.WP
open gf2_core

set_option maxHeartbeats 1600000

noncomputable section

namespace MontRoundtrip

/-- a * (b % n) % n = a * b % n -/
private lemma mul_mod_mod_right (a b n : ℕ) : a * (b % n) % n = a * b % n := by
  conv_lhs => rw [Nat.mul_mod, Nat.mod_mod]; rw [← Nat.mul_mod]

/-- (a % n) * b % n = a * b % n -/
private lemma mod_mul_left (a b n : ℕ) : (a % n) * b % n = a * b % n := by
  rw [Nat.mul_mod (a % n) b n, Nat.mod_mod_of_dvd _ (dvd_refl n), ← Nat.mul_mod]

/-! ## Value-level lemmas for R² computation -/

/-- compute_r_mod_p returns 2^64 mod P -/
private theorem compute_r_mod_p_value {P : Std.U64} (hP : ValidPrime P) :
    ∃ r, gfp.montgomery.compute_r_mod_p P = ok r ∧ r.val = 2 ^ 64 % P.val := by
  have hspec : gfp.montgomery.compute_r_mod_p P ⦃ r => r.val = 2 ^ 64 % P.val ⦄ := by
    unfold gfp.montgomery.compute_r_mod_p
    progress as ⟨i, hi⟩
    progress as ⟨i1, hi1⟩
    progress as ⟨i2, hi2⟩
    · have : i1.val = P.val := by rw [hi1]; exact U64.cast_U128_val_eq P
      have : 1 < P.val := hP.2.1; omega
    have hi_val : i.val = 2 ^ 64 := by rw [hi]; native_decide
    have hi1_val : i1.val = P.val := by rw [hi1]; exact U64.cast_U128_val_eq P
    have hi2_val : i2.val = 2 ^ 64 % P.val := by rw [hi2, hi_val, hi1_val]
    have hP_pos : 0 < P.val := by linarith [hP.2.1]
    have hi2_lt : i2.val < P.val := by rw [hi2_val]; exact Nat.mod_lt _ hP_pos
    have : (UScalar.cast .U64 i2).val = i2.val :=
      UScalar.cast_val_mod_pow_of_inBounds_eq .U64 i2 (by
        have : UScalarTy.U64.numBits = 64 := by decide
        rw [this]; nlinarith [hP.2.2])
    rw [this]; exact hi2_val
  exact spec_imp_exists hspec

/-- compute_r2_mod_p returns (2^64)² mod P -/
private theorem compute_r2_mod_p_value {P : Std.U64} (hP : ValidPrime P) :
    ∃ r, gfp.montgomery.compute_r2_mod_p P = ok r ∧ r.val = (2 ^ 64) ^ 2 % P.val := by
  obtain ⟨rmod, hrmod_eq, hrmod_val⟩ := compute_r_mod_p_value hP
  have hP_pos : 0 < P.val := by linarith [hP.2.1]
  have hrmod_lt : rmod.val < P.val := by rw [hrmod_val]; exact Nat.mod_lt _ hP_pos
  unfold gfp.montgomery.compute_r2_mod_p
  simp only [hrmod_eq, bind_tc_ok, lift]
  apply spec_imp_exists
  have hrmod128 : (UScalar.cast .U128 rmod : Std.U128).val = rmod.val := U64.cast_U128_val_eq rmod
  have hP128 : (UScalar.cast .U128 P : Std.U128).val = P.val := U64.cast_U128_val_eq P
  progress as ⟨sq, hsq⟩
  progress as ⟨rem, hrem⟩
  have hsq_val : sq.val = rmod.val * rmod.val := by rw [hsq, hrmod128]
  have hrem_val : rem.val = rmod.val * rmod.val % P.val := by rw [hrem, hsq_val, hP128]
  have hrem_lt : rem.val < P.val := by rw [hrem_val]; exact Nat.mod_lt _ hP_pos
  have hcast : (UScalar.cast .U64 rem).val = rem.val :=
    UScalar.cast_val_mod_pow_of_inBounds_eq .U64 rem (by
      have : UScalarTy.U64.numBits = 64 := by decide
      rw [this]; nlinarith [hP.2.2])
  rw [hcast, hrem_val, hrmod_val]
  have : ((2:ℕ)^64)^2 = (2:ℕ)^64 * (2:ℕ)^64 := by ring
  rw [this]; symm; exact Nat.mul_mod _ _ _

/-- R2_MOD_P returns (2^64)² mod P -/
private theorem r2_mod_p_value {P : Std.U64} (hP : ValidPrime P) :
    ∃ r, gfp.montgomery.MontConsts.R2_MOD_P P = ok r ∧ r.val = (2 ^ 64) ^ 2 % P.val := by
  obtain ⟨r, hr_eq, hr_val⟩ := compute_r2_mod_p_value hP
  refine ⟨r, ?_, hr_val⟩
  simp only [gfp.montgomery.MontConsts.R2_MOD_P]
  exact hr_eq

/-! ## P_INV value specification -/

/-- Newton step on ℤ: if 2^k ∣ P·inv - 1, then 2^(2k) ∣ P·inv·(2 - P·inv) - 1. -/
private lemma newton_step_int (P inv : ℤ) (k : ℕ)
    (h : (2 : ℤ) ^ k ∣ P * inv - 1) :
    (2 : ℤ) ^ (2 * k) ∣ P * inv * (2 - P * inv) - 1 := by
  obtain ⟨m, hm⟩ := h
  exact ⟨-(m ^ 2), by rw [show P * inv = 1 + m * 2 ^ k from by linarith]; ring⟩

/-- If a ≡ 1 (mod 2^k) on ℕ, then 2^k | a - 1 on ℤ. -/
private lemma nat_mod_to_int_dvd (a : ℕ) (k : ℕ) (h : a % 2 ^ k = 1) :
    (2 : ℤ) ^ k ∣ (↑a : ℤ) - 1 := by
  have hd := Nat.div_add_mod a (2 ^ k)
  rw [h] at hd
  set q := a / 2 ^ k
  exact ⟨↑q, by rw [show a = 2 ^ k * q + 1 from by linarith]; push_cast; ring⟩

/-- If 2^k | a - 1 on ℤ, then a*(2-a) ≡ 1 (mod 2^(2k)). -/
private lemma newton_step_mod_eq (a : ℤ) (k : ℕ) (hk_pos : 0 < k)
    (h : (2 : ℤ) ^ k ∣ a - 1) : a * (2 - a) % (2 : ℤ) ^ (2 * k) = 1 := by
  obtain ⟨m, hm⟩ := h
  have hkey : a * (2 - a) = 1 + (2 : ℤ) ^ (2 * k) * (-(m ^ 2)) := by
    rw [show a = 1 + m * 2 ^ k from by linarith]; ring
  rw [hkey, Int.add_mul_emod_self_left]
  apply Int.emod_eq_of_lt (by omega)
  have : (2 : ℕ) ^ 1 ≤ (2 : ℕ) ^ (2 * k) := Nat.pow_le_pow_right (by omega) (by omega)
  exact_mod_cast by linarith

/-- Decompose a*(2+M-b) into a*(2-a) + M-divisible part on ℤ. -/
private lemma newton_step_decomp (a b M : ℤ) (k : ℕ) (hk_le : 2 * k ≤ 64)
    (hb : b = a % M) (hM : M = (2 : ℤ) ^ 64) :
    a * (2 + M - b) % (2 : ℤ) ^ (2 * k) = a * (2 - a) % (2 : ℤ) ^ (2 * k) := by
  have hdvd : (2 : ℤ) ^ (2 * k) ∣ M := by rw [hM]; exact_mod_cast Nat.pow_dvd_pow 2 hk_le
  have hdecomp : a * (2 + M - b) = a * (2 - a) + a * (M + a - b) := by ring
  rw [hdecomp]
  have hM_dvd_extra : (2 : ℤ) ^ (2 * k) ∣ a * (M + a - b) := by
    rw [hb]
    have hediv : M + a - a % M = (1 + a / M) * M := by
      have := Int.mul_ediv_add_emod a M; linarith
    rw [hediv, show a * ((1 + a / M) * M) = a * (1 + a / M) * M from by ring]
    exact dvd_mul_of_dvd_right hdvd _
  obtain ⟨q, hq⟩ := hM_dvd_extra
  rw [hq, Int.add_mul_emod_self_left]

/-- Cast the ℕ Newton expression to ℤ and reduce to a*(2-a) form. -/
private lemma newton_step_cast_reduce (P inv : ℕ) (M : ℕ) (k : ℕ)
    (hk_le : 2 * k ≤ 64) (hM : M = 2 ^ 64) :
    (↑(P * (inv * (2 + M - P * inv % M))) : ℤ) % (2 : ℤ) ^ (2 * k) =
      (↑(P * inv) : ℤ) * (2 - ↑(P * inv)) % (2 : ℤ) ^ (2 * k) := by
  set a := (↑(P * inv) : ℤ)
  set b := (↑(P * inv % M) : ℤ)
  have hb_eq : b = a % ↑M := by simp [a, b, Int.natCast_emod]
  have hmod_le : P * inv % M ≤ 2 + M := by
    have : P * inv % M < M := Nat.mod_lt _ (by rw [hM]; positivity); omega
  have hsub : (↑(2 + M - P * inv % M) : ℤ) = 2 + ↑M - b := by
    rw [Nat.cast_sub hmod_le, Nat.cast_add]; push_cast; simp [b]
  have hcast : (↑(P * (inv * (2 + M - P * inv % M))) : ℤ) = a * (2 + ↑M - b) := by
    simp only [Nat.cast_mul, hsub, a]; ring
  rw [hcast]
  rw [newton_step_decomp a b ↑M k hk_le hb_eq (by push_cast [hM])]

/-- ℤ-based Newton step: if P*inv ≡ 1 (mod 2^k) then
    P*(inv*(2+2^64-P*inv%2^64)) ≡ 1 (mod 2^(2k)). -/
private lemma newton_step_int_mod (P inv k : ℕ) (hk_pos : 0 < k) (hk_le : 2 * k ≤ 64)
    (h : P * inv % 2 ^ k = 1) :
    P * (inv * (2 + 2 ^ 64 - P * inv % 2 ^ 64)) % 2 ^ (2 * k) = 1 := by
  set M := (2 : ℕ) ^ 64 with hM_def
  suffices hsuff : (↑(P * (inv * (2 + M - P * inv % M))) : ℤ) % (↑(2 ^ (2 * k)) : ℤ) = 1 by
    have h1 : (↑(P * (inv * (2 + M - P * inv % M)) % 2 ^ (2 * k)) : ℤ) = 1 := by
      rw [Int.natCast_emod]; exact hsuff
    exact_mod_cast h1
  rw [show (↑(2 ^ (2 * k)) : ℤ) = (2 : ℤ) ^ (2 * k) from by push_cast; ring,
      newton_step_cast_reduce P inv M k hk_le hM_def]
  exact newton_step_mod_eq (↑(P * inv)) k hk_pos (nat_mod_to_int_dvd (P * inv) k h)

/-- Wrapping Newton step on ℕ: if P·inv % 2^k = 1 with 2k ≤ 64, then
    P · (inv · (2 - P·inv) mod 2^64) % 2^(2k) = 1.
    All intermediate values use mod 2^64 (wrapping U64 arithmetic). -/
private lemma compute_p_inv_newton_step (P inv : ℕ) (k : ℕ)
    (hk_pos : 0 < k) (hk_le : 2 * k ≤ 64) (h : P * inv % 2 ^ k = 1) :
    let M := 2 ^ 64
    P * (inv * ((2 + M - P * inv % M) % M) % M) % 2 ^ (2 * k) = 1 := by
  simp only
  have hdvd : 2 ^ (2 * k) ∣ 2 ^ 64 := Nat.pow_dvd_pow 2 hk_le
  -- Strip nested % 2^64 since 2^(2k) | 2^64
  suffices hsuff : P * (inv * (2 + 2 ^ 64 - P * inv % 2 ^ 64)) % 2 ^ (2 * k) = 1 by
    calc P * (inv * ((2 + 2 ^ 64 - P * inv % 2 ^ 64) % 2 ^ 64) % 2 ^ 64) % 2 ^ (2 * k)
        = P * (inv * (2 + 2 ^ 64 - P * inv % 2 ^ 64)) % 2 ^ (2 * k) := by
          conv_lhs =>
            rw [Nat.mul_mod P _ (2 ^ (2 * k)),
                Nat.mod_mod_of_dvd (inv * ((2 + 2 ^ 64 - P * inv % 2 ^ 64) % 2 ^ 64)) hdvd,
                ← Nat.mul_mod P]
            rw [show P * (inv * ((2 + 2 ^ 64 - P * inv % 2 ^ 64) % 2 ^ 64)) =
                     P * inv * ((2 + 2 ^ 64 - P * inv % 2 ^ 64) % 2 ^ 64) from by ring]
            rw [Nat.mul_mod (P * inv) _ (2 ^ (2 * k)),
                Nat.mod_mod_of_dvd _ hdvd, ← Nat.mul_mod]
            rw [show P * inv * (2 + 2 ^ 64 - P * inv % 2 ^ 64) =
                     P * (inv * (2 + 2 ^ 64 - P * inv % 2 ^ 64)) from by ring]
      _ = 1 := hsuff
  exact newton_step_int_mod P inv k hk_pos hk_le h

/-- P is odd for valid primes P ≠ 2. -/
private lemma validPrime_odd {P : Std.U64} (hP : ValidPrime P) (hP2 : P.val ≠ 2) :
    P.val % 2 = 1 := by
  have hp := hP.1
  by_contra heven
  have h0 : P.val % 2 = 0 := by omega
  have h2dvd : 2 ∣ P.val := Nat.dvd_of_mod_eq_zero h0
  cases hp.eq_one_or_self_of_dvd 2 h2dvd with
  | inl h => omega
  | inr h => exact hP2 h.symm

/-- The compute_p_inv loop returns inv with P * inv ≡ 1 (mod 2^64). -/
private theorem compute_p_inv_loop_value_spec {P : Std.U64}
    (hP : ValidPrime P) (hP2 : P.val ≠ 2)
    (inv : Std.U64) (i : Std.I32)
    (hi_ge : 0 ≤ i.val) (hi_le : i.val ≤ 6)
    (hinv : P.val * inv.val % 2 ^ (2 ^ i.val.toNat) = 1) :
    ∃ r, gfp.montgomery.compute_p_inv_loop P inv i = ok r ∧
      P.val * r.val % 2 ^ 64 = 1 := by
  unfold gfp.montgomery.compute_p_inv_loop
  apply spec_imp_exists
  apply loop.spec
    (measure := fun ((_, i1) : Std.U64 × Std.I32) => (6 - i1.val).toNat)
    (inv := fun ((inv1, i1) : Std.U64 × Std.I32) =>
      0 ≤ i1.val ∧ i1.val ≤ 6 ∧ P.val * inv1.val % 2 ^ (2 ^ i1.val.toNat) = 1)
  · intro ⟨inv1, i1⟩ ⟨hi1_ge, hi1_le, hinv1⟩
    dsimp only
    by_cases hlt : i1 < 6#i32
    · simp only [hlt, ite_true]
      progress as ⟨i2, hi2⟩       -- wrapping_mul P inv1
      progress as ⟨i3, hi3⟩       -- wrapping_sub 2 i2
      progress as ⟨inv2, hinv2⟩   -- wrapping_mul inv1 i3
      progress as ⟨i4, hi4⟩       -- i1 + 1
      have hi1_lt : i1.val < 6 := by scalar_tac
      refine ⟨by omega, by omega, ?_, ?_⟩
      · -- Need: P.val * inv2.val % 2^(2^i4.val.toNat) = 1
        have hi4_eq : i4.val.toNat = i1.val.toNat + 1 := by omega
        rw [hi4_eq, show 2 ^ (i1.val.toNat + 1) = 2 * 2 ^ i1.val.toNat from by ring]
        -- inv2 = wrapping_mul inv1 i3, i3 = wrapping_sub 2 i2, i2 = wrapping_mul P inv1
        have hinv2_val : inv2.val = (inv1.val * i3.val) % 2 ^ 64 := by
          have h := core.num.U64.wrapping_mul_val_eq inv1 i3
          simp only [UScalar.size_UScalarTyU64, U64.size_eq] at h; rw [hinv2]; exact h
        have hi3_val : i3.val = (2 + 2 ^ 64 - i2.val) % 2 ^ 64 := by
          have h := core.num.U64.wrapping_sub_val_eq (2#u64) i2
          simp only [UScalar.size_UScalarTyU64, U64.size_eq] at h
          have h2val : (2#u64 : Std.U64).val = 2 := by native_decide
          have hi2_lt : i2.val < 18446744073709551616 := by
            have := i2.hBounds
            simp only [UScalar.size_UScalarTyU64, U64.size_eq] at this; exact this
          have hval : i3.val =
              (2 + 18446744073709551616 - i2.val) % 18446744073709551616 := by
            rw [hi3, h, h2val]
            conv_rhs => rw [Nat.add_sub_assoc (le_of_lt hi2_lt)]
          rw [hval, show (18446744073709551616 : ℕ) = 2 ^ 64 from by norm_num]
        have hi2_val : i2.val = (P.val * inv1.val) % 2 ^ 64 := by
          have h := core.num.U64.wrapping_mul_val_eq P inv1
          simp only [UScalar.size_UScalarTyU64, U64.size_eq] at h; rw [hi2]; exact h
        rw [show P.val * inv2.val = P.val * (inv1.val * i3.val % 2 ^ 64) from by rw [hinv2_val]]
        rw [hi3_val, hi2_val]
        exact compute_p_inv_newton_step P.val inv1.val (2 ^ i1.val.toNat)
          (by positivity)
          (by have : i1.val.toNat ≤ 5 := by omega
              calc 2 * 2 ^ i1.val.toNat ≤ 2 * 2 ^ 5 :=
                    Nat.mul_le_mul_left _ (Nat.pow_le_pow_right (by norm_num) this)
                _ = 64 := by norm_num) hinv1
      · -- Measure decreases
        change (6 - i4.val).toNat < (6 - i1.val).toNat; omega
    · simp only [hlt, ite_false, spec, theta, wp_return]
      have : i1.val = 6 := by scalar_tac
      have : i1.val.toNat = 6 := by omega
      rw [this, show (2 : ℕ) ^ (2 ^ 6) = 2 ^ 64 from by norm_num] at hinv1
      exact hinv1
  · exact ⟨hi_ge, hi_le, hinv⟩

/-- The P_INV computation produces pinv satisfying pinv * P ≡ -1 (mod 2^64). -/
private theorem p_inv_value_spec {P : Std.U64} (hP : ValidPrime P) (hP2 : P.val ≠ 2) :
    ∃ pinv, gfp.montgomery.MontConsts.P_INV P = ok pinv ∧
      pinv.val * P.val % 2 ^ 64 = 2 ^ 64 - 1 := by
  simp only [gfp.montgomery.MontConsts.P_INV]
  unfold gfp.montgomery.compute_p_inv
  -- Base case: P * 1 % 2^1 = 1 (P is odd)
  have h1val : (1#u64 : Std.U64).val = 1 := by native_decide
  have h0val : (0#i32 : Std.I32).val.toNat = 0 := by native_decide
  have hbase : P.val * (1#u64 : Std.U64).val % 2 ^ (2 ^ (0#i32 : Std.I32).val.toNat) = 1 := by
    rw [h0val, h1val]; simp; exact validPrime_odd hP hP2
  -- Loop gives inv with P * inv % 2^64 = 1
  obtain ⟨inv, hinv_eq, hinv_mod⟩ := compute_p_inv_loop_value_spec hP hP2
    1#u64 0#i32 (by native_decide) (by native_decide) hbase
  simp only [hinv_eq, bind_tc_ok]
  -- wrapping_neg(inv) gives pinv = (2^64 - inv.val) % 2^64
  show ∃ pinv, core.num.U64.wrapping_neg inv = ok pinv ∧ pinv.val * P.val % 2 ^ 64 = 2 ^ 64 - 1
  refine ⟨UScalar.wrapping_sub ⟨0#64⟩ inv, rfl, ?_⟩
  have hpinv_val : (UScalar.wrapping_sub (⟨0#64⟩ : Std.U64) inv).val =
      (2 ^ 64 - inv.val) % 2 ^ 64 := by
    rw [UScalar.wrapping_sub_val_eq]
    simp only [UScalar.size_UScalarTyU64, U64.size_eq]
    show (0 + (18446744073709551616 - inv.val)) % 18446744073709551616 =
         (2 ^ 64 - inv.val) % 2 ^ 64
    norm_num
  rw [hpinv_val]
  -- From P * inv % 2^64 = 1, derive (2^64 - inv) * P % 2^64 = 2^64 - 1
  have hinv_pos : 0 < inv.val := by
    by_contra h; push_neg at h
    simp [show inv.val = 0 from by omega] at hinv_mod
  have hinv_lt : inv.val < 2 ^ 64 := inv.hBounds
  rw [Nat.mod_eq_of_lt (by omega : 2 ^ 64 - inv.val < 2 ^ 64)]
  -- inv * P ≡ 1 (mod 2^64), express as inv * P = q * 2^64 + 1
  have hinv_comm : inv.val * P.val % 2 ^ 64 = 1 := by rwa [Nat.mul_comm] at hinv_mod
  set q := inv.val * P.val / 2 ^ 64
  have hqM : inv.val * P.val = q * 2 ^ 64 + 1 := by
    have := Nat.div_add_mod (inv.val * P.val) (2 ^ 64)
    omega
  -- q < P (since inv < 2^64 and inv * P = q * 2^64 + 1)
  have hq_lt_P : q < P.val := by
    by_contra h_ge; push_neg at h_ge
    have : P.val * 2 ^ 64 ≤ P.val * inv.val := by nlinarith [Nat.mul_comm inv.val P.val]
    have := Nat.le_of_mul_le_mul_left this (by linarith [hP.2.1])
    omega
  -- Key identity: (2^64 - inv) * P + 1 = 2^64 * (P - q)
  -- This directly gives (2^64 - inv) * P % 2^64 = 2^64 - 1
  suffices hkey : (2 ^ 64 - inv.val) * P.val + 1 = 2 ^ 64 * (P.val - q) by
    have hdvd : ((2 ^ 64 - inv.val) * P.val + 1) % 2 ^ 64 = 0 := by
      rw [hkey, Nat.mul_mod_right]
    omega
  have h1 : (2 ^ 64 - inv.val) * P.val = 2 ^ 64 * P.val - inv.val * P.val :=
    Nat.sub_mul _ _ _
  rw [h1, hqM]
  have hle : q * 2 ^ 64 + 1 ≤ 2 ^ 64 * P.val := by nlinarith
  omega

/-! ## Conditional subtraction value preservation -/

/-- The branchless conditional subtraction preserves the value mod P.
    Given x < 2P, cond_sub(x, P) ≡ x (mod P). -/
private theorem cond_sub_mod_eq (x P : Std.U64) (hx : x.val < 2 * P.val) (hP : 0 < P.val) :
    let result : Std.U64 := ⟨x.bv - P.bv⟩
    let borrow : Bool := decide (x.val < P.val)
    let i : Std.U64 := UScalar.cast_fromBool .U64 borrow
    let neg_i : Std.U64 := UScalar.wrapping_sub ⟨0#64⟩ i
    let correction : Std.U64 := neg_i &&& P
    (UScalar.wrapping_add result correction).val % P.val = x.val % P.val := by
  simp only
  by_cases h : x.val < P.val
  · -- borrow = true, correction = P, wrapping_add restores x
    simp only [h, decide_true]
    have : UScalar.cast_fromBool .U64 true = (⟨1#64⟩ : Std.U64) := by native_decide
    rw [this]
    have : UScalar.wrapping_sub (⟨0#64⟩ : Std.U64) (⟨1#64⟩ : Std.U64) =
           ⟨BitVec.allOnes 64⟩ := by native_decide
    rw [this]
    have : ((⟨BitVec.allOnes 64⟩ : Std.U64) &&& P) = P := by
      show (⟨BitVec.allOnes 64 &&& P.bv⟩ : UScalar .U64) = P
      congr 1; exact FpProgress.bv_allOnes_and P.bv
    rw [this]
    suffices (UScalar.wrapping_add (⟨x.bv - P.bv⟩ : Std.U64) P).val = x.val by rw [this]
    show (UScalar.wrapping_add (⟨x.bv - P.bv⟩ : Std.U64) P).bv.toNat = x.bv.toNat
    congr 1; change (x.bv - P.bv) + P.bv = x.bv; bv_omega
  · -- borrow = false, correction = 0, result = x - P
    push_neg at h
    simp only [show ¬(x.val < P.val) from not_lt.mpr h, decide_false]
    have : UScalar.cast_fromBool .U64 false = (⟨0#64⟩ : Std.U64) := by native_decide
    rw [this]
    have : UScalar.wrapping_sub (⟨0#64⟩ : Std.U64) (⟨0#64⟩ : Std.U64) = (⟨0#64⟩ : Std.U64) := by
      native_decide
    rw [this]
    have : ((⟨0#64⟩ : Std.U64) &&& P) = (⟨0#64⟩ : Std.U64) := by
      show (⟨(0 : BitVec 64) &&& P.bv⟩ : UScalar .U64) = ⟨0#64⟩
      congr 1; exact FpProgress.bv_zero_and P.bv
    rw [this, FpProgress.wrapping_add_zero_val, FpProgress.bv_sub_toNat x P h]
    conv_rhs => rw [show x.val = (x.val - P.val) + P.val from by omega]
    rw [Nat.add_mod_right]

/-! ## REDC value specification -/

/-- REDC computes t · R⁻¹ mod P, expressed as: r · R ≡ t (mod P).
    This is the fundamental correctness property of Montgomery reduction.
    Proved from `p_inv_value_spec` by tracing through the REDC computation. -/
theorem redc_value_spec {P : Std.U64} {t : Std.U128}
    (hP : ValidPrime P) (hP2 : P.val ≠ 2) (ht : t.val < P.val * 2 ^ 64) :
    ∃ r, gfp.montgomery.redc P t = ok r ∧
      r.val < P.val ∧ r.val * 2 ^ 64 % P.val = t.val % P.val := by
  obtain ⟨pinv, hpinv_eq, hpinv_val⟩ := p_inv_value_spec hP hP2
  have hP_pos : 0 < P.val := by linarith [hP.2.1]
  -- Get P_INV equation for compute_p_inv
  have hpinv_eq' : gfp.montgomery.compute_p_inv P = ok pinv := by
    simp only [gfp.montgomery.MontConsts.P_INV] at hpinv_eq; exact hpinv_eq
  -- Comprehensive WP spec: trace through REDC with value tracking
  have hspec : gfp.montgomery.redc P t ⦃ r =>
      r.val < P.val ∧ r.val * 2 ^ 64 % P.val = t.val % P.val ⦄ := by
    unfold gfp.montgomery.redc
    progress as ⟨t_lo, ht_lo⟩       -- cast U64 t
    -- P_INV: substitute ok pinv and simplify bind, keeping spec wrapper
    simp only [gfp.montgomery.MontConsts.P_INV, hpinv_eq', bind_tc_ok]
    -- Now pinv is substituted; remaining computation uses pinv
    progress as ⟨m, hm⟩             -- wrapping_mul t_lo pinv
    progress as ⟨i1, hi1⟩           -- cast U128 m
    progress as ⟨i2, hi2⟩           -- cast U128 P
    progress as ⟨mp, hmp⟩           -- checked U128 mul: i1 * i2
    progress as ⟨i3, hi3⟩           -- checked U128 add: t + mp
    · -- U128 add overflow bound
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
    progress as ⟨i4, hi4⟩           -- i3 >>> 64
    progress as ⟨u, hu⟩             -- cast U64 i4
    progress as ⟨discr, hdiscr1, hdiscr2⟩  -- overflowing_sub u P
    progress as ⟨i5, hi5⟩           -- cast_fromBool
    progress as ⟨neg_i, hneg_i⟩     -- wrapping_neg
    progress as ⟨correction, hcorrection⟩  -- AND
    -- Establish intermediate value equalities
    have h_i1_val : i1.val = m.val := by rw [hi1]; exact U64.cast_U128_val_eq m
    have h_i2_val : i2.val = P.val := by rw [hi2]; exact U64.cast_U128_val_eq P
    have hmp_val : mp.val = m.val * P.val := by rw [hmp, h_i1_val, h_i2_val]
    have hi3_val : i3.val = t.val + m.val * P.val := by rw [hi3, hmp_val]
    -- Bounds for conditional subtraction
    have hmp_lt : mp.val < 2^64 * P.val := by
      rw [hmp_val]; exact Nat.mul_lt_mul_of_pos_right m.hBounds hP_pos
    have hi3_lt : i3.val < 2 * P.val * 2^64 := by rw [hi3_val]; omega
    have hi4_lt : i4.val < 2 * P.val := by
      rw [hi4, Nat.shiftRight_eq_div_pow]
      exact Nat.div_lt_of_lt_mul (by ring_nf; linarith)
    have hu_val_eq : u.val = i4.val := by
      rw [hu]; exact UScalar.cast_val_mod_pow_of_inBounds_eq .U64 i4 (by
        have := hP.2.2; scalar_tac)
    have hu_lt_2P : u.val < 2 * P.val := by rw [hu_val_eq]; exact hi4_lt
    -- Rewrite to match cond_sub pattern
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
    constructor
    · -- Part 1: bounds
      exact FpProgress.cond_sub_val u P hu_lt_2P
    · -- Part 2: value equation
      -- Step A: divisibility — (t + m*P) is divisible by 2^64
      have ht_lo_val : t_lo.val = t.val % 2 ^ 64 := by
        rw [ht_lo]; exact UScalar.cast_val_eq .U64 t
      have hm_val : m.val = (t_lo.val * pinv.val) % 2 ^ 64 := by
        rw [hm]
        have h := core.num.U64.wrapping_mul_val_eq t_lo pinv
        simp only [UScalar.size_UScalarTyU64, U64.size_eq] at h
        convert h using 1
      have h_mP_mod : (m.val * P.val) % 2 ^ 64 =
          (t_lo.val * pinv.val * P.val) % 2 ^ 64 := by
        rw [hm_val]; exact mod_mul_left (t_lo.val * pinv.val) P.val (2 ^ 64)
      have hdiv : i3.val % 2 ^ 64 = 0 := by
        rw [hi3_val, Nat.add_mod, ← ht_lo_val, h_mP_mod]
        -- Goal: (↑t_lo + (↑t_lo * ↑pinv * ↑P) % 2^64) % 2^64 = 0
        -- Reduce (a + b%n) % n to (a + b) % n
        rw [Nat.add_mod t_lo.val, Nat.mod_mod_of_dvd _ (dvd_refl _), ← Nat.add_mod]
        -- Goal: (↑t_lo + ↑t_lo * ↑pinv * ↑P) % 2^64 = 0
        have h_ring : t_lo.val + t_lo.val * pinv.val * P.val =
            t_lo.val * (pinv.val * P.val + 1) := by ring
        rw [h_ring, Nat.mul_mod]
        have h_pinv_p1 : (pinv.val * P.val + 1) % 2 ^ 64 = 0 := by
          rw [Nat.add_mod, hpinv_val]; norm_num
        rw [h_pinv_p1, Nat.mul_zero, Nat.zero_mod]
      -- Step B: u * 2^64 = i3 (exact division)
      have hu_mul_R : u.val * 2 ^ 64 = i3.val := by
        rw [hu_val_eq, hi4, Nat.shiftRight_eq_div_pow]
        exact Nat.div_mul_cancel (Nat.dvd_of_mod_eq_zero hdiv)
      -- Step C: i3 ≡ t (mod P), since i3 = t + m*P
      have hi3_mod_P : i3.val % P.val = t.val % P.val := by
        rw [hi3_val, Nat.add_mod, Nat.mul_comm m.val P.val, Nat.mul_mod_right,
            Nat.add_zero, Nat.mod_mod_of_dvd _ (dvd_refl P.val)]
      -- Step D: conditional sub preserves mod P
      have hfinal_mod := cond_sub_mod_eq u P hu_lt_2P hP_pos
      dsimp only at hfinal_mod
      -- Chain: final * R % P = u * R % P = i3 % P = t % P
      simp only [core.num.U64.wrapping_add]
      conv_lhs => rw [Nat.mul_mod, hfinal_mod, ← Nat.mul_mod]
      rw [hu_mul_R, hi3_mod_P]
  exact spec_imp_exists hspec

/-! ## from_mont value specification -/

/-- from_mont unfolds to redc (cast U128 a) -/
private theorem from_mont_unfold (P a : Std.U64) :
    gfp.montgomery.from_mont P a = gfp.montgomery.redc P (UScalar.cast .U128 a) := by
  unfold gfp.montgomery.from_mont; rfl

/-- from_mont satisfies: result · R ≡ input (mod P) -/
theorem from_mont_value {P : Std.U64} {a : Std.U64}
    (hP : ValidPrime P) (hP2 : P.val ≠ 2) (ha : a.val < P.val) :
    ∃ r, gfp.montgomery.from_mont P a = ok r ∧
      r.val < P.val ∧ r.val * 2 ^ 64 % P.val = a.val := by
  have hcast : (UScalar.cast .U128 a : Std.U128).val = a.val := U64.cast_U128_val_eq a
  have hbound : (UScalar.cast .U128 a : Std.U128).val < P.val * 2 ^ 64 := by
    rw [hcast]; nlinarith [hP.2.1]
  obtain ⟨r, hr_eq, hr_lt, hr_val⟩ := redc_value_spec hP hP2 hbound
  refine ⟨r, ?_, hr_lt, ?_⟩
  · rw [from_mont_unfold]; exact hr_eq
  · rw [hr_val, hcast, Nat.mod_eq_of_lt ha]

/-! ## to_mont value specification -/

/-- to_mont satisfies: result ≡ a · R (mod P), expressed as
    result · R ≡ a · R² (mod P) -/
theorem to_mont_value {P : Std.U64} {a : Std.U64}
    (hP : ValidPrime P) (hP2 : P.val ≠ 2) (ha : a.val < P.val) :
    ∃ m, gfp.montgomery.to_mont P a = ok m ∧
      m.val < P.val ∧
      m.val * 2 ^ 64 % P.val = a.val * ((2 ^ 64) ^ 2 % P.val) % P.val := by
  -- Get R2_MOD_P value
  obtain ⟨r2, hr2_eq, hr2_val⟩ := r2_mod_p_value hP
  have hP_pos : 0 < P.val := by linarith [hP.2.1]
  have hr2_lt : r2.val < P.val := by rw [hr2_val]; exact Nat.mod_lt _ hP_pos
  -- Cast values
  have ha128 : (UScalar.cast .U128 a : Std.U128).val = a.val := U64.cast_U128_val_eq a
  have hr2_128 : (UScalar.cast .U128 r2 : Std.U128).val = r2.val := U64.cast_U128_val_eq r2
  -- Product bound for REDC
  have hprod_bound : a.val * r2.val < P.val * 2 ^ 64 := by
    calc a.val * r2.val < P.val * P.val := Nat.mul_lt_mul_of_lt_of_lt ha hr2_lt
      _ ≤ P.val * 2 ^ 63 := Nat.mul_le_mul_left _ hP.2.2
      _ < P.val * 2 ^ 64 := by linarith [hP.2.1]
  -- Unfold to_mont and simplify the monadic chain
  unfold gfp.montgomery.to_mont
  simp only [lift, bind_tc_ok, hr2_eq]
  -- Goal: ∃ m, (cast a * cast r2) >>= redc P ⦃ ... ⦄
  apply spec_imp_exists
  progress as ⟨prod, hprod⟩  -- checked U128 mul: cast a * cast r2
  -- REDC step
  have hprod_val : prod.val = a.val * r2.val := by rw [hprod, ha128, hr2_128]
  have hprod_lt : prod.val < P.val * 2 ^ 64 := by rw [hprod_val]; exact hprod_bound
  obtain ⟨m, hm_eq, hm_lt, hm_val⟩ := redc_value_spec hP hP2 hprod_lt
  rw [show spec = theta from rfl]
  simp only [theta, hm_eq, wp_return]
  exact ⟨hm_lt, by rw [hm_val, hprod_val, hr2_val]⟩

/-! ## Montgomery roundtrip -/

/-- **Flagship theorem**: The Rust Montgomery conversion roundtrips correctly.

    For any valid prime P > 2 and any value a < P, converting to Montgomery
    form and back yields the original value. This is proved about the
    *actual extracted Rust code*, not a mathematical model.

    ```rust
    let m = to_mont(P, a);
    let r = from_mont(P, m);
    assert_eq!(r, a);
    ```

    **Proof structure**: from the REDC value spec (r · R ≡ t mod P):
    - to_mont(a): m · R ≡ a · R²   (mod P)
    - from_mont(m): r · R ≡ m       (mod P)
    - Chain: r · R² ≡ a · R²        (mod P)
    - Cancel R² (coprime to P): r ≡ a (mod P)
    - Both < P, so r = a.
-/
theorem montgomery_roundtrip {P : Std.U64} {a : Std.U64}
    (hP : ValidPrime P) (hP2 : P.val ≠ 2) (ha : a.val < P.val) :
    ∃ r, (do
      let m ← gfp.montgomery.to_mont P a
      gfp.montgomery.from_mont P m) = ok r ∧ r.val = a.val := by
  -- Step 1: to_mont succeeds with value spec
  obtain ⟨m, hm_eq, hm_lt, hm_val⟩ := to_mont_value hP hP2 ha
  -- Step 2: from_mont succeeds with value spec
  obtain ⟨r, hr_eq, hr_lt, hr_val⟩ := from_mont_value hP hP2 hm_lt
  refine ⟨r, ?_, ?_⟩
  · -- Monadic chaining
    simp only [hm_eq]; exact hr_eq
  · -- Value: r = a
    -- hr_val: r * R % P = m (since m < P)
    -- hm_val: m * R % P = a * R² mod P % P
    -- Chain: r * R * R % P = m * R % P = a * R² mod P % P
    have h1 : r.val * 2 ^ 64 * 2 ^ 64 % P.val = m.val * 2 ^ 64 % P.val := by
      conv_lhs => rw [Nat.mul_mod (r.val * 2 ^ 64) (2 ^ 64) P.val, hr_val]
      exact mul_mod_mod_right m.val (2 ^ 64) P.val
    -- r * R² % P = a * R² % P
    have h2 : r.val * (2 ^ 64) ^ 2 % P.val = a.val * (2 ^ 64) ^ 2 % P.val := by
      have : r.val * (2 ^ 64) ^ 2 = r.val * 2 ^ 64 * 2 ^ 64 := by ring
      rw [this, h1, hm_val]
      exact mul_mod_mod_right a.val ((2 ^ 64) ^ 2) P.val
    -- Cancel R² using coprimality
    have hcop : Nat.Coprime ((2 ^ 64) ^ 2) P.val := by
      change Nat.Coprime (MontArith.R ^ 2) P.val
      exact (MontArith.R_coprime_P hP hP2).pow_left 2
    have h3 : Nat.ModEq P.val r.val a.val :=
      Nat.ModEq.cancel_right_of_coprime hcop.symm h2
    rwa [Nat.ModEq, Nat.mod_eq_of_lt hr_lt, Nat.mod_eq_of_lt ha] at h3

/-! ## Fp.new / Fp.value roundtrip -/

/-- Constructing an Fp value and reading it back gives the reduced input. -/
theorem fp_new_value_roundtrip {P : Std.U64}
    (hP : ValidPrime P) (hP2 : P.val ≠ 2) (v : Std.U64) :
    ∃ r, (do
      let fp ← gfp.Fp.new P v
      gfp.Fp.value fp) = ok r ∧ r.val = v.val % P.val := by
  have hne : ¬(P = 2#u64) := by intro h; exact hP2 (by subst h; native_decide)
  have hP_pos : 0 < P.val := by have := hP.2.1; omega
  have hspec : (do let fp ← gfp.Fp.new P v; gfp.Fp.value fp) ⦃ r =>
      r.val = v.val % P.val ⦄ := by
    unfold gfp.Fp.new
    progress   -- VALIDATED P
    progress as ⟨reduced, hreduced⟩  -- v % P
    simp only [hne, ite_false]
    unfold gfp.Fp.value
    simp only [hne, ite_false, bind_assoc_eq, bind_tc_ok]
    -- Goal: spec (do let m ← to_mont P reduced; from_mont P m) (fun r => r.val = v.val % P.val)
    have hred_lt : reduced.val < P.val := by
      rw [hreduced]; exact Nat.mod_lt _ hP_pos
    obtain ⟨r, hr_eq, hr_val⟩ := montgomery_roundtrip hP hP2 hred_lt
    rw [show spec = theta from rfl, hr_eq]
    simp only [theta, wp_return]
    rw [hr_val, hreduced]
  exact spec_imp_exists hspec

/-! ## Arithmetic consistency -/

/-- mont_add preserves value modulo P: result ≡ a + b (mod P). -/
private theorem mont_add_value_spec {P : Std.U64} {a b : Std.U64}
    (hP : ValidPrime P) (ha : a.val < P.val) (hb : b.val < P.val) :
    ∃ r, gfp.montgomery.mont_add P a b = ok r ∧
      r.val < P.val ∧ r.val % P.val = (a.val + b.val) % P.val := by
  have hP_pos : 0 < P.val := by have := hP.2.1; omega
  have hspec : gfp.montgomery.mont_add P a b ⦃ r =>
      r.val < P.val ∧ r.val % P.val = (a.val + b.val) % P.val ⦄ := by
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
    constructor
    · exact FpProgress.cond_sub_val sum P hsum_lt
    · have hfinal_mod := cond_sub_mod_eq sum P hsum_lt hP_pos
      dsimp only at hfinal_mod
      simp only [core.num.U64.wrapping_add]
      rw [hfinal_mod, hsum]
  exact spec_imp_exists hspec

/-- Addition in Fp matches modular addition of canonical values. -/
theorem fp_add_correct {P : Std.U64} {a b : Std.U64}
    (hP : ValidPrime P) (hP2 : P.val ≠ 2)
    (ha : a.val < P.val) (hb : b.val < P.val) :
    ∃ r, (do
      let sum ← gfp.montgomery.mont_add P a b
      gfp.montgomery.from_mont P sum) = ok r ∧
    ∃ va vb, gfp.montgomery.from_mont P a = ok va ∧
             gfp.montgomery.from_mont P b = ok vb ∧
             r.val = (va.val + vb.val) % P.val := by
  have hP_pos : 0 < P.val := by have := hP.2.1; omega
  -- Get the from_mont values for a and b
  obtain ⟨va, hva_eq, hva_lt, hva_val⟩ := from_mont_value hP hP2 ha
  obtain ⟨vb, hvb_eq, hvb_lt, hvb_val⟩ := from_mont_value hP hP2 hb
  -- Get the mont_add result
  obtain ⟨sum, hsum_eq, hsum_lt, hsum_mod⟩ := mont_add_value_spec hP ha hb
  -- Get from_mont of the sum
  obtain ⟨r, hr_eq, hr_lt, hr_val⟩ := from_mont_value hP hP2 hsum_lt
  refine ⟨r, ?_, va, vb, hva_eq, hvb_eq, ?_⟩
  · -- Computation succeeds
    simp only [hsum_eq]; exact hr_eq
  · -- r.val = (va.val + vb.val) % P.val
    -- We have: r * R ≡ sum (mod P), sum ≡ a + b (mod P)
    -- va * R ≡ a (mod P), vb * R ≡ b (mod P)
    -- So r * R ≡ a + b ≡ va * R + vb * R ≡ (va + vb) * R (mod P)
    -- Cancel R: r ≡ va + vb (mod P). Since r < P: r = (va + vb) % P.
    have h1 : r.val * 2 ^ 64 % P.val = sum.val % P.val := by
      rw [hr_val]; exact (Nat.mod_eq_of_lt hsum_lt).symm
    have h2 : sum.val % P.val = (a.val + b.val) % P.val := hsum_mod
    have h3 : a.val % P.val = (va.val * 2 ^ 64) % P.val := by
      rw [Nat.mod_eq_of_lt ha, ← hva_val]
    have h4 : b.val % P.val = (vb.val * 2 ^ 64) % P.val := by
      rw [Nat.mod_eq_of_lt hb, ← hvb_val]
    -- r * R ≡ (va + vb) * R (mod P)
    have h5 : r.val * 2 ^ 64 % P.val = (va.val + vb.val) * 2 ^ 64 % P.val := by
      rw [h1, h2, add_mul,
          Nat.add_mod (va.val * 2 ^ 64) (vb.val * 2 ^ 64) P.val,
          ← h3, ← h4, ← Nat.add_mod]
    -- Cancel R (coprime)
    have hcop : Nat.Coprime (2 ^ 64) P.val := by
      change Nat.Coprime MontArith.R P.val
      exact MontArith.R_coprime_P hP hP2
    have h6 : Nat.ModEq P.val r.val (va.val + vb.val) :=
      Nat.ModEq.cancel_right_of_coprime hcop.symm h5
    rwa [Nat.ModEq, Nat.mod_eq_of_lt hr_lt] at h6

/-- The Fp mul function computes redc(a*b) with value spec r·R ≡ a·b (mod P). -/
private theorem mul_value_spec {P : Std.U64} {a b : Std.U64}
    (hP : ValidPrime P) (hP2 : P.val ≠ 2)
    (ha : a.val < P.val) (hb : b.val < P.val) :
    ∃ r, gfp.Fp.Insts.CoreOpsArithMulFpFp.mul (P := P) a b = ok r ∧
      r.val < P.val ∧ r.val * 2 ^ 64 % P.val = (a.val * b.val) % P.val := by
  have hne : ¬(P = 2#u64) := by intro h; exact hP2 (by subst h; native_decide)
  have hspec : gfp.Fp.Insts.CoreOpsArithMulFpFp.mul (P := P) a b ⦃ r =>
      r.val < P.val ∧ r.val * 2 ^ 64 % P.val = (a.val * b.val) % P.val ⦄ := by
    unfold gfp.Fp.Insts.CoreOpsArithMulFpFp.mul
    simp only [hne, ite_false]
    progress as ⟨i, hi⟩       -- cast U128 a
    progress as ⟨i1, hi1⟩     -- cast U128 b
    progress as ⟨i2, hi2⟩     -- i * i1 (checked U128 mul)
    have hi_val : i.val = a.val := by rw [hi]; exact U64.cast_U128_val_eq a
    have hi1_val : i1.val = b.val := by rw [hi1]; exact U64.cast_U128_val_eq b
    have hi2_val : i2.val = a.val * b.val := by rw [hi2, hi_val, hi1_val]
    have hbound : i2.val < P.val * 2 ^ 64 := by
      rw [hi2_val]
      calc a.val * b.val < P.val * P.val :=
              Nat.mul_lt_mul_of_lt_of_lt ha hb
        _ ≤ P.val * 2 ^ 63 := Nat.mul_le_mul_left _ hP.2.2
        _ < P.val * 2 ^ 64 := by have := hP.2.1; omega
    -- Now need redc P i2 with value spec
    obtain ⟨r, hr_eq, hr_lt, hr_val⟩ := redc_value_spec hP hP2 hbound
    rw [show spec = theta from rfl, hr_eq]
    simp only [theta, wp_return, bind_tc_ok]
    exact ⟨hr_lt, by rw [hr_val, hi2_val]⟩
  exact spec_imp_exists hspec

/-- Multiplication in Fp matches modular multiplication of canonical values. -/
theorem fp_mul_correct {P : Std.U64} {a b : Std.U64}
    (hP : ValidPrime P) (hP2 : P.val ≠ 2)
    (ha : a.val < P.val) (hb : b.val < P.val) :
    ∃ r, (do
      let prod ← gfp.Fp.Insts.CoreOpsArithMulFpFp.mul (P := P) a b
      gfp.montgomery.from_mont P prod) = ok r ∧
    ∃ va vb, gfp.montgomery.from_mont P a = ok va ∧
             gfp.montgomery.from_mont P b = ok vb ∧
             r.val = (va.val * vb.val) % P.val := by
  have hP_pos : 0 < P.val := by have := hP.2.1; omega
  -- Get from_mont values
  obtain ⟨va, hva_eq, hva_lt, hva_val⟩ := from_mont_value hP hP2 ha
  obtain ⟨vb, hvb_eq, hvb_lt, hvb_val⟩ := from_mont_value hP hP2 hb
  -- Get mul result with value spec
  obtain ⟨prod, hprod_eq, hprod_lt, hprod_val⟩ := mul_value_spec hP hP2 ha hb
  -- Get from_mont of prod
  obtain ⟨r, hr_eq, hr_lt, hr_val⟩ := from_mont_value hP hP2 hprod_lt
  refine ⟨r, ?_, va, vb, hva_eq, hvb_eq, ?_⟩
  · simp only [hprod_eq]; exact hr_eq
  · -- Chain: r·R² ≡ prod·R ≡ a·b ≡ va·R·vb·R ≡ va·vb·R² (mod P)
    -- Cancel R²: r ≡ va·vb (mod P)
    have h1 : r.val * 2 ^ 64 * 2 ^ 64 % P.val = prod.val * 2 ^ 64 % P.val := by
      conv_lhs => rw [Nat.mul_mod (r.val * 2 ^ 64) (2 ^ 64) P.val, hr_val]
      exact mul_mod_mod_right prod.val (2 ^ 64) P.val
    have h2 : r.val * (2 ^ 64) ^ 2 % P.val = (a.val * b.val) % P.val := by
      have : r.val * (2 ^ 64) ^ 2 = r.val * 2 ^ 64 * 2 ^ 64 := by ring
      rw [this, h1, hprod_val]
    -- va·vb·R² ≡ a·b (mod P)
    have h3 : (va.val * vb.val) * (2 ^ 64) ^ 2 % P.val = (a.val * b.val) % P.val := by
      have : (va.val * vb.val) * (2 ^ 64) ^ 2 =
          (va.val * 2 ^ 64) * (vb.val * 2 ^ 64) := by ring
      rw [this, Nat.mul_mod, hva_val, hvb_val]
    -- r·R² ≡ va·vb·R² (mod P)
    have h4 : r.val * (2 ^ 64) ^ 2 % P.val =
        (va.val * vb.val) * (2 ^ 64) ^ 2 % P.val := by rw [h2, h3]
    -- Cancel R² (coprime)
    have hcop : Nat.Coprime ((2 ^ 64) ^ 2) P.val := by
      change Nat.Coprime (MontArith.R ^ 2) P.val
      exact (MontArith.R_coprime_P hP hP2).pow_left 2
    have h5 : Nat.ModEq P.val r.val (va.val * vb.val) :=
      Nat.ModEq.cancel_right_of_coprime hcop.symm h4
    rwa [Nat.ModEq, Nat.mod_eq_of_lt hr_lt] at h5

/-! ## max_unreduced_additions overflow safety -/

/-- The `max_unreduced_additions` function returns `ok k` with
    `k * (P-1)² ≤ u128::MAX`, proving that `k` unreduced additions
    of products bounded by `(P-1)²` cannot overflow a `u128` accumulator.

    The function computes `k = u128::MAX / (P-1)²` (integer division),
    clamped to `usize::MAX`. The overflow safety follows from
    `Nat.div_mul_le_self`: `(a / b) * b ≤ a`. -/
theorem max_unreduced_additions_spec {P : Std.U64} (hP : ValidPrime P) :
    ∃ k, gfp.Fp.Insts.Gf2_coreFieldTraitsFiniteFieldU64U128.max_unreduced_additions P = ok k
    ∧ k.val * (P.val - 1) * (P.val - 1) ≤ core.num.U128.MAX.val := by
  have hspec :
      gfp.Fp.Insts.Gf2_coreFieldTraitsFiniteFieldU64U128.max_unreduced_additions P
      ⦃ k => k.val * (P.val - 1) * (P.val - 1) ≤ core.num.U128.MAX.val ⦄ := by
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
      have hi1_val : i1.val = P.val - 1 := by
        have := hi1; have := hi_val; scalar_tac
      have hi3_val : i3.val = P.val - 1 := by
        have := hi3; have := hi2_val; scalar_tac
      rw [hi1_val, hi3_val]
      exact MontArith.p_minus_one_sq_le_u128_max hP
    -- Establish value equalities
    have hi1_val : i1.val = P.val - 1 := by
      have := hi1; have := hi_val; scalar_tac
    have hi3_val : i3.val = P.val - 1 := by
      have := hi3; have := hi2_val; scalar_tac
    have hmp_val : max_product.val = (P.val - 1) * (P.val - 1) := by
      rw [hmp, hi1_val, hi3_val]
    by_cases hmp0 : max_product = 0#u128
    · -- max_product = 0, impossible for ValidPrime (P ≥ 2 ⟹ (P-1)² ≥ 1)
      exfalso
      have hmv : max_product.val = 0 := by
        have := congrArg UScalar.val hmp0; simpa using this
      rw [hmp_val] at hmv
      have : 0 < P.val - 1 := by have := hP.2.1; omega
      have := Nat.mul_pos this this
      omega
    · simp only [show ¬(max_product = 0#u128) from hmp0, ite_false]
      progress as ⟨k, hk⟩       -- U128.MAX / max_product
      progress as ⟨i4, hi4⟩     -- cast U128 Usize.MAX
      -- Key bound: k * (P-1)² ≤ U128.MAX (from integer division property)
      have hk_mp_bound : k.val * max_product.val ≤ core.num.U128.MAX.val := by
        rw [hk]; exact Nat.div_mul_le_self _ _
      by_cases hgt : k > i4
      · -- k > i4: return Usize.MAX
        simp only [hgt, ite_true, spec, theta, wp_return]
        have hi4_val : i4.val = core.num.Usize.MAX.val := by
          rw [hi4]; exact UScalar.cast_val_mod_pow_greater_numBits_eq .U128
            core.num.Usize.MAX (by cases System.Platform.numBits_eq <;> simp [*])
        have husize_le_k : core.num.Usize.MAX.val ≤ k.val := by
          have : i4.val < k.val := hgt; omega
        calc core.num.Usize.MAX.val * (P.val - 1) * (P.val - 1)
            = core.num.Usize.MAX.val * ((P.val - 1) * (P.val - 1)) := by ring
          _ ≤ k.val * ((P.val - 1) * (P.val - 1)) :=
              Nat.mul_le_mul_right _ husize_le_k
          _ = k.val * max_product.val := by rw [← hmp_val]
          _ ≤ core.num.U128.MAX.val := hk_mp_bound
      · -- ¬(k > i4): return cast .Usize k
        simp only [hgt, ite_false, spec, theta, wp_return]
        have hi4_val : i4.val = core.num.Usize.MAX.val := by
          rw [hi4]; exact UScalar.cast_val_mod_pow_greater_numBits_eq .U128
            core.num.Usize.MAX (by cases System.Platform.numBits_eq <;> simp [*])
        have hk_le : k.val ≤ core.num.Usize.MAX.val := by
          have : ¬(i4.val < k.val) := hgt; omega
        have hcast_val : (UScalar.cast .Usize k).val = k.val :=
          UScalar.cast_val_mod_pow_of_inBounds_eq .Usize k (by
            have : core.num.Usize.MAX.val < 2 ^ UScalarTy.Usize.numBits := by scalar_tac
            omega)
        rw [hcast_val]
        calc k.val * (P.val - 1) * (P.val - 1)
            = k.val * ((P.val - 1) * (P.val - 1)) := by ring
          _ = k.val * max_product.val := by rw [← hmp_val]
          _ ≤ core.num.U128.MAX.val := hk_mp_bound
  exact spec_imp_exists hspec

/-- The `max_unreduced_additions` function computes exactly
    `min(u128::MAX / (P-1)², usize::MAX)`, which is the largest value
    representable as `usize` such that accumulating that many `(P-1)²`
    products stays within `u128`.

    Together with `max_unreduced_additions_spec`, this gives the full
    correctness specification: the returned value equals the mathematical
    formula AND satisfies the overflow safety bound. -/
theorem max_unreduced_additions_value {P : Std.U64} (hP : ValidPrime P) :
    ∃ k, gfp.Fp.Insts.Gf2_coreFieldTraitsFiniteFieldU64U128.max_unreduced_additions P = ok k
    ∧ k.val = min (core.num.U128.MAX.val / ((P.val - 1) * (P.val - 1)))
                  core.num.Usize.MAX.val := by
  have hspec :
      gfp.Fp.Insts.Gf2_coreFieldTraitsFiniteFieldU64U128.max_unreduced_additions P
      ⦃ k => k.val = min (core.num.U128.MAX.val / ((P.val - 1) * (P.val - 1)))
                         core.num.Usize.MAX.val ⦄ := by
    unfold gfp.Fp.Insts.Gf2_coreFieldTraitsFiniteFieldU64U128.max_unreduced_additions
    progress as ⟨i, hi⟩
    have hi_val : i.val = P.val := by rw [hi]; exact U64.cast_U128_val_eq P
    progress as ⟨i1, hi1, _⟩
    · have := hP.2.1; have := hi_val; scalar_tac
    progress as ⟨i2, hi2⟩
    have hi2_val : i2.val = P.val := by rw [hi2]; exact U64.cast_U128_val_eq P
    progress as ⟨i3, hi3, _⟩     -- auto-closed
    progress as ⟨max_product, hmp⟩
    · have hi1_val : i1.val = P.val - 1 := by
        have := hi1; have := hi_val; scalar_tac
      have hi3_val : i3.val = P.val - 1 := by
        have := hi3; have := hi2_val; scalar_tac
      rw [hi1_val, hi3_val]
      exact MontArith.p_minus_one_sq_le_u128_max hP
    have hi1_val : i1.val = P.val - 1 := by
      have := hi1; have := hi_val; scalar_tac
    have hi3_val : i3.val = P.val - 1 := by
      have := hi3; have := hi2_val; scalar_tac
    have hmp_val : max_product.val = (P.val - 1) * (P.val - 1) := by
      rw [hmp, hi1_val, hi3_val]
    by_cases hmp0 : max_product = 0#u128
    · exfalso
      have hmv : max_product.val = 0 := by
        have := congrArg UScalar.val hmp0; simpa using this
      rw [hmp_val] at hmv
      have : 0 < P.val - 1 := by have := hP.2.1; omega
      have := Nat.mul_pos this this; omega
    · simp only [show ¬(max_product = 0#u128) from hmp0, ite_false]
      progress as ⟨k, hk⟩
      progress as ⟨i4, hi4⟩
      have hi4_val : i4.val = core.num.Usize.MAX.val := by
        rw [hi4]; exact UScalar.cast_val_mod_pow_greater_numBits_eq .U128
          core.num.Usize.MAX (by cases System.Platform.numBits_eq <;> simp [*])
      have hk_val : k.val = core.num.U128.MAX.val / max_product.val := hk
      -- Connect k.val to the mathematical formula
      have hk_eq : k.val = core.num.U128.MAX.val / ((P.val - 1) * (P.val - 1)) := by
        rw [hk_val, hmp_val]
      by_cases hgt : k > i4
      · -- k > usize_max: return usize_max = min(k, usize_max)
        simp only [hgt, ite_true, spec, theta, wp_return]
        have husize_le : core.num.Usize.MAX.val ≤
            core.num.U128.MAX.val / ((P.val - 1) * (P.val - 1)) := by
          rw [← hk_eq]; have : i4.val < k.val := hgt; omega
        symm; exact Nat.min_eq_right husize_le
      · -- k ≤ usize_max: return k = min(k, usize_max)
        simp only [hgt, ite_false, spec, theta, wp_return]
        have hk_le : k.val ≤ core.num.Usize.MAX.val := by
          have : ¬(i4.val < k.val) := hgt; omega
        have hcast_val : (UScalar.cast .Usize k).val = k.val :=
          UScalar.cast_val_mod_pow_of_inBounds_eq .Usize k (by
            have : core.num.Usize.MAX.val < 2 ^ UScalarTy.Usize.numBits := by scalar_tac
            omega)
        rw [hcast_val, hk_eq]
        symm; exact Nat.min_eq_left (by rw [← hk_eq]; exact hk_le)
  exact spec_imp_exists hspec

/-- The unclamped division `u128::MAX / (P-1)²` is well-defined (positive divisor)
    and its product with `(P-1)²` does not exceed `u128::MAX`.
    Complements `max_unreduced_additions_value` (clamped value equality) and
    `max_unreduced_additions_spec` (overflow bound on the returned value). -/
theorem max_unreduced_additions_div_bound {P : Std.U64} (hP : ValidPrime P) :
    let mp := (P.val - 1) * (P.val - 1)
    0 < mp
    ∧ core.num.U128.MAX.val / mp * mp ≤ core.num.U128.MAX.val := by
  constructor
  · have := hP.2.1; exact Nat.mul_pos (by omega) (by omega)
  · exact Nat.div_mul_le_self _ _

/-! ## mod_pow_mont correctness -/

/-- Montgomery multiplication at the ℕ level: a * b * Rinv mod P.
    This is what redc(a * b as u128) computes for a, b < P. -/
private noncomputable def mont_mul_nat (Rinv P a b : ℕ) : ℕ := a * b * Rinv % P

/-- Specification of the mod_pow_mont loop. Mirrors the code structure exactly,
    with mont_mul_nat as the multiplication primitive. -/
private noncomputable def mont_pow_loop_spec (Rinv P base exp result : ℕ) : ℕ :=
  if exp = 0 then result
  else
    let result' := if exp % 2 = 1 then mont_mul_nat Rinv P result base else result
    let exp' := exp / 2
    let base' := if exp' > 0 then mont_mul_nat Rinv P base base else base
    mont_pow_loop_spec Rinv P base' exp' result'
termination_by exp
decreasing_by omega

private lemma mont_pow_loop_spec_step (Rinv P base exp result : ℕ) (hexp : 0 < exp) :
    mont_pow_loop_spec Rinv P base exp result =
      let result' := if exp % 2 = 1 then mont_mul_nat Rinv P result base else result
      let exp' := exp / 2
      let base' := if exp' > 0 then mont_mul_nat Rinv P base base else base
      mont_pow_loop_spec Rinv P base' exp' result' := by
  rw [mont_pow_loop_spec]; simp [show ¬(exp = 0) from by omega]

private lemma mont_pow_loop_spec_zero (Rinv P base result : ℕ) :
    mont_pow_loop_spec Rinv P base 0 result = result := by
  rw [mont_pow_loop_spec]; simp

/-- The pow loop invariant: values match the spec at every iteration. -/
private def MontPowLoopInv (Rinv P_val init_base init_exp init_result : ℕ)
    (state : Std.U64 × Std.U64 × Std.U64) : Prop :=
  let (base, exp, result) := state
  result.val < P_val ∧
  base.val < P_val ∧
  mont_pow_loop_spec Rinv P_val base.val exp.val result.val =
    mont_pow_loop_spec Rinv P_val init_base init_exp init_result

/-- Connecting redc to mont_mul_nat: redc(a*b) produces a*b*Rinv % P.
    Follows from redc_value_spec (r * R ≡ a*b mod P) and uniqueness of
    the modular inverse solution in [0, P). -/
private lemma redc_eq_mont_mul (P_val : ℕ) (hP_gt1 : 1 < P_val)
    (a_val b_val : ℕ)
    (Rinv : ℕ) (hRinv : MontArith.R * Rinv % P_val = 1 % P_val)
    (r_val : ℕ) (hr_lt : r_val < P_val)
    (hr_eq : r_val * 2 ^ 64 % P_val = (a_val * b_val) % P_val) :
    r_val = mont_mul_nat Rinv P_val a_val b_val := by
  unfold mont_mul_nat
  have hP_pos : 0 < P_val := by omega
  have hRinv1 : MontArith.R * Rinv % P_val = 1 := by
    rw [hRinv, Nat.mod_eq_of_lt hP_gt1]
  -- r * R * Rinv ≡ r (since R * Rinv ≡ 1 mod P)
  have hlhs : r_val * MontArith.R * Rinv % P_val = r_val % P_val := by
    have : r_val * MontArith.R * Rinv = r_val * (MontArith.R * Rinv) := by ring
    rw [this, Nat.mul_mod, hRinv1, mul_one, Nat.mod_mod_of_dvd _ (dvd_refl _)]
  -- r * R * Rinv ≡ a*b * Rinv (since r*R ≡ a*b mod P)
  have hrhs : r_val * MontArith.R * Rinv % P_val = a_val * b_val * Rinv % P_val := by
    conv_lhs => rw [Nat.mul_mod (r_val * MontArith.R) Rinv P_val,
                     show MontArith.R = 2 ^ 64 from rfl, hr_eq, ← Nat.mul_mod]
  -- Combine: r % P = a*b*Rinv % P, and r < P, so r = a*b*Rinv % P
  rw [Nat.mod_eq_of_lt hr_lt] at hlhs
  linarith [hlhs, hrhs]

/-- Helper: obtain redc result with value equation from the mul chain.
    Proves the monadic chain (lift cast, lift cast, mul, redc) returns
    ok r with r < P and r = mont_mul_nat Rinv P a b. -/
private lemma mont_mul_chain_value {P : Std.U64} (a b : Std.U64)
    (hP : ValidPrime P) (hP2 : P.val ≠ 2)
    (ha : a.val < P.val) (hb : b.val < P.val)
    (Rinv : ℕ) (hRinv : MontArith.R * Rinv % P.val = 1 % P.val) :
    ∃ r, (do let i1 ← lift (UScalar.cast .U128 a)
             let i2 ← lift (UScalar.cast .U128 b)
             let i3 ← i1 * i2
             gfp.montgomery.redc P i3) = ok r ∧
    r.val < P.val ∧ r.val = mont_mul_nat Rinv P.val a.val b.val := by
  have hspec : (do let i1 ← lift (UScalar.cast .U128 a)
                   let i2 ← lift (UScalar.cast .U128 b)
                   let i3 ← i1 * i2
                   gfp.montgomery.redc P i3)
      ⦃ r => r.val < P.val ∧ r.val = mont_mul_nat Rinv P.val a.val b.val ⦄ := by
    progress as ⟨i1, hi1⟩
    progress as ⟨i2, hi2⟩
    progress as ⟨i3, hi3⟩
    have hi1_val : i1.val = a.val := by rw [hi1]; exact U64.cast_U128_val_eq a
    have hi2_val : i2.val = b.val := by rw [hi2]; exact U64.cast_U128_val_eq b
    have hi3_val : i3.val = a.val * b.val := by rw [hi3, hi1_val, hi2_val]
    have hbound : i3.val < P.val * 2 ^ 64 :=
      FpProgress.redc_precond hP ha hb hi3_val
    obtain ⟨r, hr_eq, hr_lt, hr_val⟩ := redc_value_spec hP hP2 hbound
    rw [show spec = theta from rfl, hr_eq]
    simp only [theta, wp_return]
    exact ⟨hr_lt, redc_eq_mont_mul P.val (by have := hP.2.1; omega)
      a.val b.val Rinv hRinv r.val hr_lt (by rw [hr_val, hi3_val])⟩
  exact spec_imp_exists hspec

/-- The mod_pow_mont loop computes mont_pow_loop_spec. -/
private theorem mont_pow_loop_correct {P : Std.U64}
    (hP : ValidPrime P) (hP2 : P.val ≠ 2)
    (Rinv : ℕ) (hRinv : MontArith.R * Rinv % P.val = 1 % P.val)
    (base exp result : Std.U64)
    (hinv : MontPowLoopInv Rinv P.val base.val exp.val result.val (base, exp, result)) :
    gfp.montgomery.mod_pow_mont_loop P base exp result
    ⦃ r => r.val = mont_pow_loop_spec Rinv P.val base.val exp.val result.val ∧
            r.val < P.val ⦄ := by
  obtain ⟨hr_bound, hb_bound, hstate⟩ := hinv
  unfold gfp.montgomery.mod_pow_mont_loop
  apply loop.spec (γ := ℕ)
    (measure := fun ((_, exp1, _) : Std.U64 × Std.U64 × Std.U64) => exp1.val)
    (inv := MontPowLoopInv Rinv P.val base.val exp.val result.val)
  · intro ⟨base1, exp1, result1⟩ ⟨hr1, hb1, hst1⟩
    dsimp only
    by_cases hgt : exp1 > 0#u64
    · simp only [hgt, ite_true, Std.lift, bind_tc_ok]
      have hexp1_pos : 0 < exp1.val := by scalar_tac
      -- Split on if (exp1 &&& 1) = 1
      split
      · -- Odd: multiply result by base
        rename_i h_bit
        have h_bit_mod : exp1.val % 2 = 1 := by
          have h1 : (exp1 &&& 1#u64).val = exp1.val % 2 := by
            rw [UScalar.val_and]; simp [Nat.and_one_is_mod]
          have h2 : (exp1 &&& 1#u64) = 1#u64 := h_bit
          have h3 : (1#u64 : Std.U64).val = 1 := by native_decide
          have h4 : (exp1 &&& 1#u64).val = 1 := by rw [h2, h3]
          linarith [h1, h4]
        -- Progress through the U128 multiply
        progress as ⟨prod, hprod⟩  -- U128 mul
        -- Use redc_value_spec directly (to get both bound and value equation)
        have hprod_val : prod.val = result1.val * base1.val := by
          rw [hprod, U64.cast_U128_val_eq result1, U64.cast_U128_val_eq base1]
        have hbound : prod.val < P.val * 2 ^ 64 :=
          FpProgress.redc_precond hP hr1 hb1 hprod_val
        obtain ⟨result2, hresult2_eq, hresult2, hresult2_rval⟩ :=
          redc_value_spec hP hP2 hbound
        simp only [hresult2_eq, bind_tc_ok]
        -- Value equation for result2
        have hresult2_val : result2.val =
            mont_mul_nat Rinv P.val result1.val base1.val :=
          redc_eq_mont_mul P.val hP.2.1 result1.val base1.val Rinv hRinv
            result2.val hresult2 (by rw [hresult2_rval, hprod_val])
        -- Continue: exp shift
        progress as ⟨exp2, hexp2_val, _⟩
        have hexp2_eq : exp2.val = exp1.val / 2 := by
          simp [hexp2_val, Nat.shiftRight_eq_div_pow]
        have hexp2_lt : exp2.val < exp1.val := by
          rw [hexp2_eq]; exact Nat.div_lt_self hexp1_pos (by norm_num)
        split
        · -- exp2 > 0: square base
          progress as ⟨sq, hsq⟩
          -- Use redc_value_spec directly for squaring
          have hsq_val : sq.val = base1.val * base1.val := by
            rw [hsq, U64.cast_U128_val_eq base1]
          have hbound_sq : sq.val < P.val * 2 ^ 64 :=
            FpProgress.redc_precond hP hb1 hb1 hsq_val
          obtain ⟨base2, hbase2_eq, hbase2, hbase2_rval⟩ :=
            redc_value_spec hP hP2 hbound_sq
          simp only [hbase2_eq, bind_tc_ok]
          have hbase2_val : base2.val =
              mont_mul_nat Rinv P.val base1.val base1.val :=
            redc_eq_mont_mul P.val hP.2.1 base1.val base1.val Rinv hRinv
              base2.val hbase2 (by rw [hbase2_rval, hsq_val])
          refine And.intro ⟨hresult2, hbase2, ?_⟩ hexp2_lt
          rw [← hst1, mont_pow_loop_spec_step _ _ _ _ _ hexp1_pos]
          simp only [h_bit_mod, ite_true, hexp2_eq,
            show exp1.val / 2 > 0 from by scalar_tac, ite_true]
          congr 1 <;> assumption
        · -- exp2 = 0: don't square
          refine And.intro ⟨hresult2, hb1, ?_⟩ hexp2_lt
          rw [← hst1, mont_pow_loop_spec_step _ _ _ _ _ hexp1_pos]
          simp only [h_bit_mod, ite_true, hexp2_eq,
            show ¬(exp1.val / 2 > 0) from by scalar_tac, ite_false, hresult2_val]
      · -- Even: result unchanged
        rename_i h_bit
        have h_bit_mod : ¬(exp1.val % 2 = 1) := by
          have h1 : (exp1 &&& 1#u64).val = exp1.val % 2 := by
            rw [UScalar.val_and]; simp [Nat.and_one_is_mod]
          have h2 : ¬((exp1 &&& 1#u64) = 1#u64) := h_bit
          intro h3
          apply h2
          apply UScalar.val_eq_imp
          have h4 : (1#u64 : Std.U64).val = 1 := by native_decide
          rw [h1, h4, h3]
        progress as ⟨exp2, hexp2_val, _⟩
        have hexp2_eq : exp2.val = exp1.val / 2 := by
          simp [hexp2_val, Nat.shiftRight_eq_div_pow]
        have hexp2_lt : exp2.val < exp1.val := by
          rw [hexp2_eq]; exact Nat.div_lt_self hexp1_pos (by norm_num)
        split
        · -- exp2 > 0: square base
          progress as ⟨sq, hsq⟩
          -- Use redc_value_spec directly for squaring
          have hsq_val : sq.val = base1.val * base1.val := by
            rw [hsq, U64.cast_U128_val_eq base1]
          have hbound_sq : sq.val < P.val * 2 ^ 64 :=
            FpProgress.redc_precond hP hb1 hb1 hsq_val
          obtain ⟨base2, hbase2_eq, hbase2, hbase2_rval⟩ :=
            redc_value_spec hP hP2 hbound_sq
          simp only [hbase2_eq, bind_tc_ok]
          have hbase2_val : base2.val =
              mont_mul_nat Rinv P.val base1.val base1.val :=
            redc_eq_mont_mul P.val hP.2.1 base1.val base1.val Rinv hRinv
              base2.val hbase2 (by rw [hbase2_rval, hsq_val])
          refine And.intro ⟨hr1, hbase2, ?_⟩ hexp2_lt
          rw [← hst1, mont_pow_loop_spec_step _ _ _ _ _ hexp1_pos]
          simp only [h_bit_mod, ite_false, hexp2_eq,
            show exp1.val / 2 > 0 from by scalar_tac, ite_true, hbase2_val]
        · -- exp2 = 0: don't square
          refine And.intro ⟨hr1, hb1, ?_⟩ hexp2_lt
          rw [← hst1, mont_pow_loop_spec_step _ _ _ _ _ hexp1_pos]
          simp only [h_bit_mod, ite_false, hexp2_eq,
            show ¬(exp1.val / 2 > 0) from by scalar_tac, ite_false]
    · -- exp = 0: done
      simp only [hgt, ite_false, spec, theta, wp_return]
      have hexp0 : exp1.val = 0 := by scalar_tac
      constructor
      · rw [← hst1, hexp0, mont_pow_loop_spec_zero]
      · exact hr1
  · exact ⟨hr_bound, hb_bound, hstate⟩

/-- mont_mul_nat produces values less than P. -/
private lemma mont_mul_nat_lt (Rinv P a b : ℕ) (hP : 0 < P) :
    mont_mul_nat Rinv P a b < P := by
  exact Nat.mod_lt _ hP

/-- mod_pow_mont correctly computes mont_pow_loop_spec and produces a valid field element. -/
theorem mod_pow_mont_correct {P : Std.U64}
    (hP : ValidPrime P) (hP2 : P.val ≠ 2)
    (base exp : Std.U64) (hb : base.val < P.val) :
    ∃ r Rinv, MontArith.R * Rinv % P.val = 1 % P.val ∧
    gfp.montgomery.mod_pow_mont P base exp = ok r ∧
    r.val = mont_pow_loop_spec Rinv P.val base.val exp.val (2 ^ 64 % P.val) ∧
    r.val < P.val := by
  obtain ⟨Rinv, hRinv⟩ := MontArith.R_inv_exists hP hP2
  unfold gfp.montgomery.mod_pow_mont
  simp only [gfp.montgomery.MontConsts.R_MOD_P]
  obtain ⟨rmod, hrmod_eq, hrmod_val⟩ := compute_r_mod_p_value hP
  simp only [hrmod_eq, bind_tc_ok]
  have hrmod_lt : rmod.val < P.val := by
    rw [hrmod_val]; exact Nat.mod_lt _ (by have := hP.2.1; omega)
  have hinv : MontPowLoopInv Rinv P.val base.val exp.val rmod.val (base, exp, rmod) :=
    ⟨hrmod_lt, hb, rfl⟩
  have h := mont_pow_loop_correct hP hP2 Rinv hRinv base exp rmod hinv
  obtain ⟨r, hr_eq, hr_val, hr_lt⟩ := spec_imp_exists h
  exact ⟨r, Rinv, hRinv, hr_eq, by rw [hr_val, hrmod_val], hr_lt⟩

end MontRoundtrip

end
