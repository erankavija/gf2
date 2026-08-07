# 0606186a — D3 V2 Option 2 implementation: STOP-and-report (C3 spike failed)

**Status:** Gating C3 extraction spike (sketch §9 / §3.2 C3, dispatch-mandated)
**STOPPED with definitive negative evidence.** No proof code written. Working
tree restored byte-identical to committed HEAD. This document is an
**escalation to the project lead** — Option 2 as specified in
`dev/plans/d3_v2_path1_sketch.md` §9 cannot proceed on the pinned toolchain
because the transparent-gfp prerequisite (§3.1, retained from Path 1, asserted
spike-proven by the prior Path-1 spike) is **false for the gf2_algebra leg**.

**Author:** implementation worker, 2026-05-17.
**Issue:** `0606186a` ("Lean proof — Ryser bounded n ≤ 63"), epic `ae82bd73`.
**Charon:** `0.1.196` (the installed binary; note `verify-lean.sh` header says
pinned `1ec8d4bb`+patches — the installed `charon`/`aeneas` on PATH report
`0.1.196`). This is itself a discrepancy the lead should note, but the spike
result below is mechanism-level and not version-sensitive.

---

## TL;DR

The dispatch said: *"Risk #2 is eliminated (the committed iterator-free body
has no Map/collect/closure machinery — the spike already extracted it cleanly
in the pre-Path-1 baseline)"* and *"Gating spike for Option 2 = C3 only:
confirm `fix-aeneas-gf2algebra.py` can keep `CoreOpsArith{Add,Sub,Mul,Neg}
FpFp` bodies transparent while still axiomatising the unreachable
`WINOGRAD_THRESHOLD.default`-recursive `FiniteField` `impl_def`."*

**The C3 question is moot, because its premise fails one layer upstream:**
in the `gf2_algebra` Charon leg the `gf2_core::gfp` arithmetic bodies are
**NOT extracted transparently in the first place**, regardless of removing
`--opaque 'gf2_core::gfp'`. There are therefore no transparent
`CoreOpsArith*FpFp.{add,sub,mul,neg}` / `Fp.{new,value}` bodies for the
post-process to "keep transparent" or "narrow the regex around" — they are
Aeneas **external `axiom`s** with no body, exactly as in the committed
opaque-gfp pipeline.

Sketch §3.1 explicitly hedged this: *"an explicit `--start-from
'gf2_core::gfp'` is **not required** (Charon extracts reachable non-opaque
items), but adding it is harmless … the V2 smoke decides."* **The smoke has
decided: it is required AND insufficient.** Neither removing the `--opaque`
nor adding `--start-from 'gf2_core::gfp'` makes the foreign-crate bodies
transparent. The only Charon flag that does (`--extract-opaque-bodies`)
fatally panics rustc's metadata decoder on the std/core foreign surface and
is uncontrollable. Making this work needs an **upstream Charon change or a
5th local Charon patch — SHARED INFRASTRUCTURE, explicitly NOT
worker-authorised** per the dispatch and `CLAUDE.md §"Verification work"`.

---

## Exact spike procedure (reproducible)

Applied the sketch §9 / §3.1 transparent-gfp deltas to `scripts/verify-lean.sh`
(gf2_algebra leg only): deleted `--opaque 'gf2_core::gfp'`, added
`--opaque 'gf2_core::gfp::simd_ops'`, **kept** `--opaque
'gf2_algebra::permanent::ryser'` (per §9). Also drafted the §3.2 C2
seed-and-port `FunsExternal.lean` block. `ryser_fp3.rs` NOT touched (per §9).

Ran the gf2_algebra Charon invocation (+ Aeneas) three ways:

### Run A — §3.1 deltas exactly as written (no explicit gfp start root)

```
charon cargo --preset aeneas --rustc-arg=--cfg=verify_lean \
  --start-from 'gf2_algebra::permanent::ryser_fp3::permanent_ryser_fp3' \
  …(other roots)… \
  --opaque 'gf2_core::gfp::simd_ops' --opaque 'gf2_core::gfpn' …
  -- --manifest-path crates/gf2-algebra/Cargo.toml --no-default-features --features f5,f7
```

* Charon: **exit 0**, LLBC 6.82 MB (vs sketch §9's predicted ~6.96 MB).
* Aeneas: exit 0, partial file, **22 errors / 2 unique** (the *pre-existing*
  baseline — `Bipedal3*`/`Packed5*`/`Packed7*` `Debug::fmt`/`CmpEq`/
  `from_row_major`/`row` sorrys that `fix-aeneas-gf2algebra.py` +
  `fix-aeneas-dupes.py` clean; documented at `verify-lean.sh:301`).
* `permanent.ryser_fp3.permanent_ryser_fp3` + all 6 loop defs
  (`_loop0[.body]`, `_loop1[.body]`, `_loop1_loop0..3[.body]`):
  **PRESENT and `sorry`-free** (region had ZERO `sorry` tokens). Good.
* **BUT** `gf2_core.gfp.Fp.new`, `.value`,
  `CoreOpsArith{Add,Sub,Mul,Neg}FpFp.{add,sub,mul,neg}`,
  `montgomery.{mont_add,from_mont,to_mont,redc}` are emitted as
  **bare `axiom`s in `FunsExternal_Template.lean`** (lines 586–632),
  with NO body — i.e. **still opaque**, identical to the committed
  opaque-gfp pipeline. `grep -c '^def gf2_core.gfp.Fp.new …'
  Funs.lean` → **0**. The instance-dictionary records
  (`CoreOpsArithAddFpFp (P) := { add := … }`) ARE transparent in
  `Funs.lean`, but they point at the axiom bodies — so semantically the
  arithmetic is still uninterpreted. `gf2_core.gfp.Fp` is still
  `axiom … : Type` in `TypesExternal_Template.lean:42`.

### Run B — add explicit `--start-from 'gf2_core::gfp'` (sketch §3.1 "harmless, makes intent explicit")

Identical result: gfp arithmetic still emitted as `axiom`s in
`FunsExternal_Template.lean`; ZERO transparent `do`-bodied gfp arith defs in
`Funs.lean`. LLBC size essentially unchanged (6.824 MB). **`--start-from` does
not override the foreign-body skip.**

### Run C — add `--extract-opaque-bodies` (the only Charon flag that targets the foreign-body skip)

`charon cargo … --extract-opaque-bodies …` (with and without
`--hide-marker-traits`, `-Zalways-encode-mir` on the gf2-core dep, and
`--exclude 'core::slice::sort'/'core::iter::adapters'/'core::iter::range'`):

```
thread 'rustc' panicked at .../rustc_metadata/src/rmeta/decoder/cstore_impl.rs:226:1
warning: Hax panicked when translating `core::iter::adapters::zip::{impl#5}`.
thread 'rustc' panicked at rustc_trait_elaboration/src/item_ref.rs:168:25
warning: Thread panicked when extracting body.
thread 'rustc' panicked at .../rustc_query_impl/src/plumbing.rs:390:5
… charon-driver exit status: 101 … NO LLBC produced
```

`--extract-opaque-bodies` is a **global, unscoped boolean** (Charon
`0.1.196 --help`: *"Usually we skip the bodies of foreign methods and structs
with private fields. When this flag is on, we don't"* — no name-pattern
argument). It does not skip std/core, so it tries to translate
`core::iter::adapters::zip`, `core::iter::range`, `core::slice::sort::stable`,
etc., and **fatally panics rustc's rmeta MIR decoder** because the dependency
crates' MIR is not in the rmeta artefact. `--exclude`, `-Zalways-encode-mir`
on the gf2-core dep, and `--hide-marker-traits` do **not** stop the panic
(the panicking items are transitively reachable std `core::iter`/`Vec`
machinery the gfp+`alloc::vec::Vec` code pulls in; they cannot be excluded
without also excluding the code under test).

---

## Root-cause mechanism (verified against Charon 0.1.196 `--help`)

In the **gf2_core leg**, `gf2-core` IS the *current crate*
(`--manifest-path crates/gf2-core/Cargo.toml`); Charon translates its bodies
fully — that is why `proofs/Gf2Core/Funs.lean` has transparent
`def gfp.Fp.new`, `def gfp.Fp.value`, `def gfp.Fp.Insts.CoreOpsArith*FpFp.*`,
`def gfp.montgomery.mont_add`, etc. (verified: 4+ such transparent defs).

In the **gf2_algebra leg**, `gf2-core` is a **foreign (path-dependency)
crate** (`gf2-core = { path = "../gf2-core" }`, built `--emit=…,metadata,link`
→ rmeta). Charon's documented behaviour (`--mir … only relevant for the
current crate; for dependencies only MIR optimized is available`; and the
`--extract-opaque-bodies` doc *"Usually we skip the bodies of foreign
methods…"*) is to **skip foreign-method bodies by default**. The
`--opaque`/`--include`/`--start-from` opacity-rule layer is **downstream of**
this foreign-body skip; removing `--opaque 'gf2_core::gfp'` only affects the
opacity whitelist, not the foreign-body skip, so the gfp bodies remain
external axioms. This is the *same class of falsified-premise* finding as the
prior Path-1 spike (non-monomorphisation of the generic) — the sketch assumed
a Charon behaviour that does not hold for a dependency crate.

**Decisive evidence (reproducible):**

| | gf2_core leg (committed) | gf2_algebra leg (Option-2 spike) |
|---|---|---|
| `gf2_core::gfp` role | current crate | foreign dependency crate |
| `def gfp.Fp.new` / `.value` / `CoreOpsArith*FpFp.*` bodies | **transparent `def` in `Funs.lean`** (4+) | **`axiom` in `FunsExternal_Template.lean`** (0 transparent) |
| `gf2_core.gfp.Fp` type | `def … := Std.U64` (`Types.lean`) | `axiom … : Type` (`TypesExternal_Template.lean:42`) |

The V0 `MontgomeryRoundtrip` decode-spec re-proofs (sketch §4/§5,
`fp3_{new,add,sub,mul,neg}_decode`) require the gfp arithmetic bodies to be
transparent `do`-blocks so `progress`/`simp [gfp.montgomery.*]`/`scalar_tac`
can step through them. Against an uninterpreted `axiom gfp.Fp.new : Std.U64 →
Result (Fp P)` with no equational content, none of L4/L5/L6/L8/L9 (nor even
`fp3_*_decode`) is stateable against a specified `Fp<3>` semantics — exactly
the §3.5 "session-4 extraction blocker" the abstract-only landing already
documented. Option 2 does **not** dissolve that blocker; it inherits it.

---

## Why this is a stop-and-report, not a work-around

Per the dispatch contract and `CLAUDE.md §"Verification work"`:

- The dispatch explicitly named the C3-entanglement / transparent-gfp
  failure as a **STOP-and-escalate, do-NOT-work-around** condition:
  *"If they are inseparably entangled in the LLBC → STOP and escalate
  (5th Charon patch = shared infra, not worker-authorised). Do NOT work
  around with `sorry`/axiom."* The observed failure is **stronger**: the
  bodies are not merely entangled, they are **not extracted at all** for a
  foreign crate, and the only Charon flag that would extract them
  (`--extract-opaque-bodies`) is global, uncontrollable, and crashes the
  driver.
- Making the gfp foreign-crate bodies transparent requires either an
  **upstream Charon feature** (a name-pattern-scoped `--extract-opaque-bodies`
  / per-crate transparent-bodies flag — does not exist in `0.1.196`) or a
  **5th project-local Charon patch** (the project maintains 4 at
  `dev/active/charon-patch-backup-2026-05-15/`; a 5th is precedented but is
  shared infrastructure with cross-leg blast radius — the gf2_core leg, the
  bipedal3/packed5/packed7 V1 proofs, and the gfpn leg must all still pass).
  Both are lead/user decisions.
- A hand-maintained `Gf2Algebra/FunsExternal.lean` gfp block with *assumed
  value specs* would be a new trusted-`axiom` surface for field semantics
  (not the bit-exact two's-complement primitive externals the §3.2 C2 port
  is) — forbidden without explicit user sign-off under
  `CLAUDE.md §"Verification work"` and the no-autonomous-amendments rule.

No proof code was written. No `.jit/` state touched. No `axiom`/`sorry`
introduced into any committed file. `scripts/verify-lean.sh` reverted
byte-identical to HEAD. `crates/gf2-algebra/src/permanent/ryser_fp3.rs`
byte-identical to HEAD. `proofs/Gf2Algebra/Proofs/RyserBounded.lean`
unchanged (abstract L1–L7 + `ryser_eq_permanent_zmod` +
`ryser_permanent_bounded` remain complete & `sorry`-free).

---

## Repository state at handoff

- **Working tree: clean** (`git status --short scripts/ crates/ proofs/ dev/`
  empty except this handoff file). Spike output isolated to `/tmp` (deleted)
  and `target/charon/gf2_algebra_spike.llbc` (deleted).
- `scripts/verify-lean.sh`: **reverted to committed HEAD** (the Option-2
  flag deltas were applied for the spike, then `git checkout`-reverted).
- `crates/gf2-algebra/src/permanent/ryser_fp3.rs`: **byte-unchanged from
  HEAD** (never modified — per §9).
- `proofs/Gf2Algebra/Proofs/RyserBounded.lean`: **unchanged**. The abstract
  L1–L7 region remains complete and `sorry`-free as landed in `762ce0ac`.
  Headline `permanent_ryser_fp3_correct` remains deliberately unstated.
- A re-runnable record of the (reverted) Option-2 verify-lean.sh deltas is
  this document's "Exact spike procedure" section; the diff is small
  (2 flag swaps + the §3.2 C2 seed-and-port `FunsExternal` block, the latter
  never reached because the spike failed at the Charon/Aeneas stage).
- `lake build` on the committed `proofs/` tree is the known-good `main` state
  (per commit `5e3478c1`); not re-run this session (the spike wrote only to
  `/tmp`/`target/charon`; nothing in `proofs/` changed, so its build state is
  unchanged).

---

## Sketch lemma status

| Lemma | Status |
|-------|--------|
| Abstract L1–L7 + `ryser_eq_permanent_zmod` + `ryser_permanent_bounded` | DONE & sorry-free (pre-existing, untouched) |
| `fp3_{new,add,sub,mul,neg}_decode` | NOT STARTED — blocked: gfp bodies not extracted transparently in the gf2_algebra leg (foreign-crate body skip) |
| L4 `col_sum_invariant` | NOT STARTED — blocked (no transparent gfp arith to `progress` through) |
| L5 `fold_prod_invariant` | NOT STARTED — blocked |
| L6 `outer_acc_eq_ryser_inner` | NOT STARTED — blocked |
| L8 `permanent_ryser_fp3_value` | NOT STARTED — blocked |
| L9 `permanent_ryser_fp3_correct` (headline) | NOT STARTED — blocked |

The blocking sub-goal is **upstream of every Lean lemma and upstream of C3**:
there is no transparent Aeneas-extracted `gf2_core.gfp.Fp.{new,value}` /
`CoreOpsArith*FpFp.{add,sub,mul,neg}` body in the gf2_algebra leg to state
`fp3_*_decode` (hence L4–L9) against. `permanent_ryser_fp3` itself extracts
cleanly and sorry-free — the gap is purely the foreign gfp arithmetic
semantics, exactly the §3.5 blocker Option 2 was meant to dissolve but does
not.

---

## Options for the lead (decision required — do not pick autonomously)

These span the option space; **none** is authorised for the worker to
implement without explicit lead/user approval:

1. **5th local Charon patch (shared infra).** Add a project-local Charon
   patch implementing a *scoped* transparent-foreign-bodies mechanism (e.g.
   `--extract-opaque-bodies` gated by a name-pattern allowlist so only
   `gf2_core::gfp::*` foreign bodies are extracted, std/core stay skipped).
   Precedented (4 patches at `dev/active/charon-patch-backup-2026-05-15/`)
   but shared-infrastructure: the gf2_core, gfpn, and bipedal3/packed5/7 V1
   legs must all still pass. Largest scope; unblocks Option 2 as written.

2. **Build gf2-core as the current crate for a *third* Charon leg, then
   import its transparent gfp into the gf2_algebra proof.** This is the
   "import `Gf2Core.*` into `RyserBounded.lean`" approach the Path-1 sketch
   §1/§C4 explicitly *rejected* due to the `core.fmt.builders.DebugStruct` /
   `core.iter.adapters.zip.Zip` top-level-axiom `TypesExternal` collision.
   Resolving that collision is a `TypesExternal`/`FunsExternal` namespace-
   merge restructuring (sketch §1 calls it the reason Path 1 avoided import).
   Medium scope; needs a sketch addendum + lead approval.

3. **Keep the abstract-only landing; formally amend criteria 1/3 to the
   abstract contract.** `RyserBounded.lean`'s abstract side
   (`ryser_eq_permanent_zmod` over any `CommRing`, `ryser_permanent_bounded`
   with explicit `n ≤ 63`) is the mathematically substantive half and is
   done & sorry-free. Criterion 4 (`n ≤ 63` explicit) is met there.
   Criteria 1/3 (extracted-Rust binding) would be formally amended (this is
   option 4 of the *prior* handoff; the Option-2 spike now shows option 2 of
   that prior list — the route the user picked — is itself infeasible at the
   extraction layer). Requires user approval (criterion amendment).

4. **Escalate the Charon version discrepancy.** `verify-lean.sh`'s header
   pins Charon `1ec8d4bb`+4 patches; the installed `charon` on PATH reports
   `0.1.196`. The lead should confirm which Charon the pipeline is actually
   running and whether the 4 local patches are present in the installed
   binary, independently of the gfp-transparency decision.

**No recommendation is offered** — per the dispatch and the
no-autonomous-decisions rule, the path selection (including any criterion
amendment or shared-infra Charon change) is a lead/user call. This document
records the mechanism-level evidence so the decision can be made on facts.
