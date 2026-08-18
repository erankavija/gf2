# Spike 34d85cb9 — Charon/Aeneas extraction of a trait-generic gf2-core algorithm

Empirical toolchain spike for epic `6dc81018` (Reconcile field capability traits
and profile-driven kernel dispatch), criterion REQ-05. Run 2026-08-18 in the
worktree `.agents/worktrees/agent-34d85cb9` at
`2f2cbb3780876eb022cde7c9c2879b8ed7f1656b`. No proofs are authored here; no
production code and nothing under `proofs/` was modified.

Every command in the matrix below was executed from a committed `.cmd` file via
`dev/active/34d85cb9/extraction/run.sh`, which prints the command file into the
log before executing it. The command text quoted in this record is therefore
byte-identical to what ran.

---

## Question

Can Charon and Aeneas, on the toolchain `scripts/verify-lean.sh` pins, extract a
**trait-generic** `gf2-core` algorithm to Lean — as opposed to the concrete
`gfp` / `gfpn` / `gf2m` arithmetic the current pipeline covers?

The primary target is `batch_inverse<F: FiniteField>`
(`crates/gf2-core/src/field/batch_ops.rs:115`, core routine at
`crates/gf2-core/src/field/batch_ops.rs:363`). Two further `gf2-core` targets
were added once the first outcomes motivated them (§ Methodology, axes D and E).

---

## Toolchain and host

Recorded at run time in
`dev/active/34d85cb9/extraction/excerpts/toolchain-versions.txt`
(regenerate with `dev/active/34d85cb9/extraction/versions.sh`):

| Component | Version |
|---|---|
| `charon version` | `0.1.217` |
| `charon toolchain-version` | `nightly-2026-06-01` |
| `charon toolchain-path` | `~/.rustup/toolchains/nightly-2026-06-01-x86_64-unknown-linux-gnu` |
| `aeneas -version` | `5220259c` |
| `rustc --version` | `1.97.0 (2d8144b78 2026-07-07)` |
| `cargo --version` | `1.97.0 (c980f4866 2026-06-30)` |

The workspace MSRV is `rust-version = "1.95"`
(`crates/gf2-core/Cargo.toml:10`); the host default toolchain is 1.97.0, which
satisfies it. Charon runs its own pinned `nightly-2026-06-01` driver, exactly as
`scripts/verify-lean.sh` relies on (`scripts/verify-lean.sh:8-9`), so "the
workspace MSRV toolchain" is the same two-toolchain arrangement the existing
extraction already uses.

Host context (`dev/active/34d85cb9/extraction/excerpts/host-context.txt`,
regenerate with `dev/active/34d85cb9/extraction/host.sh`): AMD Ryzen 9 5900X,
24 hardware threads, 31 GiB RAM, Linux 7.1.8-arch1-3 x86_64. No run in the
matrix exceeded 6 s of wall time, so the 20-minute per-invocation timebox was
never reached; the `elapsed=` footer of every trimmed log records the measured
time.

---

## Methodology

All Charon invocations start from the `scripts/verify-lean.sh:103-131` gf2-core
invocation and change one thing at a time: `--preset aeneas`,
`--rustc-arg=--cfg=verify_lean`, and cargo args
`-- --manifest-path crates/gf2-core/Cargo.toml --no-default-features` are held
fixed throughout, and the `--opaque` list is the script's list narrowed to the
modules the target needs.

Axes varied:

- **A — start root.** Generic function (`gf2_core::field::batch_ops::batch_inverse`)
  versus module (`gf2_core::field::batch_ops`).
- **B — `gf2_core::field` opacity.** Opaqued as in `verify-lean.sh` (R1);
  opaqued with the target module re-included via `--include` (R2); the trait
  module re-included (R6); fully transparent (R7); the `FiniteField` trait alone
  re-included (R10).
- **C — monomorphisation.** `--monomorphize` (R5) and `--monomorphize-mut=all`
  (R3), both offered by `charon cargo --help` on 0.1.217.
- **D — second gf2-core target.** `FiniteFieldExt::pow`
  (`crates/gf2-core/src/field/traits.rs:1074`), a trait *default* method: a
  square-and-multiply loop with no slices and no iterator adapters, chosen to
  separate genericity from the slice/iterator shape of `batch_inverse`.
- **E — third gf2-core target.** `FieldPoly::eval`
  (`crates/gf2-core/src/field/poly.rs:1130`), an inherent generic method whose
  loop carries an `F`-typed accumulator, chosen once axis D localised the
  failure to lifted loop functions.
- **F — standalone probes.** `dev/active/34d85cb9/extraction/probe.rs`, a
  single file extracted with `charon rustc`, replaying the same
  square-and-multiply loop over four trait shapes plus a concrete `u64`
  baseline. Used to test whether the axis-D/E failure reproduces outside
  `gf2-core`. The probe is a scratch file under `dev/active/34d85cb9/`; it is
  not wired into any crate.

Aeneas was run on every Charon output that produced a `.llbc`, with
`-backend lean -split-files` as in `scripts/verify-lean.sh:220-224`, plus
`-print-error-emitters` from A2b onwards so each diagnostic names the Aeneas
source that emitted it.

`.llbc` files (0.8–2.0 MB each) and their `charon pretty-print` renderings are
not committed; `dev/active/34d85cb9/extraction/.gitignore` excludes them and
they are regenerable from the committed `.cmd` files. Their item counts are
committed in
`dev/active/34d85cb9/extraction/excerpts/llbc-item-counts.txt` (regenerate with
`dev/active/34d85cb9/extraction/llbc-summary.sh`), and the three load-bearing
fragments are committed under `dev/active/34d85cb9/extraction/excerpts/`.

---

## Findings

Trimmed logs for every run are under `dev/active/34d85cb9/logs-trimmed/`
(produced from the raw logs by `dev/active/34d85cb9/extraction/trim-logs.sh`,
which strips ANSI escapes, progress-bar redraws and the rustc dead-code warning
blocks). Each trimmed log opens with the exact command and closes with
`--- exit=<code> elapsed=<seconds>s ---`.

### Charon invocations

| Run | Axis | Command | Exit | Outcome | Evidence |
|---|---|---|---|---|---|
| R1 | A, B | `extraction/R1.cmd` | 0 | Target signature emitted, body `= <opaque>` — `--opaque 'gf2_core::field'` covers it | `logs-trimmed/R1.log`, `extraction/excerpts/R1_batch_inverse.llbc.txt` |
| R2 | A, B | `extraction/R2.cmd` | 0 | Target and both helpers transparent; 231 `fun_decls`, `has_errors=false` | `logs-trimmed/R2.log`, `extraction/excerpts/R2_batch_inverse.llbc.txt` |
| R3 | C | `extraction/R3.cmd` | 0 | `--monomorphize-mut=all` changes nothing: same 68/231/41/22 item counts as R2 | `logs-trimmed/R3.log`, `extraction/excerpts/llbc-item-counts.txt` |
| R4 | A | `extraction/R4.cmd` | 0 | Module start root: 330 `fun_decls`, adds `batch_inverse_in_place`, `batch_inverse_skip_zeros{,_in_place}` | `logs-trimmed/R4.log` |
| R5 | C | `extraction/R5.cmd` | 0 | `--monomorphize` yields an **empty** LLBC: 0 type_decls, 0 fun_decls, 0 trait_decls | `logs-trimmed/R5.log`, `extraction/excerpts/llbc-item-counts.txt` |
| R6 | B | `extraction/R6.cmd` | 101 | Charon panic on `--include 'gf2_core::field::traits'` | `logs-trimmed/R6.log` |
| R7 | B | `extraction/R7.cmd` | 101 | Same panic with the whole `gf2_core::field` module transparent | `logs-trimmed/R7.log` |
| R7b | B | `extraction/R7b.cmd` | 101 | R7 with `RUST_BACKTRACE=1`: panic located in `remove_unused_self_clause` | `logs-trimmed/R7b.log` |
| R8 | D | `extraction/R8.cmd` | 101 | `--translate-all-methods` panics with `GenericsMismatch` | `logs-trimmed/R8.log` |
| R8b | D | `extraction/R8b.cmd` | 0 | Same run without `--translate-all-methods`: `pow` transparent, 186 `fun_decls` | `logs-trimmed/R8b.log`, `extraction/excerpts/R8b_pow.llbc.txt` |
| R8c | D | `extraction/R8c.cmd` | 0 | `--include` narrowed to the method alone: `FiniteFieldExt` absent from the LLBC | `logs-trimmed/R8c.log` |
| R9 | B | `extraction/R9.cmd` | 0 | R7 under `--preset fast` does **not** panic: 238 `fun_decls` | `logs-trimmed/R9.log` |
| R10 | B | `extraction/R10.cmd` | 101 | `--include 'gf2_core::field::traits::FiniteField'` panics identically to R6/R7 | `logs-trimmed/R10.log` |
| R11 | E | `extraction/R11.cmd` | 0 | `gf2_core::field::poly` transparent | `logs-trimmed/R11.log` |
| P1–P4 | F | `extraction/P{1,2,3,4}.cmd` | 0 | Standalone probe extracts cleanly at every trait shape | `logs-trimmed/P{1,2,3,4}.log` |

R2's exact command, as executed:

```
charon cargo \
  --preset aeneas \
  --rustc-arg=--cfg=verify_lean \
  --start-from 'gf2_core::field::batch_ops::batch_inverse' \
  --opaque 'gf2_core::field' \
  --include 'gf2_core::field::batch_ops' \
  --opaque 'gf2_core::gf2m' \
  --opaque 'gf2_core::gfpn' \
  --opaque 'gf2_core::gfp::simd_ops' \
  --opaque 'gf2_core::bitvec' \
  --opaque 'gf2_core::bitslice' \
  --opaque 'gf2_core::matrix' \
  --opaque 'gf2_core::sparse' \
  --opaque 'gf2_core::alg' \
  --opaque 'gf2_core::compute' \
  --opaque 'gf2_core::kernels' \
  --opaque 'gf2_core::primitive_polys' \
  --opaque 'gf2_core::io' \
  --opaque 'gf2_core::macros' \
  --dest-file dev/active/34d85cb9/extraction/R2_gf2_core.llbc \
  -- --manifest-path crates/gf2-core/Cargo.toml --no-default-features
```

### Aeneas invocations

| Run | Input | Command | Exit | Outcome | Evidence |
|---|---|---|---|---|---|
| A2 | R2 | `extraction/A2.cmd` | 1 | `batch_inverse` body complete; both helpers `sorry` (2 errors) | `logs-trimmed/A2.log`, `extraction/A2_lean/Funs.lean` |
| A2b | R2 | `extraction/A2b.cmd` | 1 | A2 plus `-print-error-emitters`; `Funs.lean` byte-identical to A2's | `logs-trimmed/A2b.log`, `extraction/A2b_lean/Funs.lean` |
| A3 | R3 | `extraction/A3.cmd` | 1 | Identical to A2 apart from the two generated `import` module names | `logs-trimmed/A3.log`, `extraction/A3_lean/Funs.lean` |
| A4 | R4 | `extraction/A4.cmd` | 1 | 7 defs, 4 `sorry`; adds a third error class (`Unreachable`) | `logs-trimmed/A4.log`, `extraction/A4_lean/Funs.lean` |
| A8b | R8b | `extraction/A8b.cmd` | 1 | `square.default` and `frobenius.default*` complete; `pow.default_loop{,.body}` carry `sorry` **in their signatures** | `logs-trimmed/A8b.log`, `extraction/A8b_lean/Funs.lean` |
| A9 | R9 | `extraction/A9.cmd` | 1 | Aeneas refuses non-`aeneas`-preset LLBC; no Lean generated | `logs-trimmed/A9.log` |
| A11 | R11 | `extraction/A11.cmd` | 1 | 53 `sorry`; 20 lifted loop definitions, covering 10 distinct loops, lose both associated-type binders | `logs-trimmed/A11.log`, `extraction/A11_lean/Funs.lean` |
| AP1–AP4 | P1–P4 | `extraction/AP{1,2,3,4}.cmd` | 0 | Probe translates with **0** `sorry` at every trait shape | `logs-trimmed/AP{1,2,3,4}.log`, `extraction/P{1,2,3,4}_lean/Funs.lean` |

### What succeeded

**Charon extracts trait-generic `gf2-core` code.** R2 emits a transparent,
fully generic body for `batch_inverse`, with the `FiniteField` bound preserved
as a clause and the two associated types lifted to type parameters
(`extraction/excerpts/R2_batch_inverse.llbc.txt`):

```
pub fn batch_inverse<'_0, F, Clause0_Characteristic, Clause0_Wide>(elements_1: &'_0 [F]) -> Option<Vec<F>>
where
    TraitClause0: (F: FiniteField<Clause0_Characteristic, Clause0_Wide>),
{
```

The control R1, which keeps `--opaque 'gf2_core::field'` exactly as
`scripts/verify-lean.sh:109` does, emits the same signature with `= <opaque>`
in place of the body (`extraction/excerpts/R1_batch_inverse.llbc.txt`). The
single flag that turns generic `gf2-core` code from opaque into transparent is
therefore `--include` on the target module.

**Aeneas produces real Lean for the generic entry point.** The preserved
extraction `dev/active/34d85cb9/extraction/A2_lean/Funs.lean:62-88` carries a
complete, `sorry`-free `batch_inverse` whose `FiniteField` dictionary is an
explicit parameter:

```lean
def field.batch_ops.batch_inverse
  {F : Type} {Clause0_Characteristic : Type} {Clause0_Wide : Type}
  (traitsFiniteFieldInst : field.traits.FiniteField F Clause0_Characteristic
  Clause0_Wide) (elements : Slice F) :
  Result (Option (alloc.vec.Vec F))
```

**Two generic trait methods extract completely.** In
`extraction/A8b_lean/Funs.lean`, `field.traits.FiniteFieldExt.square.default`
(line 25) and `field.traits.FiniteFieldExt.frobenius.default` (line 135,
together with its two loop helpers at lines 105 and 123) are complete and
`sorry`-free, with all three type parameters bound.

### What was falsified

**F1 — The generic algorithm's arithmetic does not survive Aeneas.**
`batch_inverse`'s body is a shell that delegates to
`batch_inverse_with_scratch`, which delegates to `batch_inverse_core`; both are
`sorry` in `extraction/A2_lean/Funs.lean:39-58`. Verbatim from
`logs-trimmed/A2b.log`:

```
[Error] Region ids should not be visited directly; the visitor should catch cases that contain region ids earlier.
Compiler source: interp/InterpUtils.ml, line 877
[Warn ] Could not translate the body of function 'gf2_core::field::batch_ops::batch_inverse_with_scratch because of previous error
Definition span: 'crates/gf2-core/src/field/batch_ops.rs', lines 210:0-230:1

[Error] Unimplemented
Source: 'crates/gf2-core/src/field/batch_ops.rs', lines 394:4-400:5
Compiler source: interp/InterpMatchCtxs.ml, line 419
[Warn ] Could not translate the body of function 'gf2_core::field::batch_ops::batch_inverse_core because of previous error
```

`batch_ops.rs:394-400` is the backward reconstruction loop of Montgomery's
trick, the mathematical content of the routine. The same log separately records
that Aeneas's Lean model lacks `Iterator::any` and `Iterator::rev`, which are
the adapters these two functions use (`batch_ops.rs:222` and
`batch_ops.rs:394`):

```
[Warn ] When retrieving the builtin information for trait decl 'core::iter::traits::iterator::Iterator', could not find the information for item 'any'. The model defined in the Lean library seems to be missing the corresponding field.
```

Whether the missing iterator models cause the two errors is **not
established**: the warnings are emitted by `extract/Extract.ml` after
translation, while both errors come from `interp/`, so the ordering does not
support a causal claim either way.

Widening the start root to the whole module (A4) adds a third failing function
and a third error class without fixing any of the above: 4 of 7 defs are `sorry`
(`extraction/A4_lean/Funs.lean`).

**F2 — Making the `FiniteField` trait module transparent crashes Charon.**
Both `--include 'gf2_core::field::traits'` (R6) and making the module fully
transparent (R7, which drops the now-redundant pair
`--opaque 'gf2_core::field'` / `--include 'gf2_core::field::batch_ops'`) abort
the driver. Verbatim from
`logs-trimmed/R7b.log`:

```
thread 'main' (304862) panicked at .../index_vec-0.1.4/src/lib.rs:774:5:
index_vec index overflow: 18446744073709551615 is outside the range [0, 4294967295)
...
  11: <charon_lib::transform::simplify_output::remove_unused_self_clause::Transform as charon_lib::transform::ctx::TransformPass>::transform_ctx
  12: <charon_lib::transform::ctx::TransformCtx>::run_pass
  14: charon_lib::transform::run_transformation_passes
  15: charon_driver::main
```

The narrowest possible re-inclusion, `--include 'gf2_core::field::traits::FiniteField'`
(R10), panics identically. The crash is confined to the `aeneas` preset — the
same command under `--preset fast` (R9) completes with 238 `fun_decls` — but
that is not a usable escape hatch, because Aeneas rejects the result
(`logs-trimmed/A9.log`):

```
[Error] Invalid option detected: the serialized crate was generated by Charon without the `--preset=aeneas` option. Please regenerate the crate by calling Charon with `--preset=aeneas`.
```

Consequence: any proof over generic `gf2-core` code must keep `FiniteField`
opaque and reason about it as an abstract field, exactly as the epic's
"trait-level algebraic laws" framing intends. That is a constraint, not a
blocker.

**F3 — `--monomorphize` produces nothing.** R5 exits 0 and writes a `.llbc`
with zero items of every kind
(`extraction/excerpts/llbc-item-counts.txt`: `R5_gf2_core.llbc has_errors=False
type_decls= 0 fun_decls= 0 trait_decls= 0 trait_impls= 0`). This matches
`charon cargo --help` on 0.1.217 — "Generic items found in the crate are
skipped" — and independently reconfirms the finding already recorded at
`proofs/Gf2Algebra/Proofs/RyserBounded.lean:62-70`, on a fresh target and a
newer Charon. `--monomorphize-mut=all` (R3) is inert here: identical item counts
to R2, and the Lean it yields differs from A2's only in the two generated
`import` lines.

**F4 — Every `F`-carrying loop in `FiniteField`-generic gf2-core code loses its
associated-type binders.** This is the load-bearing falsification. In A8b, the
lifted loop function for `FiniteFieldExt::pow` has a complete body but a broken
signature (`extraction/A8b_lean/Funs.lean:38-42`):

```lean
def field.traits.FiniteFieldExt.pow.default_loop.body
  {Self : Type} (FiniteFieldExtInst : field.traits.FiniteFieldExt Self
  sorry /- Could not find: type_var_id: 1 from ExtractBase.Item-/
  sorry /- Could not find: type_var_id: 2 from ExtractBase.Item-/)
  (result : Self) (base : Self) (e : Std.U64) :
```

The `{Clause0_Clause0_Characteristic : Type}` and `{Clause0_Clause0_Wide : Type}`
binders that `pow.default` itself carries (lines 86-90) are absent, and `sorry`
occupies their argument positions. The scale of the problem shows in A11: the
`gf2_core::field::poly` extraction reports 20 occurrences each of
`Could not find: type_var_id: 1` and `... : 2`, hitting 20 lifted loop
definitions that cover 10 distinct loops — `FieldPoly.eval`, `FieldPoly.scale`,
`FieldPoly.div_rem`, `add_impl`, `slice_add` and five `mul_karatsuba_raw` loops,
each contributing a `_loop` and a `_loop.body`. Those account for 40 of the 53
`sorry` occurrences in `extraction/A11_lean/Funs.lean`; the remaining 13 come
from the four other error classes the log reports (`Internal error, please file
an issue` ×11, plus `Region ids ...`, `Unimplemented`, `Unreachable`,
`Unexpected` and two `Could not lookup the translated function`).

The failure is selective in a way that identifies it precisely:

- No loop, generic: `square.default` is complete (A8b line 25).
- Loop whose state contains no `F`: `frobenius.default_loop` is complete and
  simply has no type parameters at all (A8b line 123).
- Loop whose state contains `F`: binders lost, `sorry` emitted (A8b `pow`,
  A11 `FieldPoly.eval` at line 1288).

**F5 — the failure does not reproduce outside `gf2-core`.** The standalone
probe `extraction/probe.rs` replays the same square-and-multiply loop across a
concrete `u64` baseline, a trait with no associated types, a trait with two
associated types, a trait whose associated types appear in its own method
signatures with bounds, a trait *default* method over such a trait, a loop that
clones through the instance dictionary, and a trait carrying an associated
const. All four probe extractions (P1–P4, AP1–AP4) exit 0 with **0** `sorry`,
and every lifted loop function keeps both binders — e.g.
`extraction/P3_lean/Funs.lean:313-316`:

```lean
def WithAssoc2Ext.pow_default.default_loop
  {Self : Type} {Clause0_Clause0_Ch : Type} {Clause0_Clause0_Wide : Type}
  (WithAssoc2ExtInst : WithAssoc2Ext Self Clause0_Clause0_Ch
  Clause0_Clause0_Wide) (result : Self) (base : Self) (e : Std.U64) :
```

So genericity as such, associated types, associated types in method signatures,
trait default methods, associated consts, and cloning through the dictionary are
each individually exonerated. The trigger requires something further in the real
`FiniteField` declaration that the probe does not model, and this spike did not
isolate it. **This is left unverified**: a minimal upstream reproducer does not
exist yet.

**Not tested:** whether any generated `Funs.lean` in this spike elaborates under
`lake build`. The worktree has no built `proofs/.lake`, and seeding one requires
a Mathlib `v4.30.0-rc2` toolchain fetch that is out of proportion to the spike.
The claims above rest on Aeneas's own exit codes and its `Generated the partial
file (because of N errors...)` reports, not on Lean elaboration.

---

## Verdict

**Partially feasible, with a hard wall on loops.** Charon extraction of
trait-generic `gf2-core` code succeeds on the pinned toolchain and needs one
extra flag over the existing pipeline. Aeneas translation succeeds for generic
code without `F`-carrying loops and fails systematically for generic code with
them. `batch_inverse` specifically does **not** extract: its Montgomery-trick
core is `sorry`.

Read against epic criterion REQ-05 — "Charon/Aeneas extraction of a
representative generic algorithm over the reconciled traits succeeds on the
workspace MSRV toolchain" — the criterion is **not met as literally worded**,
because every representative *algorithm* in `gf2-core` loops over `F`.

---

## Decision needed on REQ-05 (escalation)

The lead must choose one of the following before the follow-up proof round is
planned. The spike cannot choose for them: the options trade epic scope against
upstream dependency.

- **DEC-A — Narrow REQ-05 to loop-free generic obligations.** Accept
  `FiniteFieldExt::square` / `frobenius` -class extraction as the criterion's
  evidence: trait-level algebraic laws are stated and proved over the extracted
  generic definitions, while per-backend representation refinement continues to
  use the existing concrete `gfp` / `gfpn` extraction. Deliverable today; the
  weakness is that "algorithm" becomes a generous reading of `square`.
- **DEC-B — Keep REQ-05 as worded and take an upstream dependency.** Report F4
  to Aeneas (the lifted loop function drops the type parameters its parent
  binds) and F2 to Charon (`remove_unused_self_clause` panics on the
  `FiniteField` trait), then re-run this matrix when both are fixed. The epic's
  REQ-05 then blocks on a third party with no committed date.
- **DEC-C — Descope REQ-05 to concrete instantiations.** State the generic laws
  abstractly in Lean without binding them to extracted generic Rust, and keep
  the extracted-Rust binding at the concrete field types, as issue `0606186a`
  chose for `permanent_ryser` (`proofs/Gf2Algebra/Proofs/RyserBounded.lean:77-84`).
  Precedent exists in this repository; the cost is that REQ-05 no longer
  demonstrates anything about the *reconciled traits*.

Recommendation: **DEC-A**, with F4 and F2 filed upstream as non-blocking
follow-ups. It is the only option that both lands inside the epic and produces
Lean that mentions the reconciled trait hierarchy.

---

## What the follow-up proof-sketch round should target

If DEC-A is chosen:

1. Extract with R8b's invocation (`extraction/R8b.cmd`) — start root
   `gf2_core::field::traits::FiniteFieldExt::pow`, `--opaque 'gf2_core::field'`
   re-narrowed by `--include 'gf2_core::field::traits::FiniteFieldExt'`, and
   **without** `--translate-all-methods`, which panics Charon (R8).
2. Sketch proofs against `field.traits.FiniteFieldExt.square.default` and
   `field.traits.FiniteFieldExt.frobenius.default` only. Both are complete in
   `extraction/A8b_lean/Funs.lean`; both take the `FiniteField` dictionary as a
   parameter, so their specifications are stated as trait-level algebraic laws
   over an abstract field, which is what REQ-05's second clause asks for.
3. Do not target `pow`: `pow.default` is well-formed but calls
   `pow.default_loop`, whose signature contains `sorry`.
4. Budget a `lake build` step in that round — this spike did not verify that any
   generated file elaborates, and that gap must close before a proof is written
   against these definitions.

Independently of the decision, `scripts/verify-lean.sh` needs no change: nothing
in this spike touched the existing gfp/gfpn/gf2m extraction, and every
invocation here wrote to `dev/active/34d85cb9/extraction/` instead of
`target/charon/`.

---

## Artifact index

All paths relative to the repository root.

| Path | Contents |
|---|---|
| `dev/active/34d85cb9/extraction/run.sh` | Runner: executes one `.cmd` file, tees to `logs/<run>.log` |
| `dev/active/34d85cb9/extraction/*.cmd` | The 29 exact invocations, one per run |
| `dev/active/34d85cb9/extraction/probe.rs` | Standalone probe for axis F |
| `dev/active/34d85cb9/extraction/A2_lean/` | **Preserved Lean for the primary generic target** (REQ-02) |
| `dev/active/34d85cb9/extraction/A8b_lean/` | Lean for `FiniteFieldExt::{square,pow,frobenius}` |
| `dev/active/34d85cb9/extraction/A11_lean/` | Lean for `gf2_core::field::poly` (F4 at scale) |
| `dev/active/34d85cb9/extraction/A{2b,3,4}_lean/` | Lean for the remaining Aeneas runs |
| `dev/active/34d85cb9/extraction/P{1,2,3,4}_lean/` | Lean for the probes |
| `dev/active/34d85cb9/extraction/excerpts/` | Toolchain versions, host context, LLBC item counts, three LLBC fragments |
| `dev/active/34d85cb9/logs-trimmed/` | Trimmed log per run, each opening with its command and closing with `exit=`/`elapsed=` |
| `dev/active/34d85cb9/extraction/{trim-logs,llbc-summary,versions,host}.sh` | Regenerate the derived artifacts above |
