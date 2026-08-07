# D5 — Lean4 proof sketch: `Packed5` F_5 packed-arithmetic correctness

**Issue:** `30e98ef1` (Lean F_5 and F_7 packed correctness)
**Epic:** `epic:gf2-algebra-permanent` (`ae82bd73`)
**Status:** sketch — user approval required per CLAUDE.md §Verification work
**Sibling sketches:** D2 (bipedal F_3, closed → `Bipedal3Correctness.lean`), D3 (Ryser F_3), D6 (F_7 `Packed7`, separate document)
**Pre-read of Mathlib confirmed:**
- `Field (ZMod p)` instance present at
  `proofs/.lake/packages/mathlib/Mathlib/Algebra/Field/ZMod.lean:30`
  (needs `[Fact p.Prime]`).
- `DecidableEq (ZMod n)` / closed evaluation of `ZMod 5` arithmetic is
  available (same `decide`-on-closed-`ZMod` pattern V1 already uses in
  `Bipedal3Correctness.lean:75,95`; no Mathlib gap).
- The `FpVal P ≃ ZMod P.val` bridge already exists at
  `proofs/Gf2Core/Proofs/FpField.lean:87` (`fpEquiv`) plus the
  `FpVal.instCommRing` / field transfer (`FpField.lean:114`). It is
  generic in `ValidPrime P`; specialising at `P = 5` is free (5 is
  prime, `1 < 5`, `5 ≤ 2^63`).
- No new Mathlib lemma is required for D5. All bitwise lifting uses
  Lean-core `BitVec.getLsbD_{and,or,not}` simp lemmas (already
  load-bearing in `Bipedal3Correctness.lean:152,168,184`).

This document is a *sketch only*. It lists the lemmas, names the tactic
per lemma in one line, fixes the Charon/Aeneas extraction target, and
predicts the generated def names. **No proof bodies are included.** The
implementation is dispatched only after this sketch is approved.

---

## 0. Notation

`Packed5` (`crates/gf2-algebra/src/packed/packed5.rs:207`) is a triple of
`u64` bit-planes `(b0, b1, b2)` encoding **64 independent F_5 lanes**.
Lane `i` (`0 ≤ i < 64`) stores one F_5 element as the 3-bit canonical
value `bit_i(b0) | (bit_i(b1) << 1) | (bit_i(b2) << 2)` (`packed5.rs`
module table `:9-22`). Canonical codepoints are `0..=4`; `5..=7` are
redundant and decode to `0` (`lane`, `packed5.rs:566-583`).

The 64 lanes are mutually independent (every op is a fixed straight-line
bitwise circuit on the three planes — no carry, no cross-lane data
flow), so every per-op correctness theorem reduces to a single-lane
truth-table check over the 5 canonical input values, lifted to all 64
lanes by the standard `BitVec.getLsbD_*` simp set. This is structurally
the V1 (D2) pattern with a 3-plane decoder in place of the 2-plane
bipedal `ψ`.

---

## 1. Statement

D5 is the conjunction of four operation-correctness theorems plus the
decoder lemma block plus the lane-lift block. Lean signatures (informal;
namespace `Packed5Correctness`, mirroring `Bipedal3Correctness`):

```lean
namespace Packed5Correctness

/-- 3-bit-plane lane decoder. `(b0, b1, b2 : Bool)` for one lane decode
    to the canonical F_5 value; the three redundant codepoints 5,6,7
    decode to 0 (matching `Packed5::lane`, packed5.rs:578-582). -/
def dec5 : Bool → Bool → Bool → ZMod 5
  | false, false, false => 0   -- 0
  | true,  false, false => 1   -- 1
  | false, true,  false => 2   -- 2
  | true,  true,  false => 3   -- 3
  | false, false, true  => 4   -- 4
  | true,  false, true  => 0   -- codepoint 5 → 0
  | false, true,  true  => 0   -- codepoint 6 → 0
  | true,  true,  true  => 0   -- codepoint 7 → 0

theorem packed5_add_correct (b0 b1 b2 c0 c1 c2 : Std.U64) (i : Fin 64) :
    ∃ r, packed.packed5.Packed5.add_inherent ⟨b0,b1,b2⟩ ⟨c0,c1,c2⟩ = ok r ∧
      dec5 (r.b0.bv.getLsbD i) (r.b1.bv.getLsbD i) (r.b2.bv.getLsbD i)
        = dec5 (b0.bv.getLsbD i) (b1.bv.getLsbD i) (b2.bv.getLsbD i)
          + dec5 (c0.bv.getLsbD i) (c1.bv.getLsbD i) (c2.bv.getLsbD i)

theorem packed5_sub_correct  (...) :  ... = (.. - ..)   -- analogous
theorem packed5_mul_correct  (...) :  ... = (.. * ..)   -- analogous
theorem packed5_neg_correct  (b0 b1 b2 : Std.U64) (i : Fin 64) :
    ∃ r, packed.packed5.Packed5.neg_inherent ⟨b0,b1,b2⟩ = ok r ∧
      dec5 (r.b0.bv.getLsbD i) (r.b1.bv.getLsbD i) (r.b2.bv.getLsbD i)
        = - dec5 (b0.bv.getLsbD i) (b1.bv.getLsbD i) (b2.bv.getLsbD i)

end Packed5Correctness
```

Mathlib `ZMod 5` carries the `Field` instance against which we compare.
The headline corollary (mirroring `bipedal3_correct_vs_canonical_F3`,
`Bipedal3Correctness.lean:341`) folds the four into a single
`ArithOp`-tagged statement:

```lean
theorem packed5_correct_vs_canonical_F5
    (op : ArithOp) (a b : packed.packed5.Packed5) (i : Fin 64) :
    ∃ r, Packed5.dispatch op a b = ok r ∧
      dec5_lane r i = ZMod5.dispatch op (dec5_lane a i) (dec5_lane b i)
```

### Bridge to canonical `Fp<5>`

`dec5` is the `ZMod 5`-valued lane decoder. The connection to the
production `Fp<5>` field (the issue's "against the canonical `Fp<5>`"
requirement) is the existing `fpEquiv`/`FpVal.instCommRing` transfer in
`FpField.lean` specialised at `P = 5`. Concretely: a corollary
`packed5_lane_eq_fp5` states
`Fp5_RingEquiv_ZMod5 (Packed5.lane a i) = dec5_lane a i`, where
`Fp5_RingEquiv_ZMod5 : FpVal ⟨5⟩ ≃+* ZMod 5` is the `P = 5`
instantiation of the generic equiv. This corollary is *cited, not
re-proved* — it is the same ring-iso already verified in V0
(`MontgomeryRoundtrip.lean` + `FpField.lean`). The four `*_correct`
theorems are stated directly against `dec5` for decidability; the
`Fp<5>` rephrasing is one `simp [packed5_lane_eq_fp5]` step.

---

## 2. Encoding rationale + bounded scope

`Packed5` is **Candidate D** (R1 decision, `dev/plans/r1_f5_encoding_decision.md`
§5; transliteration source noted in `packed5.rs:42-46`): bit-sliced
3-plane canonical `(b0,b1,b2)`, all ops via a **5-way decode →
cross-product → encode** Boolean circuit. There is **no LUT, no
`OnceLock`, no runtime table, no `unsafe`** (`packed5.rs:186-188`).

Why this is decidable and tractable:

1. **Pure straight-line bitwise circuit.** `add` (`packed5.rs:432`)
   is `decode5 ∘ add_circuit ∘ encode5`; `decode5`
   (`packed5.rs:70-83`) is 3 NOTs + 8 ANDs; `add_circuit`
   (`packed5.rs:109-119`) is 20 ANDs + 16 ORs; `encode5`
   (`packed5.rs:92-97`) is 2 ORs. All operations are `&`, `|`, `!`
   on `u64` — exactly the operator set V1 already lifts with
   `BitVec.getLsbD_{and,or,not}`. No `wrapping_*`, no `overflowing_*`,
   no `U128`, no shift, no `Vec`.
2. **5-element-per-lane truth table.** The per-lane spec is closed by
   `decide` over `dec5`'s 8 Bool-triples (5 canonical + 3 redundant)
   for each operand → at most `8 × 8 = 64` rows for binary ops, `8` for
   `neg`. Each row reduces to a closed `ZMod 5` equality. This is the
   same `cases <;> decide` shape as `bipedal3_add_lane`
   (`Bipedal3Correctness.lean:104-108`), just with one more Bool per
   operand (3 planes vs 2).
3. **Lane count fixed at 64.** `Packed5::LANES = 64` (`packed5.rs:326`);
   the proof quantifies `i : Fin 64`. No `n`-parametric bound, no
   multi-word `Packed5Vec` (out of scope — see §7).

The three redundant codepoints `5,6,7` are handled by `dec5` mapping
them to `0` exactly as `Packed5::lane` does (`packed5.rs:578-582`). The
arithmetic circuits never *produce* a redundant codepoint on canonical
inputs (the `decode5` selectors are all-zero on redundant inputs, so
`encode5` re-emits a canonical codepoint); the truth table verifies the
op output is canonical and correctly valued for all 25 canonical input
pairs, and that redundant inputs decode-to-0 consistently. No "no
redundant codepoint produced" invariant lemma is separately required —
it falls out of the same `decide` table (cf. D2 §8 R4, the analogous
bipedal alt-zero argument).

---

## 3. Lemma list — one-line tactic per lemma

Counts: 8 decoder lemmas, 4 per-op lane lemmas, 4 per-op word/lift
lemmas, 4 per-op `*_correct` theorems, 1 lift helper, 1 `Fp<5>` bridge
corollary, 1 headline corollary = **23 lemmas**. Line estimates are
informed by the V1 file (`Bipedal3Correctness.lean`, 359 lines for the
20-lemma F_3 analogue).

### 3.1 Decoder `dec5` (§2 of the proof)

| # | Name | Statement (informal) | Tactic | ~LoC |
|---|------|----------------------|--------|------|
| L1–L5 | `dec5_{0,1,2,3,4}` | `dec5` on each canonical Bool-triple = `0,1,2,3,4` | `rfl` ×5 | 5 |
| L6 | `dec5_red5`/`dec5_red6`/`dec5_red7` | the three redundant codepoints map to `0` | `rfl` ×3 (bundled) | 4 |
| L7 | `dec5_total` | `∀ x y z, dec5 x y z ∈ ({0,1,2,3,4} : Set (ZMod 5))` | `cases … <;> decide` | 3 |
| L8 | `dec5_red_eq_zero` | redundant codepoint ↔ `dec5 = 0` consistency (matches `Packed5::all_zero`, `packed5.rs:653-657`) | `cases … <;> decide` | 4 |

### 3.2 Per-op lane truth tables

Each lemma states that the **exact composed circuit** of the
corresponding `packed5.rs` function, applied to one lane's 6 input
Bools, decodes to the F_5 result. The circuit is copied verbatim from
the production formulas (`decode5` → `*_circuit` → `encode5`), so these
are load-bearing, not generic distributors.

| # | Name | Statement (informal) | Tactic | ~LoC |
|---|------|----------------------|--------|------|
| L9 | `packed5_add_lane` | for all `(a0,a1,a2,b0,b1,b2 : Bool)`, `dec5 (encode∘add_circuit∘decode5 …) = dec5 a + dec5 b` (in `ZMod 5`) | `cases` ×6 Bools `<;> decide` (64-row table) | 6 |
| L10 | `packed5_sub_lane` | analogous, `add_circuit`→`sub_circuit`, `+`→`-` | `cases` ×6 `<;> decide` | 6 |
| L11 | `packed5_mul_lane` | analogous, `mul_circuit`, `*` | `cases` ×6 `<;> decide` | 6 |
| L12 | `packed5_neg_lane` | for `(a0,a1,a2 : Bool)`, `dec5 (encode5 [e0,e4,e3,e2,e1]) = - dec5 a` (the `neg` selector permute, `packed5.rs:494-505`) | `cases` ×3 `<;> decide` (8-row) | 4 |

The `decide` evaluator must close 64 closed `ZMod 5` equalities for each
binary op. This is the V1 pattern at one extra Bool per operand
(V1: 16 rows, D5: 64 rows). V1's `decide` on 16 rows is sub-second; 64
rows of closed `ZMod 5` arithmetic is well within `maxHeartbeats
1200000` (the value already set in `Bipedal3Correctness.lean:41`).
Fallback if `decide` is slow: `bv_decide` over the same statement, or
`Finset.forall`-style `fin_cases` (held in reserve, cf. D2 §3.5).

### 3.3 Per-op word/lane-lift lemmas

Each `*_word` lemma lifts the **exact composed bitwise expression** of
the production formula to its Bool-per-lane form, returning the
3-tuple `(b0-lane = …) ∧ (b1-lane = …) ∧ (b2-lane = …)` matching the
`Packed5 { b0, b1, b2 }` result struct. These mirror
`bipedal3_*_word` (`Bipedal3Correctness.lean:149-192`) exactly, with a
3-tuple instead of a 2-tuple, and additionally covering the 3 `!`
(BitVec `~~~`) ops in `decode5`. V1's `*_word` lemmas close with a
**bare `simp`** (`Bipedal3Correctness.lean:152,168,184,192`), whose
default simp set already includes the `BitVec.getLsbD_*` family
(`and`/`or`/`xor`/`not` are all `@[simp]` in Lean core's BitVec
lemmas); the bipedal ops happen not to use `~~~`, but the same bare
`simp` discharges it for D5 — no extra lemma name needs to be supplied
explicitly.

| # | Name | Statement (informal) | Tactic | ~LoC |
|---|------|----------------------|--------|------|
| L13 | `packed5_add_word` | the composed add circuit's three output planes lifted lane-wise | `refine ⟨?,?,?⟩ <;> simp` (`getLsbD_{and,or,not}`) | 8 |
| L14 | `packed5_sub_word` | analogous for sub | `refine ⟨?,?,?⟩ <;> simp` | 8 |
| L15 | `packed5_mul_word` | analogous for mul | `refine ⟨?,?,?⟩ <;> simp` | 8 |
| L16 | `packed5_neg_word` | analogous for neg (3-plane permute) | `refine ⟨?,?,?⟩ <;> simp` | 7 |

### 3.4 Per-op `*_correct` theorems (against the Aeneas-extracted fn)

| # | Name | Statement | Tactic | ~LoC |
|---|------|-----------|--------|------|
| L17 | `packed5_add_correct` | §1 statement | `unfold add_inherent; unfold Insts.…add; refine ⟨_, rfl, ?_⟩; show <composed circuit>; obtain ⟨h0,h1,h2⟩ := packed5_add_word …; rw [h0,h1,h2]; exact packed5_add_lane _ …` | 14 |
| L18 | `packed5_sub_correct` | §1 | analogous | 14 |
| L19 | `packed5_mul_correct` | §1 | analogous | 14 |
| L20 | `packed5_neg_correct` | §1 | analogous (single operand) | 11 |

This is the exact `*_correct` proof shape established and *already
working* in `Bipedal3Correctness.lean:227-302` — `unfold` the inherent
wrapper, `unfold` the trait-impl method, `refine ⟨_, rfl, ?_⟩`,
`show` the composed expression, `obtain` the word lemma, `rw`, then
`exact` the lane truth table. No `progress`/Result-monad branching is
needed: like the bipedal ops, the `Packed5` circuits are
`Result`-pure (pure bitwise, no error path).

### 3.5 Lift helper + bridges

| # | Name | Statement | Tactic | ~LoC |
|---|------|-----------|--------|------|
| L21 | `getLsbD_bitwise_lift3` | 3-plane analogue of `getLsbD_bitwise_lift` (`Bipedal3Correctness.lean:203`) | `exact h_lift …` | 6 |
| L22 | `packed5_lane_eq_fp5` | `Fp5_RingEquiv_ZMod5 (Packed5.lane a i) = dec5_lane a i` (cites `fpEquiv` @ P=5) | `simp [fpEquiv, …]` (bridge cite, no new content) | 8 |
| L23 | `packed5_correct_vs_canonical_F5` | §1 headline corollary | `cases op <;> simpa [Packed5.dispatch, ZMod5.dispatch] using packed5_*_correct …` | 8 |

**Total estimated file size: ≈ 200 lines** (V1's F_3 analogue is 359
lines for 20 lemmas including doc-comment blocks; D5 has 3 more lemmas
but the proof bodies are the same shape — the line delta is the wider
truth tables and the 3-tuple word lemmas).

---

## 4. Monomorphisation / extraction target

**Rust source:** `crates/gf2-algebra/src/packed/packed5.rs`. The proof
targets **inherent wrapper methods** on `Packed5`, mirroring the
proof-target convention D2 established (`Bipedal3Correctness.lean:7-13`,
`bipedal3.rs:378-468`): four `#[inline] pub fn
{add,sub,mul,neg}_inherent` that each delegate a single tail call to
`<Self as PackedField<Fp<5>>>::{add,sub,mul,neg}`.

> **Implementation prerequisite (small Rust change, NOT a D5 proof
> deliverable):** `packed5.rs` does **not yet** have the
> `{add,sub,mul,neg}_inherent` wrappers — `bipedal3.rs:409-467` has
> them, `packed5.rs` does not. The D5 implementation issue must add the
> four inherent wrappers to `packed5.rs` (verbatim copies of the
> `bipedal3.rs:408-467` pattern, delegating to the `Packed5`
> `PackedField<Fp<5>>` impl at `packed5.rs:314`) **before** the proof.
> This is the same "fixed, non-dispatch-indirected proof target"
> rationale as D2 §5 / `bipedal3.rs:380-392`. Targeting the trait
> method directly is the fallback if inherent wrappers are rejected,
> but the inherent route is strongly preferred and is what V1 proved
> against successfully.

**Charon `--start-from` / `--opaque` additions to `scripts/verify-lean.sh`
(Step 1b, the gf2-algebra extraction, currently `verify-lean.sh:107-138`):**

Currently `verify-lean.sh:120` has `--opaque 'gf2_algebra::packed::packed5'`.
D5 changes that to:

```bash
charon cargo \
  ... \
  --start-from 'gf2_algebra::packed::packed5' \   # was --opaque
  --start-from 'gf2_algebra::packed::bipedal3' \   # unchanged (D2)
  --opaque 'gf2_algebra::packed::packed7' \        # unchanged (D6 separate)
  --opaque 'gf2_core::gfp' \                       # unchanged
  ...
```

`packed5.rs` is `#[cfg(feature = "f5")]`-gated (`packed/mod.rs:27`,
`packed5.rs:49`); the extraction invocation already passes
`--no-default-features` (`verify-lean.sh:138`). The D5 implementation
issue must add `--features f5` (or the `--all-features`/explicit-feature
equivalent) to the gf2-algebra Charon invocation so `packed5` is
compiled into the LLBC. **This is a hard extraction-config requirement**
and is recorded as a risk (§6 R3).

**Feasibility verdict (F_5): tractable.** Every `Packed5` op is a
straight-line composition of `&`/`|`/`!` on three `u64`s — strictly
within the operator set Charon/Aeneas already extracts and Lean already
lifts (proven by the *working* V1 `Bipedal3Correctness.lean`, whose
ops use the identical `&&&`/`|||`/`^^^` surface plus, for D5, the BitVec
complement `~~~` (the `!b0`/`!b1`/`!b2` in `decode5`), which is covered
by the same bare-`simp` BitVec `getLsbD` simp set V1's word lemmas
already use (§3.3). No `const fn`, no `static`, no LUT, no `Vec`, no
`OnceLock`. **F_5 carries none of the F_7 LUT-extraction risk** (see D6
§4/§6). The only non-trivial item is the wider (64-row) `decide` truth
table, which is a quantitative not a structural risk.

---

## 5. Predicted Aeneas-generated def names

Following the naming pattern observed in `proofs/Gf2Algebra/Funs.lean`
for `gf2_algebra::packed::bipedal3::*` (e.g.
`packed.bipedal3.Bipedal3.add_inherent`,
`packed.bipedal3.Bipedal3.Insts.Gf2_algebraPackedPackedFieldFp3U64U128.add`,
used at `Bipedal3Correctness.lean:232-233`), the F_5 extraction will
produce, appended to the existing `proofs/Gf2Algebra/Funs.lean`:

| Rust path | Predicted Lean def name |
|-----------|-------------------------|
| `gf2_algebra::packed::packed5::Packed5` (struct) | `gf2_algebra.packed.packed5.Packed5` (`structure … where b0 b1 b2 : Std.U64`) |
| `…::decode5` | `gf2_algebra.packed.packed5.decode5` |
| `…::encode5` | `gf2_algebra.packed.packed5.encode5` |
| `…::add_circuit` | `gf2_algebra.packed.packed5.add_circuit` |
| `…::sub_circuit` | `gf2_algebra.packed.packed5.sub_circuit` |
| `…::mul_circuit` | `gf2_algebra.packed.packed5.mul_circuit` |
| `…::Packed5::add_inherent` | `gf2_algebra.packed.packed5.Packed5.add_inherent` |
| `…::Packed5::sub_inherent` | `gf2_algebra.packed.packed5.Packed5.sub_inherent` |
| `…::Packed5::mul_inherent` | `gf2_algebra.packed.packed5.Packed5.mul_inherent` |
| `…::Packed5::neg_inherent` | `gf2_algebra.packed.packed5.Packed5.neg_inherent` |
| `<Packed5 as PackedField<Fp<5>>>::add` (etc.) | `gf2_algebra.packed.packed5.Packed5.Insts.Gf2_algebraPackedPackedFieldFp5U64U128.{add,sub,mul,neg}` (exact `Insts.` mangling per the bipedal3 precedent `Bipedal3Correctness.lean:233`; the implementation issue confirms the exact suffix from the regenerated `Funs.lean`) |

The proof file is **new**: `proofs/Gf2Algebra/Proofs/Packed5Correctness.lean`
(mirroring `Bipedal3Correctness.lean`), imported via the existing
`lean_lib Gf2Algebra` entry (`proofs/lakefile.lean:23-25`) — no
`lakefile.lean` change is needed (the `Proofs/` subtree is already
covered by `srcDir := "."`). The `lake-build` strict wrapper
(`lakefile.lean:27-40`) will fail on any `sorry` in the new file, as
required by the issue's success criterion 2.

### Result-monad shape

Like the bipedal ops, every extracted `Packed5` op returns
`Result Packed5` (Aeneas convention) but is `Result`-pure (no `do`-bind
error path — pure bitwise). The `∃ r, … = ok r ∧ …` statement form
(used verbatim at `Bipedal3Correctness.lean:227-231`) is discharged by
`refine ⟨_, rfl, ?_⟩`. No `progress`, no `spec_imp_exists`, no
`FunsExternal.lean` additions (the four existing custom defs
`wrapping_neg`/`overflowing_sub`/`U128 add`/`add_assign` are not
exercised — `Packed5` never overflows; cf. D2 §7).

---

## 6. Risks

| # | Risk | Likelihood | Mitigation |
|---|------|-----------|------------|
| R1 | `decide` on the 64-row `ZMod 5` truth table times out under `maxHeartbeats` | Low | V1's 16-row `decide` is sub-second; 4× rows of closed `ZMod 5` arithmetic stays well inside the existing `maxHeartbeats 1200000`. Fallback `bv_decide` (cf. D2 §3.5) over the same closed statement. |
| R2 | Aeneas extracts `decode5`'s `[u64; 5]` selector array as an opaque `Vec`/axiom rather than a 5-tuple, breaking the `*_word` lift | Medium | `decode5` returns a **fixed-size `[u64; 5]`**, not a `Vec`. Aeneas extracts fixed arrays as `Array U64 5#usize` with `Array.make`/`Array.index_usize` (precedent: gfp primitive tables extract as `Array.make N [...]`, `Gf2Core/Funs.lean:150`). The `*_word` lemmas state the composed expression *after* index projection; if Aeneas keeps `decode5` as a separate def, add a `decode5_val` lemma (`decode5 … = Array.make 5 [e0,e1,e2,e3,e4]` by `simp`/`unfold`) — one extra lemma, +6 LoC. This is a known, bounded shape, not an extraction blocker. |
| R3 | Charon invocation does not compile `packed5` (feature `f5` off; currently `--opaque`d) | Certain-if-unchanged | The D5 implementation issue **must** flip `verify-lean.sh:120` from `--opaque` to `--start-from` and add `--features f5` to the gf2-algebra Charon line (`verify-lean.sh:138`). Recorded as a hard config step in §4. An extraction-feasibility smoke (regenerate `Funs.lean`, confirm `Packed5.add_inherent` appears non-opaque) is part of the implementation issue's first action. |
| R4 | `Insts.` trait-impl name mangling differs from the predicted `…Fp5U64U128…` suffix | Low | The exact suffix is read from the regenerated `Funs.lean` (the bipedal precedent is `…Fp3U64U128…`, `Bipedal3Correctness.lean:233`); a one-token rename in the `unfold` line. No structural impact. |
| R5 | `fix-aeneas-gf2algebra.py` axiomatises a `Packed5`-reachable `Fp<5>` instance and breaks the `packed5_lane_eq_fp5` bridge | Low | The four `*_correct` theorems are stated against `dec5` (pure `ZMod 5`), **not** `Fp<5>` — they do not project any `Fp` instance, exactly as V1's `*_correct` avoid `Fp<3>` (`Bipedal3Correctness.lean:227`). Only the *optional* §3.5 L22 bridge corollary touches `Fp<5>`; if the post-processing axiomatises it, L22 is restated as a cited axiom-consistency note (the V1 file does not even include such a corollary — L22 is additive, not load-bearing for the issue's `add/sub/mul/neg` criteria). |
| R6 | Charon "Type error after transformations" warnings on the new `packed5` extraction | Low | The existing 13 such warnings on gf2-core are benign per `MEMORY.md`; `fix-aeneas-dupes.py` already runs on the gf2-algebra `Types.lean`/`Funs.lean` (`verify-lean.sh:288`). Escalate only if a *new* hard failure surfaces. |

**Single biggest risk:** R3 (extraction-config / feature-gate) — it is
*certain* to bite if the implementation issue forgets the
`--opaque`→`--start-from` + `--features f5` flip, but it is a
mechanical one-time config change with an immediate smoke test, not a
proof-difficulty risk. The proof itself is low-risk (it is the V1
pattern with wider tables).

---

## 7. Out of scope for D5

1. **`Packed5Vec` correctness.** D5 covers the fixed-width 64-lane
   `Packed5` element only. The variable-length `Packed5Vec`
   (`packed5.rs:695`) applies the same per-`u64`-triple circuit in a
   `for w in 0..n_words` loop with `mask_tail` (`packed5.rs:717-731`);
   once the element theorem holds, the Vec form follows by `Vec`
   induction with no new bitwise content (cf. D2 §8 #1). Separate
   follow-up sketch if requested.
2. **`Packed5Matrix`.** Trivially follows from `Packed5Vec`.
3. **Mask-tail invariant.** A `Packed5Vec`-level concern; the
   single-triple `Packed5` has no tail (all 64 lanes populated).
4. **SIMD-batched `Packed5` dispatch.** Any future
   `gf2-kernels-simd`-backed path is `unsafe` and outside the
   Aeneas-supported subset (cf. D2 §8 #4).
5. **`div` / multiplicative inverse.** The `PackedField` trait surface
   is `{add,sub,mul,neg}` (`packed/mod.rs:185-246`); there is no `div`.
   The issue's criteria name exactly `add/sub/mul/neg`. F_5 inverse is
   *not* a trait method and is out of scope.
6. **F_7 `Packed7`.** Sibling sketch D6, separate document, separate
   feasibility verdict.

---

## 8. Approval

Per CLAUDE.md §Verification work, this sketch must be reviewed by the
lead and approved by the user before the D5 implementation is
dispatched. The issue `30e98ef1` success criterion 5 (`[hard]`: amend
with written justification if infeasible) is the closure contract; for
F_5 the §4 verdict is **tractable**, so the expected path is
implementation, not amendment.

Once approved, the D5 implementation issue is dispatched as: "add the
four `{add,sub,mul,neg}_inherent` wrappers to
`crates/gf2-algebra/src/packed/packed5.rs` (verbatim copies of the
`bipedal3.rs:408-467` pattern), flip `verify-lean.sh:120`
`--opaque`→`--start-from` and add `--features f5` to the gf2-algebra
Charon line, then implement the 23 lemmas listed in §3 of
`dev/plans/d5_lean_packed5_sketch.md` in the new file
`proofs/Gf2Algebra/Proofs/Packed5Correctness.lean`, against
Charon-extracted `gf2_algebra.packed.packed5.Packed5.{add,sub,mul,neg}_inherent`,
no `sorry`." The implementation issue's success criteria mirror this
sketch's lemma list verbatim.

This document does not modify `.jit/` state; transitioning `30e98ef1`
is the lead's responsibility after user sign-off.
