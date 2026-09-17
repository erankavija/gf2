# Dense-parity measurement harness

> **Diátaxis Type:** Reference
>
> **Interface identity:** `2037941f-parity-measurement-interface-v1`
>
> **Owning issue:** `e1f9a78f`
>
> **Implements:** [`dense-parity-addendum.md`](dense-parity-addendum.md),
> identity `2037941f-dense-parity-v1`

This document is the `parity-measurement-interface` named by
[`plan.md`](plan.md). It fixes cell identity, semantic validation, route
provenance, append-only logging, checkpoint/resume, and machine-readable output
for the dense-parity questions.

The harness changes no production code and no production selection. Every
numeric setting reaches it from the frozen addendum or from the protocol's
shared settings in `dev/tools/tuning-campaign-support/src/protocol.rs`; the
launcher adds none.

## Target choice

The harness is one story-specific Cargo project, `survey/dense-harness/`
(package `dense-parity-harness`), outside the production workspace. No
established benchmark target is extended.

An established target is unsuitable here for three reasons that are properties
of the measurement, not preferences. The campaign arms are fresh child
processes speaking the canonical child-v2 framing of
`tuning_campaign_support::transport`, which a Criterion `[[bench]]` target does
not speak. The addendum's reference arm is the same sources built without the
`simd` feature, so two gf2 executables with different feature sets measure the
same cells. The M4RI arm links a separately qualified external library and a C
translation unit that reaches its public coordinate accessors, which are
`static inline` in `m4ri/mzd.h`. Extending `crates/gf2-core/benches/` with any
of those properties would change that target's behavioral identity, which
`@/inv/behavioral-evidence-validity` forbids.

The project is a standalone Cargo workspace. Its `Cargo.lock` is committed so a
receipt pins the dependency graph it measured. It is the sibling of
`survey/harness/`, the logical-buffer harness of issue `bb769456`, and follows
its structure; the git closure guard `src/inputs.rs` is the same mechanism in
both, whose one canonical home is `tuning_campaign_support` once the epic's
provenance freeze lifts.

## Cell identity and generation

`Question` has three ledger-owning variants in the order the addendum lists
them: `isolated-fused-parity`, `allocated-matvec`, `matvec-vs-m4ri`.
`dense_parity_harness::cells()` returns the complete cell table in that order,
then in each question's listed axis order: the normative primary product first,
then the exact exploratory additions in document order.

| Question | Cells | Identifier form |
|---|---:|---|
| `isolated-fused-parity` | 12 | `and-popcnt-{W}w-{warm,streaming}` |
| `allocated-matvec` | 24 | `matvec-r1024-{W}w[-tail1][-scalar-reference]-{warm,cold,streaming}` |
| `matvec-vs-m4ri` | 8 | `m4ri-gap-{R}x{C}[-retained]-warm` |

The addendum's fourth question, `scalar-reference`, owns no ledger. Its cells
are the five `matvec-r1024-{W}w-scalar-reference-warm` rows of the allocated
family, whose baseline arm is the executable built without the `simd` feature
and whose compared arm is the ordinary build.

The campaign seed is `0x2037_941f_96c9_4b81`. Cell ordinal zero carries the
campaign seed as its workload seed; ordinal $i$ carries the $i$-th subsequent
output of one `tuning_campaign_support::abtest::SplitMix64` stream started at
the campaign seed. Ordinals run across the whole table, not per question, so a
family's seeds depend on the position of its question in the addendum. Each
additional fixture bank consumes the next output of that cell's own stream,
started at its workload seed.

The three rows the addendum declares unavailable in advance,
`m4ri-gap-65x576-unqualified`, `m4ri-gap-65x4032-unqualified` and
`m4ri-gap-65x4160-unqualified`, carry no workload and no samples, so they take
no ordinal and generate no seed. They are not runner cells; the harness
publishes them as `cells::UNAVAILABLE_ROWS` with their strides and reasons, and
`dense-campaign pins` prints them, so the comparator family's receipt and table
generator retains each row with the reason the matched-operation specification
gives. Because those rows are exploratory they spend no comparison.

`dense-campaign cells` transcribes one family into a protocol version-4
campaign JSON addendum against `dev/active/f547c394/addendum.schema.json`. The
transcription is a pilot: every cell is `exploratory`,
`effect.measurement_resolution` and `effect.resolution_evidence` are null, and
`family_wise.prior_confirmatory_trials` is zero, because a version-4 addendum
derives prior comparisons from its authoritative ledger. A confirmation
addendum is derived from the committed pilot receipt by the canonical freezer
`dev/active/c7113c5a/survey/freeze-confirmation.py`; this harness writes no
confirmation addendum.

Margins, complexity budgets, search budgets, ledger paths, cache states,
objectives, metric kinds, and core arms are transcribed from the frozen
addendum's tables. `dense-campaign verify` re-derives the transcription and
compares it byte for byte with a candidate campaign JSON, so a campaign JSON
that changes a cell, margin, limit, or rule fails closed.

## Family ledgers

`tuning_campaign_support::trial_ledger::reserve` runs for every protocol
version at or above 2 and fails when the family ledger file is absent, so all
three ledger paths named by the frozen addendum exist as committed empty files
under `dev/bench_results/2037941f/`. An empty ledger is the explicit genesis
state; it reserves nothing. A family's confirmatory count is whatever that
ledger and the campaign addendum's non-exploratory cells admit; the harness
carries no confirmatory constant. The non-timed smoke reserves nothing at all
and names a throwaway ledger path under `target/`, so the committed ledgers
stay at genesis.

## Routes and provenance

`GF2_DENSE_ROUTE` selects one route in `dense-arm`. The M4RI route is the whole
of `dense-m4ri-arm`. A route names the entry point it calls; the gf2 routes
reach nothing past a public API, and the isolated route reaches the detected
bundle exactly as the public `matvec` does.

| Route | Timed body | Build identity |
|---|---|---|
| `and-popcnt-a`, `and-popcnt-b` | one `and_popcnt_fn` call on the detected bundle, behind an optimisation barrier | `conservative-portable` |
| `matvec-a`, `matvec-b` | one public `BitMatrix::matvec`, output allocation and appends inside, release outside | `conservative-portable` |
| `matvec-scalar-reference` | the same public call from the build without `simd`, which reaches the four-accumulator scalar row parity | `conservative-portable` |
| `m4ri-peer-gf2` | one public `BitMatrix::matvec` whose returned `BitVec` is also released inside the call | `conservative-portable` |
| `m4ri-mzd-mul` | `mzd_init` for `A`, `x`, `y`, packing, `mzd_mul(y, A, x, 0)`, unpacking into a gf2 `BitVec`, disposal | `external` |

`Route::check_build` refuses a route the running executable cannot serve: the
reference route requires the build without `simd` and every other route
requires the build with it, so neither executable can stand in for the other.

The `-a` and `-b` route pairs are the byte-identical public identity arms the
frozen addendum requires for a pre-candidate campaign. A candidate campaign
replaces only the compared build, through `--candidate-executable`; the route
names do not change, because the candidate is a different kernel body in the
bundle, not a different consumer entry.

Every arm reports `selected_path` from what it observes at run time: the entry
point, the lane `matvec_route` resolves for the cell's stride, the observed
rows, columns, stride and allocation base modulo 64, and the resident working
set. The isolated arm adds both operand addresses modulo 64. The M4RI arm adds
the matched operation, the qualified shape and the coordinate accessors. No arm
embeds a prior figure, file inventory, or host assertion.

Before timing is enabled, every allocated and comparator arm verifies the
constructed public objects: `verify_shape` checks the observed rows, columns,
vector length and stride against the frozen declaration, and `verify_lane`
checks that the stride and the build resolve the declared lane and that the
host detected a bundle when the SIMD lane is required. A mismatch makes the
cell unavailable; the harness never substitutes another shape.

### Output release and the allocated boundary

The allocated boundary charges the output allocation and its appends and
excludes the release, which the addendum states explicitly when it contrasts
the two gf2 arms: the comparator arm charges releasing the returned `BitVec`
and the allocated arm does not. `routes::OutputSink` implements that boundary.
Each timed call's output is retained in a `Vec` reserved to the calibrated call
count, and the batch is released in the timing helper's post-window callback,
which runs strictly outside the measured interval. The retention is bounded by
`MAX_RETAINED_OUTPUTS`; a window that would exceed the bound releases inside
itself rather than growing without limit, and the arm reports the sink's
capacity in `selected_path`.

One consequence belongs in the record rather than in a silent choice: while a
window retains its outputs, the allocator cannot reuse a freed block, so the
allocation cost the window charges is the cost of fresh blocks. The alternative,
charging the release inside the call, measures a different boundary from the one
the addendum froze. Both arms of every allocated cell pay the same retention, so
the paired statistic is unaffected; the absolute nanoseconds per `matvec` carry
that caveat.

## Cache policies

`Cache` implements the addendum's three states exactly. `warm` runs one untimed
pass of the measured operation over the cell's complete working set, including
the output the timed call writes, before calibration. `streaming` builds eight
fixture banks of at least 8 MiB each, rounded up to a whole number of tuples,
touches every initialized byte outside timing without executing the measured
operation, and rotates banks once per operation, for a reported working set of
at least 64 MiB. `cold` requires the frozen fixed call count, executes no
measured operation before the first window, and calibrates nothing.

## Semantic oracle

`dense_parity_harness::oracle::run` is deterministic, untimed, and emits no
timing sample. It reports one `PASS <case>: <n> checks` line per case. Each row
parity is recomputed bit by bit through the public `get` accessors of the
matrix and the vector, so the oracle shares no code with the measured route.

| Case group | Coverage |
|---|---|
| `matvec-cols-{C}-rows-{R}` | every pairing of the logical boundaries 0, 1, 63, 64, 65 on both the column count and the output length: every output bit against the parity oracle, the output length, `get` against the output words, zero tail padding, and immutability of the matrix and the vector against a rebuild at the same seed |
| `matvec-{W}w-{full,tail1}` | all seven word counts in both column shapes: the same product checks plus the zero padding bit of every `tail1` row and of the vector |
| `matvec-anchor-{W}w` | the five anchors at the frozen 1024-row geometry: observed rows, columns, stride, the lane the build resolves, and every output bit |
| `and-popcnt-{W}w` | all seven word counts: the bundle entry's count against an independent per-word count, its low bit against the row parity, and purity across two calls |
| `m4ri-{R}x{C}` | in `dense-m4ri-arm --oracle`: every unpacked output bit against the gf2 peer, zero tail padding, the retained arm against the fresh arm, and input immutability |

The oracle runs from both gf2 builds, so the reference arm's scalar route is
checked against the same parity oracle as the SIMD lane.

## Logging, checkpoint, and resume

Execution logging, checkpointing, and resume are the canonical mechanisms of
`tuning_campaign_support::journal`, which `benchmark-ab-runner` and the
non-timed smoke both drive: one append-only `execution.log` per stage, opened
before the first bounded run, with `campaign-start`, `cell-start`,
`cell-complete`, checkpoint and terminal records, and one immutable checkpoint
unit per cell under a manifest pinning the run's resume identity. That identity
pins the protocol document, the producing-input closure, the ordered cell list,
the arm descriptors, every arm executable, the campaign addendum and the plan.
The launcher prints the canonical log path before launching work and treats
console output as a view of that record. A paused session resumes from the same
stage and plan; completed cells are journalled as omissions and are not
measured again. `max_cells_per_session` bounds one session, and a session that
exhausts it exits 3.

The launcher verifies completion from the execution log, never from an exit
code: the terminal record is `complete` and every declared cell has one
`cell-complete`.

## Machine-readable output

The receipt directory is the canonical output of a timed run: `receipt.json`,
the plan, the pinned inputs, the execution log, the checkpoint manifest, and the
acceptance summary written by `benchmark-acceptance`. Arms emit exactly one
`zen3-benchmark-arm-result-v1` line each; the runner assembles
`zen3-benchmark-receipt-v1`. The harness defines no receipt schema of its own.

The non-timed smoke produces no receipt. Its output is
`dense-parity-nontimed-smoke-v1`, written as `handshake.json` beside the stage's
execution log: one entry per declared cell carrying the cell's declared cache
state and, for each arm, the arm name, its role, the executable digest, the
cache state the arm applied, the route provenance it observed, and its window
count, which is zero.

## Provenance artifacts

`survey/make-dense-producing-inputs.py` writes
`survey/dense-producing-inputs.json`, the content closure every receipt
snapshots: the measured crates' sources, the harness sources, the C comparator
shim, the shared campaign support, the frozen addendum, the matched-operation
specification and its qualification record, the protocol and contract
documents, and the build inputs. The build inputs carry every Cargo manifest and
lock file a timed executable is built from and the closure manifest itself,
which the window guard reads to decide what to check. The harness contract test
derives that manifest set from `cargo metadata` for both workspaces, so a new
crate on either path fails the test rather than slipping past the guard.
`survey/make-dense-parity-source-evidence.py` writes
`survey/dense-parity-source-evidence.json`, where every source claim the frozen
addendum makes records its project, commit, path, line, the verbatim line and
why. Both are regenerated rather than edited, so a claim that moves fails its
generator instead of going stale in prose.

## External comparator

`dense-m4ri-arm` builds only against the install the qualification record
[`m4ri-probe-record.txt`](m4ri-probe-record.txt) pins. `build.rs` reads the
pinned installed-library digest from that record and the compiler from the
install's own `gf2-m4ri-build-record.txt`, verifies the library against the
digest, and compiles `survey/m4ri_matvec_arm.c` with that compiler at the
qualification's own flags. A different install fails the build rather than
producing an arm whose external identity is unknown. The install is created by
`survey/run-m4ri-matvec-probe.sh` under the primary checkout's
`.agents/ext/92385645/prefix-qualified-v3` and is shared, never rebuilt
destructively.

The shim exposes exactly the charged components the matched-operation
specification defines and holds no timing loop: one fresh whole-consumer call,
and the retain, call and release of a retained-state cell.

## Entry points

The launcher is `dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/survey/run-dense-harness.sh`,
invoked from the worktree root. It exports `~/.cargo/bin` on `PATH` itself,
because the benchmark-window unit has no login shell.

```
STORY=dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations
$STORY/survey/run-dense-harness.sh build [--m4ri]
$STORY/survey/run-dense-harness.sh cells --family <family-id> --issue <8-hex> \
    --frozen-utc <YYYY-MM-DDTHH:MM:SSZ> --output <path>
$STORY/survey/run-dense-harness.sh smoke [--m4ri]
$STORY/survey/run-dense-harness.sh window --family <family-id> \
    --addendum <committed campaign JSON> --run-id <id> [--m4ri]
```

The launcher drives one Rust tool, `dense-campaign`, which a leaf may also call
directly: `pins` prints the frozen addendum's path, pinned digest, identity,
freeze time, the three family ledgers and the declared unavailable rows, and
fails when the document's bytes differ from the pin; `list` prints one family's
cells with their ordinals, arms and seeds; `cells`, `verify` and `plan` are the
transcription, the comparison and the plan projection; `smoke` drives one
non-timed session of a projected plan; `inputs` is the producing-input closure
guard.

`build` compiles the gf2 arms `conservative-portable` into
`target/e1f9a78f-arms` and the reference arm into `target/e1f9a78f-scalar-arm`,
runs the crate's own contract tests and both builds' semantic oracle, and writes
`survey/dense-harness-validation.txt`. With `--m4ri` it also compiles
`dense-m4ri-arm` against the qualified install into `target/e1f9a78f-m4ri-arm`.

`cells` writes one family's campaign JSON addendum. `--family` is one of
`2037941f-dense-isolated-fused-parity`, `2037941f-dense-allocated-matvec`,
`2037941f-dense-matvec-vs-m4ri`.

`window` refuses unless `GF2_BENCH_WINDOW=1`, the frozen prose addendum's
SHA-256 equals the pin the harness carries, and the campaign JSON matches
`dense-campaign verify`. It then rebuilds every executable it launches from the
current tree, and only afterwards runs `dense-campaign inputs`, which refuses
unless every path of the producing-input closure, plus the campaign JSON and the
family ledger, is tracked by git and identical to its committed content. A path
git does not track is a refusal, so a source added to a measured crate without
being committed stops the run. The closure check is the last step before the
launch: nothing rebuilds after it, so no executable can carry bytes the check
never saw. It then projects the plan, prints the execution log path, runs the
runner under `dev/scripts/ccx1-bench-flock.sh --full-host` until the log's
terminal record is `complete`, finalizes the receipt under
`dev/bench_results/2037941f/<family>/<run-id>-pilot`, and evaluates it with
`benchmark-acceptance`. A resumed invocation reuses the stored plan and refuses
when the current projection differs.

Each arm also refuses a hand invocation outside the window: without the child-v2
sentinel it requires `GF2_BENCH_WINDOW=1` and `GF2_BENCH=1` and exits before
reading a request.

`window` queues nothing by itself. Each timed line lives in
`dev/active/1a379447-zen3-cpu-performance/bench-window/queue.tsv` in the form
`issue<TAB>worktree<TAB>est_minutes<TAB>command`, where the command is the
`window` invocation above, and reaches the queue only after that family's
campaign JSON addendum is committed.

## Untimed release smoke

```
dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/survey/run-dense-harness.sh smoke [--m4ri]
```

The record's own `# command:` header names the invocation that wrote it, so the
committed `survey/dense-runner-smoke.txt` states whether its run covered the
M4RI family.

The command runs from the worktree root, takes no benchmark lock, never calls
`dev/scripts/ccx1-bench-flock.sh`, and writes nothing under `dev/bench_results/`.
It checks, in order:

1. **Cell generation.** Every family of the frozen addendum transcribes twice to
   identical bytes and validates against the version-4 schema through
   `FamilyAddendum::decode` and `validate`, including the comparator family when
   the wire smoke does not drive its arms.
2. **Semantics.** `dense-oracle` from both gf2 builds and, when the M4RI arm is
   built, `dense-m4ri-arm --oracle` report every case as `PASS`.
3. **The wire.** Every arm the family's plan declares runs as a fresh child
   process speaking the canonical child-v2 framing, from the projected plan and
   the throwaway campaign addendum, with a throwaway ledger path and stage under
   `target/`. The allocated family contributes its warm anchor, the frozen cold
   cell and the scalar-reference cell, so warm, the frozen cold call count and
   the reference build all reach the wire; the isolated family adds a streaming
   cell and the comparator family adds a retained-state cell. Every request
   declares zero timing windows, so each arm builds its fixture, verifies its
   shape and lane, runs the untimed arrangement its cache policy declares and
   answers with no timing window; a child that answers a zero-window request
   with a window fails the smoke.
4. **Append-only logging and resume.** `max_cells_per_session` is one, so every
   family pauses at least once and a later session completes the stage. The
   first session's execution log is a byte prefix of the final log, the resumed
   session journals a completed-in-prior-session omission, no cell carries two
   `cell-complete` records, no cell attempt is abandoned, the journal carries no
   `execution-progress` or `window-progress` record, one immutable checkpoint
   unit exists per cell, and the terminal record is `complete`.
5. **Output schema.** The stage's `handshake.json` decodes as
   `dense-parity-nontimed-smoke-v1`, names exactly the addendum's declared cells,
   and reports each arm's applied cache state and zero windows. The stage holds
   no finalized receipt.

`build` writes `survey/dense-harness-validation.txt` the same way: the
toolchain, the pins, the contract-test count, every oracle line and the
executable digests, all observed by that run.

The record is `survey/dense-runner-smoke.txt`. Every line is observed at run
time from the execution log, the checkpoint store and the handshake record; it
carries no clock reading, so a rerun on the same executables reproduces it byte
for byte. The smoke collects zero timing samples and finalizes zero receipts,
both stated in the record from observation, so it cannot serve as a pilot.

The smoke drives the arms directly rather than through `benchmark-ab-runner`.
The runner has one measurement path: a cell whose core arm resolves is measured
at the plan's pair count, and its paired statistic is computed from the median
of each execution's windows, which an execution with no window cannot supply. A
smoke that reached the arms through the runner would therefore collect timing
samples outside the benchmark window. The smoke instead speaks the runner's own
wire types and drives the runner's own journal, checkpoint store and resume
identity, so the contract it establishes is the contract the runner uses.
