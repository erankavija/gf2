/-
  Gf2Core.Proofs.Gf2mDefs — Foundation definitions for GF(2^m) proofs

  Defines ValidGf2mParams predicate and pure mathematical specifications
  for GF(2) polynomial arithmetic (shift-and-XOR multiplication with reduction).
  All proofs work against the Aeneas-generated code in Funs.lean.
-/
import Aeneas
import Gf2Core.Types
import Gf2Core.Funs

open Aeneas Aeneas.Std Result ControlFlow Error
open gf2_core

set_option maxHeartbeats 800000

/-- Valid GF(2^m) parameters: m is positive, fits in u64 shifts,
    and the primitive polynomial has degree m. -/
structure ValidGf2mParams (m : Std.Usize) (poly : Std.U64) : Prop where
  /-- Extension degree is positive -/
  m_pos : 0 < m.val
  /-- m ≤ 63 so that 1 << m fits in u64 and shift amounts are valid -/
  m_le : m.val ≤ 63
  /-- poly has degree m: 2^m ≤ poly -/
  poly_high : 2 ^ m.val ≤ poly.val
  /-- poly < 2^(m+1): degree is exactly m -/
  poly_bound : poly.val < 2 ^ (m.val + 1)

namespace Gf2mSpec

/-- One step of the shift-reduce: shift temp left by 1, reduce by poly if high bit was set. -/
def shift_reduce (temp m poly : ℕ) : ℕ :=
  let will_overflow := temp / 2 ^ (m - 1) % 2 = 1
  let shifted := temp * 2
  if will_overflow then Nat.xor shifted poly else shifted

/-- One iteration body: accumulate bit i of b into result, then shift-reduce temp.
    Returns (result', temp'). The proof argument is needed for Nat.fold compatibility. -/
def step (b m poly : ℕ) (i : ℕ) (_ : i < m) (state : ℕ × ℕ) : ℕ × ℕ :=
  let (result, temp) := state
  let bit_set := b / 2 ^ i % 2 = 1
  let result' := if bit_set then Nat.xor result temp else result
  let temp' := shift_reduce temp m poly
  (result', temp')

/-- step is proof-irrelevant: the bound argument is unused. -/
theorem step_proof_irrel (b m poly i : ℕ) (h1 h2 : i < m) (state : ℕ × ℕ) :
    step b m poly i h1 state = step b m poly i h2 state := rfl

/-- The mathematical specification: fold `step` over indices 0..m-1,
    starting from (result=0, temp=a), then mask the result to m bits. -/
def mul_raw_spec (a b m poly : ℕ) : ℕ :=
  let final := Nat.fold m (step b m poly) (0, a)
  final.1 % 2 ^ m

end Gf2mSpec
