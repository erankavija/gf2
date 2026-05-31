/-
  Gf2Algebra.Proofs.Packed7Correctness — D6 packed F_7 correctness (Path B)

  Implements the D6 proof sketch (`dev/plans/d6_lean_packed7_sketch.md`)
  for JIT issue 30e98ef1, on the user-chosen **Path B** (axiomatise the
  three LUTs with a source-faithful characterisation cross-validated by
  the exhaustive Rust tests; prove `binary_op_word` + the four
  `*_correct` theorems against the production code path).

  Proof target (sketch §4): the four inherent wrappers
  `Packed7.{add,sub,mul,neg}_inherent` defined in
  `crates/gf2-algebra/src/packed/packed7.rs`. Each wrapper delegates a
  single tail call to the corresponding `PackedField<Fp<7>>` trait method
  on `Packed7`; the trait body loads the relevant 64 KiB LUT and runs the
  8-iteration `binary_op_word` loop (packed7.rs:167-178). Targeting the
  inherent wrappers gives a stable, non-dispatch-indirected name (same
  convention as the D2 bipedal / D5 packed5 proofs).

  `Packed7` packs 16 independent F_7 lanes into one `u64`; lane `i`
  (`0 ≤ i < 16`) occupies bits `[4i, 4i+4)`. Canonical values are
  `0..=6`; codepoints `7..=15` decode to `0` (matching `Fp::<7>::new`,
  packed7.rs:281, and the LUT non-canonical guard, packed7.rs:64).

  ## Path B axiom honesty (D6 §4.3, §6 R4)

  Under Path B the three 64 KiB LUT *contents* are extracted by Charon as
  opaque external constants (`--opaque
  'gf2_algebra::packed::packed7::{ADD,SUB,MUL}_LUT'`, see
  `scripts/verify-lean.sh`) and rendered by Aeneas as opaque axioms
  `packed.packed7.{ADD,SUB,MUL}_LUT : Result (Array Std.U8 65536#usize)`
  in `Gf2Algebra/FunsExternal.lean`. This file states **exactly three
  `axiom` declarations** (`add_lut_spec`, `sub_lut_spec`,
  `mul_lut_spec`) characterising those opaque tables. Each axiom is the
  Lean transcription of the corresponding `build_*_lut` `const fn`
  source; it is *not* proved from the extracted loop (Path A is out of
  scope per the user decision of 2026-05-16 and D6 §7). The axioms are
  cross-validated out-of-band by the exhaustive Rust tests
  `test_{add,sub,mul}_lut_contract_exhaustive`
  (`crates/gf2-algebra/src/packed/packed7.rs`), which assert each of the
  65536 LUT entries against the identical contract these axioms state —
  so axiom ⟺ tested-Rust-contract is mechanically checkable. This is the
  user-accepted Path-B cost (D6 §8): correctness is verified end-to-end
  against the production `binary_op_word` code path *modulo* the
  Rust-tested table-contents axioms.

  The `binary_op_word` 8-iteration loop and the four `*_correct`
  theorems (L9–L13) are **real proofs** against the Aeneas-extracted
  production defs — no `sorry`.
-/
import Aeneas
import Mathlib.Data.ZMod.Basic
import Mathlib.Data.Nat.Bitwise
import Mathlib.Tactic.IntervalCases
import Gf2Algebra.Funs

open Aeneas Aeneas.Std Result ControlFlow Error Aeneas.Std.WP
open gf2_algebra

set_option maxHeartbeats 1600000

namespace Packed7Correctness

/-! ## §1 Decoder `dec7` + nibble extraction

The 4-bit-slot lane decoder. We model the encoding arithmetically:
nibble `i` of a word value `w : Nat` is `(w / 16^i) % 16`; byte `bi`
is `(w / 256^bi) % 256`. This is exactly the Rust `(w >> 4i) & 0xf`
(packed7.rs:280) — see `nib_shift` for the bridge to the extracted
`>>>`/`&&&`. -/

/-- 4-bit-slot lane decoder for `Packed7`. Codepoints `0..=6` decode to
themselves; codepoints `7..=15` decode to `0` (matching
`Fp::<7>::new`, packed7.rs:281). -/
def dec7 (v : Nat) : ZMod 7 := if v < 7 then (v : ZMod 7) else 0

/-- Lane `i`'s 4-bit nibble of a word value (`(w / 16^i) % 16`). -/
def nib (w i : Nat) : Nat := (w / 16 ^ i) % 16

/-- Byte `bi`'s value of a word (`(w / 256^bi) % 256`). -/
def byteN (w bi : Nat) : Nat := (w / 256 ^ bi) % 256

/-! ### §3.1 Decoder + arithmetic helpers -/

/-- §3.1 L1: `dec7` on a canonical codepoint is the identity. -/
theorem dec7_canon {v : Nat} (h : v < 7) : dec7 v = (v : ZMod 7) := by
  simp [dec7, h]

/-- §3.1 L2: `dec7` on a non-canonical codepoint (`7 ≤ v`) is `0`. -/
theorem dec7_noncanon {v : Nat} (h : 7 ≤ v) : dec7 v = 0 := by
  simp [dec7, Nat.not_lt.mpr h]

/-- §3.1 L3 totality: `dec7 v ∈ {0,…,6} ⊂ ZMod 7` for `v < 16`. -/
theorem dec7_total (v : Nat) (h : v < 16) :
    dec7 v = 0 ∨ dec7 v = 1 ∨ dec7 v = 2 ∨ dec7 v = 3
      ∨ dec7 v = 4 ∨ dec7 v = 5 ∨ dec7 v = 6 := by
  interval_cases v <;> decide

/-- §3.1 L4: a nibble is always `< 16`. -/
theorem nib_lt_16 (w i : Nat) : nib w i < 16 := by
  simp only [nib]; omega

/-- A byte value is always `< 256`. -/
theorem byteN_lt_256 (w bi : Nat) : byteN w bi < 256 := by
  simp only [byteN]; omega

/-- §3.1 L5 add-contract: closed `decide` over the `7×7` canonical grid. -/
theorem dec7_add_contract (a b : Nat) (ha : a < 7) (hb : b < 7) :
    dec7 ((a + b) % 7) = dec7 a + dec7 b := by
  interval_cases a <;> interval_cases b <;> decide

/-- §3.1 L5' sub-contract analogue. -/
theorem dec7_sub_contract (a b : Nat) (ha : a < 7) (hb : b < 7) :
    dec7 ((a + 7 - b) % 7) = dec7 a - dec7 b := by
  interval_cases a <;> interval_cases b <;> decide

/-- §3.1 L5'' mul-contract analogue. -/
theorem dec7_mul_contract (a b : Nat) (ha : a < 7) (hb : b < 7) :
    dec7 ((a * b) % 7) = dec7 a * dec7 b := by
  interval_cases a <;> interval_cases b <;> decide

/-- §3.1 L5''' neg-contract analogue (`0 - b` lane of `SUB_LUT`). -/
theorem dec7_neg_contract (b : Nat) (hb : b < 7) :
    dec7 ((0 + 7 - b) % 7) = -(dec7 b) := by
  interval_cases b <;> decide

/-- Bridge: the extracted Rust `(w >>> (4*i)) &&& 0xf` (which Aeneas
emits over `Nat`) equals `nib w i`. -/
theorem nib_shift (w i : Nat) : (w >>> (4 * i)) &&& 0xf = nib w i := by
  have h0xf : (0xf : Nat) = 2 ^ 4 - 1 := by norm_num
  have hpow : (2 : Nat) ^ (4 * i) = 16 ^ i := by rw [pow_mul]; norm_num
  have h16 : (2 : Nat) ^ 4 = 16 := by norm_num
  rw [Nat.shiftRight_eq_div_pow, h0xf, Nat.and_two_pow_sub_one_eq_mod, hpow,
    h16, nib]

/-- Bridge: the extracted Rust `(w >>> (8*bi)) &&& 0xff` equals
`byteN w bi`. -/
theorem byteN_shift (w bi : Nat) :
    (w >>> (8 * bi)) &&& 0xff = byteN w bi := by
  have h0xff : (0xff : Nat) = 2 ^ 8 - 1 := by norm_num
  have hpow : (2 : Nat) ^ (8 * bi) = 256 ^ bi := by rw [pow_mul]; norm_num
  have h256 : (2 : Nat) ^ 8 = 256 := by norm_num
  rw [Nat.shiftRight_eq_div_pow, h0xff, Nat.and_two_pow_sub_one_eq_mod, hpow,
    h256, byteN]

/-! ## §3.2 LUT characterisation axioms (Path B — the three table axioms)

Per D6 §4.3 the three 64 KiB LUTs are extracted as opaque external
constants of type `Result (Array Std.U8 65536#usize)`. The three axioms
below characterise those opaque values. Each is the Lean transcription
of the corresponding `build_*_lut` `const fn` and is cross-validated by
the exhaustive Rust `test_*_lut_contract_exhaustive` tests. They are
**not** proved from the extracted loop (Path A out of scope, D6 §4.2 /
§7). This is the user-accepted Path-B limitation (D6 §8).

Key/byte layout transcribed verbatim from the `const fn` source
(`build_add_lut`, packed7.rs:54-75; the loop writes
`lut[(bp<<8)|ap] = r0 | (r1<<4)` with `ap = a0|(a1<<4)`,
`bp = b0|(b1<<4)`, only when `a0,a1,b0,b1 < 7`, else the array
zero-init at packed7.rs:55 leaves `0`). The 16-bit key, as a `Nat`, is
`a0 + 16·a1 + 256·b0 + 4096·b1`. -/

/-- **ADD_LUT contract axiom (Path B).**

JUSTIFICATION / cited source: `build_add_lut`
(`crates/gf2-algebra/src/packed/packed7.rs:54-75`) zero-inits the array
(`packed7.rs:55`) then, for every `ap,bp < 256`, writes
`lut[(bp<<8)|ap] = r0 | (r1<<4)` with `r0 = (a0+b0)%7`,
`r1 = (a1+b1)%7`, `a0 = ap&0xf`, `a1 = ap>>4`, `b0 = bp&0xf`,
`b1 = bp>>4`, **only** under the guard `a0<7 && a1<7 && b0<7 && b1<7`
(`packed7.rs:64`); non-canonical keys keep the zero-init value
(`packed7.rs:29-30,55`). The static `ADD_LUT = build_add_lut()`
(`packed7.rs:137`).

NOT proved from the extracted loop (Path B, D6 §4.3; Path A out of
scope per the user decision 2026-05-16 / D6 §7). Cross-validated by the
exhaustive Rust test `test_add_lut_contract_exhaustive` which asserts
all 65536 `ADD_LUT` entries against this identical contract — axiom ⟺
tested-Rust-contract is mechanically checkable. -/
axiom add_lut_spec :
    ∃ L : Array Std.U8 65536#usize, packed.packed7.ADD_LUT = ok L ∧
      ∀ a0 a1 b0 b1 : Nat, a0 < 16 → a1 < 16 → b0 < 16 → b1 < 16 →
        (L.val[a0 + 16 * a1 + 256 * b0 + 4096 * b1]!).val =
          if a0 < 7 ∧ a1 < 7 ∧ b0 < 7 ∧ b1 < 7 then
            ((a0 + b0) % 7) + 16 * ((a1 + b1) % 7)
          else 0

/-- **SUB_LUT contract axiom (Path B).**

JUSTIFICATION / cited source: `build_sub_lut`
(`crates/gf2-algebra/src/packed/packed7.rs:82-103`), identical structure
to `build_add_lut` with `r0 = (a0+7-b0)%7`, `r1 = (a1+7-b1)%7`
(`packed7.rs:93-94`), guarded by `a0<7 && a1<7 && b0<7 && b1<7`
(`packed7.rs:92`); non-canonical keys keep the `packed7.rs:83`
zero-init. The static `SUB_LUT = build_sub_lut()` (`packed7.rs:143`).

NOT proved from the extracted loop (Path B, D6 §4.3). Cross-validated by
the exhaustive Rust test `test_sub_lut_contract_exhaustive`. -/
axiom sub_lut_spec :
    ∃ L : Array Std.U8 65536#usize, packed.packed7.SUB_LUT = ok L ∧
      ∀ a0 a1 b0 b1 : Nat, a0 < 16 → a1 < 16 → b0 < 16 → b1 < 16 →
        (L.val[a0 + 16 * a1 + 256 * b0 + 4096 * b1]!).val =
          if a0 < 7 ∧ a1 < 7 ∧ b0 < 7 ∧ b1 < 7 then
            ((a0 + 7 - b0) % 7) + 16 * ((a1 + 7 - b1) % 7)
          else 0

/-- **MUL_LUT contract axiom (Path B).**

JUSTIFICATION / cited source: `build_mul_lut`
(`crates/gf2-algebra/src/packed/packed7.rs:110-131`), identical
structure to `build_add_lut` with `r0 = (a0*b0)%7`, `r1 = (a1*b1)%7`
(`packed7.rs:121-122`), guarded by `a0<7 && a1<7 && b0<7 && b1<7`
(`packed7.rs:120`); non-canonical keys keep the `packed7.rs:111`
zero-init. The static `MUL_LUT = build_mul_lut()` (`packed7.rs:149`).

NOT proved from the extracted loop (Path B, D6 §4.3). Cross-validated by
the exhaustive Rust test `test_mul_lut_contract_exhaustive`. -/
axiom mul_lut_spec :
    ∃ L : Array Std.U8 65536#usize, packed.packed7.MUL_LUT = ok L ∧
      ∀ a0 a1 b0 b1 : Nat, a0 < 16 → a1 < 16 → b0 < 16 → b1 < 16 →
        (L.val[a0 + 16 * a1 + 256 * b0 + 4096 * b1]!).val =
          if a0 < 7 ∧ a1 < 7 ∧ b0 < 7 ∧ b1 < 7 then
            ((a0 * b0) % 7) + 16 * ((a1 * b1) % 7)
          else 0

/-! ## §3.3 `binary_op_word` loop-composition lemma (L9 — real proof)

`binary_op_word a b lut` runs the extracted 8-iteration loop
`packed.packed7.binary_op_word_loop` (packed7.rs:167-178). Iteration
`bi` reads byte `bi` of `a`/`b`, indexes `lut` at the 16-bit key, and
ORs the LUT result byte into output bit position `8*bi`. We
characterise the loop result byte-by-byte via the in-repo
`apply spec_imp_exists; apply loop.spec` idiom
(`MontgomeryRoundtrip.lean:215-217` precedent referenced by D6 §4.1). -/

/-- The LUT key for byte index `bi` as the `Nat`
`a0 + 16·a1 + 256·b0 + 4096·b1` — exactly the Rust
`ap | (bp << 8)` with `ap`/`bp` the `a`/`b` byte (packed7.rs:173). -/
def keyByte (a b bi : Nat) : Nat :=
  nib a (2 * bi) + 16 * nib a (2 * bi + 1)
    + 256 * nib b (2 * bi) + 4096 * nib b (2 * bi + 1)

/-- A `keyByte` is a valid 16-bit index. -/
theorem keyByte_lt (a b bi : Nat) : keyByte a b bi < 65536 := by
  have h1 := nib_lt_16 a (2 * bi)
  have h2 := nib_lt_16 a (2 * bi + 1)
  have h3 := nib_lt_16 b (2 * bi)
  have h4 := nib_lt_16 b (2 * bi + 1)
  simp only [keyByte]; omega

/-- The extracted Rust key `ap | (bp << 8)` (over `Nat`, with
`ap = (a>>>8bi)&0xff`, `bp = (b>>>8bi)&0xff`) equals `keyByte a b bi`.
The byte splits into its two nibbles; the 16-bit key recomposes the
four nibbles. -/
theorem keyByte_eq (a b bi : Nat) :
    ((a >>> (8 * bi)) &&& 0xff) ||| (((b >>> (8 * bi)) &&& 0xff) <<< 8)
      = keyByte a b bi := by
  rw [byteN_shift, byteN_shift]
  -- `bA ||| (bB <<< 8) = bB <<< 8 + bA = bB*256 + bA`, since `bA < 256 = 2^8`.
  have hbA : byteN a bi < 2 ^ 8 := by
    have := byteN_lt_256 a bi; simpa using this
  rw [Nat.lor_comm, ← Nat.shiftLeft_add_eq_or_of_lt hbA, Nat.shiftLeft_eq]
  -- Goal: byteN b bi * 2^8 + byteN a bi = keyByte a b bi.
  -- Connect each byte's two-nibble split to the four key nibbles.
  have split : ∀ w : Nat, byteN w bi = nib w (2*bi) + 16 * nib w (2*bi+1) := by
    intro w
    simp only [byteN, nib]
    have e1 : (16 : Nat) ^ (2 * bi) = 256 ^ bi := by
      rw [pow_mul, show (16 : Nat) ^ 2 = 256 from by norm_num]
    have e2 : (16 : Nat) ^ (2 * bi + 1) = 256 ^ bi * 16 := by
      rw [pow_succ, pow_mul, show (16 : Nat) ^ 2 = 256 from by norm_num]
    rw [e1, e2, ← Nat.div_div_eq_div_mul,
      show (256 : Nat) = 16 * 16 by norm_num, Nat.mod_mul]
  simp only [keyByte, split a, split b]; ring


/-- Each LUT entry is a `u8`, hence `< 256`. -/
theorem lut_byte_lt (a b : Nat) (L : Array Std.U8 65536#usize) (k : Nat) :
    (L.val[keyByte a b k]!).val < 256 := by
  have := (L.val[keyByte a b k]!).hBounds
  simpa [Std.U8, UScalar.size] using this

/-- Closed-form little-endian byte assembly of the first `k` LUT bytes:
`Σ_{j<k} (L[keyByte a b j]) · 256^j`. The loop accumulator equals
`lutSum a b L k` after `k` iterations. -/
def lutSum (a b : Nat) (L : Array Std.U8 65536#usize) : Nat → Nat
  | 0 => 0
  | k + 1 => lutSum a b L k + (L.val[keyByte a b k]!).val * 256 ^ k

/-- `lutSum … k` packs into `k` bytes, so it is `< 256^k`. -/
theorem lutSum_lt (a b : Nat) (L : Array Std.U8 65536#usize) (k : Nat) :
    lutSum a b L k < 256 ^ k := by
  induction k with
  | zero => simp [lutSum]
  | succ n ih =>
    have hb : (L.val[keyByte a b n]!).val ≤ 255 := by
      have := lut_byte_lt a b L n; omega
    have hp : (0 : Nat) < 256 ^ n := by positivity
    calc lutSum a b L (n + 1)
        = lutSum a b L n + (L.val[keyByte a b n]!).val * 256 ^ n := rfl
      _ < 256 ^ n + (L.val[keyByte a b n]!).val * 256 ^ n := by omega
      _ ≤ 256 ^ n + 255 * 256 ^ n :=
          Nat.add_le_add_left (Nat.mul_le_mul_right _ hb) _
      _ = 256 ^ (n + 1) := by ring

/-- Byte `bi` of `lutSum … k` is the `bi`-th LUT byte when `bi < k`, and
`0` when `bi ≥ k`. -/
theorem byteN_lutSum (a b : Nat) (L : Array Std.U8 65536#usize)
    (k bi : Nat) :
    byteN (lutSum a b L k) bi =
      if bi < k then (L.val[keyByte a b bi]!).val else 0 := by
  induction k with
  | zero => simp [lutSum, byteN]
  | succ k ih =>
    have hbnd := lut_byte_lt a b L k
    rcases lt_trichotomy bi k with hlt | heq | hgt
    · -- bi < k: the new high byte (multiple of 256^k, k>bi) does not
      -- touch byte bi.
      have hdvd : 256 ^ (bi + 1) ∣ (L.val[keyByte a b k]!).val * 256 ^ k := by
        have : 256 ^ (bi + 1) ∣ 256 ^ k := pow_dvd_pow 256 (by omega)
        exact Dvd.dvd.mul_left this _
      have key : byteN (lutSum a b L (k + 1)) bi = byteN (lutSum a b L k) bi := by
        simp only [byteN, lutSum]
        obtain ⟨c, hc⟩ := hdvd
        rw [hc]
        -- (s + 256^(bi+1)·c) / 256^bi % 256 = s / 256^bi % 256
        have hsplit : (256 : Nat) ^ (bi + 1) = 256 ^ bi * 256 := by
          rw [pow_succ]
        rw [hsplit, mul_assoc, Nat.add_mul_div_left _ _ (by positivity : 0 < 256 ^ bi),
          Nat.add_mul_mod_self_left]
      rw [key, ih]
      simp [hlt, Nat.lt_succ_of_lt hlt]
    · -- bi = k: the new byte.
      subst heq
      have hlow := lutSum_lt a b L bi
      simp only [lutSum, byteN]
      rw [Nat.mul_comm (L.val[keyByte a b bi]!).val (256 ^ bi),
        Nat.add_mul_div_left _ _ (by positivity : 0 < 256 ^ bi),
        Nat.div_eq_of_lt hlow, Nat.zero_add, Nat.mod_eq_of_lt hbnd]
      simp
    · -- bi > k: lutSum (k+1) < 256^(k+1) ≤ 256^bi, so byte bi = 0.
      have hsum_lt := lutSum_lt a b L (k + 1)
      have hle : 256 ^ (k + 1) ≤ 256 ^ bi :=
        Nat.pow_le_pow_right (by norm_num) (by omega)
      have : byteN (lutSum a b L (k + 1)) bi = 0 := by
        simp only [byteN]
        rw [Nat.div_eq_of_lt (by omega)]
      rw [this]; simp; omega

/-- §3.3 L9-core. The Aeneas-extracted `binary_op_word a b L`
(`@[reducible]` over `binary_op_word_loop a b L 0 0`) terminates with a
word whose byte `bi` (`bi < 8`) equals `(L.val[keyByte a b bi]!).val`.
Proved by `apply spec_imp_exists; apply loop.spec`
(`MontgomeryRoundtrip.lean:215-217` precedent) with measure `8 - i` and
the closed-form `lutSum` invariant; the body is one `Array.index_usize`
(closed by `keyByte_lt`) plus concrete `i32`/`u64` shift/mask. -/
theorem binary_op_word_spec
    (a b : Std.U64) (L : Array Std.U8 65536#usize) :
    ∃ r : Std.U64, packed.packed7.binary_op_word a b L = ok r ∧
      ∀ bi : Nat, bi < 8 →
        byteN r.val bi = (L.val[keyByte a.val b.val bi]!).val := by
  unfold packed.packed7.binary_op_word packed.packed7.binary_op_word_loop
  apply spec_imp_exists
  apply loop.spec
    (measure := fun ((_, i) : Std.U64 × Std.I32) => (8 - i.val).toNat)
    (inv := fun ((r, i) : Std.U64 × Std.I32) =>
      0 ≤ i.val ∧ i.val ≤ 8 ∧
        r.val = lutSum a.val b.val L i.val.toNat)
  · rintro ⟨r, i⟩ ⟨hi0, hi8, hrEq⟩
    dsimp only
    simp only [packed.packed7.binary_op_word_loop.body]
    by_cases hlt : i < 8#i32
    · -- Collapse the pure `lift` binds (the `&&&`/`|||`/`cast` ops are not
      -- `Result`-fallible; only `*`, `>>>`, `<<<`, `index_usize`, `+` are
      -- genuine `Result` ops needing `step`). Mirrors the
      -- `simp only [..., Std.lift, bind_tc_ok]` precedent in
      -- `MontgomeryRoundtrip.lean:1127`.
      simp only [hlt, ite_true, Std.lift, bind_tc_ok]
      have hival : i.val < 8 := by scalar_tac
      have hivalN : i.val.toNat < 8 := by omega
      -- i1 = 8 * i, with 0 ≤ i1 < 64.
      step as ⟨i1, hi1⟩
      have hi1valN : i1.val = 8 * i.val.toNat := by scalar_tac
      have hi1lt : i1.val < 64 := by omega
      have hi1ge : (0 : Int) ≤ i1.val := by omega
      step as ⟨i2, hi2⟩            -- a >>> i1
      step as ⟨i4, hi4⟩            -- b >>> i1
      step as ⟨i6, hi6⟩ by
        first
          | scalar_tac
          | (cases System.Platform.numBits_eq <;> simp [*])
      -- The `index_usize` key is `ap ||| i6` with `ap` (low byte from a)
      -- and `bp <<< 8` (= i6) inlined.
      set keyv : Std.Usize :=
        (UScalar.cast .Usize (i2 &&& 255#u64)) ||| i6 with hkeyv_def
      have hi1valNat : i1.val.toNat = 8 * i.val.toNat := by omega
      -- `Usize` is at least 16-bit on every supported platform (32 or 64).
      have hUbits : 2 ^ (16 : Nat) ≤ 2 ^ (UScalarTy.Usize.numBits) := by
        apply Nat.pow_le_pow_right (by norm_num)
        rw [UScalarTy.Usize_numBits_eq]
        cases System.Platform.numBits_eq with
        | inl h => omega
        | inr h => omega
      have hsize : (65536 : Nat) ≤ Usize.size := by
        rw [Usize.size]
        calc (65536 : Nat) = 2 ^ 16 := by norm_num
          _ ≤ 2 ^ (UScalarTy.Usize.numBits) := hUbits
          _ = 2 ^ (Usize.numBits) := by rw [Usize.numBits]
      have hkeyval : keyv.val = keyByte a.val b.val i.val.toNat := by
        have hi2and : (i2 &&& 255#u64).val = a.val >>> (8 * i.val.toNat) &&& 255 := by
          rw [UScalar.val_and, hi2, hi1valNat]; rfl
        have hi4and : (i4 &&& 255#u64).val = b.val >>> (8 * i.val.toNat) &&& 255 := by
          rw [UScalar.val_and, hi4, hi1valNat]; rfl
        have hap_le : a.val >>> (8 * i.val.toNat) &&& 255 ≤ 255 := Nat.and_le_right
        have hbp_le : b.val >>> (8 * i.val.toNat) &&& 255 ≤ 255 := Nat.and_le_right
        have hapv : (UScalar.cast .Usize (i2 &&& 255#u64)).val
            = a.val >>> (8 * i.val.toNat) &&& 255 := by
          rw [UScalar.cast_val_eq, hi2and]
          exact Nat.mod_eq_of_lt (by
            calc a.val >>> (8 * i.val.toNat) &&& 255
                ≤ 255 := hap_le
              _ < 2 ^ 16 := by norm_num
              _ ≤ 2 ^ (UScalarTy.Usize.numBits) := hUbits)
        have hi6v : i6.val = (b.val >>> (8 * i.val.toNat) &&& 255) <<< 8 := by
          rw [hi6, UScalar.cast_val_eq, hi4and]
          have hinner : (b.val >>> (8 * i.val.toNat) &&& 255)
              % 2 ^ (UScalarTy.Usize.numBits)
              = b.val >>> (8 * i.val.toNat) &&& 255 := by
            apply Nat.mod_eq_of_lt
            calc b.val >>> (8 * i.val.toNat) &&& 255
                ≤ 255 := hbp_le
              _ < 2 ^ 16 := by norm_num
              _ ≤ 2 ^ (UScalarTy.Usize.numBits) := hUbits
          rw [hinner, Nat.shiftLeft_eq]
          apply Nat.mod_eq_of_lt
          calc (b.val >>> (8 * i.val.toNat) &&& 255) * 2 ^ 8
              ≤ 255 * 2 ^ 8 := Nat.mul_le_mul_right _ hbp_le
            _ < 65536 := by norm_num
            _ ≤ Usize.size := hsize
        have hk : keyv.val
            = ((a.val >>> (8 * i.val.toNat)) &&& 0xff)
              ||| (((b.val >>> (8 * i.val.toNat)) &&& 0xff) <<< 8) := by
          rw [hkeyv_def, UScalar.val_or, hapv, hi6v]
        rw [hk, keyByte_eq]
      have hkeylt : keyv.val < 65536 := by
        rw [hkeyval]; exact keyByte_lt _ _ _
      step as ⟨i7, hi7⟩            -- Array.index_usize lut keyv
      step as ⟨i9, hi9⟩            -- (cast U64 i7) <<< i1
      step as ⟨i10, hi10⟩          -- i + 1#i32
      -- (r ||| i9).val = lutSum … (i+1) = r.val + byte · 256^i
      have hi10val : i10.val.toNat = i.val.toNat + 1 := by
        rw [hi10]; omega
      -- New Aeneas Std (0f99a049): `Array.index_usize` / `step` yields the safe,
      -- proof-carrying `getElem` form `i7 = (↑L)[↑keyv]` rather than the panic form
      -- `(↑L)[↑keyv]!`. Bridge the two with `getElem!_pos` (valid since `keyv` is in
      -- bounds: `keyv.val < 65536 = L.length`).
      have hi7val : i7.val = (L.val[keyv.val]!).val := by
        rw [hi7, getElem!_pos (L.val) keyv.val]
      have hi7le : i7.val ≤ 255 := by
        have := U8.lt_succ_max i7; omega
      have hcastU64 : (UScalar.cast .U64 i7).val = i7.val := by
        rw [UScalar.cast_val_eq]
        exact Nat.mod_eq_of_lt (by
          calc i7.val ≤ 255 := hi7le
            _ < 2 ^ (UScalarTy.U64.numBits) := by norm_num)
      have hU64size : (2 : Nat) ^ 64 = U64.size := by
        rw [U64.size_eq]; norm_num
      have hi9val : i9.val = i7.val * 2 ^ (8 * i.val.toNat) := by
        rw [hi9, hcastU64, hi1valNat, Nat.shiftLeft_eq]
        apply Nat.mod_eq_of_lt
        calc i7.val * 2 ^ (8 * i.val.toNat)
            ≤ 255 * 2 ^ (8 * i.val.toNat) := Nat.mul_le_mul_right _ hi7le
          _ < 2 ^ 64 := by
              have : 8 * i.val.toNat ≤ 56 := by omega
              calc 255 * 2 ^ (8 * i.val.toNat)
                  ≤ 255 * 2 ^ 56 :=
                    Nat.mul_le_mul_left _ (Nat.pow_le_pow_right (by norm_num) this)
                _ < 2 ^ 64 := by norm_num
          _ = U64.size := hU64size
      have hpow : (256 : Nat) ^ i.val.toNat = 2 ^ (8 * i.val.toNat) := by
        rw [pow_mul]; norm_num
      have hr_lt : r.val < 2 ^ (8 * i.val.toNat) := by
        rw [hrEq, ← hpow]; exact lutSum_lt a.val b.val L i.val.toNat
      have hr1val : (r ||| i9).val = r.val + i7.val * 2 ^ (8 * i.val.toNat) := by
        rw [UScalar.val_or, hi9val,
          show i7.val * 2 ^ (8 * i.val.toNat) = i7.val <<< (8 * i.val.toNat)
            from (Nat.shiftLeft_eq _ _).symm,
          Nat.lor_comm, ← Nat.shiftLeft_add_eq_or_of_lt hr_lt, Nat.add_comm,
          Nat.shiftLeft_eq]
      refine ⟨by omega, by omega, ?_, ?_⟩
      · rw [hr1val, hi10val, lutSum, hrEq, hi7val, hkeyval, hpow]
      · -- measure decreases: (8 - i10) < (8 - i), with i10 = i + 1, i < 8
        show (8 - i10.val).toNat < (8 - i.val).toNat
        omega
    · simp only [hlt, ite_false, spec, theta, wp_return]
      have hieq : i.val = 8 := by scalar_tac
      have hieqN : i.val.toNat = 8 := by omega
      intro bi hbi
      rw [hrEq, byteN_lutSum, hieqN]
      simp [hbi]
  · exact ⟨by decide, by decide, by simp [lutSum]⟩

/-- §3.3 L9 `binary_op_word_nib`: the nibble of lane `i` (`i : Fin 16`)
in `binary_op_word a b L` equals the corresponding low/high nibble of
the LUT result byte for byte `i/2`. -/
theorem binary_op_word_nib
    (a b : Std.U64) (L : Array Std.U8 65536#usize) (i : Fin 16) :
    ∃ r : Std.U64, packed.packed7.binary_op_word a b L = ok r ∧
      nib r.val i.val =
        (let byte := (L.val[keyByte a.val b.val (i.val / 2)]!).val
         if i.val % 2 = 0 then byte % 16 else (byte / 16) % 16) := by
  obtain ⟨r, hr, hbytes⟩ := binary_op_word_spec a b L
  refine ⟨r, hr, ?_⟩
  have hbi : i.val / 2 < 8 := by omega
  have hbyte := hbytes (i.val / 2) hbi
  -- nib r i = (r / 16^i) % 16; byteN r (i/2) = (r / 256^(i/2)) % 256.
  -- Lane i is byte i/2 nibble i%2.
  have hmod : i.val % 2 = 0 ∨ i.val % 2 = 1 := by omega
  have e256 : (256 : Nat) ^ (i.val / 2) = 16 ^ (2 * (i.val / 2)) := by
    rw [pow_mul]; norm_num
  simp only [nib, byteN] at hbyte ⊢
  rcases hmod with hm | hm
  · simp only [hm, if_pos]
    -- even lane: 16^(i) = 256^(i/2); byte%16 from the full byte (16 ∣ 256)
    have hexp : (16 : Nat) ^ i.val = 256 ^ (i.val / 2) := by
      conv_lhs => rw [show i.val = 2 * (i.val / 2) from by omega]
      rw [pow_mul, show (16 : Nat) ^ 2 = 256 from by norm_num]
    rw [hexp, ← hbyte]
    exact (Nat.mod_mod_of_dvd _ (by norm_num : (16 : Nat) ∣ 256)).symm
  · simp only [hm]
    rw [if_neg (by decide : ¬ ((1 : Nat) = 0))]
    -- odd lane: 16^i = 256^(i/2) * 16; (byte/16)%16 from the high nibble
    have hexp : (16 : Nat) ^ i.val = 256 ^ (i.val / 2) * 16 := by
      conv_lhs => rw [show i.val = 2 * (i.val / 2) + 1 from by omega]
      rw [pow_succ, pow_mul, show (16 : Nat) ^ 2 = 256 from by norm_num]
    rw [hexp, ← hbyte]
    -- LHS: Y/(D*16)%16 = Y/D/16%16  ;  RHS: (Y/D % 256)/16%16 = Y/D/16%16
    rw [← Nat.div_div_eq_div_mul,
      show (256 : Nat) = 16 * 16 from by norm_num,
      Nat.mod_mul_right_div_self,
      Nat.mod_mod_of_dvd _ (by norm_num : (16 : Nat) ∣ 16)]

/-! ## §3.4 Per-op `*_correct` theorems (against the Aeneas-extracted fn)

The byte-pair LUT couples two adjacent lanes (`2·bi` and `2·bi+1`) into
one 16-bit key; a non-canonical *partner* lane zeroes the whole result
byte (the `build_*_lut` guard, packed7.rs:64). So the per-lane theorems
carry a word-level canonicality hypothesis: every lane of both operands
holds a canonical codepoint `0..=6` — exactly the production
well-formedness invariant (packed7.rs:7-8,29-30; every `Packed7`
constructor only writes canonical nibbles). This mirrors the D5
`canon5_lane`-on-every-lane contract. -/

/-- Word-level canonicality: every one of the 16 lanes holds a canonical
F_7 codepoint (`< 7`). Carried by every value the production code
produces (packed7.rs:7-8,29-30). -/
def Canon7Word (w : Nat) : Prop := ∀ j : Fin 16, nib w j.val < 7

/-- The four byte-key nibbles are the operand lane nibbles at `2·bi` and
`2·bi+1`. -/
theorem keyByte_nibs (a b bi : Nat) :
    keyByte a b bi % 16 = nib a (2 * bi) ∧
    (keyByte a b bi / 16) % 16 = nib a (2 * bi + 1) ∧
    (keyByte a b bi / 256) % 16 = nib b (2 * bi) ∧
    (keyByte a b bi / 4096) % 16 = nib b (2 * bi + 1) := by
  have h1 := nib_lt_16 a (2 * bi)
  have h2 := nib_lt_16 a (2 * bi + 1)
  have h3 := nib_lt_16 b (2 * bi)
  have h4 := nib_lt_16 b (2 * bi + 1)
  simp only [keyByte]
  refine ⟨by omega, by omega, by omega, by omega⟩

/-- Generic per-lane correctness against `binary_op_word a b L` given the
LUT-axiom contract `hc` and word-level canonicality of both operands.
Lane `i` decodes to `op` of the decoded operand lanes mod 7. -/
private theorem lane_correct_of_lut
    (a b : Std.U64) (L : Array Std.U8 65536#usize) (i : Fin 16)
    (op : Nat → Nat → Nat)
    (hc : ∀ a0 a1 b0 b1 : Nat, a0 < 16 → a1 < 16 → b0 < 16 → b1 < 16 →
      (L.val[a0 + 16 * a1 + 256 * b0 + 4096 * b1]!).val =
        if a0 < 7 ∧ a1 < 7 ∧ b0 < 7 ∧ b1 < 7 then
          (op a0 b0 % 7) + 16 * (op a1 b1 % 7) else 0)
    (ha : Canon7Word a.val) (hb : Canon7Word b.val) :
    ∃ r : Std.U64, packed.packed7.binary_op_word a b L = ok r ∧
      dec7 (nib r.val i.val)
        = dec7 (op (nib a.val i.val) (nib b.val i.val) % 7) := by
  obtain ⟨r, hr, hnib⟩ := binary_op_word_nib a b L i
  refine ⟨r, hr, ?_⟩
  rw [hnib]
  set bi := i.val / 2 with hbi
  have h2bi : 2 * bi < 16 := by omega
  have h2bi1 : 2 * bi + 1 < 16 := by omega
  have ca0 : nib a.val (2*bi) < 7 := ha ⟨2*bi, h2bi⟩
  have ca1 : nib a.val (2*bi+1) < 7 := ha ⟨2*bi+1, h2bi1⟩
  have cb0 : nib b.val (2*bi) < 7 := hb ⟨2*bi, h2bi⟩
  have cb1 : nib b.val (2*bi+1) < 7 := hb ⟨2*bi+1, h2bi1⟩
  have hca0 : nib a.val (2*bi) < 16 := nib_lt_16 _ _
  have hca1 : nib a.val (2*bi+1) < 16 := nib_lt_16 _ _
  have hcb0 : nib b.val (2*bi) < 16 := nib_lt_16 _ _
  have hcb1 : nib b.val (2*bi+1) < 16 := nib_lt_16 _ _
  have hkey : keyByte a.val b.val bi
      = nib a.val (2*bi) + 16 * nib a.val (2*bi+1)
        + 256 * nib b.val (2*bi) + 4096 * nib b.val (2*bi+1) := by
    simp only [keyByte]
  have hcontract := hc (nib a.val (2*bi)) (nib a.val (2*bi+1))
    (nib b.val (2*bi)) (nib b.val (2*bi+1)) hca0 hca1 hcb0 hcb1
  rw [hkey, hcontract]
  have hcanon : nib a.val (2*bi) < 7 ∧ nib a.val (2*bi+1) < 7
      ∧ nib b.val (2*bi) < 7 ∧ nib b.val (2*bi+1) < 7 := ⟨ca0, ca1, cb0, cb1⟩
  rw [if_pos hcanon]
  have hr0 : op (nib a.val (2*bi)) (nib b.val (2*bi)) % 7 < 7 :=
    Nat.mod_lt _ (by decide)
  have hr1 : op (nib a.val (2*bi+1)) (nib b.val (2*bi+1)) % 7 < 7 :=
    Nat.mod_lt _ (by decide)
  have hi2 : i.val = 2 * bi ∨ i.val = 2 * bi + 1 := by omega
  rcases hi2 with hii | hii
  · have hpar : i.val % 2 = 0 := by omega
    rw [if_pos hpar]
    -- (r0 + 16*r1) % 16 = r0  since r0 < 7 < 16
    rw [show ((op (nib a.val (2*bi)) (nib b.val (2*bi)) % 7)
          + 16 * (op (nib a.val (2*bi+1)) (nib b.val (2*bi+1)) % 7)) % 16
        = op (nib a.val (2*bi)) (nib b.val (2*bi)) % 7 from by omega]
    rw [hii]
  · have hpar : i.val % 2 = 1 := by omega
    rw [if_neg (by rw [hpar]; decide : ¬ (i.val % 2 = 0))]
    rw [show ((op (nib a.val (2*bi)) (nib b.val (2*bi)) % 7)
          + 16 * (op (nib a.val (2*bi+1)) (nib b.val (2*bi+1)) % 7)) / 16 % 16
        = op (nib a.val (2*bi+1)) (nib b.val (2*bi+1)) % 7 from by omega]
    rw [hii]

/-- §3.4 L10 add-correct: per-lane add on the inherent wrapper, against
canonical `ZMod 7` addition. Word-level canonicality on both operands
(packed7.rs:7-8,29-30 — redundant codepoints never produced). -/
theorem packed7_add_correct (a b : Std.U64) (i : Fin 16)
    (ha : Canon7Word a.val) (hb : Canon7Word b.val) :
    ∃ r, packed.packed7.Packed7.add_inherent ⟨a⟩ ⟨b⟩ = ok r ∧
      dec7 (nib r.w.val i.val)
        = dec7 (nib a.val i.val) + dec7 (nib b.val i.val) := by
  unfold packed.packed7.Packed7.add_inherent
    packed.packed7.Packed7.Insts.Gf2_algebraPackedPackedFieldFp7U64U128.add
  obtain ⟨L, hL, hcL⟩ := add_lut_spec
  rw [hL]
  simp only [bind_tc_ok]
  obtain ⟨r, hr, hdec⟩ := lane_correct_of_lut a b L i (· + ·)
    (fun a0 a1 b0 b1 h0 h1 h2 h3 => hcL a0 a1 b0 b1 h0 h1 h2 h3) ha hb
  refine ⟨⟨r⟩, ?_, ?_⟩
  · simp only [hr, bind_tc_ok]
  · simpa using hdec.trans (dec7_add_contract _ _ (ha i) (hb i))

/-- §3.4 L11 sub-correct: analogous, canonical `ZMod 7` subtraction. -/
theorem packed7_sub_correct (a b : Std.U64) (i : Fin 16)
    (ha : Canon7Word a.val) (hb : Canon7Word b.val) :
    ∃ r, packed.packed7.Packed7.sub_inherent ⟨a⟩ ⟨b⟩ = ok r ∧
      dec7 (nib r.w.val i.val)
        = dec7 (nib a.val i.val) - dec7 (nib b.val i.val) := by
  unfold packed.packed7.Packed7.sub_inherent
    packed.packed7.Packed7.Insts.Gf2_algebraPackedPackedFieldFp7U64U128.sub
  obtain ⟨L, hL, hcL⟩ := sub_lut_spec
  rw [hL]
  simp only [bind_tc_ok]
  obtain ⟨r, hr, hdec⟩ := lane_correct_of_lut a b L i (fun x y => x + 7 - y)
    (fun a0 a1 b0 b1 h0 h1 h2 h3 => hcL a0 a1 b0 b1 h0 h1 h2 h3) ha hb
  refine ⟨⟨r⟩, ?_, ?_⟩
  · simp only [hr, bind_tc_ok]
  · simpa using hdec.trans (dec7_sub_contract _ _ (ha i) (hb i))

/-- §3.4 L12 mul-correct: analogous, canonical `ZMod 7` multiplication. -/
theorem packed7_mul_correct (a b : Std.U64) (i : Fin 16)
    (ha : Canon7Word a.val) (hb : Canon7Word b.val) :
    ∃ r, packed.packed7.Packed7.mul_inherent ⟨a⟩ ⟨b⟩ = ok r ∧
      dec7 (nib r.w.val i.val)
        = dec7 (nib a.val i.val) * dec7 (nib b.val i.val) := by
  unfold packed.packed7.Packed7.mul_inherent
    packed.packed7.Packed7.Insts.Gf2_algebraPackedPackedFieldFp7U64U128.mul
  obtain ⟨L, hL, hcL⟩ := mul_lut_spec
  rw [hL]
  simp only [bind_tc_ok]
  obtain ⟨r, hr, hdec⟩ := lane_correct_of_lut a b L i (· * ·)
    (fun a0 a1 b0 b1 h0 h1 h2 h3 => hcL a0 a1 b0 b1 h0 h1 h2 h3) ha hb
  refine ⟨⟨r⟩, ?_, ?_⟩
  · simp only [hr, bind_tc_ok]
  · simpa using hdec.trans (dec7_mul_contract _ _ (ha i) (hb i))

/-- §3.4 L13 neg-correct: per-lane neg on the inherent wrapper, against
canonical `ZMod 7` negation. `neg` is `binary_op_word 0 self SUB_LUT`
(packed7.rs:631-635), i.e. the `0 - x` lane of `SUB_LUT`. The constant
`0` operand is trivially canonical on every lane. -/
theorem packed7_neg_correct (a : Std.U64) (i : Fin 16)
    (ha : Canon7Word a.val) :
    ∃ r, packed.packed7.Packed7.neg_inherent ⟨a⟩ = ok r ∧
      dec7 (nib r.w.val i.val) = -(dec7 (nib a.val i.val)) := by
  unfold packed.packed7.Packed7.neg_inherent
    packed.packed7.Packed7.Insts.Gf2_algebraPackedPackedFieldFp7U64U128.neg
  obtain ⟨L, hL, hcL⟩ := sub_lut_spec
  rw [hL]
  simp only [bind_tc_ok]
  have h0val : (0#u64 : Std.U64).val = 0 := by native_decide
  have hzero : Canon7Word (0#u64 : Std.U64).val := by
    intro j; simp only [nib, h0val, Nat.zero_div, Nat.zero_mod]; decide
  obtain ⟨r, hr, hdec⟩ :=
    lane_correct_of_lut (0#u64 : Std.U64) a L i (fun x y => x + 7 - y)
      (fun a0 a1 b0 b1 h0 h1 h2 h3 => hcL a0 a1 b0 b1 h0 h1 h2 h3)
      hzero ha
  have hz : nib (0#u64 : Std.U64).val i.val = 0 := by
    simp only [nib, h0val, Nat.zero_div, Nat.zero_mod]
  rw [hz] at hdec
  refine ⟨⟨r⟩, ?_, ?_⟩
  · simp only [hr, bind_tc_ok]
  · exact hdec.trans (dec7_neg_contract (nib a.val i.val) (ha i))

/-! ## §3.5 Headline corollary + `Fp<7>` bridge note -/

/-- Tag for the four D6 ops (mirrors `Bipedal3Correctness.ArithOp` /
`Packed5Correctness.ArithOp`). -/
inductive ArithOp
  | add
  | sub
  | mul
  | neg
deriving DecidableEq

/-- Reference dispatch on `ZMod 7` per tag; `neg` ignores the rhs. -/
def ZMod7.dispatch : ArithOp → ZMod 7 → ZMod 7 → ZMod 7
  | .add, a, b => a + b
  | .sub, a, b => a - b
  | .mul, a, b => a * b
  | .neg, a, _ => -a

/-- `Packed7` dispatch on the inherent methods; `neg` ignores the rhs.
`noncomputable` because the inherent ops load the opaque (axiomatised,
Path B) LUT externals. -/
noncomputable def Packed7.dispatch :
    ArithOp → packed.packed7.Packed7 → packed.packed7.Packed7 →
      Result packed.packed7.Packed7
  | .add, x, y => packed.packed7.Packed7.add_inherent x y
  | .sub, x, y => packed.packed7.Packed7.sub_inherent x y
  | .mul, x, y => packed.packed7.Packed7.mul_inherent x y
  | .neg, x, _ => packed.packed7.Packed7.neg_inherent x

/-- Lane decoder lifted to a `Packed7` value. -/
def dec7_lane (a : packed.packed7.Packed7) (i : Fin 16) : ZMod 7 :=
  dec7 (nib a.w.val i.val)

/-- §3.5 headline corollary: `Packed7` arithmetic vs canonical F_7
(`ZMod 7`) arithmetic on every lane, for every `ArithOp` tag. The four
per-op theorems provide the case-split discharge (D6 §1 headline). The
`Canon7Word` hypotheses encode the production well-formedness invariant
(codepoints `7..=15` never produced — packed7.rs:7-8,29-30). -/
theorem packed7_correct_vs_canonical_F7
    (op : ArithOp) (a b : packed.packed7.Packed7) (i : Fin 16)
    (ha : Canon7Word a.w.val) (hb : Canon7Word b.w.val) :
    ∃ r, Packed7.dispatch op a b = ok r ∧
      dec7_lane r i = ZMod7.dispatch op (dec7_lane a i) (dec7_lane b i) := by
  cases op with
  | add =>
    obtain ⟨r, hr, hd⟩ := packed7_add_correct a.w b.w i ha hb
    exact ⟨r, by simpa [Packed7.dispatch] using hr,
      by simpa [ZMod7.dispatch, dec7_lane] using hd⟩
  | sub =>
    obtain ⟨r, hr, hd⟩ := packed7_sub_correct a.w b.w i ha hb
    exact ⟨r, by simpa [Packed7.dispatch] using hr,
      by simpa [ZMod7.dispatch, dec7_lane] using hd⟩
  | mul =>
    obtain ⟨r, hr, hd⟩ := packed7_mul_correct a.w b.w i ha hb
    exact ⟨r, by simpa [Packed7.dispatch] using hr,
      by simpa [ZMod7.dispatch, dec7_lane] using hd⟩
  | neg =>
    obtain ⟨r, hr, hd⟩ := packed7_neg_correct a.w i ha
    exact ⟨r, by simpa [Packed7.dispatch] using hr,
      by simpa [ZMod7.dispatch, dec7_lane] using hd⟩

/-! ### §3.5 Bridge to canonical `Fp<7>`

The four `*_correct` theorems are stated against `dec7` (pure `ZMod 7`)
for decidability, exactly as the D5 `packed5_*_correct` theorems are
stated against `dec5` (pure `ZMod 5`) and the V1 `bipedal3_*_correct`
against `psi` (pure `ZMod 3`). The connection to the production `Fp<7>`
field is the `fpEquiv` / `FpVal.instCommRing` ring-iso already verified
at `P = 7` (7 prime, `1 < 7`, `7 ≤ 2^63`) in
`Gf2Core/Proofs/FpField.lean:87`. Per D6 §1 / risk R7, the `Fp<7>`
rephrasing is a cited, additive note; the bridge equiv is not re-proved
here. It is *not* load-bearing for the issue's `add/sub/mul/neg`
correctness criteria, which are fully discharged by the `*_correct` /
headline theorems above against `dec7`/`ZMod 7`. -/

end Packed7Correctness
