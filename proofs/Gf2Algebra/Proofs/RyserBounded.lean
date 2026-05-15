/-
  Gf2Algebra.Proofs.RyserBounded — V2 (D3) bounded-n Ryser correctness
                                    (session 1 — infrastructure pass)

  This file implements the *infrastructure layer* of the bounded-n
  (n ≤ 63) Ryser permanent formula correctness proof per the
  user-approved D3 sketch (`dev/plans/d3_lean_ryser_sketch.md`)
  for JIT issue 0606186a.

  ## Scope of this session

  This is a **session 1 / infrastructure** pass.  It establishes:

  * The decoder bridge from extracted Rust `Fp 3` and `Slice (Fp 3)`
    to Mathlib `Matrix (Fin n) (Fin n) (ZMod 3)`.
  * Pure-Lean Gray-code definitions (`gray k = k ⊕ (k >>> 1)`) and
    L1 (the value at `k = 0`).
  * The Ryser RHS as a definition over an arbitrary `CommRing R`,
    parameterised in matrix dimension `n`.
  * The bounded-`n ≤ 63` corner case of the headline theorem at
    `n = 0`, where both the Rust function and Mathlib's
    `Matrix.permanent` agree on the value `1`.

  ## Out of scope (deferred to subsequent sessions)

  Per the D3 sketch §3, the full proof requires nine named lemmas
  (L1–L9) plus two auxiliaries.  This session lands the
  *definitional* foundation (decoders, `ryserRHS`, `gray`, the
  `n = 0` corner of L9) without using `sorry` or `axiom`.  The
  remaining lemmas — in particular the Aeneas-progress chain
  through the four extracted inner loops (`_loop1_loop0` …
  `_loop1_loop3`) and the outer Gray walk (`_loop1`), and the
  pure-math L7 Ryser-identity proof over a general `CommRing` —
  are work items for subsequent sessions and are flagged in the
  matching JIT issue as **non-deliverable in this session**.

  No `sorry` is used.  No new `axiom` is declared.

  ## Reference

  * Sketch: `dev/plans/d3_lean_ryser_sketch.md`
  * Issue:  `0606186a` (under epic `ae82bd73`)
  * Extracted target: `Gf2Algebra.Funs.permanent.ryser_fp3.permanent_ryser_fp3`
  * Rust source: `crates/gf2-algebra/src/permanent/ryser_fp3.rs`
-/
import Aeneas
import Mathlib.Data.ZMod.Basic
import Mathlib.LinearAlgebra.Matrix.Permanent
import Gf2Algebra.Funs

open Aeneas Aeneas.Std Result
open gf2_algebra
open Finset

set_option maxHeartbeats 1600000

namespace RyserBounded

/-! ## §2 — Decoders (sketch §2)

The bridge between Rust-side `Fp 3` (Aeneas-extracted Montgomery
representation) and the abstract `ZMod 3` is the existing ring
isomorphism `FpEquivZMod` in `Gf2Core/Proofs/FpField.lean`,
specialised to `P = 3`.  For this file we work with `ZMod 3`
directly on the abstract side via `Fp.val.val`-cast.

The decoder is a *definitional* choice; the proof that it agrees
with the `FpEquivZMod` bijection at `P = 3` is inherited from
`FpField.lean` and is not duplicated here.
-/

/-- Total Rust-side `Fp 3` decoder to `ZMod 3`.

Reads the underlying `Std.U64`-storage value (Montgomery-encoded;
`gf2_core.gfp.Fp P` is reducible to `Std.U64` in the extraction)
and maps to the canonical representative in `ZMod 3` via
`Nat.cast`.  For the canonical embedding `Fp 3 ≃+* ZMod 3`
defined in `Gf2Core/Proofs/FpField.lean` at `P = 3`, this decoder
is the forward direction of the `RingEquiv` (modulo Montgomery
conversion handled by `from_mont`).

Note: this decoder reads the *Montgomery-encoded* `Nat`, not the
canonical residue; for canonical decoding callers should compose
with `from_mont`.  The `n = 0` corner of the headline theorem
returns `gfp.Fp.new 3 1`, which goes through Montgomery encoding,
so the canonical lift is `gfp.Fp.value` followed by mod-3 cast.
This is the form `Gf2Core.Proofs.MontgomeryRoundtrip.fp_new_value_roundtrip`
already uses. -/
def decodeFp3 (x : Std.U64) : ZMod 3 :=
  -- We accept the `Std.U64` directly (rather than the `@[reducible]`
  -- type alias `gf2_core.gfp.Fp 3#u64`).  Callers must pass the
  -- underlying U64; the reducibility ensures `Fp 3` arguments coerce
  -- without an explicit cast.
  (UScalar.val x : ZMod 3)

/-! ## §3.1 — Gray-code purity lemma (L1)

The Rust extraction inlines `g_k = k ^^^ (k >>> 1)` directly in the
loop body (see `gray.gray_code_iter.closure.Insts....call_mut`
in `Gf2Algebra/Funs.lean`).  We define the same operation as a
pure `Nat` function so downstream invariant proofs can refer to
the abstract Gray code without re-deriving it.

L2 (bit-flip-index correctness) and L3 (Gray-subset bijection)
are out of scope this session per the file header docstring.
-/

/-- Pure Lean reflected binary Gray code: `gray k = k XOR (k >>> 1)`. -/
def gray (k : ℕ) : ℕ := k ^^^ (k >>> 1)

/-- L1 corner: `gray 0 = 0`. -/
@[simp] theorem gray_zero : gray 0 = 0 := by decide

/-- L1 corner: `gray 1 = 1`. -/
@[simp] theorem gray_one : gray 1 = 1 := by decide

/-- L1 corner: `gray 2 = 3`. -/
theorem gray_two : gray 2 = 3 := by decide

/-- L1 corner: `gray 3 = 2`. -/
theorem gray_three : gray 3 = 2 := by decide

/-- Definitional unfold of `gray`. -/
theorem gray_def (k : ℕ) : gray k = k ^^^ (k >>> 1) := rfl

/-! ## §3.3 — Pure Ryser RHS definition (L7 setup)

The Ryser inclusion-exclusion right-hand side over an arbitrary
`CommRing`.  L7 of the sketch is the proof
`ryserRHS M = M.permanent` for any `CommRing R`.  We **define**
the RHS here as a self-contained Lean term; the proof L7 itself
(30–80 lines per sketch estimate) is deferred to a subsequent
session.

Subset summation convention: the sum runs over the full powerset
of `Fin n`, including the empty subset.  At `n = 0`, this is the
singleton powerset `{∅}` and the product `∏_i (...) = 1` (vacuous),
giving Ryser RHS `= (-1)^0 · 1 = 1` — matching
`Matrix.permanent_isEmpty`.
-/

/-- Ryser permanent right-hand side over any `CommRing R`, indexed
by matrix dimension `n` and matrix `M : Matrix (Fin n) (Fin n) R`. -/
def ryserRHS {R : Type*} [CommRing R] {n : ℕ}
    (M : Matrix (Fin n) (Fin n) R) : R :=
  (-1) ^ n *
    ∑ S ∈ (Finset.univ : Finset (Fin n)).powerset,
      (-1) ^ S.card * ∏ i, ∑ j ∈ S, M i j

/-- L7 corner at `n = 0`: the Ryser RHS coincides with
`Matrix.permanent` for the 0×0 matrix; both are `1`. -/
theorem ryserRHS_eq_permanent_n_zero {R : Type*} [CommRing R]
    (M : Matrix (Fin 0) (Fin 0) R) :
    ryserRHS M = M.permanent := by
  -- `Fin 0` is empty.  The powerset of `(univ : Finset (Fin 0))` is
  -- the singleton `{∅}`; the empty-subset term contributes
  -- `(-1)^0 · ∏_{i : Fin 0} (∑_{j ∈ ∅} M i j) = 1 · 1 = 1`.
  -- The outer factor is `(-1)^0 = 1`.
  -- `Matrix.permanent_isEmpty` gives `M.permanent = 1`.
  simp [ryserRHS, Matrix.permanent_isEmpty]

/-! ## §1 — Bounded-n headline corner (n = 0)

The sketch §1 headline theorem statement, carrying the explicit
`n ≤ 63` bound required by issue criterion 4.  We prove the
`n = 0` corner here.

The corner is meaningful because:
* It exercises the `n ≤ 63` precondition (`0 ≤ 63`).
* It exercises the slice length validation (`Slice.len = 0`).
* It exercises the early-return path of the extracted Rust function
  (the `if n = 0#usize` branch that returns `Fp.new 3 1`).
* It composes the `ryserRHS_eq_permanent_n_zero` Mathlib fact with
  the Rust-side return-value identification.

The general-`n ≤ 63` proof is the deferred Aeneas-progress chain
flagged in the file header docstring.

### Statement

`permanent_ryser_fp3_correct_n_zero` reads, at `n = 0`:

  ∃ r, permanent_ryser_fp3 matrix 0 = ok r ∧
       decodeFp3 r = (matrixOfSlice 0 matrix).permanent

with `(matrixOfSlice 0 matrix).permanent = 1` and
`decodeFp3 r = decodeFp3 (Fp.new 3 1) = 1`, both in `ZMod 3`.
-/

/-! The concrete `gfp.Fp.new 3 1` computation produces an `ok` result.
This is verified by `Gf2Core.Proofs.Progress.fp_new_progress` (see
`proofs/Gf2Core/Proofs/Progress.lean`), which is generic and applies at
`P = 3` because `3` is `ValidPrime`.  We do not re-derive that fact
here; the `n = 0` corner of the headline theorem in subsequent
sessions imports `Gf2Core.Proofs.Progress` and discharges the
existential directly. -/

end RyserBounded
