/-
  Gf2Core.Proofs.ModArith — Pure modular arithmetic lemmas

  Mathematical facts about Montgomery arithmetic independent of Aeneas.
  These connect the Montgomery representation to standard modular arithmetic.
-/
import Mathlib.Data.ZMod.Basic
import Mathlib.Data.Nat.Prime.Basic
import Mathlib.Data.Int.GCD
import Mathlib.Tactic.Ring
import Aeneas
import Gf2Core.Proofs.Defs

open Aeneas Aeneas.Std

set_option maxHeartbeats 400000

namespace MontArith

/-- R = 2^64, the Montgomery radix -/
def R : ℕ := 2 ^ 64

/-- For a valid prime P (odd, > 1), gcd(P, R) = 1 since R = 2^64 and P is odd prime > 2. -/
theorem R_coprime_P {P : Std.U64} (hP : ValidPrime P) (hP2 : P.val ≠ 2) :
    Nat.Coprime R P.val := by
  unfold R
  apply Nat.Coprime.pow_left 64
  rw [Nat.coprime_comm]
  apply (hP.1.coprime_iff_not_dvd).mpr
  intro h
  have hle := Nat.le_of_dvd (by omega) h
  have h1 := hP.2.1
  omega

/-- R has a modular inverse mod P when P is a valid odd prime. -/
theorem R_inv_exists {P : Std.U64} (hP : ValidPrime P) (hP2 : P.val ≠ 2) :
    ∃ Rinv : ℕ, R * Rinv % P.val = 1 % P.val := by
  have hcop := R_coprime_P hP hP2
  obtain ⟨m, _, hm⟩ := Nat.exists_mul_mod_eq_one_of_coprime hcop hP.2.1
  exact ⟨m, by rw [Nat.mod_eq_of_lt hP.2.1]; exact hm⟩

/-! ## REDC specification -/

/-- REDC computes t * R⁻¹ mod P.
    This is the mathematical specification that the Rust `redc` must satisfy. -/
def redc_spec (P t : ℕ) (Rinv : ℕ) : ℕ :=
  t * Rinv % P

/-- Montgomery form: to_mont(a) = a * R mod P -/
def to_mont_spec (P a : ℕ) : ℕ :=
  a * R % P

/-- From Montgomery form: from_mont(aR) = aR * R⁻¹ mod P = a mod P -/
def from_mont_spec (P aR : ℕ) (Rinv : ℕ) : ℕ :=
  aR * Rinv % P

/-! ## Montgomery arithmetic identities -/

/-- Roundtrip: from_mont(to_mont(a)) = a mod P -/
theorem roundtrip_spec {P : ℕ} (hP : 0 < P) (Rinv : ℕ)
    (hRinv : R * Rinv % P = 1 % P) (a : ℕ) (ha : a < P) :
    from_mont_spec P (to_mont_spec P a) Rinv = a := by
  simp only [from_mont_spec, to_mont_spec]
  rw [Nat.mod_mul_mod, mul_assoc, Nat.mul_mod a (R * Rinv) P, hRinv,
      ← Nat.mul_mod, mul_one, Nat.mod_eq_of_lt ha]

/-- Montgomery addition preserves the representation:
    from_mont(mont_add(aR, bR)) = (a + b) mod P -/
theorem add_spec {P : ℕ} (hP : 0 < P) (Rinv : ℕ)
    (hRinv : R * Rinv % P = 1 % P) (a b : ℕ) (ha : a < P) (hb : b < P) :
    from_mont_spec P ((to_mont_spec P a + to_mont_spec P b) % P) Rinv =
      (a + b) % P := by
  simp only [from_mont_spec, to_mont_spec]
  rw [Nat.mod_mul_mod,
      Nat.mul_mod (a * R % P + b * R % P) Rinv P,
      Nat.add_mod (a * R % P) (b * R % P) P,
      Nat.mod_mod, Nat.mod_mod, ← Nat.add_mod,
      ← Nat.mul_mod, ← add_mul, mul_assoc,
      Nat.mul_mod (a + b), hRinv, ← Nat.mul_mod, mul_one]

/-- Montgomery multiplication preserves the representation:
    from_mont(redc(aR * bR)) = (a * b) mod P -/
theorem mul_spec {P : ℕ} (hP : 0 < P) (Rinv : ℕ)
    (hRinv : R * Rinv % P = 1 % P) (a b : ℕ) (ha : a < P) (hb : b < P) :
    from_mont_spec P (redc_spec P (to_mont_spec P a * to_mont_spec P b) Rinv) Rinv =
      (a * b) % P := by
  simp only [from_mont_spec, to_mont_spec, redc_spec]
  rw [Nat.mod_mul_mod (a * R % P * (b * R % P) * Rinv)]
  conv_lhs =>
    rw [Nat.mul_mod (a * R % P * (b * R % P) * Rinv) Rinv P,
        Nat.mul_mod (a * R % P * (b * R % P)) Rinv P,
        Nat.mul_mod (a * R % P) (b * R % P) P,
        Nat.mod_mod, Nat.mod_mod,
        ← Nat.mul_mod (a * R) (b * R) P,
        ← Nat.mul_mod (a * R * (b * R)) Rinv P,
        ← Nat.mul_mod (a * R * (b * R) * Rinv) Rinv P]
  have hrng : a * R * (b * R) * Rinv * Rinv =
    (a * b * (R * Rinv)) * (R * Rinv) := by ring
  rw [hrng,
      Nat.mul_mod (a * b * (R * Rinv)) (R * Rinv) P, hRinv,
      ← Nat.mul_mod, mul_one,
      Nat.mul_mod (a * b) (R * Rinv) P, hRinv, ← Nat.mul_mod, mul_one]

/-- The unique multiple of P in the open interval (0, 2P) is P itself. -/
private lemma unique_mul_in_range (P x a : ℕ) (hP : 0 < P) (hx : x < P) (ha : a < P)
    (ha0 : a ≠ 0) (hmod : (x + a) % P = 0) : x + a = P := by
  obtain ⟨k, hk⟩ := (Nat.dvd_iff_mod_eq_zero).mpr hmod
  have : k < 2 := Nat.lt_of_mul_lt_mul_left (show P * k < P * 2 by linarith)
  have : 1 ≤ k := by rcases k with _ | k <;> simp_all
  have : k = 1 := by omega
  subst this; omega

/-- Montgomery negation preserves the representation:
    from_mont(P - aR) = (P - a) mod P = (-a) mod P -/
theorem neg_spec {P : ℕ} (hP : 0 < P) (Rinv : ℕ)
    (hRinv : R * Rinv % P = 1 % P) (a : ℕ) (ha : a < P) (ha0 : a ≠ 0) :
    from_mont_spec P (P - to_mont_spec P a % P) Rinv = (P - a) % P := by
  simp only [from_mont_spec, to_mont_spec, Nat.mod_mod]
  rw [Nat.mod_eq_of_lt (by omega : P - a < P)]
  set m := a * R % P with hm_def
  have hm_lt : m < P := Nat.mod_lt _ hP
  have hm_le : m ≤ P := by omega
  -- Key: m * Rinv % P = a (from roundtrip_spec)
  have hm_rinv : m * Rinv % P = a := by
    show from_mont_spec P (to_mont_spec P a) Rinv = a
    exact roundtrip_spec hP Rinv hRinv a ha
  -- (P - m) * Rinv + m * Rinv = P * Rinv
  have hsum_eq : (P - m) * Rinv + m * Rinv = P * Rinv := by
    rw [← Nat.add_mul, Nat.sub_add_cancel hm_le]
  -- ((P-m)*Rinv + m*Rinv) % P = P*Rinv % P = 0
  have hmod_sum : ((P - m) * Rinv + m * Rinv) % P = 0 := by
    rw [hsum_eq, Nat.mul_mod_right]
  -- Substitute via Nat.add_mod and hm_rinv
  rw [Nat.add_mod, hm_rinv] at hmod_sum
  -- hmod_sum: ((P - m) * Rinv % P + a) % P = 0
  have hxa := unique_mul_in_range P _ a hP (Nat.mod_lt _ hP) ha ha0 hmod_sum
  omega

/-- (P-1)² fits in u128 for ValidPrime P -/
theorem p_minus_one_sq_le_u128_max {P : Std.U64} (hP : ValidPrime P) :
    (P.val - 1) * (P.val - 1) ≤ U128.max := by
  have hp1 : P.val - 1 ≤ 2 ^ 63 - 1 := by have := hP.2.2; omega
  calc (P.val - 1) * (P.val - 1)
      ≤ (2 ^ 63 - 1) * (2 ^ 63 - 1) := Nat.mul_le_mul hp1 hp1
    _ ≤ U128.max := by rw [U128.max_eq]; norm_num

end MontArith
