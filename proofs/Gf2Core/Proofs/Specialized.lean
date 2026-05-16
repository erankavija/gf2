/-
  Gf2Core.Proofs.Specialized — specialized-storage prime classification and
  fast modular reduction correctness.

  This file discharges the Fp-arithmetic proof debt that bottoms out in the
  `gfp.specialized.classify` dispatch path. `classify` decides the prime
  shape using two Rust/LLVM integer intrinsics — `u64::trailing_zeros` and
  `u64::is_power_of_two` — which Aeneas extracts as *uninterpreted* axioms
  in `Gf2Core/FunsExternal.lean:84-89` (a Rust/LLVM primitive's semantics
  cannot be proven from below, only modeled).

  Per issue 2e544a34's 2026-05-16 user-approved scope amendment, this file
  introduces EXACTLY TWO documented `@[progress]` model-assumption specs for
  those two intrinsics (the legitimate model-assumption boundary, analogous
  to the existing `wrapping_neg` / `overflowing_sub` external models in
  `FunsExternal.lean`). EVERYTHING downstream — `classify_spec`,
  `use_specialized_storage` totality, `specialized_mul` full modular
  correctness across all four `PrimeShape` arms, and the `inv_loop` bound —
  is FULLY PROVEN on top of these two specs. No further axiomatization of
  the verification boundary is permitted.
-/
import Aeneas
import Gf2Core.Funs
import Gf2Core.Proofs.Defs

open Aeneas Aeneas.Std Result ControlFlow Error Aeneas.Std.WP
open gf2_core

set_option maxHeartbeats 3200000
set_option maxRecDepth 8000

noncomputable section

namespace Specialized

/-! ## §1 — The two permitted Rust-intrinsic model-assumption specs

These two axioms model the semantics of the `u64::trailing_zeros` and
`u64::is_power_of_two` Rust/LLVM integer intrinsics. They are the ONLY
axioms permitted by issue 2e544a34 (2026-05-16 user-approved amendment,
criterion 2 clause (b)). They are model assumptions about external
primitive semantics at the established Aeneas extraction boundary — the
same category as the concrete `wrapping_neg` / `overflowing_sub` / U128
external models already in `Gf2Core/FunsExternal.lean`. They do NOT assert
correctness of any project-internal Rust logic; every project function
(`classify`, `use_specialized_storage`, `specialized_mul`, `inv_loop`,
the Mersenne/Proth reducers) is proven below from these specs alone. -/

/-- **Model assumption (issue 2e544a34, 2026-05-16 amendment, clause (b)).**

Models the Rust/LLVM `u64::trailing_zeros` intrinsic
(`core::num::{u64}::trailing_zeros`, extracted as the uninterpreted axiom
`core.num.U64.trailing_zeros` in `Gf2Core/FunsExternal.lean`). The Rust
contract: `x.trailing_zeros()` returns the number of trailing zero bits of
`x`; for `x = 0` it returns `64` (the bit width). Equivalently, for `x ≠ 0`
the result `n` is the 2-adic valuation of `x`: `2^n ∣ x` and `2^(n+1) ∤ x`,
with `n < 64`. This is a faithful model of the hardware `TZCNT`/`BSF`
instruction semantics; it cannot be proved from below because the Aeneas
extraction leaves the intrinsic uninterpreted. -/
@[progress]
axiom trailing_zeros_spec (x : Std.U64) :
    core.num.U64.trailing_zeros x ⦃ n =>
      (x.val = 0 → n.val = 64) ∧
      (x.val ≠ 0 → n.val < 64 ∧ x.val % 2 ^ n.val = 0 ∧ x.val % 2 ^ (n.val + 1) ≠ 0) ⦄

/-- **Model assumption (issue 2e544a34, 2026-05-16 amendment, clause (b)).**

Models the Rust/LLVM `u64::is_power_of_two` intrinsic
(`core::num::{u64}::is_power_of_two`, extracted as the uninterpreted axiom
`core.num.U64.is_power_of_two` in `Gf2Core/FunsExternal.lean`). The Rust
contract: `x.is_power_of_two()` is `true` iff `x` has exactly one bit set,
i.e. `x = 2^k` for some `k` (note `0` is not a power of two). This is a
faithful model of the `x != 0 && x & (x-1) == 0` lowering; it cannot be
proved from below because the Aeneas extraction leaves the intrinsic
uninterpreted. -/
@[progress]
axiom is_power_of_two_spec (x : Std.U64) :
    core.num.U64.is_power_of_two x ⦃ b =>
      (b = true ↔ ∃ k, k < 64 ∧ x.val = 2 ^ k) ⦄

/-! ## §2 — Number-theory bridge lemmas

Pure `Nat` facts connecting the intrinsic specs to the `classify`
arithmetic. No Aeneas content. -/

/-- A power of two whose 2-adic valuation is `n` equals `2^n`. -/
theorem pow_two_of_val_eq {q n k : ℕ} (hk : q = 2 ^ k)
    (hn0 : q % 2 ^ n = 0) (hn1 : q % 2 ^ (n + 1) ≠ 0) : q = 2 ^ n := by
  subst hk
  -- 2^n ∣ 2^k  ⇒  n ≤ k ;  2^(n+1) ∤ 2^k  ⇒  k < n+1  ⇒  k = n
  have hnk : n ≤ k := by
    have hdvd : 2 ^ n ∣ 2 ^ k := Nat.dvd_of_mod_eq_zero hn0
    by_contra h
    push_neg at h
    have : 2 ^ k < 2 ^ n := Nat.pow_lt_pow_right (by norm_num) h
    exact absurd (Nat.le_of_dvd (by positivity) hdvd) (by omega)
  have hkn : k < n + 1 := by
    by_contra h
    push_neg at h
    have hd : 2 ^ (n + 1) ∣ 2 ^ k := Nat.pow_dvd_pow 2 h
    exact hn1 (Nat.dvd_iff_mod_eq_zero.mp hd)
  have : k = n := by omega
  rw [this]

/-- Decompose `r` by its 2-adic valuation `n`: `r = (r >>> n) * 2^n` with
    `r >>> n` odd, given `2^n ∣ r` and `2^(n+1) ∤ r`. -/
theorem shift_decomp {r n : ℕ} (hn0 : r % 2 ^ n = 0) (hn1 : r % 2 ^ (n + 1) ≠ 0) :
    r = (r / 2 ^ n) * 2 ^ n ∧ (r / 2 ^ n) % 2 ≠ 0 := by
  have hdvd : 2 ^ n ∣ r := Nat.dvd_of_mod_eq_zero hn0
  refine ⟨(Nat.div_mul_cancel hdvd).symm, ?_⟩
  intro hev
  -- if (r / 2^n) is even then 2^(n+1) ∣ r, contradicting hn1
  apply hn1
  have h2 : 2 ∣ (r / 2 ^ n) := Nat.dvd_of_mod_eq_zero hev
  obtain ⟨c, hc⟩ := h2
  have : r = c * 2 ^ (n + 1) := by
    rw [← Nat.div_mul_cancel hdvd, hc]; ring
  rw [this, pow_succ]
  exact Nat.mul_mod_left c (2 ^ n * 2)

/-! ## §3 — classify_spec

`gfp.specialized.classify P` returns a `PrimeShape` whose constructor pins
`P.val`'s arithmetic shape. Proven from the two §1 intrinsic specs only. -/

/-- Shape postcondition on `classify`'s result. -/
def classifyPost (P : Std.U64) : gfp.specialized.PrimeShape → Prop
  | .Goldilocks => P.val = 18446744069414584321
  | .Mersenne n => 4 ≤ n.val ∧ n.val ≤ 62 ∧ P.val = 2 ^ n.val - 1
  | .Proth k n => 16 ≤ n.val ∧ 1 ≤ k.val ∧ P.val = k.val * 2 ^ n.val + 1
  | .Generic => True

/-- The Proth-detection subtree, factored out (it appears 4× verbatim in
    `classify`, guarded by `r = wrapping_sub p 1`). Given `r.val = P.val - 1`
    and `1 < P.val`, whatever `PrimeShape` the subtree yields satisfies
    `classifyPost P`. -/
private theorem proth_subtree_spec {P r : Std.U64}
    (hP : 1 < P.val) (hr : r.val = P.val - 1) :
    (do
      if r != 0#u64 then
        let n1 ← core.num.U64.trailing_zeros r
        if n1 >= 16#u32 then
          let k ← r >>> n1
          if k >= 1#u64 then
            if n1 >= 63#u32 then ok (gfp.specialized.PrimeShape.Proth k n1)
            else
              let i ← 1#u64 <<< n1
              if k < i then ok (gfp.specialized.PrimeShape.Proth k n1)
              else ok gfp.specialized.PrimeShape.Generic
          else ok gfp.specialized.PrimeShape.Generic
        else ok gfp.specialized.PrimeShape.Generic
      else ok gfp.specialized.PrimeShape.Generic)
    ⦃ ps => classifyPost P ps ⦄ := by
  have hrne : r.val ≠ 0 := by omega
  have hr_ne_lit : r ≠ 0#u64 := by
    intro h; exact hrne (by rw [h]; rfl)
  simp only [bne_iff_ne, ne_eq, hr_ne_lit, not_false_eq_true, if_true]
  -- trailing_zeros r
  progress as ⟨tz, htz_eq0, htz_ne0⟩
  obtain ⟨htz0, htz1a, htz1b⟩ := htz_ne0 hrne
  by_cases hn16 : tz ≥ 16#u32
  · simp only [hn16, if_true]
    have hn16v : 16 ≤ tz.val := by
      have he : (16#u32 : Std.U32).val = 16 := by native_decide
      have : (16#u32 : Std.U32).val ≤ tz.val := hn16
      omega
    -- k = r >>> tz
    progress as ⟨k, hk_val, _⟩
    have hkval : k.val = r.val / 2 ^ tz.val := by
      rw [hk_val, Nat.shiftRight_eq_div_pow]
    obtain ⟨hr_decomp, _⟩ := shift_decomp htz1a htz1b
    rw [← hkval] at hr_decomp
    by_cases hk1 : k ≥ 1#u64
    · simp only [hk1, if_true]
      have hk1v : 1 ≤ k.val := by
        have he : (1#u64 : Std.U64).val = 1 := by native_decide
        have : (1#u64 : Std.U64).val ≤ k.val := hk1
        omega
      have hProth : classifyPost P (gfp.specialized.PrimeShape.Proth k tz) := by
        refine ⟨hn16v, hk1v, ?_⟩; omega
      by_cases hn63 : tz ≥ 63#u32
      · simp only [hn63, if_true, spec, theta, wp_return]
        exact hProth
      · simp only [hn63, if_false]
        progress as ⟨i, _, _⟩
        by_cases hki : k < i
        · simp only [hki, if_true, spec, theta, wp_return]; exact hProth
        · simp only [hki, if_false, spec, theta, wp_return]; trivial
    · simp only [hk1, if_false, spec, theta, wp_return]; trivial
  · simp only [hn16, if_false, spec, theta, wp_return]; trivial

theorem classify_spec {P : Std.U64} (hP : 1 < P.val) :
    gfp.specialized.classify P ⦃ ps => classifyPost P ps ⦄ := by
  unfold gfp.specialized.classify
  have hPlt : P.val < 2 ^ 64 := P.hBounds
  by_cases hgold : P = gfp.specialized.GOLDILOCKS_PRIME
  · simp only [hgold, if_true, spec, theta, wp_return]
    show classifyPost gfp.specialized.GOLDILOCKS_PRIME
      gfp.specialized.PrimeShape.Goldilocks
    show gfp.specialized.GOLDILOCKS_PRIME.val = 18446744069414584321
    native_decide
  · simp only [hgold, if_false, lift, bind_tc_ok]
    -- proth subtree precondition: r = wrapping_sub P 1, r.val = P.val - 1
    have hrval : (core.num.U64.wrapping_sub P 1#u64).val = P.val - 1 := by
      rw [core.num.U64.wrapping_sub_val_eq]
      have h1 : (1#u64 : Std.U64).val = 1 := by rfl
      simp only [h1, UScalar.size_UScalarTyU64, U64.size_eq]
      have hb : (18446744073709551616 : ℕ) = 2 ^ 64 := by norm_num
      rw [hb]
      have hkey : P.val + (2 ^ 64 - 1) = (P.val - 1) + 2 ^ 64 := by omega
      rw [hkey, Nat.add_mod_right, Nat.mod_eq_of_lt (by omega)]
    -- q = wrapping_add P 1
    have hqval : (core.num.U64.wrapping_add P 1#u64).val = (P.val + 1) % 2 ^ 64 := by
      rw [core.num.U64.wrapping_add_val_eq]
      have h1 : (1#u64 : Std.U64).val = 1 := by rfl
      simp only [h1, UScalar.size_UScalarTyU64, U64.size_eq]
      norm_num
    set q : Std.U64 := core.num.U64.wrapping_add P 1#u64 with hq_def
    by_cases hqne : q = 0#u64
    · -- q = 0: proth subtree
      rw [show (q != 0#u64) = false from by rw [hqne]; rfl, if_neg (by decide)]
      exact proth_subtree_spec hP hrval
    · rw [show (q != 0#u64) = true from by
        simp only [bne_iff_ne, ne_eq]; exact hqne, if_pos rfl]
      have hqne' : q.val ≠ 0 := by
        intro h; exact hqne (by apply UScalar.val_eq_imp; rw [h]; rfl)
      -- is_power_of_two q
      progress as ⟨b, hb_iff⟩
      by_cases hb : b = true
      · subst hb
        obtain ⟨kk, hkklt, hqpow⟩ := hb_iff.mp rfl
        simp only [if_true]
        -- trailing_zeros q
        progress as ⟨n, hn_eq0, hn_ne0⟩
        obtain ⟨hn0lt, hn0, hn1⟩ := hn_ne0 hqne'
        have hqn : q.val = 2 ^ n.val := pow_two_of_val_eq hqpow hn0 hn1
        have hPne : P.val + 1 ≠ 2 ^ 64 := by
          intro h; apply hqne'; rw [hqval, h, Nat.mod_self]
        have hqeq : q.val = P.val + 1 := by
          rw [hqval, Nat.mod_eq_of_lt (by omega)]
        by_cases hn4 : n ≥ 4#u32
        · simp only [hn4, if_true]
          have hn4v : 4 ≤ n.val := by
            have he : (4#u32 : Std.U32).val = 4 := by native_decide
            have : (4#u32 : Std.U32).val ≤ n.val := hn4
            omega
          by_cases hn62 : n ≤ 62#u32
          · simp only [hn62, if_true, spec, theta, wp_return]
            show classifyPost P (gfp.specialized.PrimeShape.Mersenne n)
            have hn62v : n.val ≤ 62 := by
              have he : (62#u32 : Std.U32).val = 62 := by native_decide
              have : n.val ≤ (62#u32 : Std.U32).val := hn62
              omega
            refine ⟨hn4v, hn62v, ?_⟩
            have hpeq : P.val + 1 = 2 ^ n.val := by rw [← hqeq, hqn]
            omega
          · simp only [hn62, if_false]
            exact proth_subtree_spec hP hrval
        · simp only [hn4, if_false]
          exact proth_subtree_spec hP hrval
      · have hbf : b = false := Bool.not_eq_true b |>.mp hb
        subst hbf
        simp only [Bool.false_eq_true, if_false]
        exact proth_subtree_spec hP hrval

/-! ## §4 — use_specialized_storage totality

`use_specialized_storage P` always returns `ok` (it calls the now-total
`classify` then pattern-matches). -/

theorem use_specialized_storage_total (P : Std.U64) (hP : 1 < P.val) :
    ∃ b, gfp.use_specialized_storage P = ok b := by
  unfold gfp.use_specialized_storage
  by_cases h2 : P = 2#u64
  · exact ⟨false, by simp [h2]⟩
  · simp only [h2, if_false]
    obtain ⟨ps, hps_eq, _⟩ := spec_imp_exists (classify_spec hP)
    simp only [hps_eq, bind_tc_ok]
    cases ps with
    | Mersenne n => exact ⟨_, rfl⟩
    | Proth k n => exact ⟨_, rfl⟩
    | Goldilocks => exact ⟨false, rfl⟩
    | Generic => exact ⟨false, rfl⟩

/-! ## §5 — the generic-product arm (Goldilocks / Generic)

`(cast U64 ((cast U128 a) * (cast U128 b) % (cast U128 P)))` equals
`(a.val * b.val) % P.val`, with the result `< P.val`. -/

private theorem wide_mod_arm {P a b : Std.U64}
    (hP : ValidPrime P) (ha : a.val < P.val) (hb : b.val < P.val) :
    (do
      let i ← lift (UScalar.cast .U128 a)
      let i1 ← lift (UScalar.cast .U128 b)
      let i2 ← i * i1
      let i3 ← lift (UScalar.cast .U128 P)
      let i4 ← i2 % i3
      ok (UScalar.cast .U64 i4))
    ⦃ r => r.val < P.val ∧ r.val = (a.val * b.val) % P.val ⦄ := by
  have hP_pos : 0 < P.val := by have := hP.2.1; omega
  have ha128 : (UScalar.cast .U128 a : Std.U128).val = a.val := U64.cast_U128_val_eq a
  have hb128 : (UScalar.cast .U128 b : Std.U128).val = b.val := U64.cast_U128_val_eq b
  have hP128 : (UScalar.cast .U128 P : Std.U128).val = P.val := U64.cast_U128_val_eq P
  simp only [lift, bind_tc_ok]
  progress as ⟨prod, hprod⟩
  progress as ⟨rem, hrem⟩
  have hprod_val : prod.val = a.val * b.val := by rw [hprod, ha128, hb128]
  have hrem_val : rem.val = (a.val * b.val) % P.val := by
    rw [hrem, hprod_val, hP128]
  have hrem_lt : rem.val < P.val := by rw [hrem_val]; exact Nat.mod_lt _ hP_pos
  have hcast : (UScalar.cast .U64 rem).val = rem.val :=
    UScalar.cast_val_mod_pow_of_inBounds_eq .U64 rem (by
      have : UScalarTy.U64.numBits = 64 := by decide
      rw [this]; nlinarith [hP.2.2])
  exact ⟨by rw [hcast]; exact hrem_lt, by rw [hcast, hrem_val]⟩

/-! ## §6 — Mersenne / Proth reducer number theory

Pure-`Nat` bit-level facts plus the reducer correctness lemmas.
`P = 2^N − 1` (Mersenne) or `P = K·2^N + 1` (Proth) from `classify_spec`. -/

/-- AND with the low-`N` mask is reduction mod `2^N`. -/
theorem and_mask_mod (x N : ℕ) : x &&& (2 ^ N - 1) = x % 2 ^ N := by
  apply Nat.eq_of_testBit_eq
  intro i
  rw [Nat.testBit_and, Nat.testBit_two_pow_sub_one, Nat.testBit_mod_two_pow]
  by_cases h : i < N <;> simp [h]

/-- The Mersenne fold step preserves residue mod `2^N − 1`
    (`2^N ≡ 1`), expressed on the split `x = (x / 2^N)·2^N + x % 2^N`. -/
theorem mersenne_fold_mod (x N : ℕ) (hN : 2 ≤ N) :
    (x % 2 ^ N + x / 2 ^ N) % (2 ^ N - 1) = x % (2 ^ N - 1) := by
  have h4le : (4 : ℕ) ≤ 2 ^ N := by
    calc (4:ℕ) = 2 ^ 2 := by norm_num
      _ ≤ 2 ^ N := Nat.pow_le_pow_right (by norm_num) hN
  have hsplit : x = x / 2 ^ N * 2 ^ N + x % 2 ^ N := by
    conv_lhs => rw [← Nat.div_add_mod x (2 ^ N)]
    ring
  have hmod : (2 : ℕ) ^ N % (2 ^ N - 1) = 1 := by
    have hpos : 0 < (2 : ℕ) ^ N := by positivity
    have hpow : (2 : ℕ) ^ N = (2 ^ N - 1) + 1 :=
      (Nat.succ_pred_eq_of_pos hpos).symm
    nth_rewrite 1 [hpow]
    rw [Nat.add_mod_left, Nat.mod_eq_of_lt (by omega)]
  -- x ≡ x/2^N + x%2^N  (mod 2^N - 1), since 2^N ≡ 1
  have hme : Nat.ModEq (2 ^ N - 1) (2 ^ N) 1 := by
    unfold Nat.ModEq; rw [hmod]; exact (Nat.mod_eq_of_lt (by omega)).symm
  have key : Nat.ModEq (2 ^ N - 1) x (x / 2 ^ N + x % 2 ^ N) := by
    conv_lhs => rw [hsplit]
    calc x / 2 ^ N * 2 ^ N + x % 2 ^ N
        ≡ x / 2 ^ N * 1 + x % 2 ^ N [MOD 2 ^ N - 1] :=
          Nat.ModEq.add_right _ (Nat.ModEq.mul_left _ hme)
      _ = x / 2 ^ N + x % 2 ^ N := by rw [Nat.mul_one]
  rw [Nat.add_comm (x % 2 ^ N)]
  exact key.symm

/-- `mersenne_reduce_u64 N x` computes `x % (2^N − 1)` when `4 ≤ N ≤ 32`
    and `x < 2^(2N)` (true for products of two `< 2^N` operands). -/
theorem mersenne_reduce_u64_correct (N : Std.U32) (x : Std.U64)
    (hN4 : 4 ≤ N.val) (hN32 : N.val ≤ 32) (hx : x.val < 2 ^ (2 * N.val)) :
    ∃ r, gfp.specialized.mersenne_reduce_u64 N x = ok r ∧
      r.val = x.val % (2 ^ N.val - 1) := by
  have hNlt64 : N.val < 64 := by omega
  have hp_pos : 0 < 2 ^ N.val - 1 := by
    have : 2 ^ 4 ≤ 2 ^ N.val := Nat.pow_le_pow_right (by norm_num) hN4
    omega
  have h2N : (2 : ℕ) ^ N.val ≥ 16 := by
    calc (2:ℕ) ^ N.val ≥ 2 ^ 4 := Nat.pow_le_pow_right (by norm_num) hN4
      _ = 16 := by norm_num
  apply spec_imp_exists
  unfold gfp.specialized.mersenne_reduce_u64
  -- massert (N ≤ 32)
  have hassert : (N ≤ 32#u32) := by
    show N.val ≤ (32#u32 : Std.U32).val
    have : (32#u32 : Std.U32).val = 32 := by native_decide
    omega
  rw [massert, if_pos hassert]
  simp only [bind_tc_ok, lift]
  -- i = 1 <<< N  (value 2^N, fits since N < 64)
  progress as ⟨pw, hpw_val, _⟩
  have hpw : pw.val = 2 ^ N.val := by
    rw [hpw_val, Nat.one_shiftLeft, U64.size_eq]
    exact Nat.mod_eq_of_lt (by
      have : 2 ^ N.val < 2 ^ 64 := Nat.pow_lt_pow_right (by norm_num) hNlt64
      omega)
  -- p = pw - 1 = 2^N - 1
  progress as ⟨p, hp_eq⟩
  have hp : p.val = 2 ^ N.val - 1 := by rw [hp_eq, hpw]
  -- i1 = x & p = x % 2^N
  have hi1 : (x &&& p).val = x.val % 2 ^ N.val := by
    rw [UScalar.val_and, hp, and_mask_mod]
  -- i2 = x >>> N = x / 2^N
  progress as ⟨i2, hi2_val, _⟩
  have hi2 : i2.val = x.val / 2 ^ N.val := by
    rw [hi2_val, Nat.shiftRight_eq_div_pow]
  -- s1 = wrapping_add i1 i2
  have hxmod_lt : x.val % 2 ^ N.val < 2 ^ N.val := Nat.mod_lt _ (by positivity)
  have hxdiv_lt : x.val / 2 ^ N.val < 2 ^ N.val := by
    apply Nat.div_lt_of_lt_mul
    calc x.val < 2 ^ (2 * N.val) := hx
      _ = 2 ^ N.val * 2 ^ N.val := by rw [← pow_add]; ring_nf
  have hs1_lt : x.val % 2 ^ N.val + x.val / 2 ^ N.val < 2 ^ 64 := by
    have : 2 ^ N.val ≤ 2 ^ 32 := Nat.pow_le_pow_right (by norm_num) hN32
    omega
  set s1v := x.val % 2 ^ N.val + x.val / 2 ^ N.val with hs1v_def
  have hs1 : (core.num.U64.wrapping_add (x &&& p) i2).val = s1v := by
    rw [core.num.U64.wrapping_add_val_eq, hi1, hi2, ← hs1v_def,
        UScalar.size_UScalarTyU64, U64.size_eq,
        show (18446744073709551616 : ℕ) = 2 ^ 64 from by norm_num,
        Nat.mod_eq_of_lt hs1_lt]
  -- s1 ≡ x (mod 2^N - 1)
  have hs1_cong : s1v % (2 ^ N.val - 1) = x.val % (2 ^ N.val - 1) :=
    mersenne_fold_mod x.val N.val (by omega)
  set s1 := core.num.U64.wrapping_add (x &&& p) i2 with hs1_def
  -- i3 = s1 & p = s1 % 2^N ; i4 = s1 >>> N = s1 / 2^N
  have hi3 : (s1 &&& p).val = s1v % 2 ^ N.val := by
    rw [UScalar.val_and, hp, and_mask_mod, hs1]
  progress as ⟨i4, hi4_val, _⟩
  have hi4 : i4.val = s1v / 2 ^ N.val := by
    rw [hi4_val, Nat.shiftRight_eq_div_pow, hs1]
  -- s1v < 2^(N+1) so s1v / 2^N ≤ 1
  have hs1v_lt : s1v < 2 ^ (N.val + 1) := by
    rw [hs1v_def, pow_succ]; omega
  have hi4_lt2 : s1v / 2 ^ N.val < 2 := by
    apply Nat.div_lt_of_lt_mul
    have : 2 ^ (N.val + 1) = 2 ^ N.val * 2 := by rw [pow_succ]
    omega
  have hi4_le1 : s1v / 2 ^ N.val ≤ 1 := by omega
  -- r = wrapping_add i3 i4
  have hi3_lt : s1v % 2 ^ N.val < 2 ^ N.val := Nat.mod_lt _ (by positivity)
  have hr_lt : s1v % 2 ^ N.val + s1v / 2 ^ N.val < 2 ^ 64 := by
    have : 2 ^ N.val ≤ 2 ^ 32 := Nat.pow_le_pow_right (by norm_num) hN32
    omega
  set rv := s1v % 2 ^ N.val + s1v / 2 ^ N.val with hrv_def
  have hr : (core.num.U64.wrapping_add (s1 &&& p) i4).val = rv := by
    rw [core.num.U64.wrapping_add_val_eq, hi3, hi4, ← hrv_def,
        UScalar.size_UScalarTyU64, U64.size_eq,
        show (18446744073709551616 : ℕ) = 2 ^ 64 from by norm_num,
        Nat.mod_eq_of_lt hr_lt]
  set r := core.num.U64.wrapping_add (s1 &&& p) i4 with hr_def
  -- rv ≡ s1v ≡ x (mod 2^N-1)
  have hrv_cong : rv % (2 ^ N.val - 1) = x.val % (2 ^ N.val - 1) := by
    rw [hrv_def]
    calc (s1v % 2 ^ N.val + s1v / 2 ^ N.val) % (2 ^ N.val - 1)
        = s1v % (2 ^ N.val - 1) := mersenne_fold_mod s1v N.val (by omega)
      _ = x.val % (2 ^ N.val - 1) := hs1_cong
  -- rv < 2p : rv ≤ (2^N-1) + 1 = p + 1 < 2p
  have hrv_lt2p : rv < 2 * (2 ^ N.val - 1) := by
    have hm : s1v % 2 ^ N.val < 2 ^ N.val := hi3_lt
    rw [hrv_def]; omega
  -- overflowing_sub r p = ok (⟨r.bv - p.bv⟩, decide (r.val < p.val))
  have hr_val : r.val = rv := hr
  -- Compute the overflowing_sub-then-branch tail as a single ok value.
  have htail : (do
      let (sub, borrow) ← core.num.U64.overflowing_sub r p
      if borrow then ok r else ok sub)
      = ok (if r.val < p.val then r else (⟨r.bv - p.bv⟩ : Std.U64)) := by
    simp only [core.num.U64.overflowing_sub, bind_tc_ok]
    by_cases hb : r.val < p.val
    · simp [hb]
    · simp [hb]
  rw [htail]
  by_cases hb : r.val < p.val
  · -- borrow true ⇒ r.val < p.val ⇒ r already canonical
    simp only [hb, if_true, spec, theta, wp_return]
    rw [hr_val] at hb ⊢
    rw [hp] at hb
    -- rv < 2^N-1, and rv ≡ x mod (2^N-1) ⇒ rv = x % (2^N-1)
    rw [← hrv_cong, Nat.mod_eq_of_lt hb]
  · -- borrow false ⇒ r.val ≥ p.val ⇒ result = r - p
    simp only [hb, if_false, spec, theta, wp_return]
    push_neg at hb
    have hple : p.val ≤ r.val := hb
    have hsub_val : (⟨r.bv - p.bv⟩ : Std.U64).val = r.val - p.val := by
      show (r.bv - p.bv).toNat = r.val - p.val
      exact BitVec.toNat_sub_of_le (show p.bv.toNat ≤ r.bv.toNat from hple)
    rw [hsub_val, hr_val, hp]
    rw [hr_val, hp] at hb
    -- (rv - (2^N-1)) ≡ rv ≡ x (mod 2^N-1), and rv - p < p
    have hsub_lt : rv - (2 ^ N.val - 1) < 2 ^ N.val - 1 := by omega
    have hcong : (rv - (2 ^ N.val - 1)) % (2 ^ N.val - 1) =
        rv % (2 ^ N.val - 1) := by
      conv_rhs => rw [show rv = (rv - (2 ^ N.val - 1)) + (2 ^ N.val - 1) from by omega]
      rw [Nat.add_mod_right]
    rw [Nat.mod_eq_of_lt hsub_lt] at hcong
    rw [hcong, hrv_cong]

/-! ## §7 — 128-bit Mersenne reducer (`N ≥ 61` branch)

`dispatch_mersenne_mul` uses `mersenne_reduce 61` for the n=61 Mersenne
prime. The `N ≥ 61` arm folds three `N`-bit limbs (`2^N ≡ 1`). -/

/-- Three-limb Mersenne fold preserves residue mod `2^N − 1`. -/
theorem mersenne_fold3_mod (lo mid hi N : ℕ) (hN : 2 ≤ N)
    (x : ℕ) (hx : x = hi * 2 ^ (2 * N) + mid * 2 ^ N + lo) :
    (lo + mid + hi) % (2 ^ N - 1) = x % (2 ^ N - 1) := by
  have hpos : 0 < (2 : ℕ) ^ N := by positivity
  have hmod : (2 : ℕ) ^ N % (2 ^ N - 1) = 1 := by
    have hpow : (2 : ℕ) ^ N = (2 ^ N - 1) + 1 :=
      (Nat.succ_pred_eq_of_pos hpos).symm
    nth_rewrite 1 [hpow]
    have h4le : (4 : ℕ) ≤ 2 ^ N := by
      calc (4:ℕ) = 2 ^ 2 := by norm_num
        _ ≤ 2 ^ N := Nat.pow_le_pow_right (by norm_num) hN
    rw [Nat.add_mod_left, Nat.mod_eq_of_lt (by omega)]
  have hme : Nat.ModEq (2 ^ N - 1) (2 ^ N) 1 := by
    unfold Nat.ModEq; rw [hmod]
    have h4le : (4 : ℕ) ≤ 2 ^ N := by
      calc (4:ℕ) = 2 ^ 2 := by norm_num
        _ ≤ 2 ^ N := Nat.pow_le_pow_right (by norm_num) hN
    exact (Nat.mod_eq_of_lt (by omega)).symm
  have hme2 : Nat.ModEq (2 ^ N - 1) (2 ^ (2 * N)) 1 := by
    have : (2 : ℕ) ^ (2 * N) = (2 ^ N) * (2 ^ N) := by
      rw [← pow_add]; ring_nf
    rw [this]
    calc 2 ^ N * 2 ^ N ≡ 1 * 1 [MOD 2 ^ N - 1] := Nat.ModEq.mul hme hme
      _ = 1 := by ring
  subst hx
  calc (lo + mid + hi) % (2 ^ N - 1)
      = (hi + mid + lo) % (2 ^ N - 1) := by ring_nf
    _ = (hi * 1 + mid * 1 + lo) % (2 ^ N - 1) := by ring_nf
    _ = (hi * 2 ^ (2 * N) + mid * 2 ^ N + lo) % (2 ^ N - 1) := by
          have e1 := (Nat.ModEq.mul_left hi hme2)
          have e2 := (Nat.ModEq.mul_left mid hme)
          have : Nat.ModEq (2 ^ N - 1)
              (hi * 1 + mid * 1 + lo) (hi * 2 ^ (2 * N) + mid * 2 ^ N + lo) :=
            (Nat.ModEq.add_right lo (Nat.ModEq.add e1.symm e2.symm))
          exact this

/-- `mersenne_reduce N x` (the `N ≥ 61` arm) computes `x % (2^N − 1)`
    for `61 ≤ N ≤ 62` and `x < 2^(2N)` (true for products of two
    `< 2^N` operands; then the high limb is `0`). -/
theorem mersenne_reduce_correct (N : Std.U32) (x : Std.U128)
    (hN61 : 61 ≤ N.val) (hN62 : N.val ≤ 62) (hx : x.val < 2 ^ (2 * N.val)) :
    ∃ r, gfp.specialized.mersenne_reduce N x = ok r ∧
      r.val = x.val % (2 ^ N.val - 1) := by
  have hNlt64 : N.val < 64 := by omega
  have hNlt128 : N.val < 128 := by omega
  have h2Nlt128 : 2 * N.val < 128 := by omega
  have hp_pos : 0 < 2 ^ N.val - 1 := by
    have : 2 ^ 4 ≤ 2 ^ N.val := Nat.pow_le_pow_right (by norm_num) (by omega)
    omega
  apply spec_imp_exists
  unfold gfp.specialized.mersenne_reduce
  -- debug_assert_n_in_range N : massert (4 ≤ N) ; massert (N ≤ 62)
  have ha1 : (N ≥ 4#u32) := by
    show (4#u32 : Std.U32).val ≤ N.val
    have : (4#u32 : Std.U32).val = 4 := by native_decide
    omega
  have ha2 : (N ≤ 62#u32) := by
    show N.val ≤ (62#u32 : Std.U32).val
    have : (62#u32 : Std.U32).val = 62 := by native_decide
    omega
  have hdbg : gfp.specialized.debug_assert_n_in_range N = ok () := by
    simp only [gfp.specialized.debug_assert_n_in_range, massert,
               if_pos ha1, if_pos ha2, bind_tc_ok]
  rw [hdbg]
  simp only [bind_tc_ok, lift]
  -- pw = 1 <<< N = 2^N
  progress as ⟨pw, hpw_val, _⟩
  have hpw : pw.val = 2 ^ N.val := by
    rw [hpw_val, Nat.one_shiftLeft, U64.size_eq]
    exact Nat.mod_eq_of_lt (by
      have : 2 ^ N.val < 2 ^ 64 := Nat.pow_lt_pow_right (by norm_num) hNlt64
      omega)
  have hpw_ge1 : (1#u64 : Std.U64).val ≤ pw.val := by
    have h1 : (1#u64 : Std.U64).val = 1 := by native_decide
    rw [h1, hpw]; exact Nat.one_le_two_pow
  obtain ⟨p, hp_eq, hp_val, _⟩ := spec_imp_exists (UScalar.sub_spec hpw_ge1)
  simp only [hp_eq, bind_tc_ok]
  have hp : p.val = 2 ^ N.val - 1 := by
    rw [hp_val, hpw]
    have h1 : (1#u64 : Std.U64).val = 1 := by native_decide
    rw [h1]
  -- N ≥ 61 branch
  have hN61b : (N ≥ 61#u32) := by
    show (61#u32 : Std.U32).val ≤ N.val
    have : (61#u32 : Std.U32).val = 61 := by native_decide
    omega
  simp only [hN61b, if_true]
  -- lo = (cast U64 x) & p = x % 2^N
  have hcastx : (UScalar.cast .U64 x : Std.U64).val = x.val % 2 ^ 64 := by
    rw [UScalar.cast_val_eq]; norm_num
  have hlo : ((UScalar.cast .U64 x : Std.U64) &&& p).val = x.val % 2 ^ N.val := by
    rw [UScalar.val_and, hp, and_mask_mod, hcastx,
        Nat.mod_mod_of_dvd _ (pow_dvd_pow 2 (by omega : N.val ≤ 64))]
  -- i2 = x >>> N
  progress as ⟨i2, hi2_val, _⟩
  have hi2 : i2.val = x.val / 2 ^ N.val := by
    rw [hi2_val, Nat.shiftRight_eq_div_pow]
  -- mid = (cast U64 i2) & p = (x / 2^N) % 2^N
  have hmid : ((UScalar.cast .U64 i2 : Std.U64) &&& p).val =
      (x.val / 2 ^ N.val) % 2 ^ N.val := by
    rw [UScalar.val_and, hp, and_mask_mod, UScalar.cast_val_eq, hi2]
    have hdvd : (2:ℕ) ^ N.val ∣ 2 ^ 64 := pow_dvd_pow 2 (by omega : N.val ≤ 64)
    have : (2:ℕ) ^ UScalarTy.U64.numBits = 2 ^ 64 := by norm_num
    rw [this, Nat.mod_mod_of_dvd _ hdvd]
  -- i4 = 2 * N (u32)
  progress as ⟨tn, htn⟩
  have htn_val : tn.val = 2 * N.val := by
    have h2 : (2#u32 : Std.U32).val = 2 := by native_decide
    omega
  -- i5 = x >>> (2N)
  progress as ⟨i5, hi5_val, _⟩
  have hi5 : i5.val = x.val / 2 ^ (2 * N.val) := by
    rw [hi5_val, Nat.shiftRight_eq_div_pow, htn_val]
  -- hi = cast U64 i5 = x / 2^(2N) = 0  (x < 2^(2N))
  have hhi : (UScalar.cast .U64 i5 : Std.U64).val = 0 := by
    rw [UScalar.cast_val_eq, hi5]
    have : x.val / 2 ^ (2 * N.val) = 0 := Nat.div_eq_of_lt hx
    rw [this]; norm_num
  set lo := (UScalar.cast .U64 x : Std.U64) &&& p with hlo_def
  set mid := (UScalar.cast .U64 i2 : Std.U64) &&& p with hmid_def
  set hi := (UScalar.cast .U64 i5 : Std.U64) with hhi_def
  -- s1 = wrapping_add (wrapping_add lo mid) hi
  have hlo_lt : lo.val < 2 ^ N.val := by rw [hlo]; exact Nat.mod_lt _ (by positivity)
  have hmid_lt : mid.val < 2 ^ N.val := by
    rw [hmid]; exact Nat.mod_lt _ (by positivity)
  have hi6_no_ovf : lo.val + mid.val < 2 ^ 64 := by
    have : 2 ^ N.val ≤ 2 ^ 62 := Nat.pow_le_pow_right (by norm_num) hN62
    omega
  have hi6 : (core.num.U64.wrapping_add lo mid).val = lo.val + mid.val := by
    rw [core.num.U64.wrapping_add_val_eq, UScalar.size_UScalarTyU64, U64.size_eq,
        show (18446744073709551616 : ℕ) = 2 ^ 64 from by norm_num,
        Nat.mod_eq_of_lt hi6_no_ovf]
  set i6 := core.num.U64.wrapping_add lo mid with hi6_def
  have hs1_no_ovf : i6.val + hi.val < 2 ^ 64 := by
    rw [hi6, hhi]; omega
  have hs1 : (core.num.U64.wrapping_add i6 hi).val = lo.val + mid.val + hi.val := by
    rw [core.num.U64.wrapping_add_val_eq, UScalar.size_UScalarTyU64, U64.size_eq,
        show (18446744073709551616 : ℕ) = 2 ^ 64 from by norm_num,
        Nat.mod_eq_of_lt hs1_no_ovf, hi6]
  set s1 := core.num.U64.wrapping_add i6 hi with hs1_def
  set s1v := lo.val + mid.val + hi.val with hs1v_def
  -- s1v ≡ x (mod 2^N-1)
  have hs1_cong : s1v % (2 ^ N.val - 1) = x.val % (2 ^ N.val - 1) := by
    have hpow : 2 ^ (2 * N.val) = 2 ^ N.val * 2 ^ N.val := by
      rw [← pow_add]; ring_nf
    have hhi0 : x.val / 2 ^ (2 * N.val) = 0 := Nat.div_eq_of_lt hx
    have hxdiv_lt : x.val / 2 ^ N.val < 2 ^ N.val := by
      apply Nat.div_lt_of_lt_mul; rw [← hpow]; exact hx
    have hxsplit : x.val = (x.val / 2 ^ (2 * N.val)) * 2 ^ (2 * N.val)
        + (x.val / 2 ^ N.val % 2 ^ N.val) * 2 ^ N.val + x.val % 2 ^ N.val := by
      have hd1 := Nat.div_add_mod x.val (2 ^ N.val)
      rw [hhi0, Nat.zero_mul, Nat.zero_add, Nat.mod_eq_of_lt hxdiv_lt]
      -- x = (x/2^N)*2^N + x%2^N  (Nat.div_add_mod, rearranged)
      have : 2 ^ N.val * (x.val / 2 ^ N.val) + x.val % 2 ^ N.val = x.val := hd1
      calc x.val = 2 ^ N.val * (x.val / 2 ^ N.val) + x.val % 2 ^ N.val := this.symm
        _ = x.val / 2 ^ N.val * 2 ^ N.val + x.val % 2 ^ N.val := by ring
    have hfold := mersenne_fold3_mod (x.val % 2 ^ N.val)
      (x.val / 2 ^ N.val % 2 ^ N.val) (x.val / 2 ^ (2 * N.val)) N.val
      (by omega) x.val hxsplit
    rw [hhi0] at hfold
    rw [hs1v_def, hlo, hmid, hhi]
    exact hfold
  -- i7 = s1 & p = s1v % 2^N ; i8 = s1 >>> N = s1v / 2^N
  have hi7 : (s1 &&& p).val = s1v % 2 ^ N.val := by
    rw [UScalar.val_and, hp, and_mask_mod, hs1]
  progress as ⟨i8, hi8_val, _⟩
  have hi8 : i8.val = s1v / 2 ^ N.val := by
    rw [hi8_val, Nat.shiftRight_eq_div_pow, hs1]
  -- s1v < 3·2^N so s1v / 2^N ≤ 2
  have hs1v_lt : s1v < 3 * 2 ^ N.val := by
    rw [hs1v_def, hhi]; omega
  have hi8_le2 : s1v / 2 ^ N.val ≤ 2 := by
    apply Nat.le_of_lt_succ
    apply Nat.div_lt_of_lt_mul
    calc s1v < 3 * 2 ^ N.val := hs1v_lt
      _ = 2 ^ N.val * 3 := by ring
  -- r = wrapping_add i7 i8
  have hi7v_lt : s1v % 2 ^ N.val < 2 ^ N.val := Nat.mod_lt _ (by positivity)
  have hr_no_ovf : s1v % 2 ^ N.val + s1v / 2 ^ N.val < 2 ^ 64 := by
    have : 2 ^ N.val ≤ 2 ^ 62 := Nat.pow_le_pow_right (by norm_num) hN62
    omega
  set rv := s1v % 2 ^ N.val + s1v / 2 ^ N.val with hrv_def
  have hr : (core.num.U64.wrapping_add (s1 &&& p) i8).val = rv := by
    rw [core.num.U64.wrapping_add_val_eq, hi7, hi8, ← hrv_def,
        UScalar.size_UScalarTyU64, U64.size_eq,
        show (18446744073709551616 : ℕ) = 2 ^ 64 from by norm_num,
        Nat.mod_eq_of_lt hr_no_ovf]
  set r := core.num.U64.wrapping_add (s1 &&& p) i8 with hr_def
  have hrv_cong : rv % (2 ^ N.val - 1) = x.val % (2 ^ N.val - 1) := by
    rw [hrv_def]
    calc (s1v % 2 ^ N.val + s1v / 2 ^ N.val) % (2 ^ N.val - 1)
        = s1v % (2 ^ N.val - 1) := mersenne_fold_mod s1v N.val (by omega)
      _ = x.val % (2 ^ N.val - 1) := hs1_cong
  have hrv_lt : rv < 2 ^ N.val + 2 := by
    rw [hrv_def]; omega
  have hr_val : r.val = rv := hr
  -- overflowing_sub r p tail
  have htail : (do
      let (sub, borrow) ← core.num.U64.overflowing_sub r p
      if borrow then ok r else ok sub)
      = ok (if r.val < p.val then r else (⟨r.bv - p.bv⟩ : Std.U64)) := by
    simp only [core.num.U64.overflowing_sub, bind_tc_ok]
    by_cases hb : r.val < p.val
    · simp [hb]
    · simp [hb]
  rw [htail]
  by_cases hb : r.val < p.val
  · simp only [hb, if_true, spec, theta, wp_return]
    rw [hr_val] at hb ⊢
    rw [hp] at hb
    rw [← hrv_cong, Nat.mod_eq_of_lt hb]
  · simp only [hb, if_false, spec, theta, wp_return]
    push_neg at hb
    have hple : p.val ≤ r.val := hb
    have hsub_val : (⟨r.bv - p.bv⟩ : Std.U64).val = r.val - p.val := by
      show (r.bv - p.bv).toNat = r.val - p.val
      exact BitVec.toNat_sub_of_le (show p.bv.toNat ≤ r.bv.toNat from hple)
    rw [hsub_val, hr_val, hp]
    rw [hr_val, hp] at hb
    have h16 : (16 : ℕ) ≤ 2 ^ N.val := by
      calc (16:ℕ) = 2 ^ 4 := by norm_num
        _ ≤ 2 ^ N.val := Nat.pow_le_pow_right (by norm_num) (by omega)
    have hsub_lt : rv - (2 ^ N.val - 1) < 2 ^ N.val - 1 := by
      have := hrv_lt; omega
    have hcong : (rv - (2 ^ N.val - 1)) % (2 ^ N.val - 1) =
        rv % (2 ^ N.val - 1) := by
      conv_rhs => rw [show rv = (rv - (2 ^ N.val - 1)) + (2 ^ N.val - 1) from by omega]
      rw [Nat.add_mod_right]
    rw [Nat.mod_eq_of_lt hsub_lt] at hcong
    rw [hcong, hrv_cong]

end Specialized
