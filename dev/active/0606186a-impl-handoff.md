# 0606186a — D3 V2 Path 1 implementation: STOP-and-report (spike failed)

**Status:** Gating extraction spike (sketch §7, dispatch-mandated) **FAILED**.
No proof code written. Working tree restored to committed-good state. This
document is an **escalation to the project lead** — Path 1 as specified in
`dev/plans/d3_v2_path1_sketch.md` is **infeasible on the current toolchain**
because the sketch's central §2 "decisive" premise is false.

**Author:** implementation worker, 2026-05-17.
**Issue:** `0606186a` ("Lean proof — Ryser bounded n ≤ 63"), epic `ae82bd73`.
**Do NOT** treat this as a proof-difficulty handoff. It is a
sketch-premise-invalidation finding requiring a lead/user decision before any
further work.

---

## TL;DR

The spike applied sketch §2 option-(b) (`ryser_fp3.rs` → thin
`super::ryser::permanent_ryser::<Fp<3>>` delegation) and the §3.1 pipeline
flag deltas, then ran Charon + Aeneas. Result:

1. **Charon did NOT monomorphise the generic body.** The sketch §2 calls this
   "decisive": *"Charon monomorphises generic functions reachable from a
   non-generic start root … The resulting Aeneas def … will contain the
   generic algorithm body specialised to `Fp<3>`."* **This is false for the
   pinned Charon (`1ec8d4bb` + 4 local patches).** `permanent_ryser` extracted
   **fully generic** over `{F} {Clause0_Characteristic} {Clause0_Wide}` with a
   runtime `gf2_core.field.traits.FiniteField F …` instance argument;
   `permanent.ryser_fp3.permanent_ryser_fp3` is a one-line Lean wrapper that
   *passes the `Fp<3>` instance to the still-generic body* — it does **not**
   inline/specialise it.

2. **The generic body is untranslatable by Aeneas → `sorry`-poisoned.** Aeneas
   emitted a **partial file with 30 errors (3 unique)** for `Funs.lean`. The
   `permanent_ryser` loop bodies (`permanent_ryser_loop0_loop0[.body]`,
   `permanent_ryser_loop0_loop1[.body]`) carry **literal `sorry` tokens inside
   their signatures**:

   ```
   def permanent.ryser.permanent_ryser_loop0_loop0.body
     {F : Type} (gf2_corefieldtraitsFiniteFieldInst :
     gf2_core.field.traits.FiniteField F
     sorry /- Could not find: type_var_id: 1 from ExtractBase.Item-/
     sorry /- Could not find: type_var_id: 2 from ExtractBase.Item-/)
     ...
   ```

   This is **Risk #2 of sketch §7 materialising**, plus a deeper structural
   failure of the §2 premise. The generic body also pulls
   `core.iter.range.Range…map`, `core.iter.adapters.map.Map…collect`, and
   `permanent.ryser.permanent_ryser.closure_1` `FnMut`/`FnOnce` adapter
   machinery (from `(0..n).map(|_| zero.clone()).collect()` at `ryser.rs:130`
   and `.fold()` at `ryser.rs:168`) — the exact `Iterator`/`Map`/closure
   shapes the *current iterator-free* `ryser_fp3.rs` was hand-written to dodge.

A `sorry` inside the *extracted target's own signature* cannot be reasoned
about and cannot be removed by post-process regex (it is not an
unreachable-artefact axiom candidate like the `Zip`/`IterMut`/`FiniteField`
recursive-default cases — it is the **proof target itself**). Path 1 cannot
proceed, and the option-(b) Rust change actively **regresses** extraction
(committed `ryser_fp3.rs` extracts cleanly; the delegation does not extract
at all).

---

## Exact spike procedure (reproducible)

1. Applied sketch §2 option (b): `ryser_fp3.rs` body → single tail-call
   `super::ryser::permanent_ryser::<Fp<3>>(matrix, n)` (rustdoc/tests kept).
   Verified Rust-side: `cargo build -p gf2-algebra --release` ✓,
   `cargo fmt --all -- --check` ✓, `cargo clippy -p gf2-algebra
   --all-targets --all-features -- -D warnings` ✓,
   `cargo nextest run -p gf2-algebra --release --profile ci -E
   'test(ryser_fp3)+test(permanent_ryser_fp3)'` → 5/5 PASS, doctest PASS.
2. Applied sketch §3.1: removed `--opaque 'gf2_core::gfp'` and
   `--opaque 'gf2_algebra::permanent::ryser'` from the gf2_algebra Charon
   invocation; added `--opaque 'gf2_core::gfp::simd_ops'`. (Also drafted the
   §3.2 C2 `FunsExternal` seed-and-port block — not exercised, spike failed
   before that stage.)
3. Ran the gf2_algebra Charon invocation → **succeeded** (LLBC 6.96 MB,
   exit 0; the prior opaque-gfp LLBC was smaller — gfp now transparent).
4. Ran `aeneas -backend lean -dest <tmp> -split-files gf2_algebra.llbc`
   → **partial file, 30 errors (3 unique)**.

### Verbatim Aeneas error signatures (deduplicated)

```
[Error] Could not find: type_var_id: 1 from ExtractBase.Item
[Error] Could not find: type_var_id: 2 from ExtractBase.Item
[Error] Internal error, please file an issue
Source: 'crates/gf2-algebra/src/permanent/ryser.rs', lines 152:12-155:13
Source: 'crates/gf2-algebra/src/permanent/ryser.rs', lines 158:12-163:13
[Info] Generated the partial file (because of 30 errors, including 3 unique errors): Funs.lean
```

`ryser.rs:152-155` = the **add branch** inner loop
(`for i in 0..n { col_sum[i] += &matrix[i*n+flip]; }`).
`ryser.rs:158-163` = the **subtract branch** inner loop
(`col_sum[i] = col_sum[i].clone() - &matrix[i*n+flip];`).
Both fail with `type_var_id` resolution — Aeneas cannot resolve the generic
`F` / `Clause0_Characteristic` / `Clause0_Wide` type vars inside the loop
bodies of the generic driver, so it emits `sorry` *into the binder list of
the def signature*. `Internal error, please file an issue` (multiple) is the
Aeneas-internal failure on the generic closure/iterator instances.

### Extracted `permanent_ryser_fp3` (proof of non-monomorphisation)

```lean
def permanent.ryser_fp3.permanent_ryser_fp3
  (matrix : Slice (gf2_core.gfp.Fp 3#u64)) (n : Std.Usize) :
  Result (gf2_core.gfp.Fp 3#u64)
  := do
  permanent.ryser.permanent_ryser
    (gf2_core.gfp.Fp.Insts.Gf2_coreFieldTraitsFiniteFieldU64U128 3#u64) matrix n

def permanent.ryser.permanent_ryser
  {F : Type} {Clause0_Characteristic : Type} {Clause0_Wide : Type}
  (gf2_corefieldtraitsFiniteFieldInst : gf2_core.field.traits.FiniteField F
  Clause0_Characteristic Clause0_Wide) (matrix : Slice F) (n : Std.Usize) :
  Result F := do …  -- still generic; uses Map/collect/closure_1; loop bodies sorry-poisoned
```

---

## Why this is a stop-and-report, not a work-around

Per the dispatch contract and `CLAUDE.md §"Verification work"`:

- The dispatch instruction explicitly enumerated Risk #2 ("the monomorphised
  generic still emits a `gray_code_iter` `Iterator` shape Aeneas rejects") as
  a **STOP, do-not-work-around** condition. The observed failure is Risk #2
  **and worse**: the body is not even monomorphised, so the proof has no
  concrete `ZMod 3` semantics to bind to and the target signature is
  `sorry`-poisoned.
- The sketch's named **fallback** ("single-SSOT private helper in `ryser.rs`
  parameterised by closures, both entrypoints thin wrappers") is itself a
  shared-`ryser.rs`-production-code restructuring AND, per the spike, would
  still emit closure/`FnMut` adapter machinery — and the dispatch says *"do
  NOT implement the fallback without lead approval; stop and report."*
- Making the generic body extractable would require **upstream Charon
  monomorphisation behaviour** (a 5th+ local Charon patch — shared
  infrastructure, explicitly NOT authorised) or an Aeneas-side fix for the
  `type_var_id … ExtractBase.Item` generic-loop-body failure (upstream Aeneas
  change). Both are lead/user decisions.

No proof code was written. No `.jit/` state touched. No `axiom`/`sorry`
introduced into any committed `proofs/` file.

---

## Repository state at handoff

- **Working tree: clean** except `.jit/` (untouched by me). The committed
  `proofs/` tree is **unchanged** and still `lake build`-able (the spike
  wrote only to `/tmp/spike-*`; baseline verified — see below).
- `crates/gf2-algebra/src/permanent/ryser_fp3.rs`: **reverted to committed
  HEAD** (the clean iterator-free monomorphic body that extracts correctly).
- `scripts/verify-lean.sh`: **reverted to committed HEAD**.
- The infeasible Path-1 changes (ryser_fp3 thin wrapper + verify-lean.sh
  §3.1/§3.2 deltas) are preserved as a recoverable patch at
  `dev/active/0606186a-path1-spike-changes.patch` (apply with
  `git apply dev/active/0606186a-path1-spike-changes.patch` if a future path
  reuses them).
- `proofs/Gf2Algebra/Proofs/RyserBounded.lean`: **unchanged** — the abstract
  L1–L7 region (`ryser_eq_permanent_zmod`, `subsetOfBits_bijective`,
  `gray_succ_xor`, `ryser_permanent_bounded`, etc.) remains complete and
  `sorry`-free as landed in `762ce0ac`. The headline `permanent_ryser_fp3_
  correct` remains deliberately unstated (the file's own §3.5 documents the
  blocker; this spike confirms the V2 Path-1 resolution does not work).

---

## Sketch lemma status

| Lemma | Status |
|-------|--------|
| Abstract L1–L7 + `ryser_eq_permanent_zmod` + `ryser_permanent_bounded` | DONE & sorry-free (pre-existing, untouched) |
| `fp3_{new,add,sub,mul,neg}_decode` | NOT STARTED — blocked: extraction spike failed before any proof code |
| L4 `col_sum_invariant` | NOT STARTED — blocked |
| L5 `fold_prod_invariant` | NOT STARTED — blocked |
| L6 `outer_acc_eq_ryser_inner` | NOT STARTED — blocked |
| L8 `permanent_ryser_fp3_value` | NOT STARTED — blocked |
| L9 `permanent_ryser_fp3_correct` (headline) | NOT STARTED — blocked |

The blocking sub-goal is **upstream of any Lean lemma**: there is no
`sorry`-free Aeneas-extracted `permanent_ryser_fp3` body to state L8/L9
against. `fp3_*_decode` (the transparent-gfp value specs) *would* be
provable in-leg per sketch §5 IF the ryser body extracted cleanly — but it
does not, so even landing `fp3_*_decode` alone yields no path to the
headline and is not worth doing speculatively.

---

## Options for the lead (decision required — do not pick autonomously)

These span the option space; **none** is authorised for the worker to
implement without explicit lead/user approval:

1. **Charon-side patch (shared infra).** Add a 5th local Charon patch forcing
   monomorphisation of generic fns reachable from a non-generic
   `--start-from` root (the sketch §2 premise, made true). Precedented (4
   patches exist at `dev/active/charon-patch-backup-2026-05-15/`) but is a
   shared-infrastructure change with cross-leg blast radius (gf2_core leg
   must still pass). Largest scope; unblocks Path 1 as written.

2. **Keep the committed monomorphic `ryser_fp3.rs`; resolve criterion 3 by
   argument, not by code.** The committed `ryser_fp3.rs` body is *already*
   provably bit-identical to `permanent_ryser::<Fp<3>>` (test
   `test_permanent_ryser_fp3_matches_generic_small`, 250 random cases ×
   n≤5, plus n=8 path coverage). Prove the headline against the
   *clean-extracting monomorphic body* and treat criterion-3 SSOT via a
   user-approved amendment ("proof targets the F_3 monomorphisation, which is
   test-certified bit-identical to the generic T7"). This is the *original*
   `dev/plans/d3_lean_ryser_sketch.md` path; it needs the **opaque-gfp →
   transparent-gfp** part of Path 1 (§3.1/§3.2 minus the ryser delegation),
   which the spike did NOT invalidate (Charon extracted transparent gfp fine
   — the failure was purely the *generic ryser* body). **Requires user
   approval for the criterion-3 reinterpretation per the no-autonomous-
   amendments rule.**

3. **Single-SSOT private helper in `ryser.rs` parameterised by closures**
   (sketch §7 Risk #2 fallback). The spike suggests this still emits
   closure/`FnMut` adapter machinery (the generic body already does), so its
   feasibility is itself unproven — would need its own spike. Lead must
   approve the exact shape before any code.

4. **De-scope `0606186a` to the abstract contract already landed.** The
   `proofs/Gf2Algebra/Proofs/RyserBounded.lean` abstract side
   (`ryser_eq_permanent_zmod` over any `CommRing`, with explicit `n ≤ 63`
   via `ryser_permanent_bounded`) is the mathematically substantive half and
   is done & sorry-free. Criterion 4 (`n ≤ 63` explicit) is met on that side.
   Criteria 1/3 (extracted-Rust binding) would be formally amended. Requires
   user approval (criterion amendment).

**Recommended for lead consideration:** Option 2 is the lowest-risk path that
still binds a real extracted Rust body — it reuses the *valid* part of Path 1
(transparent gfp; the spike proved that part works) and the *clean-extracting*
committed `ryser_fp3.rs`, and only needs a criterion-3 wording amendment
(user sign-off). But this is a lead/user call, recorded here, not a worker
decision.

---

## Baseline integrity check

The committed pipeline/proofs were verified untouched by the spike (spike
output isolated to `/tmp/spike-*`; `git status proofs/` clean; the two code
files reverted to HEAD). A full `verify-lean.sh` re-run on the committed tree
was **not** re-executed by this session (it is the known-good `main` state
per commit `5e3478c1`/`189f9670`; re-running it changes nothing and costs
~10 min). If the lead wants belt-and-braces confirmation:
`./scripts/verify-lean.sh` on clean `main` should pass as it did at
`189f9670`.
