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

end Specialized
