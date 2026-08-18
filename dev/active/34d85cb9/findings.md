# Spike 34d85cb9 — Charon/Aeneas extraction of a trait-generic gf2-core algorithm

Empirical toolchain spike for epic `6dc81018` (Reconcile field capability traits
and profile-driven kernel dispatch), criterion REQ-05. Run 2026-08-18 in the
worktree `.agents/worktrees/agent-34d85cb9` at
`2f2cbb3780876eb022cde7c9c2879b8ed7f1656b`, and extended 2026-08-19 with a
second leg against newest upstream Charon/Aeneas (see *Upgrade leg* below). No proofs are authored here; no
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

This is the **pinned pair** the baseline leg ran on; the newest-upstream pair is
recorded under *Upgrade leg* below. Two toolchains are in play and the criterion
turns on the first. All of it is captured at run time in
`dev/active/34d85cb9/extraction/excerpts/toolchain-versions.txt` (regenerate
with `dev/active/34d85cb9/extraction/versions.sh`):

| Component | Version | Role |
|---|---|---|
| `RUSTUP_TOOLCHAIN=1.95.0 cargo --version` | `1.95.0 (f2d3ce0bd 2026-03-21)` | builds `gf2-core` |
| `RUSTUP_TOOLCHAIN=1.95.0 rustc --version` | `1.95.0 (59807616e 2026-04-14)` | builds `gf2-core` |
| `charon version` | `0.1.217` | |
| `charon toolchain-version` | `nightly-2026-06-01` | rustc that produces MIR |
| `charon toolchain-path` | `~/.rustup/toolchains/nightly-2026-06-01-x86_64-unknown-linux-gnu` | |
| `aeneas -version` | `5220259c` | |

**Every run in both legs executed on the workspace MSRV.** The workspace MSRV is
`rust-version = "1.95"` (`crates/gf2-core/Cargo.toml:10`), and each Charon
command file begins with `RUSTUP_TOOLCHAIN=1.95.0`, so the pin is part of the
`.cmd` contract rather than an ambient property of the shell — for example
`extraction/R2.cmd:1`:

```
RUSTUP_TOOLCHAIN=1.95.0 charon cargo \
```

Charon still drives its own pinned `nightly-2026-06-01` rustc for the MIR it
consumes; that is a property of how Charon was built, not selectable per run,
and it is exactly the arrangement `scripts/verify-lean.sh:8-9` already relies
on. This host's *default* toolchain is 1.97.0, recorded in the receipt only to
show what the runs did **not** use.

Host context (`dev/active/34d85cb9/extraction/excerpts/host-context.txt`,
regenerate with `dev/active/34d85cb9/extraction/host.sh`): AMD Ryzen 9 5900X,
24 hardware threads, 31 GiB RAM, Linux 7.1.8-arch1-3 x86_64.

**Stopping rule, declared before any run:** the dispatch instructions for this
issue set a 20-minute per-invocation timebox — kill the invocation and record
the timeout as its outcome. It was never triggered. The slowest invocation
across both legs took 7 s; the `elapsed=` footer of every trimmed log records
the measured time, so the whole distribution is checkable, not just the
maximum.

---

## Methodology

All Charon invocations start from the `scripts/verify-lean.sh:103-131` gf2-core
invocation and change one thing at a time: the `RUSTUP_TOOLCHAIN=1.95.0` MSRV
pin, `--preset aeneas`, `--rustc-arg=--cfg=verify_lean`, and cargo args
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
- **F — standalone probes.** `dev/active/34d85cb9/extraction/probe-P1.rs`
  through `probe-P4.rs`, extracted with `charon rustc`, replaying the same
  square-and-multiply loop over four trait shapes plus a concrete `u64`
  baseline. The probe grew across runs, so each run has its own committed
  source and `P{N}.cmd` names `probe-P{N}.rs` exactly; every probe result below
  is therefore regenerable from the command that produced it. The probes are
  scratch files under `dev/active/34d85cb9/`; none is wired into any crate.

Aeneas was run on every Charon output that produced a `.llbc`, with
`-backend lean -split-files` as in `scripts/verify-lean.sh:220-224`, plus
`-print-error-emitters` from A2b onwards so each diagnostic names the Aeneas
source that emitted it.

`.llbc` files and their `charon pretty-print` renderings are not committed:
across both legs the 27 of them total 49.6 MB, ranging from 31,190 bytes (R5,
which emits nothing) to 14,426,493 bytes. `.gitignore` excludes them and they
are regenerable from the committed `.cmd` files. Their **per-file byte sizes and
item counts** are committed instead, in
`dev/active/34d85cb9/extraction/excerpts/llbc-item-counts.txt` and
`dev/active/34d85cb9/upgrade/excerpts/llbc-item-counts.txt` (regenerate with
`dev/active/34d85cb9/extraction/llbc-summary.sh`), and the three load-bearing
fragments are committed under `dev/active/34d85cb9/extraction/excerpts/`.

---

## Findings

### MSRV execution

Both legs were executed end to end with the workspace toolchain pinned to the
MSRV (`RUSTUP_TOOLCHAIN=1.95.0`, carried in every Charon `.cmd` file). **No
outcome anywhere in this record depends on the toolchain choice.** The evidence
that this is so, rather than an assertion:

- Every exit code is unchanged — the same five Charon runs panic on the pinned
  pair (R6, R7, R7b, R8, R10 → 101), the same runs succeed, and the same Aeneas
  legs exit 1.
- Every generated `Funs.lean` and `Types.lean` in `extraction/A*_lean/` and
  `upgrade/extraction/A*_lean/` is **byte-identical** to the version produced
  before the pin, so the diff of this rework touches no Aeneas output.
- Every LLBC item count and byte size is unchanged
  (`extraction/excerpts/llbc-item-counts.txt`,
  `upgrade/excerpts/llbc-item-counts.txt`).
- Both pipeline-regression legs reproduce their earlier verdicts exactly,
  including the 311/428 non-comment line counts
  (`upgrade/excerpts/pipeline-regen-diff.txt`), and each summary records the
  `cargo 1.95.0` banner it ran under
  (`upgrade/logs/pipeline-{baseline,new}-summary.txt`).

The only artefact that changed is the probe Lean, and only in its crate name
(`namespace probe` → `namespace probe_P1`), a mechanical consequence of splitting
`probe.rs` into one source per run for F4 below.

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
RUSTUP_TOOLCHAIN=1.95.0 charon cargo \
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
probes `extraction/probe-P1.rs` … `probe-P4.rs` replay the same
square-and-multiply loop across a
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

**Not tested, and deliberately out of scope:** whether any generated
`Funs.lean` in this spike elaborates under `lake build`. The worktree has no
built `proofs/.lake` and seeding one needs a Mathlib `v4.30.0-rc2` toolchain
fetch. This is scoped out against the issue's criteria rather than deferred
work: REQ-01 asks for the Charon/Aeneas invocations and their outcomes, and
REQ-02 asks that the generated Lean be preserved on success or the falsifying
evidence recorded on failure — neither requires Lean elaboration, and this issue
explicitly authors no proofs. Every claim above rests on Aeneas's own exit codes
and its `Generated the partial file (because of N errors...)` reports. The
follow-up proof round is where elaboration must be verified, and it is named as
a required step there.

---

## Upgrade leg — newest upstream Charon/Aeneas

Scope extension from the epic owner (2026-08-19): after the baseline matrix,
update the local Charon/Aeneas to newest upstream and extend the matrix.
Everything above this heading is the **pinned-pair** leg and is unchanged by
what follows.

### How the toolchain is installed on this host

Determined by inspection, not assumption. It is **not** `cargo install` and not
release binaries: it is a from-source build tree at `/data/aeneas-build`, an
Aeneas clone whose nested `./charon` is a Charon clone at the commit named in
`/data/aeneas-build/charon-pin`, with the built binaries copied into
`~/.cargo/bin`. Two properties of that tree matter here:

- `/data/aeneas-build/charon` carried an **uncommitted** working-tree patch
  (`git status --short` → ` M charon/src/transform/normalize/expand_associated_types.rs`,
  18 insertions), the implied-clause propagation `scripts/verify-lean.sh:18-28`
  calls load-bearing. It is preserved here as
  `upgrade/charon-local-patch-487f0320.patch`.
- `proofs/lakefile.lean:5` requires the Lean backend from that same tree
  (`require aeneas from "/data/aeneas-build" / "backends" / "lean"`).

Because a `git checkout` there would put both the uncommitted patch and the
Lean backend of the committed proofs at risk — for every worker on this shared
host, not just this spike — the new pair was built from **fresh clones** under
`/data/aeneas-upgrade-34d85cb9`, leaving `/data/aeneas-build` untouched.

### Target selection

Every claim in this subsection is captured at run time in
`upgrade/excerpts/upstream-topology.txt` (regenerate with
`upgrade/topology.sh`, which is read-only network access: `git ls-remote` plus
raw-file fetches, touching no local repository).

| | Commit | Receipt |
|---|---|---|
| Aeneas | `c10cc997` (`main` HEAD) | `git ls-remote … aeneas HEAD refs/heads/main` |
| Newest Aeneas tag | `daa85d7e` (`nightly-2026.08.18-daa85d7`) | `git ls-remote --tags … aeneas \| tail -5` |
| main vs newest tag | `status=ahead ahead_by=2 behind_by=0` | GitHub compare API |
| Charon | `340b1af4` | the `charon-pin` of **both** `c10cc997` and `daa85d7e` |
| Charon `main` | `4c346c14` | `git ls-remote … charon HEAD refs/heads/main` |

Charon `main` (`4c346c14`) is ahead of the pin and was deliberately not used:
`dev/active/150d7d79/150d7d79-toolchain-upgrade.md:12` records the rule that the
two halves move together to the pinned commit. The same receipt shows
`charon/rust-toolchain` at `340b1af4` requires `nightly-2026-06-01`, already
installed with all four components, so no new toolchain was downloaded.

### Exact upgrade commands

Backups first, checksum-verified equal to the live binaries:

```
cp -p ~/.cargo/bin/charon        ~/.cargo/bin/charon.bak-2026-08-19
cp -p ~/.cargo/bin/charon-driver ~/.cargo/bin/charon-driver.bak-2026-08-19
cp -p ~/.cargo/bin/aeneas        ~/.cargo/bin/aeneas.bak-2026-08-19
```

Fetch at the exact commits (no `git checkout`, so nothing depends on branch
state), then build:

```
mkdir -p /data/aeneas-upgrade-34d85cb9/charon
cd /data/aeneas-upgrade-34d85cb9/charon
git init && git remote add origin https://github.com/AeneasVerif/charon
git fetch --depth 1 origin 340b1af4df92608d0911fc2ba26eef3fd3a30ab4
git reset --hard FETCH_HEAD

mkdir -p /data/aeneas-upgrade-34d85cb9/aeneas
cd /data/aeneas-upgrade-34d85cb9/aeneas
git init && git remote add origin https://github.com/AeneasVerif/aeneas
git fetch --depth 1 origin c10cc997a2cc885f5b2c9ef3929cc05632cb106c
git reset --hard FETCH_HEAD

# aeneas expects the charon clone at ./charon (src/charon is a symlink to ../charon)
mv /data/aeneas-upgrade-34d85cb9/charon /data/aeneas-upgrade-34d85cb9/aeneas/charon

cd /data/aeneas-upgrade-34d85cb9/aeneas/charon/charon
CARGO_PROFILE_RELEASE_DEBUG=true cargo build --release
mkdir -p ../bin && cp -f target/release/charon target/release/charon-driver ../bin/

cd /data/aeneas-upgrade-34d85cb9/aeneas/src
eval $(opam env) && dune build
```

The Charon build log is committed at `upgrade/logs-trimmed/charon-build.log`;
its final line is cargo's own measurement, `Finished \`release\` profile
[optimized + debuginfo] target(s) in 59.25s`, and is the sole source for any
build-duration statement here. The OCaml toolchain that built Aeneas — `dune
--version` `3.21.1`, `ocaml -version` `5.4.1`, opam switch `default` — is
recorded in `upgrade/excerpts/toolchain-versions-new.txt` alongside the pair
versions.

`cargo build --release` is invoked directly rather than through Charon's
`make build-charon-rust`, because that target runs `cargo fmt` first and would
rewrite the freshly fetched sources.

Resulting versions, recorded at run time in
`upgrade/excerpts/toolchain-versions-new.txt`:

| | Pinned pair (baseline leg) | Newest upstream (upgrade leg) |
|---|---|---|
| `charon version` | `0.1.217` | `0.1.232` |
| Charon commit | `487f0320` + 1 local patch | `340b1af4`, unpatched |
| `charon toolchain-version` | `nightly-2026-06-01` | `nightly-2026-06-01` |
| `aeneas -version` | `5220259c` | `c10cc99` |
| workspace `cargo`/`rustc` | `1.95.0` (MSRV) | `1.95.0` (MSRV) |

### The new binaries were NOT installed into `~/.cargo/bin`

Copying them over `~/.cargo/bin/{charon,charon-driver,aeneas}` was **denied by
the permission system**, as was creating a shim directory of symlinks. That
denial was not worked around. The upgrade leg instead invokes the newly built
binaries by absolute path — every `upgrade/extraction/*.cmd` file begins with
`/data/aeneas-upgrade-34d85cb9/...` instead of a bare `charon` / `aeneas` — which
measures exactly the same thing while leaving the host installation, and the
other worker sharing this host, untouched.

Consequence for the lead's step 5: **there was nothing to restore.** The host
install was verified still on the pinned pair after all upgrade-leg runs —
`charon version` → `0.1.217`, `aeneas -version` → `aeneas 5220259c`, and each
of the three binaries still checksum-identical to its `.bak-2026-08-19` copy
(`upgrade/excerpts/toolchain-versions-new.txt`). Actually replacing the host
binaries needs the user's approval.

### The project-local Charon patch is now obsolete

`git apply --check upgrade/charon-local-patch-487f0320.patch` against `340b1af4`
fails:

```
error: patch failed: charon/src/transform/normalize/expand_associated_types.rs:827
error: charon/src/transform/normalize/expand_associated_types.rs: patch does not apply
```

That is not a blocker but a fix: the patch existed to remove
"Could not compute the value of …" warnings, and the **unpatched** new Charon
emits *fewer* of them than the patched old one on the committed gf2-core
extraction — 0 versus 2 (counted in
`upgrade/logs-trimmed/pipeline-{new,baseline}.log`). Upstream has subsumed the
patch. This also removes the obvious confound from the regression check below:
the new-pair results are not an artefact of a missing local patch.

### Matrix outcomes, per version

Same 13 Charon runs and the Aeneas legs, re-run through
`upgrade/extraction/run.sh` (identical contract to the baseline runner). Full
per-run logs in `upgrade/logs-trimmed/`.

| Run | Pinned pair | Newest upstream | Change |
|---|---|---|---|
| R1 | exit 0, body `= <opaque>` | exit 0, same | — |
| R2 | exit 0, 231 `fun_decls`, target transparent | exit 0, 226 `fun_decls`, target transparent | — |
| R3 | inert vs R2 | inert vs R2 | — |
| R4 | exit 0, 330 `fun_decls` | exit 0, 275 `fun_decls` | — |
| R5 | exit 0, **empty LLBC** | exit 0, **empty LLBC** | F3 unchanged |
| R6 | **exit 101, panic** | **exit 0**, 227 `fun_decls` | **F2 fixed** |
| R7 | **exit 101, panic** | **exit 0**, 227 `fun_decls`, `has_errors=true` | **F2 fixed** |
| R8 | **exit 101, `GenericsMismatch`** | **exit 0**, 186 `fun_decls` | **fixed** |
| R8b | exit 0, 186 `fun_decls` | exit 0, 186 `fun_decls` | — |
| R9 | exit 0 | exit 0 | — |
| R10 | **exit 101, panic** | **exit 0**, 227 `fun_decls` | **F2 fixed** |
| R11 | exit 0, 548 `fun_decls` | exit 0, 488 `fun_decls` | — |

Aeneas:

| Run | Pinned pair | Newest upstream | Change |
|---|---|---|---|
| A2 | exit 1; `Region ids …` + `Unimplemented`; 2 `sorry` | exit 1; 2× `Unimplemented`; 2 `sorry` | **F1 persists**, better diagnostics |
| A4 | exit 1; 4 `sorry` of 7 defs | exit 1 | F1 persists |
| A6/A7 | not reachable (Charon panicked) | exit 1; 15 defs incl. `FiniteField::*.default`; 6× `Unsupported use of dyn traits` | **newly reachable** |
| A8b | exit 1; 4 `sorry`, type-var binders lost | exit 1; 4 `sorry`, **byte-identical failure** | **F4 persists** |
| A11 | exit 1; 20+20 `type_var_id`, 53 `sorry` | exit 1; 22+22 `type_var_id`, 48 `sorry` | **F4 persists** |

Two findings deserve emphasis.

**F2 is fixed upstream.** All four baseline Charon panics — the
`index_vec index overflow` in `remove_unused_self_clause` (R6/R7/R10) and the
`GenericsMismatch` under `--translate-all-methods` (R8) — are gone. Making the
`FiniteField` trait module transparent now works, and A6/A7 translate 15
definitions including `FiniteField` default methods that were previously
unreachable. The proof-facing constraint recorded under F2 above ("any proof
over generic gf2-core code must keep `FiniteField` opaque") **no longer holds on
newest upstream**.

**F4 is not fixed, and it is the one that matters.** The lifted loop function
still drops the two associated-type binders its parent binds. On the new pair
the emitted text is character-for-character what the baseline produced
(`upgrade/excerpts/A8b_pow_loop_signature.txt` versus
`extraction/A8b_lean/Funs.lean:70-73`):

```lean
def field.traits.FiniteFieldExt.pow.default_loop
  {Self : Type} (FiniteFieldExtInst : field.traits.FiniteFieldExt Self
  sorry /- Could not find: type_var_id: 1 from ExtractBase.Item-/
  sorry /- Could not find: type_var_id: 2 from ExtractBase.Item-/)
```

F1 also persists, though the diagnosis improved: `batch_inverse_with_scratch`
now fails with a precise Rust span, `batch_ops.rs:222:7-222:38` — the
`elements.iter().any(F::is_zero)` call — reported from
`llbc/RegionsHierarchy.ml:180`. `batch_inverse_core` still fails at
`batch_ops.rs:394:4-400:5` from `interp/InterpMatchCtxs.ml:419`, unchanged.
F3 is unchanged: `--monomorphize` still yields an LLBC with zero items.

### Committed-pipeline regression check

`upgrade/pipeline-regression.sh <baseline|new>` derives a harness from the
committed `scripts/verify-lean.sh` by four `sed` substitutions — redirect
`REPO_ROOT`, redirect `PROOFS_DIR`/LLBC paths into a scratch tree, swap the
`charon`/`aeneas` command words for the chosen pair, and truncate before
Step 4 — then diffs the regenerated Lean against committed `proofs/`. Running
it on **both** pairs gives a control, which turns out to be essential.

**Step 4 (`lake build`) was not run**, for the same reason and with the same
scoping: no built `proofs/.lake`, and neither REQ-01 nor REQ-02 turns on
elaboration. So this check answers whether extraction still reproduces the
committed generated Lean; it does **not** answer whether the hand-written proofs
still elaborate. That question belongs to the upgrade-migration issue proposed
below, which lists it as required work.

Results (`upgrade/excerpts/pipeline-regen-diff.txt`):

| | Pinned pair | Newest upstream |
|---|---|---|
| extraction stages exit | **0** | **1** |
| `Gf2Core/Types.lean` | identical | identical |
| `Gf2Core/Funs.lean` | **identical** | 311 non-comment lines differ |
| `Gf2Algebra/Types.lean` | comment-only (38) | comment-only (38) |
| `Gf2Algebra/Funs.lean` | 6 non-comment lines | 428 non-comment lines |

The pinned pair reproduces `Gf2Core` byte-for-byte. Its only `Gf2Algebra`
delta is pre-existing drift unrelated to this spike: source-line-number
comments, plus one function (`packed.packed5.Packed5.to_raw_planes`) that
current `gf2-algebra` sources define and the committed Lean predates.

The new pair fails, and the proximate cause is a committed post-processing
script aborting under `set -e` (`upgrade/logs-trimmed/pipeline-new.log:255`):

```
fix-aeneas-gf2algebra: expected exactly one Zip Iterator instance, got 0
```

Note what the error classes do **not** show
(`upgrade/excerpts/pipeline-new-breakage.txt`). The new Aeneas is not noisier —
it is quieter. The pinned pair emits five `[Error]` classes on the committed
extraction (16 `Assertion failed: new value doesn't have the same type as its
destination`, 2 `Could not find: trait`, 2 `Found type error in the output of
charon`, 24 `Internal error, please file an issue`, 2 `Internal error: please
file an issue`); the new pair emits one (the same 24 `Internal error`, split
2 in gf2-core and 22 in gf2-algebra). Those 24 are therefore **not** a
regression — they are present on the pinned pair too, and the pipeline
tolerates them today.

So the upgrade is not blocked by upstream being worse. It is blocked by
upstream being **different**: `scripts/fix-aeneas-gf2algebra.py` hard-fails on
the changed output, and `proofs/Gf2Core/Funs.lean` — which the hand-written
proofs in `proofs/Gf2Core/Proofs/` are written against — no longer regenerates
identically. That is the same shape of work as issue `150d7d79`, which needed
three extraction-pipeline fixes and four hand-proof files repaired.

### Upgrade verdict: blocked, and it is a migration project

Not adoptable as a drop-in. Adopting `charon 340b1af4` + `aeneas c10cc997`
requires, at minimum:

1. Repairing or retiring `scripts/fix-aeneas-gf2algebra.py`'s Zip-instance
   assumption, which currently aborts the pipeline.
2. Re-auditing `scripts/fix-aeneas-dupes.py` and `scripts/fix-aeneas-sorrys.py`
   against the changed output.
3. Re-verifying every hand-written proof in `proofs/Gf2Core/Proofs/` against a
   regenerated `Funs.lean` that differs by 311 non-comment lines, with a real
   `lake build` — the step this spike could not run.
4. Dropping the project-local Charon patch and the `verify-lean.sh:16-33`
   banner text that describes it, since upstream subsumed it.

Against that cost, the benefit for **this epic** is narrow: the upgrade fixes
F2, which was never the binding constraint, and leaves F4 — the finding that
actually decides REQ-05 — exactly as it was. The upgrade should therefore be
tracked as its own migration issue, in the manner of `150d7d79`, and REQ-05
should be decided on the evidence as it stands rather than waiting for it.

---

## Verdict

### Feasibility on the pinned pair (charon 0.1.217 + aeneas 5220259c)

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

### Feasibility on newest upstream (charon 340b1af4 + aeneas c10cc997)

**The same verdict, for the same reason.** The upgrade fixes F2 (all four
Charon panics) and improves F1's diagnosis, but F4 — the finding that decides
REQ-05 — reproduces character-for-character, and F3 is unchanged. Newest
upstream does not move REQ-05.

The one substantive gain is that `FiniteField` can now be made transparent, so
the F2-derived constraint "any generic proof must treat the field abstractly"
is no longer forced by the toolchain. It remains a reasonable *choice* for the
epic's trait-level-laws framing.

### Pipeline-compatibility status of the upgrade

**Blocked.** The pinned pair regenerates `proofs/Gf2Core/{Types,Funs}.lean`
byte-identically; the new pair aborts `scripts/fix-aeneas-gf2algebra.py` and
changes `Gf2Core/Funs.lean` by 311 non-comment lines. Adoption is a migration
project on the scale of issue `150d7d79`, not a drop-in, and it is not on
REQ-05's critical path. The host installation was left on the pinned pair;
replacing `~/.cargo/bin/{charon,charon-driver,aeneas}` was denied by the
permission system and needs the user's approval.

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
  to Aeneas — the lifted loop function drops the type parameters its parent
  binds — and re-run this matrix when it is fixed. The upgrade leg already
  tested the optimistic version of this option and it did not pay: newest
  upstream (`charon 340b1af4` + `aeneas c10cc997`) fixes F2 but reproduces F4
  character-for-character, so REQ-05 would block on an unreported, undated
  third-party fix. F2 needs no report; it is already fixed upstream.
- **DEC-C — Descope REQ-05 to concrete instantiations.** State the generic laws
  abstractly in Lean without binding them to extracted generic Rust, and keep
  the extracted-Rust binding at the concrete field types, as issue `0606186a`
  chose for `permanent_ryser` (`proofs/Gf2Algebra/Proofs/RyserBounded.lean:77-84`).
  Precedent exists in this repository; the cost is that REQ-05 no longer
  demonstrates anything about the *reconciled traits*.

Recommendation: **DEC-A**, with F4 filed upstream as a non-blocking follow-up.
It is the only option that both lands inside the epic and produces Lean that
mentions the reconciled trait hierarchy. The upgrade leg strengthens this: the
newest upstream pair has now been measured, and it does not change the answer.

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
| `dev/active/34d85cb9/extraction/probe-P{1,2,3,4}.rs` | Standalone probe sources, one per probe run |
| `dev/active/34d85cb9/extraction/A2_lean/` | **Preserved Lean for the primary generic target** (REQ-02) |
| `dev/active/34d85cb9/extraction/A8b_lean/` | Lean for `FiniteFieldExt::{square,pow,frobenius}` |
| `dev/active/34d85cb9/extraction/A11_lean/` | Lean for `gf2_core::field::poly` (F4 at scale) |
| `dev/active/34d85cb9/extraction/A{2b,3,4}_lean/` | Lean for the remaining Aeneas runs |
| `dev/active/34d85cb9/extraction/P{1,2,3,4}_lean/` | Lean for the probes |
| `dev/active/34d85cb9/extraction/excerpts/` | Toolchain versions, host context, LLBC item counts, three LLBC fragments |
| `dev/active/34d85cb9/logs-trimmed/` | Trimmed log per run, each opening with its command and closing with `exit=`/`elapsed=` |
| `dev/active/34d85cb9/extraction/{trim-logs,llbc-summary,versions,host}.sh` | Regenerate the derived artifacts above |
| `dev/active/34d85cb9/upgrade/extraction/` | The upgrade leg's `.cmd` files, runner, and generated Lean |
| `dev/active/34d85cb9/upgrade/logs-trimmed/` | Trimmed log per upgrade-leg run, including both pipeline legs |
| `dev/active/34d85cb9/upgrade/excerpts/` | New-pair versions, LLBC counts, F4 signature, pipeline diff and breakage tables |
| `dev/active/34d85cb9/upgrade/pipeline-regression.sh` | Runs the committed verify-lean.sh extraction stages on either pair and diffs against `proofs/` |
| `dev/active/34d85cb9/upgrade/charon-local-patch-487f0320.patch` | The project-local Charon patch as found in `/data/aeneas-build/charon` |
| `dev/active/34d85cb9/upgrade/excerpts/upstream-topology.txt` | Branch/tag topology receipt for the "newest upstream" claim |
| `dev/active/34d85cb9/upgrade/topology.sh` | Regenerates the topology receipt (read-only network) |
| `dev/active/34d85cb9/upgrade/versions-new.sh` | Regenerates the new-pair, OCaml and host-install receipts |
| `dev/active/34d85cb9/upgrade/logs-trimmed/charon-build.log` | Charon release build log; its `Finished` line is the build-duration receipt |
