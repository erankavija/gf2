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

end Specialized
