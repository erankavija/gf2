# D6 — Lean4 proof sketch: `Packed7` F_7 packed-arithmetic correctness

**Issue:** `30e98ef1` (Lean F_5 and F_7 packed correctness)
**Epic:** `epic:gf2-algebra-permanent` (`ae82bd73`)
**Status:** sketch — user approval required per CLAUDE.md §Verification work
**Sibling sketches:** D2 (bipedal F_3, closed), D3 (Ryser F_3), D5 (F_5 `Packed5`, separate document)
**Pre-read of Mathlib confirmed:**
- `Field (ZMod p)` instance present at
  `proofs/.lake/packages/mathlib/Mathlib/Algebra/Field/ZMod.lean:30`
  (needs `[Fact p.Prime]`); `7` is prime — same instance D5 uses at
  `p = 5`.
- `FpVal P ≃ ZMod P.val` bridge exists generically at
  `proofs/Gf2Core/Proofs/FpField.lean:87`; `P = 7` is `ValidPrime`
  (prime, `1 < 7`, `7 ≤ 2^63`). Free specialisation, as in D5 §1.
- Closed `ZMod 7` arithmetic is `decide`-able (same as `ZMod 3`/`ZMod 5`).
- **Mathlib gap relevant to D6:** no lemma library exists for "a `static`
  array initialised by a Rust `const fn` nested loop has a given closed
  characterising function". This is *not* a Mathlib gap per se (it is an
  Aeneas-extraction-shape problem, see §4) but it means D6 cannot lean
  on any off-the-shelf table-correctness combinator the way D5 leans on
  `BitVec.getLsbD_*`. The proof must either evaluate the extracted loop
  or axiomatise the table with a characterisation; both are spelled out
  in §4.

This document is a *sketch only*. **No proof bodies are included.** It
fixes the extraction target, names the tactic per lemma, and — per the
honesty requirement of the dispatch and CLAUDE.md §"Previous incidents"
(`afac2262`, `8889e712`) — gives a **truthful F_7 LUT-extraction
feasibility verdict in §4 and §6 rather than an optimistic one**.

---

## 0. Notation

`Packed7` (`crates/gf2-algebra/src/packed/packed7.rs:211`) is a single
`u64` `w` packing **16 independent F_7 lanes** at 4-bit-aligned slots:
lane `i` (`0 ≤ i < 16`) occupies bits `[4i, 4i+4)`
(`packed7.rs:184-188`). Canonical values are `0..=6`; the slot's high
bit (`4i+3`) is reserved and zero for canonical values (since
`6 = 0b0110`). The single redundant 4-bit codepoint `7` decodes via
`Fp::<7>::new(7) = 0` (`packed7.rs:280-281`, R2 §… `r2_f7_encoding_decision.md`).

Binary ops route through three 64 KiB compile-time lookup tables
`ADD_LUT` / `SUB_LUT` / `MUL_LUT` (`packed7.rs:137,143,149`), each a
`static [u8; 65536]` initialised by a `const fn`
(`build_add_lut`/`build_sub_lut`/`build_mul_lut`, `packed7.rs:54,82,110`).
The per-word op is `binary_op_word` (`packed7.rs:167-178`): an
8-iteration `while` loop over byte pairs, each iteration doing one LUT
index `lut[ap | (bp << 8)]` and an OR-shift accumulate. `neg` is
`binary_op_word(0, self.w, &SUB_LUT)` (`packed7.rs:631-635`).

The 16 lanes are independent (each byte pair drives one LUT lookup
yielding two independent nibble results), so per-op correctness reduces
to: (a) LUT-entry correctness over the 49 ordered `(a,b) ∈ F_7²`
canonical pairs per op, and (b) a byte-decompose/recompose composition
lemma over the 8-iteration `binary_op_word` loop, lifted to all 16
lanes. Item (a) is the part with the extraction risk.

---

## 1. Statement

D6 is the conjunction of four operation-correctness theorems plus the
nibble decoder block plus the LUT-characterisation block plus the
`binary_op_word` loop-composition lemma. Lean signatures (informal;
namespace `Packed7Correctness`, mirroring `Bipedal3Correctness`):

```lean
namespace Packed7Correctness

/-- Nibble decoder. A 4-bit slot value `v : Nat` (`v < 16`) decodes to
    the canonical F_7 value; `7..=15` decode to 0 (matching
    `Fp::<7>::new`, packed7.rs:281, which reduces mod 7 — note 7..=13
    reduce to 0..=6, but canonical packings never produce nibble ≥ 7
    and the LUT yields 0 on non-canonical input, packed7.rs:29-30 /
    :64). For the proof we define dec7 as the LUT contract: 0..=6 ↦
    itself, 7..=15 ↦ 0. -/
def dec7 (v : Nat) : ZMod 7 := if v < 7 then (v : ZMod 7) else 0

/-- Extract lane `i`'s nibble from a u64 word. -/
def nib (w : BitVec 64) (i : Fin 16) : Nat := (w.toNat >>> (4 * i.val)) &&& 0xf

theorem packed7_add_correct (a b : Std.U64) (i : Fin 16) :
    ∃ r, packed.packed7.Packed7.add_inherent ⟨a⟩ ⟨b⟩ = ok r ∧
      dec7 (nib r.w.bv i) = dec7 (nib a.bv i) + dec7 (nib b.bv i)   -- ZMod 7

theorem packed7_sub_correct (...) :  ... = (.. - ..)   -- analogous, SUB_LUT
theorem packed7_mul_correct (...) :  ... = (.. * ..)   -- analogous, MUL_LUT
theorem packed7_neg_correct (a : Std.U64) (i : Fin 16) :
    ∃ r, packed.packed7.Packed7.neg_inherent ⟨a⟩ = ok r ∧
      dec7 (nib r.w.bv i) = - dec7 (nib a.bv i)                     -- 0 - a via SUB_LUT

end Packed7Correctness
```

`ZMod 7` carries the Mathlib `Field` instance. Headline corollary
`packed7_correct_vs_canonical_F7` folds the four into one
`ArithOp`-tagged statement (same shape as
`bipedal3_correct_vs_canonical_F3`, `Bipedal3Correctness.lean:341`).
The `Fp<7>` bridge (`fpEquiv` @ P=7) corollary
`packed7_lane_eq_fp7` is cited, not re-proved (as in D5 §1).

---

## 2. Encoding rationale + bounded scope

`Packed7` is **Candidate A** (R2 decision,
`dev/plans/r2_f7_encoding_decision.md`; transliteration source noted
`packed7.rs:35-36`): 4-bit slots, 16 lanes/`u64`, three 64 KiB
compile-time LUTs. The encoding choice is *not* re-litigated here; D6
verifies the implementation as written.

Lane count fixed at 16 (`packed7.rs:216,500`); the proof quantifies
`i : Fin 16`. No multi-word `Packed7Vec` (out of scope, §7).

The decidable part of the proof (analogue of D5's truth tables) is the
**LUT-entry characterisation**: for every canonical key
`key = ap | (bp << 8)` with `ap = a0 | (a1<<4)`, `bp = b0 | (b1<<4)`,
`a0,a1,b0,b1 < 7`, the table entry equals `((a0+b0)%7) | (((a1+b1)%7)<<4)`
(and analogously for sub/mul). There are `7⁴ = 2401` canonical keys per
op (the other `65536 − 2401` keys are non-canonical and the LUT holds
`0` there per `build_*_lut`'s `if a0<7 && a1<7 && b0<7 && b1<7` guard,
`packed7.rs:64`). The arithmetic on each key is closed `Nat`/`ZMod 7`
and `decide`-able **once the table entry is in hand** — the entire
difficulty is *getting the symbolic table entry in hand* (§4).

---

## 3. Lemma list — one-line tactic per lemma

Counts depend on which §4 path is taken. The lemma list below is for
**Path B (axiomatise-LUT-with-characterisation)** — the realistic path
(§4/§6 explain why Path A is high-risk). Path-A counts noted inline.

### 3.1 Nibble decoder + extraction helpers

| # | Name | Statement (informal) | Tactic | ~LoC |
|---|------|----------------------|--------|------|
| L1 | `dec7_canon` | `v < 7 → dec7 v = (v : ZMod 7)` | `simp [dec7]` | 3 |
| L2 | `dec7_noncanon` | `7 ≤ v → dec7 v = 0` | `simp [dec7]` | 3 |
| L3 | `dec7_total` | `dec7 v ∈ {0,…,6}` for `v < 16` | `interval_cases v <;> decide` | 4 |
| L4 | `nib_lt_16` | `nib w i < 16` | `simp [nib]; omega` (mask by `0xf`) | 4 |
| L5 | `nib_recompose` | recomposing 16 nibbles via `Σ (nib·16^?)` round-trips a `u64` (the byte/slot decompose↔recompose identity) | `bv_decide` (pure `BitVec 64` shift/mask identity) | 8 |

### 3.2 LUT characterisation (the load-bearing, high-risk block)

| # | Name | Statement (informal) | Tactic (Path B) | ~LoC |
|---|------|----------------------|-----------------|------|
| L6 | `add_lut_spec` | `∀ key < 65536, (extracted ADD_LUT).index key` decoded = the per-nibble-pair add contract (0 on non-canonical) | **Path B:** stated as an `axiom` characterising the *opaque-extracted* `ADD_LUT` global, justified in a `-- SAFETY/JUSTIFICATION` block citing `build_add_lut`'s source (`packed7.rs:54-75`) line-by-line. **Path A:** `intro key hk; <loop-invariant proof over the 256×256 `while` in `build_add_lut`>` — see §4. | Path B: ~12 (axiom + justification). Path A: ~120–250 (loop invariant). |
| L7 | `sub_lut_spec` | analogous for `SUB_LUT` (`build_sub_lut`, `packed7.rs:82-103`) | as L6 | as L6 |
| L8 | `mul_lut_spec` | analogous for `MUL_LUT` (`build_mul_lut`, `packed7.rs:110-131`) | as L6 | as L6 |

### 3.3 `binary_op_word` loop-composition lemma

| # | Name | Statement (informal) | Tactic | ~LoC |
|---|------|----------------------|--------|------|
| L9 | `binary_op_word_nib` | for any `lut` satisfying a per-byte-pair characterisation `H`, `nib (binary_op_word a b lut) i = H_nib(nib a i, nib b i)` for every `i : Fin 16` | induction on the 8-iteration `while` (Aeneas extracts it as a `Result` loop; `progress`/loop-invariant pattern from `MontgomeryRoundtrip.lean`'s loop handling), each step `Array.index_usize_spec` (`Aeneas/Std/Array/Array.lean:105`) + the byte-split bv identity from L5 | 35–55 |

### 3.4 Per-op `*_correct` theorems

| # | Name | Statement | Tactic | ~LoC |
|---|------|-----------|--------|------|
| L10 | `packed7_add_correct` | §1 | `unfold add_inherent; unfold Insts.…add; refine ⟨_, rfl, ?_⟩; apply binary_op_word_nib (H := add_lut_spec); <close per-nibble add via `decide` on the 49 canonical pairs + L6 non-canonical branch>` | 18 |
| L11 | `packed7_sub_correct` | §1 | analogous (SUB_LUT, L7) | 18 |
| L12 | `packed7_mul_correct` | §1 | analogous (MUL_LUT, L8) | 18 |
| L13 | `packed7_neg_correct` | §1 | analogous, `a := 0`, SUB_LUT (`0 - x`); `decide` on 7 canonical values | 15 |

### 3.5 Bridges

| # | Name | Statement | Tactic | ~LoC |
|---|------|-----------|---------|------|
| L14 | `packed7_lane_eq_fp7` | `Fp7_RingEquiv_ZMod7 (Packed7.lane a i) = dec7 (nib a.w.bv i)` (cites `fpEquiv` @ P=7) | `simp [fpEquiv, …]` (cite, no new content) | 8 |
| L15 | `packed7_correct_vs_canonical_F7` | §1 headline corollary | `cases op <;> simpa [Packed7.dispatch, ZMod7.dispatch] using packed7_*_correct …` | 8 |

**Estimated file size:**
- **Path B (axiomatise-LUT):** ≈ 200 lines (comparable to D5; the
  axiom-justification blocks replace D5's truth-table bodies).
- **Path A (prove-LUT):** ≈ 400–550 lines (the three `*_lut_spec`
  loop-invariant proofs dominate, ~120–250 LoC each, plus heartbeat
  risk — see §4/§6).

---

## 4. Monomorphisation / extraction target — **F_7 LUT feasibility (highest-risk item)**

**Rust source:** `crates/gf2-algebra/src/packed/packed7.rs`. Proof
target: inherent wrappers `Packed7::{add,sub,mul,neg}_inherent`, the
same proof-target convention D2/D5 use.

> **Implementation prerequisite (Rust, NOT a proof deliverable):**
> `packed7.rs` has **no** `{add,sub,mul,neg}_inherent` wrappers (only
> `bipedal3.rs:409-467` has them). The D6 implementation issue must add
> them to `packed7.rs` (verbatim copies of the `bipedal3.rs:408-467`
> pattern, delegating to the `Packed7` `PackedField<Fp<7>>` impl at
> `packed7.rs:488`), and flip `verify-lean.sh:121`
> `--opaque 'gf2_algebra::packed::packed7'` → `--start-from`, adding
> `--features f7` to the gf2-algebra Charon line (`verify-lean.sh:138`).
> Same mechanical config step as D5 §4 R3.

### 4.1 The core question

`binary_op_word` (`packed7.rs:167-178`) is an 8-iteration `while` loop —
**tractable**, identical in shape to loops Aeneas already extracts and
proves in `MontgomeryRoundtrip.lean` (the Newton/REDC loops). That part
is **not** the risk.

The risk is the LUT. `static ADD_LUT: [u8; 65536] = build_add_lut();`
(`packed7.rs:137`). `build_add_lut` (`packed7.rs:54-75`) is a `const fn`
with a **doubly-nested `while` loop** (`while ap < 256 { while bp < 256
{ … lut[key] = … ; bp += 1 } ap += 1 }`) performing **65536
`Array.set` writes** to build the table.

**How Charon/Aeneas extracts a `static` initialised by a `const fn`:**
verified precedent in this repo — loop-computed globals extract as a
`def NAME : Result T := <symbolic body of the initialiser>` annotated
`@[global_simps, irreducible]`. Concrete example: the Montgomery
constant `R2_MOD_P` (`Gf2Core/Funs.lean:566-568`) extracts as
`def gfp.montgomery.MontConsts.R2_MOD_P (P) : Result U64 :=
gfp.montgomery.compute_r2_mod_p P` — i.e. the *symbolic computation*,
not a literal. Its value is established in
`MontgomeryRoundtrip.lean:64-92` (`compute_r2_mod_p_value`) by
**symbolically unfolding the loop and proving a characterising value
lemma**, NOT by `native_decide` evaluation.

Applying that precedent to `ADD_LUT`: the extracted Lean def is
(predicted)
`def …packed7.ADD_LUT : Result (Array U8 65536#usize) :=
…packed7.build_add_lut` — the symbolic 65536-iteration nested loop over
`Array U8 65536`, where `Array α n = { l : List α // l.length = n.val }`
(`Aeneas/Std/Array/Array.lean:15`), i.e. a **`List`-backed** structure.
`Array.set` (`Array/Array.lean:114`) is `List.set` on a 65536-element
list.

### 4.2 Path A — prove the LUT contents (symbolically unfold the loop)

Prove `∀ key < 65536, ADD_LUT.index key = expected(key)` by a loop
invariant over the 256×256 `while`: "after `ap` outer iterations and
`bp` inner, every already-written `key` slot holds the add contract and
every not-yet-written slot holds `0`". Mirrors the
`compute_r2_mod_p_value` technique (`MontgomeryRoundtrip.lean:64`)
generalised from a scalar accumulator to a 65536-element array
accumulator.

**Honest feasibility assessment of Path A:**
- It is **mathematically possible** — the loop is bounded and total,
  the invariant is standard, and Aeneas's `progress`/loop machinery
  (used successfully for the REDC/Newton loops in
  `MontgomeryRoundtrip.lean`) supports `while`-loop induction.
- It is **high-risk in practice** for three reasons:
  1. **Scale.** The `R2_MOD_P` precedent unfolds a loop with a *scalar*
     state. Here the loop state is an `Array U8 65536` = a 65536-element
     `List`. Each of the three `*_lut_spec` proofs threads a loop
     invariant over a `List.set` chain of length 65536. Lean's kernel
     does not need to *evaluate* all 65536 entries (the invariant is
     symbolic in `ap`/`bp`), but the `Array.set`/`Array.index_usize`
     `simp`/`omega` side-goals on a `List`-backed 65536-length
     structure are heavy and the `maxHeartbeats` (currently `1200000`
     in `Bipedal3Correctness.lean:41`) may need raising, with
     attendant CI-time risk (cf. D3 §7.2 `bv_decide` heartbeat risk).
  2. **`native_decide` is not available as a shortcut.** `key` is
     *symbolic* in `*_correct` (it ranges over all canonical lane
     inputs). One cannot `decide`/`native_decide` `ADD_LUT.index key`
     for symbolic `key`; only the *characterising property* (the loop
     invariant) closes it. `native_decide` could evaluate
     `ADD_LUT.index <literal>` but the proof needs the universally
     quantified statement, so the loop invariant is unavoidable.
  3. **Doubly-nested loop invariant.** `compute_r2_mod_p`'s precedent is
     a *single* loop. `build_add_lut` is *nested* (`ap` outer, `bp`
     inner). The invariant must be stated at two levels (outer: "all
     `ap' < ap` rows fully written"; inner: "row `ap`, columns
     `bp' < bp` written"). This roughly doubles the proof obligation
     vs. the single-loop precedent and is a known source of cycle
     churn (cf. CLAUDE.md incident `467d835e`, 10 cycles renegotiating
     proof shape).

  Estimate: each `*_lut_spec` is ~120–250 LoC of genuinely novel loop
  reasoning with non-trivial heartbeat tuning. Three of them.
  **This is the dominant cost of D6 and the single biggest risk.**

### 4.3 Path B — axiomatise the LUT with a proven-by-source characterisation

Add `--opaque 'gf2_algebra::packed::packed7::ADD_LUT'` (and `SUB_LUT`,
`MUL_LUT`) to the Charon invocation so Aeneas emits the three tables as
**opaque external constants** (the same `--opaque`/axiom mechanism the
pipeline already uses for `gf2_core::gfp` instances —
`Gf2Algebra/Funs.lean:138` axiomatises
`gf2_core.gfp.Fp.Insts.CoreOpsArithAddFpFp`; precedent for axiomatising
an extraction-opaqued item is established and accepted in this repo).
Then state L6/L7/L8 as **axioms** characterising those opaque tables:

```lean
/-- ADD_LUT contract. JUSTIFICATION: build_add_lut (packed7.rs:54-75)
    writes lut[(bp<<8)|ap] = r0 | (r1<<4) with r0=(a0+b0)%7,
    r1=(a1+b1)%7 exactly when a0,a1,b0,b1 < 7, else leaves 0
    (array zero-init, packed7.rs:55). This axiom is the Lean transcription
    of that source; it is NOT proved from the extracted loop (Path A)
    — see d6 §4. The axiom is validated out-of-band by a Rust
    proptest/exhaustive test asserting ADD_LUT against the same
    contract (packed7.rs test module). -/
axiom add_lut_spec : ∀ a0 a1 b0 b1 : Nat, a0<7→a1<7→b0<7→b1<7→ …
```

**Honest assessment of Path B:**
- It **does** discharge the issue's `add/sub/mul/neg` correctness
  theorems against the *production `binary_op_word` code path* (L9–L13
  are real proofs against the extracted Rust loop). The Rust algorithm
  *is* verified end-to-end **modulo** the table-contents axiom.
- It **introduces three `axiom` declarations**, which D3 §7.3 explicitly
  commits *not* to do for the F_3 Ryser work, and which weakens the
  proof: the LUT *contents* are asserted, not derived. This must be
  surfaced to the user as an explicit scope decision, not hidden.
- It is **low-risk and bounded** (~200 LoC, no heartbeat tuning, V1-like
  proof shape for L9–L13).
- The axiom is empirically cross-checked by the *existing* Rust test
  surface for `packed7` (the `#[cfg(test)]` modules at
  `packed7.rs:820`/`:2132` already validate the LUTs against scalar
  `Fp<7>` — `packed/mod.rs:13-20` cross-checking strategy), so the
  axiom is not unfounded; it is "trusted because exhaustively tested in
  Rust", which is a weaker guarantee than a Lean proof but a *stated,
  honest* one.

### 4.4 Verdict

**F_7 LUT-extraction feasibility: NEEDS-SPIKE, with axiomatise-fallback
(Path B) as the realistic default.**

- Path A (fully prove the LUT) is *possible but high-risk*: three
  doubly-nested 65536-iteration array-loop invariant proofs, no
  `native_decide` shortcut for the symbolic-key statement, heartbeat
  tuning required, ~360–750 LoC of novel reasoning. This is the
  incident-class CLAUDE.md warns about (`467d835e`/`8889e712`):
  open-ended proof difficulty without a de-risked design.
- **Recommended scope:** dispatch F_7 as **Path B (axiomatise the three
  LUTs with a source-faithful characterisation, prove `binary_op_word` +
  the four `*_correct` against the production path, cross-validate the
  axioms with the existing Rust exhaustive tests)** — *unless the user
  explicitly wants Path A and accepts the spike*. If Path A is desired,
  it must be preceded by a **1-session extraction-feasibility spike**:
  flip the Charon config, regenerate `Funs.lean`, and confirm (i) how
  Aeneas actually renders the `static`+`const fn` (literal `Array.make
  65536 [...]` vs. symbolic loop def vs. opaque), and (ii) that a
  *single* `*_lut_spec` row-invariant lemma closes within
  `maxHeartbeats` before committing to all three. The spike outcome
  determines whether Path A is even attempted.
- F_5 (D5) is independently **tractable** and should not be blocked on
  the F_7 decision; the two are separable (`packed5` is pure bitwise,
  `packed7` is the LUT case). Recommended dispatch order:
  **D5 first (low-risk), then D6 spike, then D6 implementation on the
  spike-selected path.**

This verdict is deliberately not optimistic. The dispatch and CLAUDE.md
require the sketch to inform the user's approval decision truthfully:
F_5 is a clean V1-pattern transfer; F_7's LUT is the genuine open
question and Path B (with three justified, Rust-test-validated axioms)
is the honest realistic deliverable unless the user funds the Path-A
spike.

---

## 5. Predicted Aeneas-generated def names

Following the `proofs/Gf2Algebra/Funs.lean` pattern for
`packed.bipedal3.*` (and the `@[global_simps, irreducible]` global
convention from `Gf2Core/Funs.lean:566`):

| Rust path | Predicted Lean def name |
|-----------|-------------------------|
| `gf2_algebra::packed::packed7::Packed7` (struct) | `gf2_algebra.packed.packed7.Packed7` (`structure … where w : Std.U64`) |
| `…::build_add_lut` | `gf2_algebra.packed.packed7.build_add_lut : Result (Array Std.U8 65536#usize)` |
| `…::build_sub_lut` / `…::build_mul_lut` | `…packed7.build_sub_lut` / `…packed7.build_mul_lut` |
| `…::ADD_LUT` (static) | `gf2_algebra.packed.packed7.ADD_LUT` — `@[global_simps, irreducible] def … : Result (Array Std.U8 65536#usize) := …packed7.build_add_lut` (Path A), or an **opaque external axiom `axiom …packed7.ADD_LUT : Array Std.U8 65536#usize`** if `--opaque`d (Path B) |
| `…::SUB_LUT` / `…::MUL_LUT` | analogous |
| `…::binary_op_word` | `gf2_algebra.packed.packed7.binary_op_word : Std.U64 → Std.U64 → Array Std.U8 65536#usize → Result Std.U64` |
| `…::Packed7::add_inherent` (etc.) | `gf2_algebra.packed.packed7.Packed7.{add,sub,mul,neg}_inherent` |
| `<Packed7 as PackedField<Fp<7>>>::add` (etc.) | `gf2_algebra.packed.packed7.Packed7.Insts.Gf2_algebraPackedPackedFieldFp7U64U128.{add,sub,mul,neg}` (exact `Insts.` mangling confirmed from regenerated `Funs.lean`; bipedal precedent `…Fp3U64U128…`, `Bipedal3Correctness.lean:233`) |

New proof file `proofs/Gf2Algebra/Proofs/Packed7Correctness.lean`,
covered by the existing `lean_lib Gf2Algebra` (`lakefile.lean:23-25`,
`srcDir := "."`) — no `lakefile.lean` change. The `lake-build` strict
wrapper (`lakefile.lean:27-40`) fails on `sorry`; under **Path B**, the
three `axiom`s are *not* `sorry` and pass the strict gate, but the
`code-review` gate must be told (in the issue description) that Path B
intentionally axiomatises the LUT contents — this is a scope decision
requiring the §8 user approval, consistent with the issue's `[hard]`
criterion 5 ("amended with a written justification").

### `FunsExternal` / axioms

- **Path A:** no new `FunsExternal.lean` content; the loop uses
  `Array.set`/`Array.index_usize` (Aeneas built-ins) only — same as the
  bipedal ops needing no externals (D2 §7).
- **Path B:** three table axioms in `Packed7Correctness.lean` itself
  (not `FunsExternal.lean`), plus the `--opaque` Charon flags. The
  `binary_op_word` 8-loop, `Array.index_usize`, and byte-split BitVec
  identities require no externals.

---

## 6. Risks

| # | Risk | Likelihood | Mitigation |
|---|------|-----------|------------|
| **R1** | **(biggest)** Path A `*_lut_spec` loop-invariant proofs over a 65536-element `List`-backed array do not close within `maxHeartbeats`, or the doubly-nested invariant balloons cycle count | **High (Path A)** / N/A (Path B) | The §4.4 verdict: do not commit to Path A without a 1-session spike confirming a single row-invariant closes. Default to Path B. If Path A spike fails, Path B is the contracted deliverable with user sign-off. |
| R2 | Aeneas renders `static + const fn` in an unanticipated shape (e.g. fully fails to translate the nested-loop `const fn`, emitting `sorry` in `Funs.lean`) | Medium | The spike (§4.4) reveals this on attempt 1. If the `const fn` is untranslatable, **Path B is forced** (opaque + axiom) — and is fine, because L9–L13 still verify the production `binary_op_word` path. |
| R3 | Charon config not flipped (`packed7` still `--opaque`, feature `f7` off) | Certain-if-unchanged | Mechanical one-time `verify-lean.sh:121` `--opaque`→`--start-from` + `--features f7` + (Path B) `--opaque …::ADD_LUT/SUB_LUT/MUL_LUT`. Smoke: regenerate `Funs.lean`, confirm `Packed7.add_inherent` non-opaque. Same as D5 R3. |
| R4 | Path B axioms are wrong (transcription error vs. `build_*_lut` source) | Low | The axioms are validated by the *existing* exhaustive Rust tests for `packed7` (`packed7.rs` `#[cfg(test)]` modules `:820`/`:2132`, cross-checking vs scalar `Fp<7>` per `packed/mod.rs:13-20`). The implementation issue must add (if not present) an explicit `proptest`/exhaustive Rust test asserting each LUT against the exact contract the axiom states, so axiom ⟺ tested-Rust-contract is mechanically checkable. |
| R5 | `Insts.` mangling differs from predicted `…Fp7U64U128…` | Low | One-token `unfold` rename, read from regenerated `Funs.lean` (bipedal precedent). No structural impact. |
| R6 | Path B's three axioms trip the `code-review` gate (CLAUDE.md: correctness is always `[hard]`, axioms weaken it) | Medium | This is a **scope decision, not a bug**: the issue itself is `[aspirational]` with criterion 5 explicitly allowing "amended with a written justification". Path B's axiom rationale + Rust-test cross-validation IS that written justification, recorded in the issue description and surfaced for §8 user approval *before* dispatch. Hiding it would repeat the `8889e712` incident; surfacing it here is the correct handling. |
| R7 | `fix-aeneas-gf2algebra.py` post-processing collides with the new `packed7` symbols | Low | Same `fix-aeneas-dupes.py`/`fix-aeneas-gf2algebra.py` passes already run on gf2-algebra (`verify-lean.sh:288-298`); the `*_correct` theorems do not project `Fp<7>` instances (stated against `dec7`/`ZMod 7`), as V1 avoids `Fp<3>`. Escalate only on a new hard failure. |

**Single biggest risk:** R1 — the Path-A LUT loop-invariant proof is
the open-ended, incident-class item. The mitigation is the §4.4
spike-gated decision and the Path-B fallback, both surfaced for user
approval rather than discovered mid-implementation.

---

## 7. Out of scope for D6

1. **`Packed7Vec` correctness.** D6 covers fixed-width 16-lane
   `Packed7` only. The multi-word `Packed7Vec` applies the same
   `binary_op_word` per `u64` in a loop with `mask_tail`; follows by
   `Vec` induction once the element theorem holds (cf. D2 §8 #1,
   D5 §7 #1).
2. **`Packed7Matrix`.** Trivially follows from `Packed7Vec`.
3. **Mask-tail invariant.** A Vec-level concern; single-word `Packed7`
   has no tail.
4. **SIMD-batched `Packed7`.** `unsafe`, outside the Aeneas subset.
5. **`div` / inverse.** Trait surface is `{add,sub,mul,neg}`
   (`packed/mod.rs:185-246`); no `div`. Issue criteria name exactly
   `add/sub/mul/neg`.
6. **Proving the LUT contents from the `const fn` (Path A)** is *in
   scope only if* the user funds the §4.4 spike and the spike succeeds.
   The default contracted deliverable is **Path B**.
7. **F_5 `Packed5`.** Sibling sketch D5, separate document — tractable,
   not blocked on D6.

---

## 8. Approval

Per CLAUDE.md §Verification work, this sketch must be reviewed by the
lead and approved by the user before D6 implementation is dispatched.
**The user's approval must explicitly choose the F_7 path**, because the
two paths have materially different correctness guarantees:

- **Path B (recommended default):** prove `binary_op_word` + the four
  `*_correct` theorems against the production code path; **axiomatise
  the three LUT contents** with a source-faithful characterisation,
  cross-validated by exhaustive Rust tests. ≈200 LoC, low-risk, ~1–2
  sessions. Correctness is verified *modulo* the (Rust-tested) table
  axioms — a stated, honest limitation.
- **Path A (only with a funded spike):** additionally prove the three
  LUT contents from the extracted `const fn` nested loops. ≈400–550
  LoC, high-risk (R1), spike-gated, 3–6+ sessions with heartbeat-tuning
  and cycle-churn risk. No axioms; fully self-contained.

Issue `30e98ef1` criterion 5 (`[hard]`: "If the encodings turn out
infeasible to verify in the timeframe, the issue is amended with a
written justification … rather than left half-proved") is satisfied by
this sketch's §4.4 verdict: F_7 is **not** infeasible (Path B is a
real, bounded deliverable), but the *fully-self-contained* form (Path A)
is high-risk and the scope choice is escalated to the user here rather
than discovered mid-loop.

This document does not modify `.jit/` state; transitioning `30e98ef1`
is the lead's responsibility after user sign-off.
