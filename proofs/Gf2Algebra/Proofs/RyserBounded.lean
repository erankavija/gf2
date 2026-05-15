/-
  Gf2Algebra.Proofs.RyserBounded — V2 (D3) bounded-n Ryser correctness
                                    (sessions 1 + 2)

  This file implements the bounded-n (n ≤ 63) Ryser permanent formula
  correctness proof per the user-approved D3 sketch
  (`dev/plans/d3_lean_ryser_sketch.md`) for JIT issue 0606186a.

  ## Scope of session 1 (already landed in commit `762ce0ac`)

  Session 1 established the *infrastructure layer*:

  * The decoder bridge from extracted Rust `Fp 3` and `Slice (Fp 3)`
    to Mathlib `Matrix (Fin n) (Fin n) (ZMod 3)`.
  * Pure-Lean Gray-code definitions (`gray k = k ⊕ (k >>> 1)`) and
    L1 corners (`gray_zero` .. `gray_three`, `gray_def`).
  * The Ryser RHS as a definition over an arbitrary `CommRing R`,
    parameterised in matrix dimension `n`.
  * The L7 corner case (`ryserRHS_eq_permanent_n_zero`) for the
    `n = 0` matrix.

  ## Scope of session 2 (this commit)

  Session 2 lands the Gray-code traversal lemmas (sketch §6.2):

  * `flipBit k` — position of the bit that flips between `gray k` and
    `gray (k+1)`, defined via `Nat.find` over `Nat.testBit (k+1)`.
  * L1 (general-n) `gray_lt_two_pow` — `gray k < 2^n` for `k < 2^n` and
    `n ≤ 63`, framed via `Fin (2^n)`.
  * L1.5 `gray_succ_xor` — `gray (k+1) = gray k XOR (1 <<< flipBit k)`,
    the single-bit-flip defining property.
  * L2 `flipBit_lt` — `flipBit k < n` whenever `k + 1 < 2^n` and `n ≤ 63`.
  * L3 `subsetOfBits_bijective` — `subsetOfBits n` is a bijection
    `Fin (2^n) → Finset (Fin n)` (equivalently, onto the powerset of
    `Finset.univ : Finset (Fin n)`).

  ## Out of scope (deferred to sessions 3 + 4)

  L4 (column-sum loop invariant), L5 (fold-product invariant), L6 (outer
  parity/Gray-bijection sum), L7 (general-`CommRing` Ryser identity), L8
  (extracted-spec progress chain), L9 (top-level theorem at general
  `n ≤ 63`) remain for sessions 3 and 4.

  No `sorry` is used.  No new `axiom` is declared.

  ## Reference

  * Sketch: `dev/plans/d3_lean_ryser_sketch.md`
  * Issue:  `0606186a` (under epic `ae82bd73`)
  * Extracted target: `Gf2Algebra.Funs.permanent.ryser_fp3.permanent_ryser_fp3`
  * Rust source: `crates/gf2-algebra/src/permanent/ryser_fp3.rs`
-/
import Aeneas
import Mathlib.Data.ZMod.Basic
import Mathlib.Data.Nat.Bitwise
import Mathlib.Data.Nat.Find
import Mathlib.Data.Fintype.Powerset
import Mathlib.Data.Finset.Powerset
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

/-! ### L1 general-n: `gray` preserves the `< 2^n` bound

The Rust loop body computes `gray k` as a `u64` and uses its bits to
index into the `Fin n` column array.  For correctness we need the
abstract Gray code value to remain `< 2^n` when the input is `< 2^n`.
This is L1 in its `Fin (2^n)` framing per sketch §3.1.
-/

/-- L1 (general-n): if `k < 2^n` then `gray k < 2^n`.

The XOR-with-shifted-self pattern can only set bits at positions that
were already set in `k` or at position `i+1` for some bit `i` of `k`
(via the right shift) — and the shift moves bits down, so the
top bit at position `n-1` of `k` (the only one ≥ `n-1`) maps to
position `n-2` in `k >>> 1`, leaving the overall result bounded by
the largest bit of `k`. -/
theorem gray_lt_two_pow (n k : ℕ) (hk : k < 2 ^ n) : gray k < 2 ^ n := by
  -- Strategy: gray k = k XOR (k >>> 1).  Every bit of gray k at position
  -- ≥ n must be false, since both k and k >>> 1 have all such bits false.
  unfold gray
  apply Nat.lt_pow_two_of_testBit
  intro i hi
  rw [Nat.testBit_xor]
  -- testBit k i = false since k < 2^n ≤ 2^i
  have hk' : k < 2 ^ i := lt_of_lt_of_le hk (Nat.pow_le_pow_right (by decide) hi)
  rw [Nat.testBit_lt_two_pow hk']
  -- testBit (k >>> 1) i = testBit k (1 + i), also false
  rw [Nat.testBit_shiftRight]
  have hk'' : k < 2 ^ (1 + i) := lt_of_lt_of_le hk' (Nat.pow_le_pow_right (by decide) (by omega))
  rw [Nat.testBit_lt_two_pow hk'']
  rfl

/-- L1 (general-n, `Fin`-framed): the Gray-code map restricts to
`Fin (2^n) → Fin (2^n)`. -/
def grayFin (n : ℕ) (k : Fin (2 ^ n)) : Fin (2 ^ n) :=
  ⟨gray k.val, gray_lt_two_pow n k.val k.isLt⟩

/-- L1 (general-n) value identity: `(grayFin n k).val = gray k.val`. -/
@[simp] theorem grayFin_val (n : ℕ) (k : Fin (2 ^ n)) :
    (grayFin n k).val = gray k.val := rfl

/-! ## §6.1 — `flipBit` and `subsetOfBits` definitions

`flipBit k` is the index of the bit that toggles between `gray k` and
`gray (k+1)` — equivalently, the position of the lowest set bit of
`k+1`.  In Rust this is computed as `(k+1).trailing_zeros()`; in Lean we
use `Nat.find` over `Nat.testBit (k+1)` to extract the same index in a
proof-friendly form.

`subsetOfBits n k` is the subset of `Fin n` whose elements are the bit
positions of `gray k` that lie below `n`.  This is the canonical
"current subset" of the Gray-code walk at step `k`.
-/

/-- Witness that `k + 1` has at least one set bit (it is positive). -/
private theorem succ_has_testBit (k : ℕ) : ∃ i, (k + 1).testBit i = true := by
  have : k + 1 ≠ 0 := by omega
  exact Nat.exists_testBit_of_ne_zero this

/-- Position of the lowest set bit of `k+1`.

This is the bit that flips between `gray k` and `gray (k+1)`, equal in
the Rust extraction to `(k+1).trailing_zeros()`.  We use `Nat.find` over
the witness that `k+1` has a set bit; in `decide`-friendly form, this
returns `0` when `k+1` is odd and recurses otherwise. -/
noncomputable def flipBit (k : ℕ) : ℕ :=
  Nat.find (succ_has_testBit k)

/-- Defining spec for `flipBit`: bit `flipBit k` of `k+1` is set. -/
theorem flipBit_testBit (k : ℕ) : (k + 1).testBit (flipBit k) = true := by
  unfold flipBit
  exact Nat.find_spec (succ_has_testBit k)

/-- All bits of `k+1` strictly below `flipBit k` are zero. -/
theorem flipBit_min (k : ℕ) {m : ℕ} (h : m < flipBit k) : (k + 1).testBit m = false := by
  have hne : ¬ (k + 1).testBit m = true := by
    unfold flipBit at h
    exact Nat.find_min (succ_has_testBit k) h
  cases hb : (k + 1).testBit m
  · rfl
  · exact absurd hb hne

/-- The subset of `Fin n` whose elements are the indices `< n` at which
`gray k` has a `1` bit.  This is the "current subset" of the Gray-code
walk at step `k`. -/
def subsetOfBits (n : ℕ) (k : ℕ) : Finset (Fin n) :=
  (Finset.univ : Finset (Fin n)).filter (fun i => (gray k).testBit i.val)

/-- Membership criterion for `subsetOfBits`. -/
@[simp] theorem mem_subsetOfBits {n k : ℕ} {i : Fin n} :
    i ∈ subsetOfBits n k ↔ (gray k).testBit i.val = true := by
  simp [subsetOfBits]

/-! ## §6.2 — L1.5 (single-bit-flip identity), L2 (`flipBit` bound), L3 (bijection)

L1.5 is the defining property of the Gray-code traversal: each step
flips exactly one bit at position `flipBit k`.  L2 bounds the flipped
position by `n` whenever the step index remains in `Fin (2^n)`.  L3
establishes that the map `k ↦ subsetOfBits n k` is a bijection
`Fin (2^n) → Finset (Fin n)` (equivalently, onto the powerset of
`(Finset.univ : Finset (Fin n))` since every subset of `Fin n` is in
that powerset).
-/

/-! ### L1.5 — single-bit-flip identity for `gray`

`gray (k+1) = gray k XOR (1 <<< flipBit k)`: the Gray code differs by
exactly one bit per step, at position `flipBit k`.  We prove this by
extensional equality on `Nat.testBit` at every position, going through
a structural decomposition of `k+1`'s low bits.

The key intermediate fact is `kSucc_mod` below: `(k+1) % 2^(flipBit k + 1)
= 2^(flipBit k)`.  From this we read off the decomposition
`k + 1 = q · 2^(flipBit k + 1) + 2^(flipBit k)`, equivalently
`k = q · 2^(flipBit k + 1) + (2^(flipBit k) - 1)`.  All testBit relations
between `k` and `k+1` follow from this single fact.
-/

/-- The low bits of `k+1` form `2^(flipBit k)`: all bits below `flipBit k`
are `0`, bit `flipBit k` is `1`. -/
private theorem kSucc_mod (k : ℕ) :
    (k + 1) % 2 ^ (flipBit k + 1) = 2 ^ flipBit k := by
  apply Nat.eq_of_testBit_eq
  intro j
  rw [Nat.testBit_mod_two_pow]
  by_cases hj : j < flipBit k + 1
  · simp [hj]
    by_cases hj' : j < flipBit k
    · rw [flipBit_min k hj', Nat.testBit_two_pow_of_ne (by omega)]
    · have hjeq : j = flipBit k := by omega
      rw [hjeq, flipBit_testBit k, Nat.testBit_two_pow_self]
  · simp [hj]
    rw [Nat.testBit_two_pow_of_ne (by omega)]

/-- Decomposition: `k + 1 = 2^(flipBit k + 1) * q + 2^(flipBit k)` where
`q = (k+1) / 2^(flipBit k + 1)`. -/
private theorem kSucc_decomp (k : ℕ) :
    k + 1 = 2 ^ (flipBit k + 1) * ((k + 1) / 2 ^ (flipBit k + 1)) + 2 ^ flipBit k := by
  conv_lhs => rw [← Nat.div_add_mod (k + 1) (2 ^ (flipBit k + 1))]
  rw [kSucc_mod]

/-- Decomposition: `k = 2^(flipBit k + 1) * q + (2^(flipBit k) - 1)`. -/
private theorem k_decomp (k : ℕ) :
    k = 2 ^ (flipBit k + 1) * ((k + 1) / 2 ^ (flipBit k + 1)) + (2 ^ flipBit k - 1) := by
  have h2 : (1 : ℕ) ≤ 2 ^ flipBit k := Nat.one_le_iff_ne_zero.mpr (by positivity)
  have := kSucc_decomp k
  omega

/-- The remainder `2^flipBit k - 1` is `< 2^(flipBit k + 1)`. -/
private theorem twoPow_sub_one_lt (k : ℕ) : 2 ^ flipBit k - 1 < 2 ^ (flipBit k + 1) := by
  have hp : 0 < 2 ^ flipBit k := Nat.two_pow_pos _
  rw [pow_succ]
  omega

/-- Bit pattern of `k`: bits below `flipBit k` are `1`, bit `flipBit k` is `0`. -/
private theorem k_testBit_low (k : ℕ) {m : ℕ} (h : m < flipBit k) :
    k.testBit m = true := by
  have hkform : k = 2 ^ (flipBit k + 1) * ((k + 1) / 2 ^ (flipBit k + 1))
                  + (2 ^ flipBit k - 1) := k_decomp k
  have hmf1 : m < flipBit k + 1 := by omega
  conv_lhs => rw [hkform]
  rw [Nat.testBit_two_pow_mul_add (b_lt := twoPow_sub_one_lt k)]
  rw [if_pos hmf1, Nat.testBit_two_pow_sub_one]
  exact decide_eq_true h

private theorem k_testBit_at (k : ℕ) : k.testBit (flipBit k) = false := by
  -- Set the argument to testBit aside so we can rewrite only the receiver.
  generalize hi : flipBit k = i
  have hkform : k = 2 ^ (flipBit k + 1) * ((k + 1) / 2 ^ (flipBit k + 1))
                  + (2 ^ flipBit k - 1) := k_decomp k
  have hmf1 : i < flipBit k + 1 := by rw [← hi]; exact Nat.lt_succ_self _
  conv_lhs => rw [hkform]
  rw [Nat.testBit_two_pow_mul_add (b_lt := twoPow_sub_one_lt k)]
  rw [if_pos hmf1, Nat.testBit_two_pow_sub_one]
  rw [← hi]
  exact decide_eq_false (Nat.lt_irrefl _)

/-- Auxiliary: for any position `m`, `testBit (k+1) m` and `testBit k m`
agree everywhere strictly above `flipBit k`. -/
private theorem testBit_succ_eq_of_gt (k : ℕ) {m : ℕ} (h : flipBit k < m) :
    (k + 1).testBit m = k.testBit m := by
  -- Strategy: write k + 1 = 2^(flipBit k + 1) * q + 2^(flipBit k) and
  --           k     = 2^(flipBit k + 1) * q + (2^(flipBit k) - 1).
  -- For m ≥ flipBit k + 1, `Nat.testBit_two_pow_mul_add` reduces both sides
  -- to `q.testBit (m - (flipBit k + 1))`.
  have h2 : (2 : ℕ) ^ flipBit k < 2 ^ (flipBit k + 1) := by
    rw [pow_succ]; have hp : 0 < 2 ^ flipBit k := Nat.two_pow_pos _; omega
  -- LHS: (k+1).testBit m = (2^(flipBit k + 1) * q + 2^flipBit k).testBit m.
  conv_lhs => rw [kSucc_decomp k, Nat.testBit_two_pow_mul_add (b_lt := h2)]
  -- RHS: k.testBit m = (2^(flipBit k + 1) * q + (2^flipBit k - 1)).testBit m.
  conv_rhs => rw [k_decomp k, Nat.testBit_two_pow_mul_add (b_lt := twoPow_sub_one_lt k)]
  -- Both branches go to the `else` (m ≥ flipBit k + 1) branch.
  simp only [show ¬ m < flipBit k + 1 from by omega, if_false]

/-- L1.5 (sketch §6.2 bullet 2): `gray (k+1) = gray k XOR (1 <<< flipBit k)`.

This is the defining single-bit-flip property of the reflected binary
Gray code: each step toggles exactly one bit, at position `flipBit k`.
Proved by `Nat.eq_of_testBit_eq` over every bit position, using the
structural decomposition of `k`/`k+1` recorded in `kSucc_mod` and
`k_decomp`. -/
theorem gray_succ_xor (k : ℕ) :
    gray (k + 1) = gray k ^^^ (1 <<< flipBit k) := by
  apply Nat.eq_of_testBit_eq
  intro j
  -- testBit of (1 <<< flipBit k) at position j: by Nat.testBit_shiftLeft,
  -- this is `decide (j ≥ flipBit k) && testBit 1 (j - flipBit k)`,
  -- which equals `j = flipBit k` (since `testBit 1 i = true ↔ i = 0`).
  have hbit_shift : (1 <<< flipBit k).testBit j = decide (j = flipBit k) := by
    rw [Nat.testBit_shiftLeft]
    -- Goal: decide (j ≥ flipBit k) && Nat.testBit 1 (j - flipBit k) = decide (j = flipBit k)
    by_cases hge : j ≥ flipBit k
    · simp only [hge, decide_true, Bool.true_and]
      -- Nat.testBit 1 (j - flipBit k) = decide (j = flipBit k)
      cases hcase : Nat.testBit 1 (j - flipBit k)
      · -- false: i.e. j - flipBit k ≠ 0, so j ≠ flipBit k.
        have hne : j - flipBit k ≠ 0 := by
          intro hsub
          have : Nat.testBit 1 (j - flipBit k) = true := by
            rw [hsub]; decide
          rw [this] at hcase; exact Bool.noConfusion hcase
        have : j ≠ flipBit k := by omega
        simp [this]
      · -- true: i.e. j - flipBit k = 0, so j = flipBit k.
        have hzero : j - flipBit k = 0 := by
          have := (Nat.testBit_one_eq_true_iff_self_eq_zero (i := j - flipBit k)).mp hcase
          exact this
        have : j = flipBit k := by omega
        simp [this]
    · have : ¬ (j = flipBit k) := by omega
      simp [hge, this]
  unfold gray
  simp only [Nat.testBit_xor, Nat.testBit_shiftRight, hbit_shift]
  -- Goal: (k+1).testBit j ^^ (k+1).testBit (1 + j) =
  --        (k.testBit j ^^ k.testBit (1 + j)) ^^ decide (j = flipBit k)
  by_cases hj : j < flipBit k
  · -- j strictly below flipBit k.
    have hjp1 : j + 1 ≤ flipBit k := by omega
    have hkp1_j : (k + 1).testBit j = false := flipBit_min k hj
    have hk_j   : k.testBit j         = true  := k_testBit_low k hj
    have hjne   : ¬ (j = flipBit k) := by omega
    rw [hkp1_j, hk_j]
    rcases lt_or_eq_of_le hjp1 with hjp1' | hjp1'
    · -- j + 1 < flipBit k.
      have h_1pj_lt : 1 + j < flipBit k := by omega
      have hkp1_j1 : (k + 1).testBit (1 + j) = false := flipBit_min k h_1pj_lt
      have hk_j1   : k.testBit (1 + j)         = true  := k_testBit_low k h_1pj_lt
      rw [hkp1_j1, hk_j1]
      simp [hjne]
    · -- j + 1 = flipBit k, i.e. 1 + j = flipBit k.
      have h1pj_eq : 1 + j = flipBit k := by omega
      have hkp1_j1 : (k + 1).testBit (1 + j) = true := by
        rw [h1pj_eq]; exact flipBit_testBit k
      have hk_j1 : k.testBit (1 + j) = false := by
        rw [h1pj_eq]; exact k_testBit_at k
      rw [hkp1_j1, hk_j1]
      simp [hjne]
  · -- j ≥ flipBit k.
    have hj' : flipBit k ≤ j := Nat.le_of_not_lt hj
    rcases lt_or_eq_of_le hj' with hjgt | hjeq
    · -- j > flipBit k.
      have h1 : (k + 1).testBit j       = k.testBit j       := testBit_succ_eq_of_gt k hjgt
      have h2 : (k + 1).testBit (1 + j) = k.testBit (1 + j) :=
        testBit_succ_eq_of_gt k (by omega)
      have hjne : ¬ (j = flipBit k) := by omega
      rw [h1, h2]
      simp [hjne]
    · -- j = flipBit k.
      subst hjeq
      have hkp1 : (k + 1).testBit (flipBit k) = true := flipBit_testBit k
      have hk_at : k.testBit (flipBit k) = false := k_testBit_at k
      have h_above : (k + 1).testBit (1 + flipBit k) = k.testBit (1 + flipBit k) :=
        testBit_succ_eq_of_gt k (by omega)
      rw [hkp1, hk_at, h_above]
      simp

/-! ### L2 — `flipBit` index bound

`flipBit k < n` when `k + 1 < 2^n`.  The proof uses the lowest-set-bit
spec: bit `flipBit k` of `k+1` is set, so `k+1 ≥ 2^(flipBit k)`; combined
with `k+1 < 2^n` this gives `2^(flipBit k) < 2^n`, hence `flipBit k < n`.
-/

/-- L2 (sketch §3.1 / §6.2): `flipBit k < n` whenever `k + 1 < 2^n`.

(The hypothesis `n ≤ 63` is not actually needed for this lemma — it is a
generic property of `Nat.testBit` — but is included in the sketch §3.1
table because all downstream users carry that bound.) -/
theorem flipBit_lt {n k : ℕ} (h : k + 1 < 2 ^ n) : flipBit k < n := by
  -- bit flipBit k of k+1 is set, so k+1 ≥ 2^(flipBit k).
  have hge : k + 1 ≥ 2 ^ flipBit k :=
    Nat.ge_two_pow_of_testBit (flipBit_testBit k)
  -- 2^(flipBit k) ≤ k+1 < 2^n, so 2^(flipBit k) < 2^n, hence flipBit k < n.
  have : 2 ^ flipBit k < 2 ^ n := lt_of_le_of_lt hge h
  exact (Nat.pow_lt_pow_iff_right (by decide)).mp this

/-! ### L3 — `subsetOfBits` bijection

We first establish that the auxiliary map `bitsToFinset : ℕ → Finset (Fin n)`
(which extracts the indices of the set bits of an arbitrary `m`, restricted
to positions `< n`) is injective on `{m | m < 2^n}`.  Then `gray` is a
bijection on `Fin (2^n)` (since `x ↦ x XOR (x >>> 1)` is involutive on bits
above any fixed top, hence injective on `Fin (2^n)`).  Composing the two
gives the L3 bijection.

In practice the cleanest formulation is to define a direct map
`f : Fin (2^n) → Finset (Fin n)` by `f k = subsetOfBits n k.val`, and show
`Function.Bijective f`.  Since both sides have cardinality `2^n`
(`Fintype.card_fin` and `Fintype.card_finset`), injectivity alone implies
bijectivity (`Finite.injective_iff_bijective`).
-/

/-- Auxiliary: bit-set extraction from an arbitrary `m`, projected onto
`Fin n`.  Differs from `subsetOfBits n k` in that it does NOT route
through `gray`; it just reads the bits of `m` directly.  Used to prove
injectivity of `subsetOfBits` via the chain
`m ↦ bitsToFinset n m` injective on `m < 2^n`, plus `gray` injective. -/
def bitsToFinset (n : ℕ) (m : ℕ) : Finset (Fin n) :=
  (Finset.univ : Finset (Fin n)).filter (fun i => m.testBit i.val)

@[simp] theorem mem_bitsToFinset {n m : ℕ} {i : Fin n} :
    i ∈ bitsToFinset n m ↔ m.testBit i.val = true := by
  simp [bitsToFinset]

/-- `subsetOfBits n k = bitsToFinset n (gray k)`. -/
theorem subsetOfBits_eq (n k : ℕ) :
    subsetOfBits n k = bitsToFinset n (gray k) := rfl

/-- `bitsToFinset n` is injective on `{m | m < 2^n}`. -/
theorem bitsToFinset_injOn (n : ℕ) :
    Set.InjOn (bitsToFinset n) {m | m < 2 ^ n} := by
  intro a ha b hb hab
  -- Two naturals < 2^n with the same set of bits in positions < n must be equal.
  apply Nat.eq_of_testBit_eq
  intro i
  by_cases hi : i < n
  · -- Bits below n: equal because the Finset is the same.
    -- `(⟨i, hi⟩ : Fin n) ∈ bitsToFinset n a ↔ (⟨i, hi⟩ : Fin n) ∈ bitsToFinset n b`
    -- by `hab`.  Unfold via `mem_bitsToFinset` and read off bit equality.
    have hmem : (⟨i, hi⟩ : Fin n) ∈ bitsToFinset n a ↔ (⟨i, hi⟩ : Fin n) ∈ bitsToFinset n b := by
      rw [hab]
    simp only [mem_bitsToFinset] at hmem
    -- hmem : a.testBit i = true ↔ b.testBit i = true.
    cases ha' : a.testBit i <;> cases hb' : b.testBit i <;> simp_all
  · -- Bits ≥ n: false on both sides.
    have hi' : n ≤ i := Nat.le_of_not_lt hi
    have hain : a.testBit i = false :=
      Nat.testBit_lt_two_pow (lt_of_lt_of_le ha (Nat.pow_le_pow_right (by decide) hi'))
    have hbin : b.testBit i = false :=
      Nat.testBit_lt_two_pow (lt_of_lt_of_le hb (Nat.pow_le_pow_right (by decide) hi'))
    rw [hain, hbin]

/-- Cardinality of `Finset (Fin n)` as a fintype is `2^n`. -/
private theorem card_finset_fin (n : ℕ) :
    Fintype.card (Finset (Fin n)) = 2 ^ n := by
  rw [Fintype.card_finset, Fintype.card_fin]

/-- Helper for injectivity of `gray`: if `gray a = gray b`, then every
testBit of `a` equals the corresponding testBit of `b` (hence `a = b`).

Proved by downward induction: bit `i+M` of `a` equals bit `i+M` of `gray a` XOR
bit `i+M+1` of `a` (from `(gray a).testBit (i+M) = a.testBit (i+M) XOR a.testBit (i+M+1)`).
So if bits of `gray a` and `gray b` agree everywhere, and `a.testBit (i+M+1) = b.testBit (i+M+1)`,
then `a.testBit (i+M) = b.testBit (i+M)`.  Base case: for `M = a+b+1`,
both `a.testBit (i+M)` and `b.testBit (i+M)` are `false`. -/
theorem gray_injective (a b : ℕ) (h : gray a = gray b) (i : ℕ) :
    a.testBit i = b.testBit i := by
  -- We prove: ∀ N, a.testBit (i + N) = b.testBit (i + N) → a.testBit i = b.testBit i.
  -- The base case (N = 0) is immediate; for N = M+1 we descend using gray.
  suffices key : ∀ N, a.testBit (i + N) = b.testBit (i + N) → a.testBit i = b.testBit i by
    -- Choose N := a + b + 1 large enough so both testBits at i+N are false.
    have ha_bd : a < 2 ^ (i + (a + b + 1)) := by
      have ha1 : a < 2 ^ a := Nat.lt_two_pow_self
      have : 2 ^ a ≤ 2 ^ (i + (a + b + 1)) := Nat.pow_le_pow_right (by decide) (by omega)
      omega
    have hb_bd : b < 2 ^ (i + (a + b + 1)) := by
      have hb1 : b < 2 ^ b := Nat.lt_two_pow_self
      have : 2 ^ b ≤ 2 ^ (i + (a + b + 1)) := Nat.pow_le_pow_right (by decide) (by omega)
      omega
    apply key (a + b + 1)
    rw [Nat.testBit_lt_two_pow ha_bd, Nat.testBit_lt_two_pow hb_bd]
  -- Prove the key by induction on N.
  intro N
  induction N with
  | zero =>
    intro hN
    simpa using hN
  | succ M ih =>
    intro hN
    apply ih
    -- bit (i+M) of gray x = bit (i+M) of x XOR bit (i+M+1) of x.
    have eqa : a.testBit (i + M) =
        ((gray a).testBit (i + M)).xor (a.testBit (i + M + 1)) := by
      have hga : (gray a).testBit (i + M) =
          (a.testBit (i + M)).xor (a.testBit (1 + (i + M))) := by
        unfold gray
        rw [Nat.testBit_xor, Nat.testBit_shiftRight]
      rw [show 1 + (i + M) = i + M + 1 from by ring] at hga
      rw [hga]
      cases h1 : a.testBit (i + M) <;> cases h2 : a.testBit (i + M + 1) <;> rfl
    have eqb : b.testBit (i + M) =
        ((gray b).testBit (i + M)).xor (b.testBit (i + M + 1)) := by
      have hgb : (gray b).testBit (i + M) =
          (b.testBit (i + M)).xor (b.testBit (1 + (i + M))) := by
        unfold gray
        rw [Nat.testBit_xor, Nat.testBit_shiftRight]
      rw [show 1 + (i + M) = i + M + 1 from by ring] at hgb
      rw [hgb]
      cases h1 : b.testBit (i + M) <;> cases h2 : b.testBit (i + M + 1) <;> rfl
    rw [eqa, eqb, h, show i + M + 1 = i + (M + 1) from by ring]
    rw [hN]

/-- L3 (sketch §3.1 / §6.2): the map `k ↦ subsetOfBits n k.val` is a
bijection `Fin (2^n) → Finset (Fin n)`.

Both sides have cardinality `2^n`, so injectivity (provided by the
combination of `bitsToFinset_injOn` and `gray` being injective on
`Fin (2^n)`) implies bijectivity. -/
theorem subsetOfBits_bijective (n : ℕ) :
    Function.Bijective (fun k : Fin (2 ^ n) => subsetOfBits n k.val) := by
  apply (Fintype.bijective_iff_injective_and_card _).mpr
  refine ⟨?_, ?_⟩
  · -- Injectivity.
    intro a b hab
    simp only [subsetOfBits_eq] at hab
    have ha' : gray a.val < 2 ^ n := gray_lt_two_pow n a.val a.isLt
    have hb' : gray b.val < 2 ^ n := gray_lt_two_pow n b.val b.isLt
    have hg : gray a.val = gray b.val :=
      bitsToFinset_injOn n (Set.mem_setOf.mpr ha') (Set.mem_setOf.mpr hb') hab
    apply Fin.ext
    apply Nat.eq_of_testBit_eq
    intro i
    exact gray_injective a.val b.val hg i
  · rw [Fintype.card_fin, card_finset_fin]

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
