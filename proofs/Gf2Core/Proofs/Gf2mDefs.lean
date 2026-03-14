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

/-- The specification loop, mirroring the Aeneas loop structure.
    Iterates from index `i` to `m-1`, accumulating GF(2^m) product. -/
def specLoop (b m poly i result temp : ℕ) : ℕ × ℕ :=
    if i ≥ m then (result, temp)
    else
      let bit_set := b / 2 ^ i % 2 = 1
      let result' := if bit_set then Nat.xor result temp else result
      let temp' := shift_reduce temp m poly
      specLoop b m poly (i + 1) result' temp'
  termination_by m - i

/-- The mathematical specification of GF(2^m) multiplication.
    Runs the spec loop from i=0 with result=0, temp=a, then masks to m bits. -/
def mul_raw_spec (a b m poly : ℕ) : ℕ :=
  (specLoop b m poly 0 0 a).1 % 2 ^ m

/-- The mathematical specification of GF(2^m) addition: bitwise XOR. -/
def add_raw_spec (a b : ℕ) : ℕ := a ^^^ b

/-- The mathematical specification of GF(2^m) exponentiation (square-and-multiply). -/
def pow_raw_spec (base exp m poly : ℕ) : ℕ :=
  if exp = 0 then 1
  else
    let result := pow_raw_spec base (exp / 2) m poly
    let squared := mul_raw_spec result result m poly
    if exp % 2 = 1 then mul_raw_spec squared base m poly
    else squared
termination_by exp
decreasing_by omega

/-- The mathematical specification of GF(2^m) multiplicative inverse.
    Returns 0 for zero input. For nonzero a, computes a^(2^m - 2). -/
def inverse_raw_spec (a m poly : ℕ) : ℕ :=
  if a = 0 then 0
  else pow_raw_spec a (2 ^ m - 2) m poly

end Gf2mSpec
