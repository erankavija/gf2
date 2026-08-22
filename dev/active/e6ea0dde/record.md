# Record e6ea0dde — clean-exit extraction of the loop-free generic field operations

Extraction record for epic `6dc81018`, criterion
`@/issue/6dc81018/requirement/REQ-05`. Run 2026-08-22 in the worktree
`.agents/worktrees/agent-e6ea0dde` at `f9befb215fab3a0affdfc31ad16da2146aca0948`.
No proofs are authored here; no production code and nothing under `proofs/` is
modified.

**What this record establishes.** The Charon and Aeneas invocations for the
`FiniteFieldExt::square` / `frobenius` class both complete with exit code 0 on
the pinned toolchain pair and the workspace MSRV, with the falsified `pow`
default excluded at the invocation boundary by a Charon `--exclude` pattern.
The generated `Funs.lean` carries the four target definitions and zero `sorry`
occurrences.

**What this record does not establish.** The generated Lean does *not*
elaborate as generated. Aeneas emits the `FiniteField` trait as a Lean
`structure` with six field names that each occur two or three times, and Lean
rejects it. That defect is upstream, is independent of the `pow` exclusion, and
no invocation-boundary option in the pinned pair avoids it — six probes and the
Aeneas naming code establish that below. Nothing is repaired, filtered, or
hand-edited in response; the blocking evidence stands as the record's finding,
per `@/inv/falsification-preserved`.

Every command in the matrices below is executed from a committed `.cmd` file via
`dev/active/e6ea0dde/extraction/run.sh`, which prints the command file into the
log before executing it. The command text quoted in this record is therefore
byte-identical to what ran.

---

## Question

The spike record `dev/active/34d85cb9/findings.md` documents an Aeneas
invocation (A8b) that exits 1: the blanket-default `pow` item its finding F4
falsifies poisons the run, and the proof-obligation sketch
`dev/active/1ac74567/proof-sketch.md` elaborates its lemma statements only
against a tree from which the `pow` chain is deleted by hand. The epic owner
ruled that this does not demonstrate a successful extraction: the invocation
itself must exit 0 and the output must elaborate as generated.

So: can the pinned pair be driven to exit 0 on this class with `pow` kept out at
the invocation boundary, and does what it then generates elaborate verbatim?

---

## Toolchain and host

Captured at run time in
`dev/active/e6ea0dde/extraction/excerpts/toolchain-versions.txt` (regenerate
with `dev/active/e6ea0dde/extraction/versions.sh`). Three toolchains are in
play:

| Component | Version | Role |
|---|---|---|
| `RUSTUP_TOOLCHAIN=1.95.0 cargo --version` | `1.95.0 (f2d3ce0bd 2026-03-21)` | builds `gf2-core` |
| `RUSTUP_TOOLCHAIN=1.95.0 rustc --version` | `1.95.0 (59807616e 2026-04-14)` | builds `gf2-core` |
| `charon version` | `0.1.217` | the pinned pair |
| `charon toolchain-version` | `nightly-2026-06-01` | rustc that produces MIR |
| `aeneas -version` | `5220259c` | the pinned pair |
| `proofs/lean-toolchain` | `leanprover/lean4:v4.30.0-rc2` | elaboration |

The workspace MSRV is `rust-version = "1.95"`
(`crates/gf2-core/Cargo.toml:10`), and each Charon command file begins with
`RUSTUP_TOOLCHAIN=1.95.0`, so the pin is part of the `.cmd` contract rather than
an ambient property of the shell. The pinned pair is the host installation at
`~/.cargo/bin/{charon,charon-driver,aeneas}`; no host binary is swapped or
upgraded. One diagnostic probe (Q6) invokes the already-built newest-upstream
pair by absolute path under `/data/aeneas-upgrade-34d85cb9/`, the tree the
spike's upgrade leg built; it writes nothing outside this record.

The elaboration harness reads the `proofs/` lake project's environment through
`lake env`, which needs that project's `.lake` to be materialized. A git
worktree does not carry one, so for these runs `proofs/.lake` is a symlink to
the built project of the primary checkout. The symlink is a local environment
fixup: it is gitignored, it is removed after the runs, and `lake env` builds
nothing.

---

## The invocation chain

Three Charon runs and three Aeneas runs make up the chain. `X1` is the spike's
`R8b`/`A8b` pair reproduced here as a control; `X2` and `X3` are the two
invocation-boundary exclusions of `pow`.

| Run | Command | Exit | Outcome | Evidence |
|---|---|---|---|---|
| X1 | `extraction/X1.cmd` | 0 | The spike's R8b, dest-file redirected here | `logs-trimmed/X1.log` |
| AX1 | `extraction/AX1.cmd` | **1** | Reproduces A8b: 7 defs, 4 `sorry` in the `pow` chain | `logs-trimmed/AX1.log`, `extraction/AX1_lean/` |
| X2 | `extraction/X2.cmd` | 0 | `pow` opaque; start roots `square` and `frobenius` | `logs-trimmed/X2.log` |
| AX2 | `extraction/AX2.cmd` | **0** | 4 defs, 0 `sorry`; `pow.default` becomes an axiom in the external template | `logs-trimmed/AX2.log`, `extraction/AX2_lean/` |
| X3 | `extraction/X3.cmd` | 0 | `pow` excluded; start roots `square` and `frobenius` | `logs-trimmed/X3.log` |
| AX3 | `extraction/AX3.cmd` | **0** | 4 defs, 0 `sorry`; `pow.default` absent entirely | `logs-trimmed/AX3.log`, `extraction/AX3_lean/` |

`AX1_lean/Types.lean` is byte-identical to the spike's
`dev/active/34d85cb9/extraction/A8b_lean/Types.lean`, and `AX1_lean/Funs.lean`
is identical to the spike's modulo the generated module prefix, so the control
confirms this worktree reproduces the spike's toolchain behaviour exactly.

**X3 is the chain of record.** Its exact command, as executed:

```
RUSTUP_TOOLCHAIN=1.95.0 charon cargo \
  --preset aeneas \
  --rustc-arg=--cfg=verify_lean \
  --start-from 'gf2_core::field::traits::FiniteFieldExt::square' \
  --start-from 'gf2_core::field::traits::FiniteFieldExt::frobenius' \
  --opaque 'gf2_core::field' \
  --include 'gf2_core::field::traits::FiniteFieldExt' \
  --exclude 'gf2_core::field::traits::FiniteFieldExt::pow' \
  --opaque 'gf2_core::gf2m' \
  --opaque 'gf2_core::gfpn' \
  --opaque 'gf2_core::gfp' \
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
  --dest-file dev/active/e6ea0dde/extraction/X3_gf2_core.llbc \
  -- --manifest-path crates/gf2-core/Cargo.toml --no-default-features
```

followed by

```
aeneas \
  -backend lean \
  -dest dev/active/e6ea0dde/extraction/AX3_lean \
  -split-files \
  -print-error-emitters \
  dev/active/e6ea0dde/extraction/X3_gf2_core.llbc
```

Three things change relative to the spike's R8b: the start roots become the two
target methods instead of `pow`, `--exclude` keeps `pow` out of the translation,
and the dest paths move into this record. Everything else — the `aeneas` preset,
the `verify_lean` cfg, the opacity list, the cargo args — is the spike's line.

### What is excluded, and why

Spike finding F4 falsifies the extraction of every `F`-carrying loop in
`FiniteField`-generic `gf2-core` code: the lifted loop function drops the
associated-type binders its parent binds and Aeneas emits `sorry` into its
*signature*. `FiniteFieldExt::pow`
(`crates/gf2-core/src/field/traits.rs:1074`) is a square-and-multiply loop over
`Self`, so it is exactly that case, and it is the single item that makes A8b
exit 1. F4 reproduces character-for-character on newest upstream and is tracked
for upstream report as `@/issue/4dd5372a/requirement/REQ-04`.

Excluding `pow` costs the target class nothing, because neither target calls
`pow.default`. `frobenius`'s body calls the trait method through the dictionary
(`crates/gf2-core/src/field/traits.rs:1130`), and the extraction preserves that:
`AX3_lean/Funs.lean:81` ends `frobenius.default` with

```lean
  FiniteFieldExtInst.pow self exp
```

`pow` therefore survives as a *field* of the extracted `FiniteFieldExt`
structure (`AX3_lean/Types.lean:122`) whose value each backend supplies, which
is where the sketch already puts the obligation (`LawfulPow` in
`dev/active/1ac74567/elaboration/statements.lean:55-56`). What the exclusion
removes is only the falsified *default body* and the two lifted loop items under
it.

The two mechanisms differ in one observable way. Under `--opaque` (X2) Aeneas
emits `pow.default` as an `axiom` into `FunsExternal_Template.lean`; under
`--exclude` (X3) the item is absent from the LLBC and no axiom is emitted. X3 is
the chain of record because it leaves nothing about the falsified item to be
supplied later.

---

## Verbatim elaboration

`dev/active/e6ea0dde/elaboration/elaborate.sh` stages the generated tree into a
fresh `mktemp` directory and elaborates it under the `proofs/` lake environment.
The committed receipt is `dev/active/e6ea0dde/elaboration/elaborate.log`.

The only filesystem operation between Aeneas and Lean is the rename
`FunsExternal_Template.lean` → `FunsExternal.lean` that the template's own
generated header instructs; the harness asserts byte-identity across it with
`cmp`, and asserts that `Types.lean` and `Funs.lean` reach Lean unchanged.

Stage 1 fails:

```
X3Gf2Core/Types.lean:94:2: error: Field `corecloneCloneInst` has already been declared
X3Gf2Core/Types.lean:119:20: error(lean.unknownIdentifier): Unknown identifier `field.traits.FiniteField`
exit=1  X3Gf2Core/Types.lean
```

The second error is the cascade of the first: the `structure` command aborts, so
the name it would introduce is unknown to the `FiniteFieldExt` declaration
below it. `FunsExternal.lean` and `Funs.lean` then fail only for the missing
`Types.olean`.

### The defect

`AX3_lean/Types.lean:72-112` declares the extracted `FiniteField` trait as a
Lean structure whose fields include the trait's supertrait bounds and the bounds
on its two associated types. Six field names occur more than once, listed per
tree in `extraction/excerpts/tree-summary.txt`: `corecloneCloneInst` (three
times), `corecmpEqInst`, `corecmpPartialEqInst`, `corefmtDebugInst`,
`coreopsarithAddInst`, `coreopsarithAddAssignInst`.

Each collision pairs a bound on `Self` with a bound on `Self_Characteristic` or
`Self_Wide`. The source is `crates/gf2-core/src/field/traits.rs:44-70`: the
trait requires `Clone + PartialEq + Eq + Hash + Debug + Add + … + AddAssign` of
`Self`, `type Characteristic: Clone + Debug + PartialEq + Eq`, and
`type Wide: Clone + Add<Output = Self::Wide> + AddAssign`. Under the `aeneas`
preset the two associated types become type parameters and their bounds become
clauses of the trait itself, so all of them land in one Lean structure.

Aeneas names such a field from the trait reference alone
(`ctx_compute_trait_parent_clause_name`, host-local `aeneas 5220259c` at
`/data/aeneas-build/src/extract/ExtractBase.ml:1998-2016`, via
`trait_name_with_generics_to_simple_name` at
`/data/aeneas-build/src/TranslateCore.ml:56`). Type-variable arguments
contribute nothing to that name, so `Clone Self`, `Clone Self_Characteristic`
and `Clone Self_Wide` all name themselves `corecloneCloneInst`. Aeneas takes
this to be safe on the strength of a hard-coded backend assumption — the Lean
branch of `/data/aeneas-build/src/Main.ml:516-517` reads

```ocaml
(* Lean can disambiguate the field names *)
record_fields_short_names := true;
```

which holds across structures and not within one. The alternative branch does
not help either: it prefixes the clause with the *trait declaration's* name,
which is identical for every clause of `FiniteField`. There is no command-line
option for it in any case; the assignment is unconditional for the Lean backend.

This is the same emission that `scripts/fix-aeneas-dupes.py` exists to repair
for the committed `gfp`/`gfpn`/`gf2m` pipeline. Repairing it here is out of this
issue's contract, which asks for output that elaborates as generated.

### Why no invocation-boundary option avoids it

The exclusion has to keep the colliding clauses out of the emitted structure
without removing what the two targets need. `square.default` reads
`corecloneCloneInst` and `coreopsarithMulInst` off the `FiniteField` dictionary
(`AX3_lean/Funs.lean:31-32`), and `frobenius.default` reads `characteristic`,
whose result type is the `Characteristic` parameter. Both the `Clone Self`
clause and the `Characteristic` parameter are therefore load-bearing, and they
are two of the three occurrences of the colliding name.

Five probes test the options that could plausibly change the emission, plus one
on newest upstream. All run through the same `.cmd` contract:

| Probe | Change from X3 | Charon | Aeneas | Colliding field names |
|---|---|---|---|---|
| Q1 | `--exclude` on the `Wide` associated type | 0 | 0 | unchanged, all six |
| Q2 | `--remove-adt-clauses` | **1** | not reached | Charon rejects the flag |
| Q3 | `--hide-marker-traits` | 0 | 0 | unchanged, all six |
| Q4 | `--exclude` on the `FiniteField` trait | 0 | **2** | Aeneas raises `Not_found` |
| Q5 | `--no-normalize` | 0 | **2** | Aeneas raises `Not_found` |
| Q6 | newest-upstream pair, X3 otherwise | 0 | 0 | unchanged, all six |

Reading the rows:

- **Q1** shows item-level exclusion does not reach a trait's clause list. The
  `Wide` bounds survive `--exclude` on `Wide` itself.
- **Q2** is rejected by Charon with
  `` `--remove-adt-clauses` should be used with `--lift-associated-types='*'` to avoid missing clause errors ``
  (`logs-trimmed/Q2.log`). The flag is redundant regardless: the `aeneas` preset
  already sets `remove_adt_clauses`, `lift_associated_types = ["*"]`,
  `hide_marker_traits`, `remove_unused_self_clauses` and
  `duplicate_defaulted_methods`
  (host-local `charon 0.1.217` at
  `/data/aeneas-build/charon/charon/src/options.rs:425-437`). Every relevant
  simplification pass is on already, and the pass itself strips clauses from
  *type* declarations, not from trait declarations.
- **Q3** confirms the marker-trait filter is orthogonal: `Clone`, `Debug`,
  `PartialEq`, `Eq`, `Add` and `AddAssign` are ordinary traits.
- **Q4** removes the trait the collision lives in, and Aeneas aborts with an
  uncaught `Not_found` while extracting `FiniteFieldExt`, whose parent clause
  now dangles. It writes a two-structure `Types.lean` and no `Funs.lean`.
- **Q5** switches normalization off, which is the only way to stop the
  associated types from becoming type parameters. Aeneas again aborts with
  `Not_found`; the partial tree it leaves has no `FiniteField` structure at all
  and `sorry` in its place at `AQ5_lean/Types.lean:74`. The absence of
  collisions in that tree is the absence of the declaration, not a fix.
- **Q6** answers the obvious escalation question. The newest-upstream pair the
  migration issue `4dd5372a` tracks — `charon 0.1.232` and `aeneas c10cc99` —
  produces the same six colliding names. The upgrade does not resolve this.

The `--preset=aeneas` marker is not negotiable either: the spike's A9 records
Aeneas refusing an LLBC produced without it.

So the defect is reachable from no option the pinned pair offers, and from none
the tracked upgrade offers. Clearing it needs either an upstream fix to the
clause-field naming or a change to the bounds `FiniteField` declares on its
associated types, which is a production-API decision outside this issue.

---

## Diagnostic stages

Stages 2 and 3 of the harness exist to bound the blocker. They apply
`scripts/fix-aeneas-dupes.py` to the stage-1 tree. **A repaired tree is not an
answer to REQ-02**; these stages establish only that the duplicate-field
emission is the *sole* obstacle between this extraction and a tree that
elaborates.

The pass rewrites eight field names in `Types.lean` and changes `Funs.lean` not
at all; the full diff is in `elaboration/elaborate.log`. On that tree:

- `Types.lean`, `FunsExternal.lean` and `Funs.lean` each elaborate with exit 0.
- **Axiom dependencies of the extracted definitions**, from `#print axioms`:

  | Definition | Axioms |
  |---|---|
  | `FiniteFieldExt.square.default` | none |
  | `FiniteFieldExt.frobenius.default` | `propext`, `Classical.choice`, `Quot.sound`, `…frobenius.default_loop.body._native.decide.ax_3` |
  | `FiniteFieldExt.frobenius.default_loop` | the same four |
  | `FiniteFieldExt.frobenius.default_loop.body` | the same four |

  The fourth axiom is Lean's own native-decide certificate for a string-length
  side condition, printed in full in the log:

  ```
  axiom gf2_core.field.traits.FiniteFieldExt.frobenius.default_loop.body._native.decide.ax_3 : decide
      ("Frobenius exponent overflow".toByteArray.size ≤ Aeneas.Std.U32.max) =
    true
  ```

  This matches the set the sketch's own elaboration receipt records, so the
  exclusion of `pow` changes neither target's axiom dependencies.

- The proof sketch's lemma statements elaborate against this tree with exit 0,
  the nine `declaration uses 'sorry'` warnings being the deliberately unproved
  bodies. `elaboration/Statements.lean` differs from
  `dev/active/1ac74567/elaboration/statements.lean` in exactly two respects: the
  generated module prefix, which follows the LLBC file name and is `X3Gf2Core`
  here rather than `R8bGf2Core`; and one docstring citation in L4, readdressed
  from `A8b_lean/Funs.lean:114` to `AX3_lean/Funs.lean:47`, the line of the
  `checked_mul` it describes in the tree it now elaborates against. Every lemma
  statement is byte-identical, and every dictionary field the statements project
  (`corecloneCloneInst`, `coreopsarithMulInst`, `characteristic`, `pow`) keeps
  its name.

---

## Status against the issue's criteria

- **REQ-01 — met.** X3 exits 0 and AX3 exits 0 on the workspace MSRV with the
  pinned pair, with the falsified `pow` default excluded by
  `--exclude 'gf2_core::field::traits::FiniteFieldExt::pow'` at the invocation
  boundary. No generated file is edited.
- **REQ-02 — not met, blocked upstream.** The generated tree does not elaborate
  as generated: Aeneas emits a Lean structure with repeated field names. The
  axiom dependencies of `square` and `frobenius` are recorded above, read off
  the diagnostic tree.
- **REQ-03 — the sketch half is met on the diagnostic tree.** The sketch's
  lemma statements elaborate with exit 0 against the newly generated
  definitions, with the two adjustments recorded above; they have not been
  elaborated against a verbatim tree, because no verbatim tree elaborates.

---

## Artifact index

All paths relative to the repository root.

| Path | Contents |
|---|---|
| `dev/active/e6ea0dde/extraction/run.sh` | Runner: executes one `.cmd` file, tees to `logs/<run>.log` |
| `dev/active/e6ea0dde/extraction/X{1,2,3}.cmd`, `AX{1,2,3}.cmd` | The chain's six exact invocations |
| `dev/active/e6ea0dde/extraction/Q{1..6}.cmd`, `AQ{1,3,4,5,6}.cmd` | The exclusion-mechanism probes |
| `dev/active/e6ea0dde/extraction/AX3_lean/` | **The generated Lean of record** |
| `dev/active/e6ea0dde/extraction/AX{1,2}_lean/` | Control and `--opaque`-variant Lean |
| `dev/active/e6ea0dde/extraction/AQ*_lean/` | Probe output, including the two partial trees |
| `dev/active/e6ea0dde/extraction/excerpts/toolchain-versions.txt` | Toolchain identities, captured at run time |
| `dev/active/e6ea0dde/extraction/excerpts/tree-summary.txt` | Per-tree definitions, `sorry` counts and colliding field names |
| `dev/active/e6ea0dde/extraction/{versions,tree-summary,trim-logs}.sh` | Regenerate the derived artifacts above |
| `dev/active/e6ea0dde/logs-trimmed/` | Trimmed log per run, each opening with its command and closing with `exit=`/`elapsed=` |
| `dev/active/e6ea0dde/elaboration/elaborate.sh` | Three-stage elaboration harness |
| `dev/active/e6ea0dde/elaboration/elaborate.log` | Its committed receipt, including the stage-1 failure |
| `dev/active/e6ea0dde/elaboration/Statements.lean` | The sketch's lemma statements, readdressed to this tree |

`.llbc` files are not committed: `.gitignore:62` excludes them and they are
regenerable from the committed `.cmd` files. Raw logs are excluded by
`dev/active/e6ea0dde/.gitignore`; `logs-trimmed/` is the committed evidence.
