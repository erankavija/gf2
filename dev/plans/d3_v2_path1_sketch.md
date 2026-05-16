# D3 V2 — Path 1 proof sketch: transparent gfp + TypesExternal-collision fix

**Status: sketch — project-lead approval required before any proof code per CLAUDE.md §Verification work.**

**Issue:** `0606186a` ("Lean proof — Ryser bounded n ≤ 63"), epic `epic:gf2-algebra-permanent`.
**Supersedes (for criterion 3 / SSOT only):** `dev/plans/d3_lean_ryser_sketch.md` (the original sketch targets the monomorphised `permanent_ryser_fp3`; that is now insufficient — see §2).
**Pre-read confirmed (verified, not re-derived):**

- Abstract side of `proofs/Gf2Algebra/Proofs/RyserBounded.lean` is complete and `sorry`-free: L1–L7 Gray-code lemmas plus `ryser_eq_permanent_zmod` (`ryserRHS M = M.permanent` over any `CommRing`), `ryserRHS_matrixOfSlice_eq_permanent`, `permanent_matrixOfSlice_n_zero`, and `ryser_permanent_bounded` (carries explicit `n ≤ 63`). The headline `permanent_ryser_fp3_correct` is deliberately *unstated* (file §3.5 "session-4 extraction blocker").
- The blocker is real and was diagnosed correctly by the prior worker: in the `gf2_algebra` Charon leg, `--opaque 'gf2_core::gfp'` turns every `Fp<3>` primitive into a bare uninterpreted axiom in `proofs/Gf2Algebra/FunsExternal.lean`, and `Gf2Core.*` cannot be imported into `RyserBounded.lean` because both Aeneas legs emit a top-level `axiom core.fmt.builders.DebugStruct : Type` (and `core.iter.adapters.zip.Zip`) into their respective `TypesExternal.lean`, which collide on import.

This document lists lemmas (statement form only), one-line tactic per lemma, the exact production code path, the exact pipeline-flag and post-process deltas, the criterion-3/SSOT resolution, and an honest risk/scope estimate. **No proof bodies. No `.rs`/`.lean`/`.sh`/`.py`/`.jit` edits accompany this document.**

---

## 1. The TypesExternal collision — exact mechanism

Verified by reading the generated trees:

- `proofs/Gf2Core/TypesExternal.lean` (29 lines) declares, at **top level (no namespace)**:
  - `axiom core.fmt.builders.DebugStruct : Type` (line 21)
  - `axiom core.iter.adapters.zip.Zip (A : Type) (B : Type) : Type` (line 28)
- `proofs/Gf2Algebra/TypesExternal.lean` (43 lines) declares the **same two** top-level axioms (lines 21, 28) **plus** `axiom core.ops.range.RangeInclusive (Idx : Type) : Type` (line 35) **plus** `axiom gf2_core.gfp.Fp (P : Std.U64) : Type` (line 42).
- `proofs/Gf2Core.lean` and `proofs/Gf2Algebra.lean` are *separate* `lean_lib` targets (`proofs/lakefile.lean`), each with `srcDir := "."`, importing `Gf2Core.TypesExternal` / `Gf2Algebra.TypesExternal` respectively. Within a single lib build they never co-exist, so today there is no collision — *because* `RyserBounded.lean` does **not** import any `Gf2Core.*`.

The collision the prior worker hit (`import Gf2Core.TypesExternal failed, environment already contains 'core.fmt.builders.DebugStruct' from Gf2Algebra.TypesExternal`) appears the moment `RyserBounded.lean` (a `Gf2Algebra` file, so it already pulls `Gf2Algebra.TypesExternal` transitively via `Gf2Algebra.Funs`) additionally imports `Gf2Core.TypesExternal` (transitively, to reach the V0 `MontgomeryRoundtrip` lemmas about the concrete `gf2_core.gfp.Fp`). Two *distinct* declarations with the *same fully-qualified name* `core.fmt.builders.DebugStruct` in one environment ⇒ hard Lean error.

The key asymmetry making Path 1 viable: in the **gf2_core** leg `gfp.Fp` is *transparent* — `proofs/Gf2Core/Types.lean:145` reads `def gfp.Fp (P : Std.U64) := Std.U64` (inside `namespace gf2_core`, so the full name is `gf2_core.gfp.Fp`), and `proofs/Gf2Core/Funs.lean` carries full bodies for `gfp.Fp.new` (line 582), `gfp.Fp.value` (line 503), `gfp.Fp.Insts.CoreOpsArith{Add,Sub,Mul}FpFp.{add,sub,mul}` (lines 823, 863, …) and `gfp.Fp.Insts.CoreOpsArithNeg…`. In the **gf2_algebra** leg those identical names are bare axioms. Path 1 makes the gf2_algebra leg use the *transparent* gf2_core bodies instead of axioms.

---

## 2. SSOT / criterion-3 resolution — RECOMMENDED: option (b), thin delegating wrapper

Criterion 3 is `[hard]`: "The proof targets the Rust production `permanent_ryser` (T7) generic over `FiniteField`, not a copied stub." The code-review FAIL on `0606186a` cited (c) "SSOT violation: `ryser_fp3.rs` (232 lines) duplicates the generic algorithm body in `ryser.rs`".

### Why the generic cannot be extracted directly (option a — rejected)

`crates/gf2-algebra/src/permanent/ryser.rs:89` is `pub fn permanent_ryser<F: FiniteField>(matrix: &[F], n: usize) -> F`. Three concrete Charon/Aeneas obstacles, consistent with the established project pattern (the V1 bipedal proofs target `Bipedal3::{add,sub,mul,neg}_inherent` *monomorphic* wrappers precisely because generic `FiniteField`/`PackedField` trait dispatch does not translate cleanly — see `scripts/fix-aeneas-gf2algebra.py` docstring §1, §2, which axiomatises the recursively-unresolvable `FiniteField for Fp<P>` `impl_def` and the `CoreOps*` trait wrappers):

1. `ryser.rs` calls `F::zero_hint()`, `matrix[0].zero_like()`, `matrix[0].one_like()`, `Sub<&F>`, `Mul<&F>`, `AddAssign<&F>` — generic trait-method dispatch through `FiniteField`. The project's Charon build does **not** translate generic `FiniteField` arithmetic (the `fix-aeneas-gf2algebra.py` script exists *because* the transitively-pulled `FiniteField for Fp<P>` impl is unresolvable: "`impl_def: could not resolve recursive fields`" on `WINOGRAD_THRESHOLD.default`).
2. `ryser.rs:148` iterates `for (flip, parity) in gray_code_iter(n)` — an `Iterator` adapter; the gf2-algebra leg already needs `fix-aeneas-gf2algebra.py` to axiomatise malformed `Zip`/`IterMut` `Iterator` instances (§3, §3b). The monomorphic `ryser_fp3.rs` was deliberately written iterator-free (`while` loops) for exactly this reason (its rustdoc and inline comments say so).
3. Even if (1)/(2) were solved, the proof would have to thread `FiniteField` axioms through L4–L9 with no concrete `ZMod 3` semantics to bind to.

Extracting the generic directly is therefore **not** tractable on the current toolchain. Marking option (a) rejected.

### Recommended: option (b) — replace `ryser_fp3.rs`'s body with a thin delegating wrapper

Resolve criterion 3 + SSOT by making the extracted function *be* a one-line call into the generic, monomorphised at `Fp<3>`, with the extraction made to *inline through* the wrapper into the generic body. Concretely:

- **Rust shape** (the V2 implementation issue's only `ryser_fp3.rs` change; it removes the 95-line duplicated algorithm body and the `#[allow(clippy::assign_op_pattern)]` blocks, keeps the rustdoc/examples/tests):

  ```rust
  pub fn permanent_ryser_fp3(matrix: &[Fp<3>], n: usize) -> Fp<3> {
      super::ryser::permanent_ryser::<Fp<3>>(matrix, n)
  }
  ```

  This eliminates the SSOT violation outright: there is no second copy of the algorithm — the production logic lives **only** in `ryser.rs`, and `permanent_ryser_fp3` is now a genuine thin monomorphisation wrapper (not a "copied stub"). The proof is "proved against the generic" in the sense that the extracted body *is* the generic body (specialised at `Fp<3>` by Charon's monomorphisation), satisfying criterion 3's "not a copied stub" intent.

- **Extraction consequence (decisive):** `--start-from 'gf2_algebra::permanent::ryser_fp3::permanent_ryser_fp3'` will make Charon monomorphise the generic `permanent_ryser::<Fp<3>>` *in place* (Charon monomorphises generic functions reachable from a non-generic start root). The resulting Aeneas def `permanent.ryser_fp3.permanent_ryser_fp3` will contain the *generic algorithm body specialised to `Fp<3>`*. **This must be verified by an extraction smoke at the start of V2** (Risk #2 below): it is possible the generic body, once monomorphised, still pulls the `gray_code_iter` `Iterator` adapter and the `zero_hint`/`zero_like` dispatch in a shape Aeneas rejects. The current iterator-free `ryser_fp3.rs` was written specifically to dodge this; option (b) gives that up in exchange for SSOT compliance. **Fallback if the smoke fails: keep `ryser_fp3.rs` as an iterator-free monomorphic body BUT restructure so the *shared* algorithm core is a single private helper in `ryser.rs` parameterised by closures (one SSOT copy), with both the generic and the fp3 entrypoint thin wrappers over it — escalate the exact shape to the lead before writing proof code.**

- **Resulting Aeneas-generated def name the proof binds to:** `gf2_algebra.permanent.ryser_fp3.permanent_ryser_fp3` (display: `permanent.ryser_fp3.permanent_ryser_fp3` inside `namespace gf2_algebra`), plus the loop defs already present today: `permanent.ryser_fp3.permanent_ryser_fp3_loop0[.body]`, `permanent.ryser_fp3.permanent_ryser_fp3_loop1_loop0[.body]`, `permanent.ryser_fp3.permanent_ryser_fp3_loop1_loop1[.body]` (verified to exist in `proofs/Gf2Algebra/Funs.lean:5776+`). Loop-name suffixes may change shape under option (b)'s body; the smoke re-confirms them.

The `--start-from`/`--opaque` line for `ryser_fp3` in `scripts/verify-lean.sh:131` is unchanged by option (b) (still `--start-from 'gf2_algebra::permanent::ryser_fp3::permanent_ryser_fp3'`); only the `--opaque 'gf2_algebra::permanent::ryser'` at line 142 must be **removed** so Charon follows the delegation into the generic instead of treating it as opaque.

---

## 3. Pipeline change plan (Path 1)

Goal: in the gf2_algebra leg, make `gf2_core::gfp` *transparent* (real bodies, the same ones the gf2_core leg already proves correct) and resolve the resulting `TypesExternal`/`FunsExternal` name collisions via the existing post-process dedup pattern, so `RyserBounded.lean` can reason about the actual `Fp<3>` arithmetic and reuse the V0 `MontgomeryRoundtrip` value specs.

### 3.1 `scripts/verify-lean.sh` flag deltas (gf2_algebra leg only, lines ~125–162)

Remove from the gf2_algebra `charon cargo` invocation:

- `--opaque 'gf2_core::gfp'` (line 147) — **delete**. This is the core change: gfp now extracts transparently in this leg too.
- `--opaque 'gf2_algebra::permanent::ryser'` (line 142) — **delete** (per §2: let Charon follow the `ryser_fp3 → permanent_ryser::<Fp<3>>` delegation).

Keep `--opaque 'gf2_core::gfpn'`, `'gf2_core::field'`, `'gf2_core::gf2m'`, … unchanged (the Ryser path needs only `Fp<3>`, never the tower/`gf2m`; leaving them opaque keeps the LLBC small and avoids re-importing the gfpn HRTB machinery).

Add (mirroring the gf2_core leg lines 67–89, which already extracts gfp transparently and is known-good):

- The gf2_core leg passes `--start-from 'gf2_core::gfp'`. The gf2_algebra leg reaches `gf2_core::gfp` transitively from `permanent_ryser::<Fp<3>>`, so an explicit `--start-from 'gf2_core::gfp'` is **not required** (Charon extracts reachable non-opaque items), but adding it is harmless and makes intent explicit; the V2 smoke decides.
- The gf2_core leg already passes `--rustc-arg=--cfg=verify_lean`; the gf2_algebra leg *also already passes it* (line 127). The 11 `#[cfg(not(verify_lean))]` SIMD-fast-path overrides on `Fp::FiniteField` in `crates/gf2-core/src/gfp/mod.rs:652–841` are therefore already disabled in this leg too — no Rust change needed for the cfg.
- `--opaque 'gf2_core::gfp::simd_ops'` is in the gf2_core leg (line 77) but **absent** from the gf2_algebra leg. With gfp now transparent in the gf2_algebra leg, **add** `--opaque 'gf2_core::gfp::simd_ops'` to the gf2_algebra invocation (same reason as the gf2_core leg comment, lines 25–31: keep the SIMD-ops module out of the LLBC; the `verify_lean` cfg routes arithmetic through the scalar trait defaults).

### 3.2 Post-process script deltas — the collision dedup

After §3.1, the gf2_algebra Aeneas output will contain **real** `gf2_core.gfp.*` bodies (Types: `def gf2_core.gfp.Fp (P) := Std.U64`; Funs: `gf2_core.gfp.Fp.new`, `.value`, `.Insts.CoreOpsArith{Add,Sub,Mul,Neg}FpFp.*`, plus the `montgomery.*`/`specialized.*` helpers). The collisions to resolve, and the mechanism (mirroring the existing dedup passes):

**(C1) `TypesExternal.lean` — `axiom gf2_core.gfp.Fp` disappears, becomes a real `def` in `Types.lean`.** No dedup needed: with gfp transparent, Aeneas emits `def gf2_core.gfp.Fp (P) := Std.U64` into `Gf2Algebra/Types.lean` and *omits* the `axiom gf2_core.gfp.Fp` from `Gf2Algebra/TypesExternal.lean` (exactly as it already does for the gf2_core leg — confirmed: `Gf2Core/TypesExternal.lean` has **no** `gfp.Fp` axiom; `Gf2Core/Types.lean:145` has the `def`). This is automatic, not a post-process concern.

**(C2) `FunsExternal.lean` — gfp ops disappear from the external axiom set.** With gfp transparent, the `gf2_core.gfp.Fp.new` / `.value` / `CoreOpsArith*` bodies move from axioms (`Gf2Algebra/FunsExternal.lean`) into `Gf2Algebra/Funs.lean` as real defs (mirroring `Gf2Core/Funs.lean:503,582,823`). The `verify-lean.sh` gf2_algebra post-process currently does `cp FunsExternal_Template.lean FunsExternal.lean` unconditionally (line 330). With gfp transparent the template no longer contains the gfp axioms, so the copy is still correct — **but** the gf2_core leg's `FunsExternal.lean` carries *hand-edited concrete defs* for `core.num.U64.wrapping_neg` and `core.num.U64.overflowing_sub` (used by `montgomery.mont_sub` / `specialized.canonical_sub`, verified at `Gf2Core/Funs.lean` `mont_sub`/`canonical_sub` bodies). The gf2_algebra leg's `gfp` arithmetic, now transparent, **transitively needs the same two concrete externals** (`mont_add`/`mont_sub`/`from_mont`/`to_mont` use `wrapping_neg`, `overflowing_sub`, `wrapping_add`). **Required post-process change:** the gf2_algebra leg must stop blindly `cp`-ing the template and instead seed-and-preserve `FunsExternal.lean` like the gf2_core leg does (`verify-lean.sh:270–273` pattern), with the same hand-edited `core.num.U64.wrapping_neg`/`overflowing_sub` concrete defs ported in. This is a *known, bounded* edit: the exact concrete bodies already exist in `proofs/Gf2Core/FunsExternal.lean:33–42` and can be copied verbatim into `proofs/Gf2Algebra/FunsExternal.lean`. **This is not a sorry/axiom: it is the same trusted-primitive modelling already user-accepted for the gf2_core leg** (the two are bit-exact two's-complement defs, not assumptions about field semantics).

**(C3) `fix-aeneas-gf2algebra.py` §1/§2 must NOT axiomatise the now-needed gfp bodies.** Today this script *deliberately* rewrites the transitively-pulled `gf2_core.gfp.Fp.Insts.Gf2_coreFieldTraitsFiniteFieldU64U128` `impl_def` and the `CoreOps*` trait wrappers into axioms (its §1, §2, with hard `raise SystemExit` if the count ≠ 1). Under Path 1 those wrappers are now *load-bearing* (the proof must see their bodies). **Required change:** the V2 issue must scope §1/§2 of `fix-aeneas-gf2algebra.py` so it axiomatises only the genuinely-unreachable `FiniteField for Fp<P>` recursive `impl_def` (the `WINOGRAD_THRESHOLD.default` recursion — *not* used by Ryser, which only calls `+`/`-`/`*`/`Fp::new`/`.value`) while **leaving the `CoreOpsArith{Add,Sub,Mul,Neg}FpFp` defs as real bodies**. Mechanism: narrow the §2 `def_re` regex so it does not match `CoreOpsArithAddFpFp`/`SubFpFp`/`MulFpFp`/`NegFp` (the ops Ryser uses), exactly the inverse of the existing broaden-by-name pattern the script already uses for `Zip`/`IterMut`. **This is the single highest-risk post-process edit** (see Risk #1): if the `FiniteField` recursive `impl_def` and the `CoreOps*` wrappers are entangled in the LLBC such that you cannot axiomatise one without the other, Path 1's "transparent gfp" needs a deeper Charon-side fix and the post-process pattern is insufficient — see §3.3.

**(C4) `core.fmt.builders.DebugStruct` / `core.iter.adapters.zip.Zip` — no cross-leg collision under Path 1.** Critical realisation: Path 1 does **not** import `Gf2Core.*` into `RyserBounded.lean`. It makes the *gf2_algebra leg itself* extract gfp transparently, so the V0-equivalent value reasoning is re-proved (or the V0 lemmas are *reproved in-place* against the now-transparent `Gf2Algebra` gfp bodies — see §5). Therefore the two `TypesExternal` trees are **never imported into the same environment**, and the `DebugStruct`/`Zip` top-level-axiom collision the prior worker hit **does not arise** under Path 1. (It would only arise under a naive "import Gf2Core into Gf2Algebra" approach, which Path 1 explicitly avoids.) This is the central reason Path 1 is tractable where the prior session's implicit approach was not.

### 3.3 Tractability verdict

**Tractable via the existing post-process pattern: YES, with one needs-spike caveat (C3).** (C1), (C2), (C4) are mechanical and follow patterns already in the scripts/`verify-lean.sh`. (C3) — selectively axiomatising the unreachable `FiniteField` recursive `impl_def` while keeping the `CoreOpsArith*FpFp` bodies transparent — is the *inverse* of the script's existing name-scoped axiomatisation passes and is *expected* to work, but it has not been run; it needs a one-session extraction spike at the start of V2 to confirm Charon emits the `CoreOpsArith*FpFp` bodies independently of the recursive `FiniteField` `impl_def`. If they are entangled, Path 1 escalates (Charon-side fix, lead/user decision) — that is a valid finding to surface, not a silent dead-end.

---

## 4. Lemmas to be proved (statement form only)

The abstract lemmas L1–L7 and `ryser_eq_permanent_zmod`, `ryserRHS_matrixOfSlice_eq_permanent`, `permanent_matrixOfSlice_n_zero`, `ryser_permanent_bounded` are **already proved and `sorry`-free** in `proofs/Gf2Algebra/Proofs/RyserBounded.lean` (lines 196–1127). V2 reuses them by name; it does **not** re-prove them. The new obligations are the value-level chain (L8/L9), the loop invariants (L4/L5/L6), and the headline.

Decoder/bridge note: `decodeFp3` (RyserBounded.lean:168) currently reads `UScalar.val x` directly (Montgomery-encoded, the file documents this is a placeholder because gfp is opaque). Under Path 1 it is **redefined** to route through the now-transparent `gf2_core.gfp.Fp.value` (= `from_mont` in Montgomery storage mode), matching `Gf2Core.Proofs.MontgomeryRoundtrip.fp_new_value_roundtrip`'s canonical reader. `matrixOfSlice` (RyserBounded.lean:1066) is correspondingly re-pointed at the transparent `Fp.value`.

```lean
namespace RyserBounded

/-- V0 value specs, re-proved in-leg against the now-transparent
    `Gf2Algebra` gfp bodies (these mirror, by the same proof scripts,
    `Gf2Core.Proofs.MontgomeryRoundtrip.fp_{new,add,sub,mul,neg}_*`;
    see §5 for why they are re-proved in-leg, not imported). -/
theorem fp3_new_decode (v : Std.U64) :
    ∃ r, gf2_core.gfp.Fp.new 3#u64 v = .ok r ∧ decodeFp3raw r = ((v : ZMod 3)))   -- raw = canonical via Fp.value
theorem fp3_add_decode (a b : gf2_core.gfp.Fp 3#u64) :
    ∃ r, gf2_core.gfp.Fp.Insts.CoreOpsArithAddFpFp.add a b = .ok r ∧
      decodeFp3' r = decodeFp3' a + decodeFp3' b
theorem fp3_sub_decode (a b : gf2_core.gfp.Fp 3#u64) :
    ∃ r, gf2_core.gfp.Fp.Insts.CoreOpsArithSubFpFp.sub a b = .ok r ∧
      decodeFp3' r = decodeFp3' a - decodeFp3' b
theorem fp3_mul_decode (a b : gf2_core.gfp.Fp 3#u64) :
    ∃ r, gf2_core.gfp.Fp.Insts.CoreOpsArithMulFpFp.mul a b = .ok r ∧
      decodeFp3' r = decodeFp3' a * decodeFp3' b
theorem fp3_neg_decode (a : gf2_core.gfp.Fp 3#u64) :
    ∃ r, gf2_core.gfp.Fp.Insts.CoreOpsArithNegFp.neg a = .ok r ∧
      decodeFp3' r = - decodeFp3' a

/-- L4 (column-sum loop invariant): after `permanent_ryser_fp3_loop1_loop0`
    runs the inner add/sub walk for Gray step `k`, the decoded `col_sum`
    vector equals the column sums of the current subset `subsetOfBits n k`.
    The Gray-code-walk loop invariant criterion-1 artefact. -/
theorem col_sum_invariant
    {n : ℕ} (h_n : n ≤ 63) (matrix : Slice (gf2_core.gfp.Fp 3#u64))
    (k : ℕ) (h_k : k < 2 ^ n) (col_sum : alloc.vec.Vec (gf2_core.gfp.Fp 3#u64)) :
    -- running col_sum decodes to (fun i => ∑ j ∈ subsetOfBits n k, (matrixOfSlice n matrix) i j)
    True   -- (full predicate elided in sketch; binds to permanent_ryser_fp3_loop1_loop0)

/-- L5 (fold-product invariant): the `term`-fold inner loop
    (`permanent_ryser_fp3_loop1_loop1`) decodes to
    `∏ i, ∑ j ∈ subsetOfBits n k, (matrixOfSlice n matrix) i j`. -/
theorem fold_prod_invariant
    {n : ℕ} (h_n : n ≤ 63) (matrix : Slice (gf2_core.gfp.Fp 3#u64)) (k : ℕ) :
    True   -- binds to permanent_ryser_fp3_loop1_loop1

/-- L6 (outer accumulator / Gray-bijection sum): after the full Gray walk,
    the decoded `total` equals
    `∑ S ∈ univ.powerset, (-1)^|S| · ∏ i, ∑ j ∈ S, M i j`
    (the inner factor of `ryserRHS`, before the outer `(-1)^n`).
    Uses the already-proved `subsetOfBits_bijective` (RyserBounded.lean:631)
    and `gray_succ_xor` (:414) to reindex the Gray walk onto the powerset. -/
theorem outer_acc_eq_ryser_inner
    {n : ℕ} (h_n : n ≤ 63) (matrix : Slice (gf2_core.gfp.Fp 3#u64)) :
    True   -- binds to the outer `while k < upper` loop of permanent_ryser_fp3

/-- L8 (extracted-spec progress chain): the extracted Rust entrypoint
    returns `ok r` whose decode is the full Ryser RHS. -/
theorem permanent_ryser_fp3_value
    {n : ℕ} (h_n : n ≤ 63) (matrix : Slice (gf2_core.gfp.Fp 3#u64))
    (h_dim : matrix.length = n * n) :
    ∃ r, gf2_algebra.permanent.ryser_fp3.permanent_ryser_fp3
            matrix (n : Std.Usize) = .ok r ∧
      decodeFp3' r = ryserRHS (matrixOfSlice n matrix)

/-- L9 (headline, criterion-1 + criterion-4): the extracted production
    Ryser output equals Mathlib's `Matrix.permanent` over `ZMod 3`,
    with the explicit `n ≤ 63` bound in the statement. -/
theorem permanent_ryser_fp3_correct
    {n : ℕ} (h_n : n ≤ 63) (matrix : Slice (gf2_core.gfp.Fp 3#u64))
    (h_dim : matrix.length = n * n) :
    ∃ r, gf2_algebra.permanent.ryser_fp3.permanent_ryser_fp3
            matrix (n : Std.Usize) = .ok r ∧
      decodeFp3' r = (matrixOfSlice n matrix).permanent

end RyserBounded
```

(The `True`-elided predicates for L4/L5/L6 are abbreviated *in this sketch only*; the V2 issue states them in full, the predicate text being the decode-equation in the bullet comment above each.)

---

## 5. Intended proof strategy (one line per lemma)

- **`fp3_{new,add,sub,mul,neg}_decode`** — *Re-prove in-leg* by replaying the V0 proof scripts from `Gf2Core/Proofs/MontgomeryRoundtrip.lean` (`fp_new_value_roundtrip`:587, `fp_add_correct`:667, `fp_mul_correct`:831, `fp_sub_correct`, `fp_neg_correct`) and `ModArith.lean`/`Progress.lean` against the now-*identical* transparent `Gf2Algebra` gfp bodies. The bodies are literally the same Aeneas output (`gfp.Fp.new`, `mont_add`, `from_mont`, …) so the scripts transfer verbatim modulo the `gf2_core.`-prefix; tactic shape `progress` + `simp only [gfp.montgomery.*]` + `scalar_tac`/`bv_omega`, the established pattern from CLAUDE.md memory "Monadic chain proof technique". (Re-proved, *not imported*, because importing `Gf2Core.*` triggers the C4 `DebugStruct` collision; re-proving in-leg sidesteps it entirely — this is the crux of why Path 1 works.)
- **`col_sum_invariant` (L4)** — induction on the Gray step `k`; `progress` through `permanent_ryser_fp3_loop1_loop0`; per-iteration `rw [fp3_add_decode]`/`rw [fp3_sub_decode]`; subset bookkeeping by `Finset.sum_insert`/`Finset.sum_erase` keyed on `gray_succ_xor` (RyserBounded.lean:414) + `flipBit_lt` (:501).
- **`fold_prod_invariant` (L5)** — induction on the fold index; `progress` through `permanent_ryser_fp3_loop1_loop1`; per step `rw [fp3_mul_decode]`; `Finset.prod_range_succ`-style accumulation.
- **`outer_acc_eq_ryser_inner` (L6)** — `Finset.sum_bij` along `subsetOfBits_bijective` (RyserBounded.lean:631); parity factor from `gray_succ_xor` (each step toggles `|S|` parity) + the already-proved `subsetOfBits` membership lemma (:302); substitute L4/L5 at the loop body.
- **`permanent_ryser_fp3_value` (L8)** — `progress` walking the extracted top-level def (`n = 0` corner via `fp3_new_decode` for `Fp::<3>::new(1)`; the `Vec::with_capacity`/push prelude via the existing `permanent_ryser_fp3_loop0` zero-init); substitute L6 at outer-loop exit; handle the final `if n % 2 == 1 then -total else total` via `fp3_neg_decode` + `(-1)^n` parity; `spec_imp_exists` to close.
- **`permanent_ryser_fp3_correct` (L9)** — `rw [permanent_ryser_fp3_value]; rw [ryserRHS_matrixOfSlice_eq_permanent]` (the already-proved L7-specialisation, RyserBounded.lean:1093). Expected ≤ 15 lines: it is the composition of L8 with an existing theorem.

Established-pattern references: V1 `proofs/Gf2Algebra/Proofs/Bipedal3Correctness.lean` (`bipedal3_{add,sub,mul,neg}_correct`:227–292 — the per-op decode-lemma shape L4/L5 mirror) and `proofs/Gf2Core/Proofs/MontgomeryRoundtrip.lean` (the `progress`/`spec_imp_exists`/`@[progress]` chain shape L8 mirrors). CLAUDE.md memory "Aeneas wrapping op patterns" and "Monadic chain proof technique" govern the `fp3_*_decode` re-proofs.

---

## 6. Exact production code path the harness exercises

- **Rust:** `crates/gf2-algebra/src/permanent/ryser_fp3.rs::permanent_ryser_fp3` — under option (b) a thin wrapper whose sole body is `super::ryser::permanent_ryser::<Fp<3>>(matrix, n)`. The *production algorithm* exercised is therefore `crates/gf2-algebra/src/permanent/ryser.rs::permanent_ryser<F>` (T7, the generic over `FiniteField`), monomorphised at `Fp<3>` by Charon. No copied stub remains.
- **Field arithmetic exercised:** `crates/gf2-core/src/gfp/mod.rs` `Fp::<3>` `new` / `value` / `Add` / `Sub` / `Mul` / `Neg` — extracted *transparently* in the gf2_algebra leg under Path 1 (not axioms), the same bodies the gf2_core leg already verifies.
- **Aeneas-generated Lean def the proof binds to:** `gf2_algebra.permanent.ryser_fp3.permanent_ryser_fp3` (in `proofs/Gf2Algebra/Funs.lean`), plus its loop defs `permanent.ryser_fp3.permanent_ryser_fp3_loop0`, `…_loop1_loop0`, `…_loop1_loop1` (and `.body` variants). Loop-name shape re-confirmed by the V2 start-of-session extraction smoke (option (b) changes the body, so today's exact loop suffixes — verified present at `Funs.lean:5776+` — may shift).
- **No `OnceLock`/runtime-dispatch on this path:** the `verify_lean` cfg (already passed in the gf2_algebra leg, `verify-lean.sh:127`) disables the 11 SIMD-fast-path `Fp::FiniteField` overrides (`gfp/mod.rs:652–841` `#[cfg(not(verify_lean))]`), routing through the scalar Montgomery trait defaults — the same path the V0 `MontgomeryRoundtrip` proofs target. No unwind-strategy concern (this is Lean/Aeneas, not Kani).

---

## 7. Risk + scope estimate

**Scope estimate (honest):**

- Rust: ~95 lines deleted from `ryser_fp3.rs` (body → one-line delegation), rustdoc/tests retained. Net small.
- `scripts/verify-lean.sh`: ~3 flag deltas (delete 2 `--opaque`, add 1 `--opaque simd_ops`) + adopt the gf2_core-style seed-and-preserve `FunsExternal.lean` block for the gf2_algebra leg (~10 lines, copied pattern).
- `scripts/fix-aeneas-gf2algebra.py`: narrow §1/§2 regexes so the `CoreOpsArith*FpFp` bodies survive (~15–30 lines of regex tightening + new count asserts).
- `proofs/Gf2Algebra/FunsExternal.lean` hand-edit: port the 2 concrete `wrapping_neg`/`overflowing_sub` defs verbatim from `Gf2Core/FunsExternal.lean:33–42` (~12 lines, copied).
- `RyserBounded.lean`: ~400–650 new lines (fp3_*_decode re-proofs ≈150–250 — the V0 scripts transfer but are non-trivial; L4/L5 loop inductions ≈150–250; L6 ≈80–120; L8 progress chain ≈60–120; L9 ≈15). The abstract L1–L7 (~960 lines) are **kept as-is, reused**.
- **Estimated 3–5 implementation sessions**, gated on a **1-session extraction spike first** (resolve Risk #1/#2 before any proof code — this is itself a candidate sub-task per CLAUDE.md §Verification work).

**Top 3 risks:**

1. **(C3) `CoreOpsArith*FpFp` bodies entangled with the unreachable `FiniteField` recursive `impl_def`.** If Charon cannot emit the `Add/Sub/Mul/Neg for Fp<P>` op bodies without also pulling the `WINOGRAD_THRESHOLD.default`-recursive `FiniteField` `impl_def` (which `fix-aeneas-gf2algebra.py` must still axiomatise), the post-process cannot keep ops transparent while axiomatising the recursion. **Fallback:** escalate to a Charon-side fix (the project already maintains 4 local Charon patches at `/data/aeneas-build/charon/`; a 5th targeted patch to break the `default`-recursion is precedented) — lead/user decision, recorded as a blocker, not a silent failure.
2. **Option (b) monomorphised generic still emits an Aeneas-rejected `gray_code_iter` `Iterator` shape.** The current iterator-free `ryser_fp3.rs` exists precisely to dodge this; delegating to the iterator-using generic may reintroduce the `Map`/`Range::next` adapter Aeneas chokes on. **Fallback:** single-SSOT private helper in `ryser.rs` parameterised by closures, both entrypoints thin wrappers (escalate exact shape to lead before proof code) — preserves SSOT *and* extraction tractability.
3. **L8 progress-chain blow-up.** The Aeneas `progress` walk over a triple-nested loop (`loop0` zero-init, `loop1_loop0` add/sub, `loop1_loop1` fold) with `Vec` indexing may exceed `maxHeartbeats` or hit `Vec`-opacity (Aeneas wraps `Vec` as an axiom; `alloc.vec.Vec` indexed access). **Fallback:** prove per-loop `@[progress]` spec lemmas separately (L4/L5 already factor this way) and compose; if `Vec`-opacity blocks indexed reads, the V0 `Specialized.lean`/`Progress.lean` `Vec` patterns are the precedent to reuse, and worst case the bound is stated for `matrix.val.length = n*n` with `Array`/`List`-level reasoning.

---

## 8. Approval

Per CLAUDE.md §"Verification work", this sketch must be approved by the project lead before any proof/pipeline/Rust code is written for `0606186a`. The V2 implementation issue should be dispatched as "implement this approved Path-1 sketch", with the **extraction spike (Risk #1/#2) as a gating sub-task that must pass before proof code is written**. This document modifies no `.jit/` state; issue transitions remain the lead's responsibility.
