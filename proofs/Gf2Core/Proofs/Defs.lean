/-
  Gf2Core.Proofs.Defs — Foundation definitions for Fp field proofs

  Defines ValidPrime predicate and FpVal wrapper type that bridges
  the Aeneas Result monad to pure Mathlib typeclasses.
-/
import Aeneas
import Gf2Core.Types
import Gf2Core.Funs

open Aeneas Aeneas.Std Result ControlFlow Error
open gf2_core

set_option maxHeartbeats 400000

/-- A prime P that fits in u64 Montgomery arithmetic: prime, > 1, ≤ 2^63.
    The bound P ≤ 2^63 ensures P * R < 2^128 where R = 2^64. -/
def ValidPrime (P : Std.U64) : Prop :=
  Nat.Prime P.val ∧ 1 < P.val ∧ P.val ≤ 2 ^ 63

/-- Wrapper around the Montgomery representation of an Fp element.
    Avoids typeclass conflicts with bare U64 by carrying the `< P` invariant. -/
structure FpVal (P : Std.U64) where
  /-- The Montgomery representation (stored value in [0, P)) -/
  mont : Std.U64
  /-- Invariant: the Montgomery representation is less than the modulus -/
  h_lt : mont.val < P.val

namespace FpVal

/-- R = 2^64, the Montgomery radix -/
def R : ℕ := 2 ^ 64

/-- ValidPrime implies P > 1 as a U64 comparison -/
theorem validPrime_gt_one {P : Std.U64} (hP : ValidPrime P) : 1 < P.val :=
  hP.2.1

/-- ValidPrime implies P ≤ 2^63 -/
theorem validPrime_le_bound {P : Std.U64} (hP : ValidPrime P) : P.val ≤ 2 ^ 63 :=
  hP.2.2

/-- ValidPrime implies P > 0 -/
theorem validPrime_pos {P : Std.U64} (hP : ValidPrime P) : 0 < P.val := by
  have := hP.2.1; omega

/-- ValidPrime implies VALIDATED succeeds (spec form for @[progress]) -/
@[progress]
theorem validated_progress {P : Std.U64} (hP : ValidPrime P) :
    gfp.Fp.VALIDATED P ⦃ fun _ => True ⦄ := by
  simp only [gfp.Fp.VALIDATED]
  progress
  · have := hP.2.1; scalar_tac
  progress as ⟨i, hi⟩
  progress
  · have := hP.2.2
    have : i.val = 2^63 := by rw [hi]; native_decide
    scalar_tac

end FpVal
