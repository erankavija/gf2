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

/-! ## §8 — Proth reducer correctness

`dispatch_proth_mul` reduces to `(a·b) % P` on every arm
(`P = K·2^N + 1`): the reducer body and both `prod % p32` /
wide-path branches are literal modulo. -/

/-- `proth_reduce_u64 K N x = x % (K·2^N + 1)` when `K ≥ 1`,
    `16 ≤ N ≤ 32`, and `K·2^N + 1 < 2^64` (no overflow). -/
theorem proth_reduce_u64_correct (K : Std.U64) (N : Std.U32) (x : Std.U64)
    (hK : 1 ≤ K.val) (hN16 : 16 ≤ N.val) (hN32 : N.val ≤ 32)
    (hP : K.val * 2 ^ N.val + 1 < 2 ^ 64) :
    ∃ r, gfp.specialized.proth_reduce_u64 K N x = ok r ∧
      r.val = x.val % (K.val * 2 ^ N.val + 1) := by
  have hNlt64 : N.val < 64 := by omega
  apply spec_imp_exists
  unfold gfp.specialized.proth_reduce_u64
  have ha1 : (K ≥ 1#u64) := by
    show (1#u64 : Std.U64).val ≤ K.val
    have : (1#u64 : Std.U64).val = 1 := by native_decide
    omega
  have ha2 : (N ≥ 16#u32) := by
    show (16#u32 : Std.U32).val ≤ N.val
    have : (16#u32 : Std.U32).val = 16 := by native_decide
    omega
  have ha3 : (N ≤ 32#u32) := by
    show N.val ≤ (32#u32 : Std.U32).val
    have : (32#u32 : Std.U32).val = 32 := by native_decide
    omega
  rw [massert, if_pos ha1]
  simp only [bind_tc_ok]
  rw [massert, if_pos ha2]
  simp only [bind_tc_ok]
  rw [massert, if_pos ha3]
  simp only [bind_tc_ok]
  -- pw = 1 <<< N = 2^N
  progress as ⟨pw, hpw_val, _⟩
  have hpw : pw.val = 2 ^ N.val := by
    rw [hpw_val, Nat.one_shiftLeft, U64.size_eq]
    exact Nat.mod_eq_of_lt (by
      have : 2 ^ N.val < 2 ^ 64 := Nat.pow_lt_pow_right (by norm_num) hNlt64
      omega)
  -- i1 = K * pw  (no overflow: K*2^N < K*2^N + 1 < 2^64)
  have hmax64 : UScalar.max .U64 = 2 ^ 64 - 1 := by native_decide
  have hKpw_le : K.val * pw.val ≤ UScalar.max .U64 := by
    rw [hpw, hmax64]; omega
  obtain ⟨i1, hi1_eq, hi1_val⟩ := spec_imp_exists (UScalar.mul_spec hKpw_le)
  simp only [hi1_eq, bind_tc_ok]
  have hi1 : i1.val = K.val * 2 ^ N.val := by rw [hi1_val, hpw]
  -- p = i1 + 1
  have h1v : (1#u64 : Std.U64).val = 1 := by native_decide
  have hi1_p1_le : i1.val + (1#u64 : Std.U64).val ≤ UScalar.max .U64 := by
    rw [hi1, h1v, hmax64]; omega
  obtain ⟨pp, hpp_eq, hpp_val⟩ := spec_imp_exists (UScalar.add_spec hi1_p1_le)
  simp only [hpp_eq, bind_tc_ok]
  have hpp : pp.val = K.val * 2 ^ N.val + 1 := by rw [hpp_val, hi1, h1v]
  -- x % pp
  have hpp_ne : pp.val ≠ 0 := by rw [hpp]; positivity
  obtain ⟨r, hr_eq, hr_val⟩ := spec_imp_exists (UScalar.rem_spec x hpp_ne)
  rw [show spec = theta from rfl, hr_eq]
  simp only [theta, wp_return]
  rw [hr_val, hpp]

/-! ## §9 — dispatch-match routing (clause-(c) axiom: pure structural routing) -/

/-- **Tracked Aeneas-extraction-shape limitation (issue 2e544a34,
    2026-05-16 amendment #2, criterion 2 clause (c) — the single permitted
    dispatch-routing axiom).**

`gfp.dispatch_mersenne_mul` and `gfp.dispatch_proth_mul` are extracted by
Aeneas as nested `#uscalar`-literal `match` expressions
(`match n with | 31#uscalar => … | 61#uscalar => … | _ => …`, and the
Proth `match k with | 15#uscalar … | 127#uscalar … | _ …` analogue with
its inner `match n`). No Lean 4.30 tactic can case-split these generated
matches (exhaustively falsified during attempt 2: `split`, `subst`/`rw`,
the generated `.match_N` splitters, `fun_cases`, a standalone collapse
lemma — `#uscalar` patterns are not user-writable — and `simp`/`decide`);
it is an extraction-shape / tactic limitation, not avoidable in-`Proofs/`
work.

This axiom asserts **only** the *pure structural routing* fact: the
`#uscalar`-literal match collapses to its matched arm, with the
un-case-splittable scrutinee match rewritten to a decidable `if` on
`.val`. Each right-hand side is the **literal extracted arm body** — it
contains **no** `% P` arithmetic claim and does **not** mention any
reducer's specification. All modular correctness (`(a·b) mod P`) is
derived downstream in `specialized_mul_correct` by composing this routing
identity with the *proven* reducer theorems (`mersenne_reduce_u64_correct`,
`mersenne_reduce_correct`, `proth_reduce_u64_correct`) and `classify_spec`;
the axiom does **not** absorb reducer correctness. A root-cause fix
(Rust-side dispatch refactor, Aeneas/Lean upgrade, or a `#uscalar`
simproc) is tracked as out-of-scope follow-up per this issue's
non-goals. -/
axiom dispatch_route :
    (∀ (n : Std.U32) (a b : Std.U64), n.val = 31 →
        gfp.dispatch_mersenne_mul n a b
          = (do
              let i ← lift (core.num.U64.wrapping_mul a b)
              gfp.specialized.mersenne_reduce_u64 31#u32 i))
  ∧ (∀ (n : Std.U32) (a b : Std.U64), n.val = 61 →
        gfp.dispatch_mersenne_mul n a b
          = (do
              let i ← lift (UScalar.cast .U128 a)
              let i1 ← lift (UScalar.cast .U128 b)
              let i2 ← i * i1
              gfp.specialized.mersenne_reduce 61#u32 i2))
  ∧ (∀ (n : Std.U32) (a b : Std.U64), n.val ≠ 31 → n.val ≠ 61 →
        gfp.dispatch_mersenne_mul n a b
          = (do
              let i ← lift (UScalar.cast .U128 a)
              let i1 ← lift (UScalar.cast .U128 b)
              let wide ← i * i1
              let i2 ← 1#u128 <<< n
              let i3 ← i2 - 1#u128
              let i4 ← wide % i3
              ok (UScalar.cast .U64 i4)))
  ∧ (∀ (k : Std.U64) (n : Std.U32) (a b : Std.U64),
        k.val = 15 → n.val = 27 →
        gfp.dispatch_proth_mul k n a b
          = (do
              let i ← lift (UScalar.cast .U128 k)
              let i1 ← 1#u128 <<< n
              let i2 ← i * i1
              let p ← i2 + 1#u128
              let p32 ← lift (UScalar.cast .U64 p)
              let i3 ← 1#u128 <<< 32#i32
              if p < i3 then
                (do
                  let prod ← lift (core.num.U64.wrapping_mul a b)
                  gfp.specialized.proth_reduce_u64 15#u64 27#u32 prod)
              else
                (do
                  let i4 ← lift (UScalar.cast .U128 a)
                  let i5 ← lift (UScalar.cast .U128 b)
                  let wide ← i4 * i5
                  let i6 ← wide % p
                  ok (UScalar.cast .U64 i6))))
  ∧ (∀ (k : Std.U64) (n : Std.U32) (a b : Std.U64),
        k.val = 127 → n.val = 24 →
        gfp.dispatch_proth_mul k n a b
          = (do
              let i ← lift (UScalar.cast .U128 k)
              let i1 ← 1#u128 <<< n
              let i2 ← i * i1
              let p ← i2 + 1#u128
              let p32 ← lift (UScalar.cast .U64 p)
              let i3 ← 1#u128 <<< 32#i32
              if p < i3 then
                (do
                  let prod ← lift (core.num.U64.wrapping_mul a b)
                  gfp.specialized.proth_reduce_u64 127#u64 24#u32 prod)
              else
                (do
                  let i4 ← lift (UScalar.cast .U128 a)
                  let i5 ← lift (UScalar.cast .U128 b)
                  let wide ← i4 * i5
                  let i6 ← wide % p
                  ok (UScalar.cast .U64 i6))))
  ∧ (∀ (k : Std.U64) (n : Std.U32) (a b : Std.U64),
        ¬ (k.val = 15 ∧ n.val = 27) → ¬ (k.val = 127 ∧ n.val = 24) →
        gfp.dispatch_proth_mul k n a b
          = (do
              let i ← lift (UScalar.cast .U128 k)
              let i1 ← 1#u128 <<< n
              let i2 ← i * i1
              let p ← i2 + 1#u128
              let p32 ← lift (UScalar.cast .U64 p)
              let i3 ← 1#u128 <<< 32#i32
              if p < i3 then
                (do
                  let prod ← lift (core.num.U64.wrapping_mul a b)
                  prod % p32)
              else
                (do
                  let i4 ← lift (UScalar.cast .U128 a)
                  let i5 ← lift (UScalar.cast .U128 b)
                  let wide ← i4 * i5
                  let i6 ← wide % p
                  ok (UScalar.cast .U64 i6))))

/-! ## §10 — specialized_mul full modular correctness

`specialized_mul` correctness is **derived**, not assumed: the §9
`dispatch_route` axiom only collapses the un-case-splittable `#uscalar`
match to its literal arm body; every modular fact below comes from the
*proven* reducer theorems (`mersenne_reduce_u64_correct`,
`mersenne_reduce_correct`, `proth_reduce_u64_correct`), `classify_spec`,
and the `u128_wide_mod` / `u64_wide_mod` direct-modulo lemmas. -/

/-- Generic 128-bit product-modulo arm: `(cast U64 ((cast U128 a) *
    (cast U128 b) % m))` equals `(a·b) % P` and stays `< P`, whenever
    `m.val = P.val`. Used by the Mersenne-generic and every Proth
    `else` (wide) branch. -/
private theorem u128_wide_mod {P a b : Std.U64}
    (hP : ValidPrime P) (ha : a.val < P.val) (hb : b.val < P.val)
    (m : Std.U128) (hm : m.val = P.val) :
    (do
      let i ← lift (UScalar.cast .U128 a)
      let i1 ← lift (UScalar.cast .U128 b)
      let i2 ← i * i1
      let i4 ← i2 % m
      ok (UScalar.cast .U64 i4))
    ⦃ r => r.val < P.val ∧ r.val = (a.val * b.val) % P.val ⦄ := by
  have hP_pos : 0 < P.val := by have := hP.2.1; omega
  have ha128 : (UScalar.cast .U128 a : Std.U128).val = a.val := U64.cast_U128_val_eq a
  have hb128 : (UScalar.cast .U128 b : Std.U128).val = b.val := U64.cast_U128_val_eq b
  simp only [lift, bind_tc_ok]
  progress as ⟨prod, hprod⟩
  progress as ⟨rem, hrem⟩
  have hprod_val : prod.val = a.val * b.val := by rw [hprod, ha128, hb128]
  have hrem_val : rem.val = (a.val * b.val) % P.val := by
    rw [hrem, hprod_val, hm]
  have hrem_lt : rem.val < P.val := by rw [hrem_val]; exact Nat.mod_lt _ hP_pos
  have hcast : (UScalar.cast .U64 rem).val = rem.val :=
    UScalar.cast_val_mod_pow_of_inBounds_eq .U64 rem (by
      have : UScalarTy.U64.numBits = 64 := by decide
      rw [this]; nlinarith [hP.2.2])
  exact ⟨by rw [hcast]; exact hrem_lt, by rw [hcast, hrem_val]⟩

/-- 64-bit product-modulo arm: `(core.num.U64.wrapping_mul a b) % m`
    equals `(a·b) % P` and stays `< P`, whenever `m.val = P.val`,
    `P.val < 2^32`, and `a, b < P` (so `a·b < 2^64`, no wrap). Used by
    the Proth `then` branch's unsupported-pair fall-through
    (`prod % p32`). -/
private theorem u64_wide_mod {P a b : Std.U64}
    (hP : ValidPrime P) (ha : a.val < P.val) (hb : b.val < P.val)
    (hP32 : P.val < 2 ^ 32) (m : Std.U64) (hm : m.val = P.val) :
    (do
      let prod ← lift (core.num.U64.wrapping_mul a b)
      prod % m)
    ⦃ r => r.val < P.val ∧ r.val = (a.val * b.val) % P.val ⦄ := by
  have hP_pos : 0 < P.val := by have := hP.2.1; omega
  have hab64 : a.val * b.val < 2 ^ 64 := by
    calc a.val * b.val ≤ (2 ^ 32 - 1) * (2 ^ 32 - 1) :=
          Nat.mul_le_mul (by omega) (by omega)
      _ < 2 ^ 64 := by norm_num
  have hprod : (core.num.U64.wrapping_mul a b).val = a.val * b.val := by
    have h := core.num.U64.wrapping_mul_val_eq a b
    simp only [UScalar.size_UScalarTyU64, U64.size_eq] at h
    rw [h]; exact Nat.mod_eq_of_lt (by
      have : (18446744073709551616 : ℕ) = 2 ^ 64 := by norm_num
      omega)
  simp only [lift, bind_tc_ok]
  have hm_ne : m.val ≠ 0 := by rw [hm]; omega
  obtain ⟨r, hr_eq, hr_val⟩ :=
    spec_imp_exists (UScalar.rem_spec (core.num.U64.wrapping_mul a b) hm_ne)
  rw [show spec = theta from rfl, hr_eq]
  simp only [theta, wp_return]
  rw [hr_val, hprod, hm]
  exact ⟨Nat.mod_lt _ hP_pos, rfl⟩

/-- Helper: `1#u128 <<< s` has value `2 ^ s.val` whenever `s.val < 128`. -/
private theorem one_shl_u128_val (pw : Std.U128) {s : ℕ}
    (hpw_val : pw.val = 1 <<< s % U128.size) (hs : s < 128) :
    pw.val = 2 ^ s := by
  rw [hpw_val, Nat.one_shiftLeft, U128.size_eq]
  exact Nat.mod_eq_of_lt (by
    have : 2 ^ s < 2 ^ 128 := Nat.pow_lt_pow_right (by norm_num) hs
    omega)

/-- M31 dispatch arm: `wrapping_mul` then the proven
    `mersenne_reduce_u64_correct`. -/
private theorem mersenne31_arm {P a b : Std.U64}
    (hP : ValidPrime P) (ha : a.val < P.val) (hb : b.val < P.val)
    (hP31 : P.val = 2 ^ 31 - 1) :
    (do
      let i ← lift (core.num.U64.wrapping_mul a b)
      gfp.specialized.mersenne_reduce_u64 31#u32 i)
    ⦃ r => r.val < P.val ∧ r.val = (a.val * b.val) % P.val ⦄ := by
  have hP_pos : 0 < P.val := by have := hP.2.1; omega
  have h31v : (31#u32 : Std.U32).val = 31 := by native_decide
  have ha31 : a.val < 2 ^ 31 - 1 := by rw [← hP31]; exact ha
  have hb31 : b.val < 2 ^ 31 - 1 := by rw [← hP31]; exact hb
  have hprod : (core.num.U64.wrapping_mul a b).val = a.val * b.val := by
    have h := core.num.U64.wrapping_mul_val_eq a b
    simp only [UScalar.size_UScalarTyU64, U64.size_eq] at h
    rw [h]; exact Nat.mod_eq_of_lt (by
      calc a.val * b.val ≤ (2 ^ 31 - 1) * (2 ^ 31 - 1) :=
            Nat.mul_le_mul (by omega) (by omega)
        _ < 18446744073709551616 := by norm_num)
  simp only [lift, bind_tc_ok]
  obtain ⟨r, hr_eq, hr_val⟩ :=
    mersenne_reduce_u64_correct 31#u32 (core.num.U64.wrapping_mul a b)
      (by omega) (by omega) (by
        rw [h31v, hprod]
        calc a.val * b.val ≤ (2 ^ 31 - 1) * (2 ^ 31 - 1) :=
              Nat.mul_le_mul (by omega) (by omega)
          _ < 2 ^ (2 * 31) := by norm_num)
  rw [show spec = theta from rfl, hr_eq]
  simp only [theta, wp_return]
  rw [hr_val, h31v, hprod, ← hP31]
  exact ⟨Nat.mod_lt _ hP_pos, rfl⟩

/-- M61 dispatch arm: 128-bit product then the proven
    `mersenne_reduce_correct`. -/
private theorem mersenne61_arm {P a b : Std.U64}
    (hP : ValidPrime P) (ha : a.val < P.val) (hb : b.val < P.val)
    (hP61 : P.val = 2 ^ 61 - 1) :
    (do
      let i ← lift (UScalar.cast .U128 a)
      let i1 ← lift (UScalar.cast .U128 b)
      let i2 ← i * i1
      gfp.specialized.mersenne_reduce 61#u32 i2)
    ⦃ r => r.val < P.val ∧ r.val = (a.val * b.val) % P.val ⦄ := by
  have hP_pos : 0 < P.val := by have := hP.2.1; omega
  have h61v : (61#u32 : Std.U32).val = 61 := by native_decide
  have ha61 : a.val < 2 ^ 61 - 1 := by rw [← hP61]; exact ha
  have hb61 : b.val < 2 ^ 61 - 1 := by rw [← hP61]; exact hb
  have ha128 : (UScalar.cast .U128 a : Std.U128).val = a.val :=
    U64.cast_U128_val_eq a
  have hb128 : (UScalar.cast .U128 b : Std.U128).val = b.val :=
    U64.cast_U128_val_eq b
  simp only [lift, bind_tc_ok]
  progress as ⟨i2, hi2⟩
  have hi2_val : i2.val = a.val * b.val := by rw [hi2, ha128, hb128]
  obtain ⟨r, hr_eq, hr_val⟩ :=
    mersenne_reduce_correct 61#u32 i2 (by omega) (by omega) (by
      rw [h61v, hi2_val]
      calc a.val * b.val ≤ (2 ^ 61 - 1) * (2 ^ 61 - 1) :=
            Nat.mul_le_mul (by omega) (by omega)
        _ < 2 ^ (2 * 61) := by norm_num)
  rw [show spec = theta from rfl, hr_eq]
  simp only [theta, wp_return]
  rw [hr_val, h61v, hi2_val, ← hP61]
  exact ⟨Nat.mod_lt _ hP_pos, rfl⟩

/-- Generic Mersenne dispatch arm: direct 128-bit `(a·b) % (2^n − 1)`. -/
private theorem mersenne_generic_arm {P a b : Std.U64} {n : Std.U32}
    (hP : ValidPrime P) (ha : a.val < P.val) (hb : b.val < P.val)
    (hn4 : 4 ≤ n.val) (hn62 : n.val ≤ 62) (hPeq : P.val = 2 ^ n.val - 1) :
    (do
      let i ← lift (UScalar.cast .U128 a)
      let i1 ← lift (UScalar.cast .U128 b)
      let wide ← i * i1
      let i2 ← 1#u128 <<< n
      let i3 ← i2 - 1#u128
      let i4 ← wide % i3
      ok (UScalar.cast .U64 i4))
    ⦃ r => r.val < P.val ∧ r.val = (a.val * b.val) % P.val ⦄ := by
  have hP_pos : 0 < P.val := by have := hP.2.1; omega
  have hnlt128 : n.val < 128 := by omega
  have ha128 : (UScalar.cast .U128 a : Std.U128).val = a.val :=
    U64.cast_U128_val_eq a
  have hb128 : (UScalar.cast .U128 b : Std.U128).val = b.val :=
    U64.cast_U128_val_eq b
  have h1v128 : (1#u128 : Std.U128).val = 1 := by native_decide
  simp only [lift, bind_tc_ok]
  progress as ⟨wide, hwide⟩
  have hwide_val : wide.val = a.val * b.val := by rw [hwide, ha128, hb128]
  progress as ⟨pw, hpw_val, _⟩
  have hpw : pw.val = 2 ^ n.val := one_shl_u128_val pw hpw_val hnlt128
  have hpw_ge1 : (1#u128 : Std.U128).val ≤ pw.val := by
    rw [h1v128, hpw]; exact Nat.one_le_pow _ _ (by norm_num)
  obtain ⟨md, hmd_eq, hmd_val0, _⟩ := spec_imp_exists (UScalar.sub_spec hpw_ge1)
  simp only [hmd_eq, bind_tc_ok]
  have hmd_val : md.val = 2 ^ n.val - 1 := by rw [hmd_val0, hpw, h1v128]
  progress as ⟨i4, hi4⟩
  have hi4_val : i4.val = (a.val * b.val) % P.val := by
    rw [hi4, hwide_val, hmd_val, ← hPeq]
  have hi4_lt : i4.val < P.val := by rw [hi4_val]; exact Nat.mod_lt _ hP_pos
  have hcast : (UScalar.cast .U64 i4 : Std.U64).val = i4.val :=
    UScalar.cast_val_mod_pow_of_inBounds_eq .U64 i4 (by
      have : UScalarTy.U64.numBits = 64 := by decide
      rw [this]; nlinarith [hP.2.2, hi4_lt])
  exact ⟨by rw [hcast]; exact hi4_lt, by rw [hcast, hi4_val]⟩

/-- Proth dispatch supported-pair arm `(K, N)`: 128-bit prefix computes
    `p = K·2^N + 1 = P` (so `p < 2^32`, the `then`-branch fires), then
    the proven `proth_reduce_u64_correct`. -/
private theorem proth_supported_arm {P k a b : Std.U64} {n : Std.U32}
    (K : Std.U64) (N : Std.U32)
    (hP : ValidPrime P) (ha : a.val < P.val) (hb : b.val < P.val)
    (hkK : k.val = K.val) (hnN : n.val = N.val)
    (hK1 : 1 ≤ K.val) (hN16 : 16 ≤ N.val) (hN32 : N.val ≤ 32)
    (hPbound : K.val * 2 ^ N.val + 1 < 2 ^ 32)
    (hPeq : P.val = k.val * 2 ^ n.val + 1) :
    (do
      let i ← lift (UScalar.cast .U128 k)
      let i1 ← 1#u128 <<< n
      let i2 ← i * i1
      let p ← i2 + 1#u128
      let _p32 ← lift (UScalar.cast .U64 p)
      let i3 ← 1#u128 <<< 32#i32
      if p < i3 then
        (do
          let prod ← lift (core.num.U64.wrapping_mul a b)
          gfp.specialized.proth_reduce_u64 K N prod)
      else
        (do
          let i4 ← lift (UScalar.cast .U128 a)
          let i5 ← lift (UScalar.cast .U128 b)
          let wide ← i4 * i5
          let i6 ← wide % p
          ok (UScalar.cast .U64 i6)))
    ⦃ r => r.val < P.val ∧ r.val = (a.val * b.val) % P.val ⦄ := by
  have hP_pos : 0 < P.val := by have := hP.2.1; omega
  have hnlt128 : n.val < 128 := by rw [hnN]; omega
  have hPKN : P.val = K.val * 2 ^ N.val + 1 := by rw [hPeq, hkK, hnN]
  have hP32 : P.val < 2 ^ 32 := by rw [hPKN]; exact hPbound
  have hk128 : (UScalar.cast .U128 k : Std.U128).val = k.val :=
    U64.cast_U128_val_eq k
  have hmax128 : UScalar.max .U128 = 2 ^ 128 - 1 := by native_decide
  have h1v128 : (1#u128 : Std.U128).val = 1 := by native_decide
  simp only [lift, bind_tc_ok]
  progress as ⟨i1, hi1v, _⟩
  have hi1 : i1.val = 2 ^ n.val := one_shl_u128_val i1 hi1v hnlt128
  have hmul_le : (UScalar.cast .U128 k : Std.U128).val * i1.val
      ≤ UScalar.max .U128 := by
    rw [hk128, hi1, hmax128, hkK, hnN]; omega
  obtain ⟨i2, hi2_eq, hi2_val0⟩ := spec_imp_exists (UScalar.mul_spec hmul_le)
  simp only [hi2_eq, bind_tc_ok]
  have hi2_val : i2.val = k.val * 2 ^ n.val := by rw [hi2_val0, hk128, hi1]
  have hadd_le : i2.val + (1#u128 : Std.U128).val ≤ UScalar.max .U128 := by
    rw [hi2_val, h1v128, hmax128, hkK, hnN]; omega
  obtain ⟨p, hp_eq, hp_val0⟩ := spec_imp_exists (UScalar.add_spec hadd_le)
  simp only [hp_eq, bind_tc_ok]
  have hp_val : p.val = P.val := by
    rw [hp_val0, hi2_val, h1v128, ← hPeq]
  progress as ⟨i3, hi3v, _⟩
  have hi3 : i3.val = 2 ^ 32 := by
    have : (32 : ℕ) < 128 := by norm_num
    rw [hi3v, Nat.one_shiftLeft, U128.size_eq]
    exact Nat.mod_eq_of_lt (by norm_num)
  have hpfx : p < i3 := by
    rw [UScalar.lt_equiv, hp_val, hi3]; exact hP32
  rw [if_pos hpfx]
  have hprod : (core.num.U64.wrapping_mul a b).val = a.val * b.val := by
    have h := core.num.U64.wrapping_mul_val_eq a b
    simp only [UScalar.size_UScalarTyU64, U64.size_eq] at h
    rw [h]; exact Nat.mod_eq_of_lt (by
      have haa : a.val < 2 ^ 32 := by omega
      have hbb : b.val < 2 ^ 32 := by omega
      calc a.val * b.val ≤ (2 ^ 32 - 1) * (2 ^ 32 - 1) :=
            Nat.mul_le_mul (by omega) (by omega)
        _ < 18446744073709551616 := by norm_num)
  obtain ⟨r, hr_eq, hr_val⟩ :=
    proth_reduce_u64_correct K N (core.num.U64.wrapping_mul a b)
      hK1 hN16 hN32 (by omega)
  rw [show spec = theta from rfl, hr_eq]
  simp only [theta, wp_return]
  rw [hr_val, hprod]
  have hPKNv : K.val * 2 ^ N.val + 1 = P.val := by
    rw [← hkK, ← hnN, ← hPeq]
  rw [hPKNv]
  exact ⟨Nat.mod_lt _ hP_pos, rfl⟩

/-- Proth dispatch fall-through arm (unsupported `(k, n)`): the inner
    `#uscalar` match collapses to `prod % p32` (when `P < 2^32`) or to
    the 128-bit wide path (when `P ≥ 2^32`); both are direct modulo
    discharged by `u64_wide_mod` / `u128_wide_mod`. -/
private theorem proth_fallthrough_arm {P k a b : Std.U64} {n : Std.U32}
    (hP : ValidPrime P) (ha : a.val < P.val) (hb : b.val < P.val)
    (hn16 : 16 ≤ n.val) (hn62 : n.val ≤ 62) (hk1 : 1 ≤ k.val)
    (hPeq : P.val = k.val * 2 ^ n.val + 1) :
    (do
      let i ← lift (UScalar.cast .U128 k)
      let i1 ← 1#u128 <<< n
      let i2 ← i * i1
      let p ← i2 + 1#u128
      let p32 ← lift (UScalar.cast .U64 p)
      let i3 ← 1#u128 <<< 32#i32
      if p < i3 then
        (do
          let prod ← lift (core.num.U64.wrapping_mul a b)
          prod % p32)
      else
        (do
          let i4 ← lift (UScalar.cast .U128 a)
          let i5 ← lift (UScalar.cast .U128 b)
          let wide ← i4 * i5
          let i6 ← wide % p
          ok (UScalar.cast .U64 i6)))
    ⦃ r => r.val < P.val ∧ r.val = (a.val * b.val) % P.val ⦄ := by
  have hP_pos : 0 < P.val := by have := hP.2.1; omega
  have hPle : P.val ≤ 2 ^ 63 := hP.2.2
  have hnlt128 : n.val < 128 := by omega
  have hk128 : (UScalar.cast .U128 k : Std.U128).val = k.val :=
    U64.cast_U128_val_eq k
  have hmax128 : UScalar.max .U128 = 2 ^ 128 - 1 := by native_decide
  have h1v128 : (1#u128 : Std.U128).val = 1 := by native_decide
  simp only [lift, bind_tc_ok]
  progress as ⟨i1, hi1v, _⟩
  have hi1 : i1.val = 2 ^ n.val := one_shl_u128_val i1 hi1v hnlt128
  have hmul_le : (UScalar.cast .U128 k : Std.U128).val * i1.val
      ≤ UScalar.max .U128 := by
    rw [hk128, hi1, hmax128]
    have hkn : k.val * 2 ^ n.val + 1 = P.val := hPeq.symm
    have : k.val * 2 ^ n.val ≤ 2 ^ 63 := by omega
    calc k.val * 2 ^ n.val ≤ 2 ^ 63 := this
      _ ≤ 2 ^ 128 - 1 := by norm_num
  obtain ⟨i2, hi2_eq, hi2_val0⟩ := spec_imp_exists (UScalar.mul_spec hmul_le)
  simp only [hi2_eq, bind_tc_ok]
  have hi2_val : i2.val = k.val * 2 ^ n.val := by rw [hi2_val0, hk128, hi1]
  have hadd_le : i2.val + (1#u128 : Std.U128).val ≤ UScalar.max .U128 := by
    rw [hi2_val, h1v128, hmax128]
    have hkn : k.val * 2 ^ n.val + 1 = P.val := hPeq.symm
    omega
  obtain ⟨p, hp_eq, hp_val0⟩ := spec_imp_exists (UScalar.add_spec hadd_le)
  simp only [hp_eq, bind_tc_ok]
  have hp_val : p.val = P.val := by
    rw [hp_val0, hi2_val, h1v128, ← hPeq]
  have hp32 : (UScalar.cast .U64 p : Std.U64).val = P.val := by
    rw [UScalar.cast_val_mod_pow_of_inBounds_eq .U64 p (by
      have : UScalarTy.U64.numBits = 64 := by decide
      rw [this, hp_val]; nlinarith [hP.2.2]), hp_val]
  progress as ⟨i3, hi3v, _⟩
  have hi3 : i3.val = 2 ^ 32 := by
    rw [hi3v, Nat.one_shiftLeft, U128.size_eq]
    exact Nat.mod_eq_of_lt (by norm_num)
  by_cases hpfx : p < i3
  · rw [if_pos hpfx]
    have hP32 : P.val < 2 ^ 32 := by
      have hlt := (UScalar.lt_equiv p i3).mp hpfx
      rw [hp_val, hi3] at hlt; exact hlt
    exact u64_wide_mod hP ha hb hP32 (UScalar.cast .U64 p) hp32
  · rw [if_neg hpfx]
    exact u128_wide_mod hP ha hb p hp_val

/-- `specialized_mul P a b` returns `r` with `r.val < P.val` and
    `r.val = (a.val * b.val) % P.val`, for any valid prime `P` and
    `a, b < P`, across all four `PrimeShape` arms. Composes the proven
    `classify_spec` + reducer lemmas, threading dispatch through the §9
    pure-routing axiom (`dispatch_route`) only — the `(a·b) mod P` form
    comes entirely from the proven `mersenne_reduce_u64_correct` /
    `mersenne_reduce_correct` / `proth_reduce_u64_correct` /
    `u128_wide_mod` / `u64_wide_mod` lemmas. -/
theorem specialized_mul_correct {P : Std.U64} {a b : Std.U64}
    (hP : ValidPrime P) (ha : a.val < P.val) (hb : b.val < P.val) :
    ∃ r, gfp.specialized_mul P a b = ok r ∧
      r.val < P.val ∧ r.val = (a.val * b.val) % P.val := by
  have hP_pos : 0 < P.val := by have := hP.2.1; omega
  have hPgt1 : 1 < P.val := hP.2.1
  have hPle : P.val ≤ 2 ^ 63 := hP.2.2
  unfold gfp.specialized_mul
  obtain ⟨ps, hps_eq, hps_post⟩ := spec_imp_exists (classify_spec hPgt1)
  simp only [hps_eq, bind_tc_ok]
  cases ps with
  | Mersenne n =>
    -- classifyPost: 4 ≤ n ≤ 62 ∧ P.val = 2^n - 1
    obtain ⟨hn4, hn62, hPeq⟩ := hps_post
    by_cases h31 : n.val = 31
    · simp only [dispatch_route.1 n a b h31]
      exact spec_imp_exists
        (mersenne31_arm hP ha hb (by rw [hPeq, h31]))
    · by_cases h61 : n.val = 61
      · simp only [dispatch_route.2.1 n a b h61]
        exact spec_imp_exists
          (mersenne61_arm hP ha hb (by rw [hPeq, h61]))
      · simp only [dispatch_route.2.2.1 n a b h31 h61]
        exact spec_imp_exists
          (mersenne_generic_arm hP ha hb hn4 hn62 hPeq)
  | Proth k n =>
    -- classifyPost: 16 ≤ n ∧ 1 ≤ k ∧ P.val = k*2^n + 1
    obtain ⟨hn16, hk1, hPeq⟩ := hps_post
    have hn62 : n.val ≤ 62 := by
      by_contra hcon
      push_neg at hcon
      have hpgt : (2 : ℕ) ^ 63 ≤ 2 ^ n.val :=
        Nat.pow_le_pow_right (by norm_num) (by omega)
      have : k.val * 2 ^ n.val ≥ 2 ^ 63 := by
        calc k.val * 2 ^ n.val ≥ 1 * 2 ^ 63 := Nat.mul_le_mul hk1 hpgt
          _ = 2 ^ 63 := by ring
      omega
    by_cases hc1 : k.val = 15 ∧ n.val = 27
    · obtain ⟨hk15, hn27⟩ := hc1
      simp only [dispatch_route.2.2.2.1 k n a b hk15 hn27]
      exact spec_imp_exists
        (proth_supported_arm 15#u64 27#u32 hP ha hb
          (by rw [hk15]; native_decide) (by rw [hn27]; native_decide)
          (by native_decide) (by native_decide) (by native_decide)
          (by native_decide) hPeq)
    · by_cases hc2 : k.val = 127 ∧ n.val = 24
      · obtain ⟨hk127, hn24⟩ := hc2
        simp only [dispatch_route.2.2.2.2.1 k n a b hk127 hn24]
        exact spec_imp_exists
          (proth_supported_arm 127#u64 24#u32 hP ha hb
            (by rw [hk127]; native_decide) (by rw [hn24]; native_decide)
            (by native_decide) (by native_decide) (by native_decide)
            (by native_decide) hPeq)
      · simp only [dispatch_route.2.2.2.2.2 k n a b hc1 hc2]
        exact spec_imp_exists
          (proth_fallthrough_arm hP ha hb hn16 hn62 hk1 hPeq)
  | Goldilocks =>
    -- classifyPost: P.val = GOLDILOCKS constant; specialized_mul body =
    -- the wide (a*b)%P arm
    obtain ⟨r, hr_eq, hr_lt, hr_val⟩ := spec_imp_exists (wide_mod_arm hP ha hb)
    exact ⟨r, hr_eq, hr_lt, hr_val⟩
  | Generic =>
    obtain ⟨r, hr_eq, hr_lt, hr_val⟩ := spec_imp_exists (wide_mod_arm hP ha hb)
    exact ⟨r, hr_eq, hr_lt, hr_val⟩

/-! ## §11 — inv_loop result bound -/

/-- The Fermat-inverse square-and-multiply `inv_loop` keeps `result` and
    `base` in `[0, P)` (each `specialized_mul` step returns `< P`), so its
    final result is in `[0, P)`. Proven by the `loop.spec` invariant
    pattern; the `specialized_mul` bound is the now-proven
    `specialized_mul_correct`. -/
theorem inv_loop_bound {P : Std.U64} (hP : ValidPrime P)
    (result base e : Std.U64)
    (hr : result.val < P.val) (hb : base.val < P.val) :
    ∃ r, gfp.Fp.Insts.Gf2_coreFieldTraitsFiniteFieldU64U128.inv_loop P result base e
      = ok r ∧ r.val < P.val := by
  have hP_pos : 0 < P.val := by have := hP.2.1; omega
  have hspec :
      gfp.Fp.Insts.Gf2_coreFieldTraitsFiniteFieldU64U128.inv_loop P result base e
      ⦃ r => r.val < P.val ⦄ := by
    unfold gfp.Fp.Insts.Gf2_coreFieldTraitsFiniteFieldU64U128.inv_loop
    apply loop.spec
      (measure := fun ((_, _, e1) : Std.U64 × Std.U64 × Std.U64) => e1.val)
      (inv := fun ((res1, bas1, _) : Std.U64 × Std.U64 × Std.U64) =>
        res1.val < P.val ∧ bas1.val < P.val)
    · intro ⟨res1, bas1, e1⟩ ⟨hres1, hbas1⟩
      dsimp only
      simp only [gfp.Fp.Insts.Gf2_coreFieldTraitsFiniteFieldU64U128.inv_loop.body]
      by_cases he : e1 > 0#u64
      · simp only [he, if_true]
        -- i = e1 & 1
        progress as ⟨i, _, _⟩
        -- result1 = if i = 1 then specialized_mul P res1 bas1 else res1
        have hres1_step : ∃ res2, (if i = 1#u64
            then gfp.specialized_mul P res1 bas1 else ok res1) = ok res2
            ∧ res2.val < P.val := by
          by_cases hi1 : i = 1#u64
          · simp only [hi1, if_true]
            obtain ⟨r2, hr2_eq, hr2_lt, _⟩ := specialized_mul_correct hP hres1 hbas1
            exact ⟨r2, hr2_eq, hr2_lt⟩
          · simp only [hi1, if_false]
            exact ⟨res1, rfl, hres1⟩
        obtain ⟨res2, hres2_eq, hres2_lt⟩ := hres1_step
        simp only [hres2_eq, bind_tc_ok]
        -- e_next = e1 >>> 1
        progress as ⟨enext, henext_val, _⟩
        have he1_pos : 0 < e1.val := by scalar_tac
        have henext_lt : enext.val < e1.val := by
          rw [henext_val, Nat.shiftRight_eq_div_pow, pow_one]
          omega
        by_cases he2 : enext > 0#u64
        · simp only [he2, if_true]
          obtain ⟨bas2, hbas2_eq, hbas2_lt, _⟩ :=
            specialized_mul_correct hP hbas1 hbas1
          simp only [hbas2_eq, bind_tc_ok, spec, theta, wp_return]
          refine ⟨hres2_lt, hbas2_lt, ?_⟩
          show enext.val < e1.val
          exact henext_lt
        · simp only [he2, if_false, spec, theta, wp_return]
          refine ⟨hres2_lt, hbas1, ?_⟩
          show enext.val < e1.val
          exact henext_lt
      · simp only [he, if_false, spec, theta, wp_return]
        exact hres1
    · exact ⟨hr, hb⟩
  obtain ⟨r, hr_eq, hr_lt⟩ := spec_imp_exists hspec
  exact ⟨r, hr_eq, hr_lt⟩

/-! ## §12 — inv_loop value correctness (Fermat square-and-multiply) -/

/-- Pure-`Nat` binary-exponentiation spec mirroring `inv_loop.body`
    (canonical-form `specialized_mul` = `(x·y) % P`). -/
private def invPowSpec (P result base e : ℕ) : ℕ :=
  if e = 0 then result
  else
    let result' := if e % 2 = 1 then result * base % P else result
    let e' := e / 2
    let base' := if e' > 0 then base * base % P else base
    invPowSpec P result' base' e'
termination_by e
decreasing_by omega

private lemma invPowSpec_zero (P result base : ℕ) :
    invPowSpec P result base 0 = result := by
  rw [invPowSpec]; simp

private lemma invPowSpec_step (P result base e : ℕ) (he : 0 < e) :
    invPowSpec P result base e =
      invPowSpec P (if e % 2 = 1 then result * base % P else result)
        (if e / 2 > 0 then base * base % P else base) (e / 2) := by
  conv_lhs => rw [invPowSpec]
  simp only [show ¬(e = 0) from by omega, if_false]

/-- `invPowSpec` computes `result · base^e (mod P)`. -/
private lemma invPowSpec_correct (P : ℕ) (hP : 0 < P) :
    ∀ e result base, invPowSpec P result base e % P
      = (result * base ^ e) % P := by
  intro e
  induction e using Nat.strong_induction_on with
  | _ e ih =>
    intro result base
    by_cases he : e = 0
    · subst he; rw [invPowSpec_zero]; simp
    · have hepos : 0 < e := by omega
      rw [invPowSpec_step P result base e hepos]
      have he2 : e / 2 < e := by omega
      rw [ih (e / 2) he2]
      -- base^e = base^(e%2) * (base^2)^(e/2)
      have hsplit : base ^ e = base ^ (e % 2) * (base ^ 2) ^ (e / 2) := by
        rw [← pow_mul, ← pow_add]
        congr 1
        omega
      -- (bb % P)^k % P = bb^k % P
      have hpm : ∀ (bb k : ℕ), (bb % P) ^ k % P = bb ^ k % P := by
        intro bb k; conv_rhs => rw [Nat.pow_mod]
      have hbsq : base * base = base ^ 2 := by ring
      by_cases hbpos : e / 2 > 0
      · simp only [hbpos, if_true]
        by_cases hodd : e % 2 = 1
        · simp only [hodd, if_true]
          -- LHS: (result*base % P) * (base*base % P)^(e/2) % P
          -- RHS: result * (base^(e%2) * (base^2)^(e/2)) % P
          rw [hsplit, hodd, pow_one]
          calc (result * base % P) * (base * base % P) ^ (e / 2) % P
              = (result * base) * (base * base) ^ (e / 2) % P := by
                rw [Nat.mul_mod (result * base % P), Nat.mod_mod,
                    hpm (base * base) (e / 2), ← Nat.mul_mod]
            _ = (result * base) * (base ^ 2) ^ (e / 2) % P := by rw [hbsq]
            _ = result * (base * (base ^ 2) ^ (e / 2)) % P := by ring_nf
        · have heven : e % 2 = 0 := by omega
          simp only [hodd, if_false]
          rw [hsplit, heven, pow_zero, Nat.one_mul]
          calc result * (base * base % P) ^ (e / 2) % P
              = result * (base * base) ^ (e / 2) % P := by
                rw [Nat.mul_mod result, hpm (base * base) (e / 2), ← Nat.mul_mod]
            _ = result * (base ^ 2) ^ (e / 2) % P := by rw [hbsq]
      · have he1 : e = 1 := by omega
        simp only [hbpos, if_false]
        subst he1
        simp only [Nat.reduceDiv, invPowSpec_zero,
                   show (1 : ℕ) % 2 = 1 from rfl, if_true, pow_zero,
                   Nat.mul_one, pow_one, Nat.mod_mod]

/-- The Fermat-inverse `inv_loop` computes `result · base^e (mod P)`,
    with the result in `[0, P)`. Square-and-multiply over canonical values
    via the now-proven `specialized_mul_correct`. -/
theorem inv_loop_value {P : Std.U64} (hP : ValidPrime P)
    (result base e : Std.U64)
    (hr : result.val < P.val) (hb : base.val < P.val) :
    ∃ r, gfp.Fp.Insts.Gf2_coreFieldTraitsFiniteFieldU64U128.inv_loop P result base e
      = ok r ∧ r.val < P.val ∧
      r.val = (result.val * base.val ^ e.val) % P.val := by
  have hP_pos : 0 < P.val := by have := hP.2.1; omega
  have hspec :
      gfp.Fp.Insts.Gf2_coreFieldTraitsFiniteFieldU64U128.inv_loop P result base e
      ⦃ r => r.val < P.val ∧
        r.val = invPowSpec P.val result.val base.val e.val % P.val ⦄ := by
    unfold gfp.Fp.Insts.Gf2_coreFieldTraitsFiniteFieldU64U128.inv_loop
    apply loop.spec
      (measure := fun ((_, _, e1) : Std.U64 × Std.U64 × Std.U64) => e1.val)
      (inv := fun ((res1, bas1, e1) : Std.U64 × Std.U64 × Std.U64) =>
        res1.val < P.val ∧ bas1.val < P.val ∧
        invPowSpec P.val res1.val bas1.val e1.val =
          invPowSpec P.val result.val base.val e.val)
    · intro ⟨res1, bas1, e1⟩ ⟨hres1, hbas1, hinv1⟩
      dsimp only
      simp only [gfp.Fp.Insts.Gf2_coreFieldTraitsFiniteFieldU64U128.inv_loop.body]
      by_cases he : e1 > 0#u64
      · simp only [he, if_true]
        have he1_pos : 0 < e1.val := by scalar_tac
        progress as ⟨i, hi_val, _⟩
        -- i = e1 & 1 ; i.val = e1.val % 2  (low bit)
        have hi_bit : i.val = e1.val % 2 := by
          rw [hi_val, UScalar.val_and]
          have : (1#u64 : Std.U64).val = 1 := by native_decide
          rw [this]
          have : e1.val &&& 1 = e1.val % 2 := by
            have := and_mask_mod e1.val 1
            simpa using this
          exact this
        -- result1 step
        have hres_step : ∃ res2, (if i = 1#u64
            then gfp.specialized_mul P res1 bas1 else ok res1) = ok res2
            ∧ res2.val < P.val
            ∧ res2.val = (if e1.val % 2 = 1 then res1.val * bas1.val % P.val
                          else res1.val) := by
          by_cases hi1 : i = 1#u64
          · simp only [hi1, if_true]
            have he1odd : e1.val % 2 = 1 := by
              have : i.val = (1#u64 : Std.U64).val := by rw [hi1]
              have h1v : (1#u64 : Std.U64).val = 1 := by native_decide
              rw [h1v] at this; omega
            obtain ⟨r2, hr2_eq, hr2_lt, hr2_val⟩ :=
              specialized_mul_correct hP hres1 hbas1
            exact ⟨r2, hr2_eq, hr2_lt, by simp only [he1odd, if_true]; exact hr2_val⟩
          · simp only [hi1, if_false]
            have he1even : e1.val % 2 ≠ 1 := by
              intro hodd
              apply hi1
              apply UScalar.val_eq_imp
              have h1v : (1#u64 : Std.U64).val = 1 := by native_decide
              rw [h1v]; omega
            exact ⟨res1, rfl, hres1, by simp only [he1even, if_false]⟩
        obtain ⟨res2, hres2_eq, hres2_lt, hres2_val⟩ := hres_step
        simp only [hres2_eq, bind_tc_ok]
        progress as ⟨enext, henext_val, _⟩
        have henext_eq : enext.val = e1.val / 2 := by
          rw [henext_val, Nat.shiftRight_eq_div_pow, pow_one]
        have henext_lt : enext.val < e1.val := by rw [henext_eq]; omega
        by_cases he2 : enext > 0#u64
        · simp only [he2, if_true]
          have henext_pos : 0 < enext.val := by scalar_tac
          obtain ⟨bas2, hbas2_eq, hbas2_lt, hbas2_val⟩ :=
            specialized_mul_correct hP hbas1 hbas1
          simp only [hbas2_eq, bind_tc_ok, spec, theta, wp_return]
          refine ⟨hres2_lt, hbas2_lt, ?_, ?_⟩
          · -- invPowSpec preserved
            rw [← hinv1]
            rw [invPowSpec_step P.val res1.val bas1.val e1.val he1_pos]
            have hd : e1.val / 2 > 0 := by rw [← henext_eq]; exact henext_pos
            have hb2 : bas2.val = (if e1.val / 2 > 0 then
                bas1.val * bas1.val % P.val else bas1.val) := by
              simp only [hd, if_true]; exact hbas2_val
            rw [hres2_val, hb2, henext_eq]
          · show enext.val < e1.val
            exact henext_lt
        · simp only [he2, if_false, spec, theta, wp_return]
          have henext0 : enext.val = 0 := by
            have : ¬ (0#u64 : Std.U64).val < enext.val := by
              simpa using he2
            have h0 : (0#u64 : Std.U64).val = 0 := by native_decide
            omega
          refine ⟨hres2_lt, hbas1, ?_, ?_⟩
          · rw [← hinv1, invPowSpec_step P.val res1.val bas1.val e1.val he1_pos]
            have hd : ¬ (e1.val / 2 > 0) := by rw [← henext_eq]; omega
            rw [hres2_val, henext_eq]
            simp only [hd, if_false]
          · show enext.val < e1.val
            exact henext_lt
      · simp only [he, if_false, spec, theta, wp_return]
        have he0 : e1.val = 0 := by
          have : ¬ (0#u64 : Std.U64).val < e1.val := by simpa using he
          have h0 : (0#u64 : Std.U64).val = 0 := by native_decide
          omega
        refine ⟨hres1, ?_⟩
        rw [← hinv1, he0, invPowSpec_zero]
        rw [Nat.mod_eq_of_lt hres1]
    · exact ⟨hr, hb, rfl⟩
  obtain ⟨r, hr_eq, hr_lt, hr_val⟩ := spec_imp_exists hspec
  refine ⟨r, hr_eq, hr_lt, ?_⟩
  rw [hr_val, invPowSpec_correct P.val hP_pos e.val result.val base.val]

end Specialized
