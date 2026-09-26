# Logical-buffer measurement harness

> **Diátaxis Type:** Reference
>
> **Interface identity:** `2037941f-logical-measurement-interface-v1`
>
> **Owning issue:** `bb769456`
>
> **Implements:** [`logical-buffer-addendum.md`](logical-buffer-addendum.md),
> identity `2037941f-logical-buffer-v1`

This document is the `logical-measurement-interface` named by
[`plan.md`](plan.md). It fixes cell identity, semantic validation, route
provenance, append-only logging, checkpoint/resume, and machine-readable output
for the four logical-buffer questions. The downstream baseline leaves
(`c489745b` isolated XOR, `8197174d` row XOR, `c49e78bc` coding route,
`65c0e13d` ISA-L) consume the entry points named below.

The harness changes no production code and no production selection. Every
numeric setting reaches it from the frozen addendum or from the protocol's
shared settings in `dev/tools/tuning-campaign-support/src/protocol.rs`; the
launcher adds none.

## Target choice

The harness is one story-specific Cargo project,
`survey/harness/` (package `logical-buffer-harness`), outside the production
workspace. No established benchmark target is extended.

An established target is unsuitable here for two reasons that are properties of
the measurement, not preferences. The campaign arms are fresh child processes
speaking the canonical child-v2 framing of
`tuning_campaign_support::transport`, which a Criterion `[[bench]]` target does
not speak. The ISA-L arm additionally links a C translation unit compiled at the
comparator specification's own flags, and the gf2 arms must stay
`conservative-portable`, so the two arms are separate executables with separate
build identities. Extending `crates/gf2-core/benches/` with either property
would change that target's behavioral identity, which
`@/inv/behavioral-evidence-validity` forbids.

The project is a standalone Cargo workspace. Its `Cargo.lock` is committed so a
receipt pins the dependency graph it measured.

## Cell identity and generation

`Question` has four variants in the order the addendum lists them:
`isolated-xor`, `public-row-xor`, `nr-bg2-construction`, `isal-base-gap`.
`logical_buffer_harness::cells()` returns the complete cell table in that order,
then in each question's listed axis order: the normative primary product first,
then the exact exploratory additions in document order.

| Question | Cells | Identifier form |
|---|---:|---|
| `isolated-xor` | 17 | `xor-{W}w-{a64,o8}-{warm,streaming}` |
| `public-row-xor` | 17 | `row-xor-{W}w-{full,tail63}-{warm,streaming}` |
| `nr-bg2-construction` | 6 | `nr-construct-{suffix}-{warm,cold}` |
| `isal-base-gap` | 12 | `isal-base-gap-{W}w-a64-{warm,streaming}` |

The campaign seed is `0x2037_941f_3ea1_22df`. Cell ordinal zero carries the
campaign seed as its workload seed; ordinal $i$ carries the $i$-th subsequent
output of one `tuning_campaign_support::abtest::SplitMix64` stream started at
the campaign seed. Ordinals run across the whole table, not per question, so a
family's seeds depend on the position of its question in the addendum. Each
additional fixture bank consumes the next output of that cell's own stream,
started at its workload seed.

The declared unavailable row `isal-dispatched-xor-gen` carries no workload and
no samples, so it takes no ordinal and generates no seed. It is not a runner
cell: the runner's only unavailable path is an unresolvable core arm. The
ISA-L family's receipt and table generator in `65c0e13d` carries that row with
the NASM-absence reason from [`isal-comparator.md`](isal-comparator.md); the
row spends no comparison, and the family's confirmatory reservation is the
five scalar-gap cells the addendum's Amendment 1 declares ($m = 5$).

`logical-campaign cells` transcribes one family into a protocol version-4
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
addendum's tables. `logical-campaign verify` re-derives the transcription and
compares it byte for byte with a candidate campaign JSON, so a campaign JSON
that changes a cell, margin, limit, or rule fails closed.

## Family ledgers

`tuning_campaign_support::trial_ledger::reserve` runs for every protocol
version at or above 2 and fails when the family ledger file is absent, so all
four ledger paths named by the frozen addendum exist as committed empty files
under `dev/bench_results/2037941f/`. An empty ledger is the explicit genesis
state; it reserves nothing. A family's confirmatory count is whatever that
ledger and the campaign addendum's non-exploratory cells admit; the harness
carries no confirmatory constant. The non-timed smoke opens no ledger, and its
throwaway campaign addendum names a ledger path under `target/`, so the
committed ledgers stay at genesis.

## Routes and provenance

`GF2_LOGICAL_ROUTE` selects one route in `logical-arm`. The ISA-L route is the
whole of `logical-isal-arm`. A route names the production entry point it calls;
none of them reaches past a public API.

| Route | Timed body | Build identity |
|---|---|---|
| `public-xor-a`, `public-xor-b` | `gf2_core::kernels::ops::xor_inplace` once per logical operation | `conservative-portable` |
| `resolved-xor` | one `resolve_xor_inplace(W)` pointer resolved outside timing, invoked once per operation | `conservative-portable` |
| `row-xor-a`, `row-xor-b` | one public `BitMatrix::row_xor(dst, src)` per operation, cycling the eight directed row pairs | `conservative-portable` |
| `nr-construct-a`, `nr-construct-b` | one whole `QuasiCyclicLdpc::nr_5g_rate_matched(bg, n, k)` per operation | `conservative-portable` |
| `isal-peer-gf2` | fresh 64-byte-aligned destination, copy of source zero, `xor_inplace` with source one, output observation | `conservative-portable` |
| `isal-xor-gen-base` | fresh 64-byte-aligned destination, `void *` array construction, `xor_gen_base`, output observation | `external` |

`resolved-xor` is exploratory attribution at $W \in \{8, 9\}$ only. The `-a`
and `-b` route pairs are the byte-identical public identity arms the frozen
addendum requires for a pre-candidate campaign; they differ only in the arm name
the runner reports.

Every arm reports `selected_path` from what it observes at run time: the
production entry point, the backend `select_backend_for_size` resolves for the
cell's word count, and the cell's observed geometry. The row arm adds the
allocation base address and every directed pair's source and destination address
modulo 64. The NR arm adds the lifting factor, dense dimensions, stride, and a
digest of the returned sparse parity-check structure, all read from the returned
public objects. The ISA-L arm reports the linked symbol and the qualified
dispatch availability. No arm embeds a prior figure, file inventory, or host
assertion.

The ISA-L arm links `raid/raid_base.c` from the pinned ISA-L checkout described
in [`isal-comparator.md`](isal-comparator.md), verifying the three pinned
SHA-256 values before compiling it with that specification's flags. The arm
refuses to build without the pinned source.

## Cache policies

`Cache` implements the addendum's three states exactly. `warm` runs one
untimed pass of the measured operation over the cell's complete working set
before calibration. `streaming` builds eight fixture banks of at least 8 MiB
each, rounded up to a whole number of tuples or matrices, touches every
initialized byte outside timing without executing the measured operation, and
rotates banks once per operation, for a reported working set of at least
64 MiB. `cold` requires the frozen fixed call count, executes no measured
operation before the first window, and calibrates nothing.

## Semantic oracle

`logical_buffer_harness::oracle::run` is deterministic, untimed, and emits no
timing sample. It reports one `PASS <case>: <n> checks` line per case, followed
by `[<facts>]` where the case establishes observed facts: the row cases report
the stride, column count and observed offsets they checked, and the NR cases
the lifting factor, dense dimensions, stride with its band, non-zero count and
structure digest they checked.

| Case group | Coverage |
|---|---|
| `xor-bits-{L}` | logical bit lengths 0, 1, 63, 64, 65: every output word, every bit by `word >> b & 1`, logical length, zero tail padding, source immutability |
| `xor-{W}w-{a64,o8}` | all seven word counts in both layouts: observed address modulo 64, every output word, source immutability, call parity after two applications |
| `row-xor-{W}w-{full,tail63}` | all seven word counts, full and `tail63` shapes, the four bidirectional row-pair groups: every row word against an independent model, `get(row, col)` against `row_words`, zero padding bit, and every directed pair's observed source and destination offset against the production allocation's own stride |
| `nr-construct-{suffix}` | the five selected-route targets: lifting factor, dense dimensions, stride and its mid-range band, and sparse-structure digest read from the returned public objects; the digest against the canonical TS 38.212 expansion the harness computes from the public shift table; and the seeded boundary messages of each route against the linear-code law, encoded through two independently constructed objects |
| `isal-{W}w` | in `logical-isal-arm --oracle`: 32-byte alignment, poisoned fresh destination, pointer-array order, every output word and bit against the gf2 peer, source immutability |

A mismatch in the NR group makes that cell unavailable; the harness never
substitutes another target.

## Logging, checkpoint, and resume

Execution logging, checkpointing, and resume are the canonical mechanisms of
`tuning_campaign_support::journal`, which `benchmark-ab-runner run` drives: one
append-only `execution.log` per stage, opened
before the first bounded run, with `campaign-start`, `cell-start`,
`cell-complete`, checkpoint and terminal records, and one immutable checkpoint
unit per cell under a manifest pinning the run's resume identity. The launcher
prints the canonical log path before launching work and treats console output as
a view of that record. A paused session resumes from the same stage and plan;
completed cells are not measured again. `max_cells_per_session` bounds one
session, and a session that exhausts it exits 3.

The launcher verifies completion from the execution log, never from an exit
code: the terminal record is `complete` and every declared cell has one
`cell-complete`.

## Machine-readable output

The receipt directory is the canonical output of a timed run: `receipt.json`,
the plan, the pinned inputs, the execution log, the checkpoint manifest, and the
acceptance summary written by `benchmark-acceptance`. Arms emit exactly one
`zen3-benchmark-arm-result-v1` line each; the runner assembles
`zen3-benchmark-receipt-v1`. The harness defines no receipt schema of its own.

The non-timed smoke produces no receipt. Its output is the shared
`zen3-arm-smoke-record-v1` record of each family, which `check-smoke.py`
projects into `survey/logical-runner-smoke.txt`.

## Provenance artifacts

`survey/make-logical-producing-inputs.py` writes
`survey/logical-producing-inputs.json`, the content closure every receipt
snapshots: the two measured crates' sources, the isolated SIMD kernels, the
harness sources, the shared campaign support, the frozen addendum, the
comparator specification, the protocol and contract documents, and the build
inputs. The build inputs carry every Cargo manifest and lock file a timed
executable is built from — the harness workspace manifest and lock, the
manifests of `gf2-core`, `gf2-coding` and `gf2-kernels-simd`, and the root
workspace manifest, root lock and `dev/tools/tuning-campaign-support` manifest
that the window's `--locked -p tuning-campaign-support` build resolves — and the
closure manifest itself, which the window guard reads to decide what to check.
The harness contract test derives that manifest set from `cargo metadata` for
both workspaces, so a new crate on either path fails the test rather than
slipping past the guard.

The source sections are a snapshot of the tree they were enumerated from, so
`make-logical-producing-inputs.py --check` regenerates the closure from the
current tree, writes nothing, and exits non-zero naming every added and removed
path when the committed manifest differs. The window runs it immediately before
the closure guard, so a source added to a measured crate after the last
regeneration refuses the run instead of being timed outside the closure. A
harness contract test copies the committed build inputs into a scratch tree,
adds a source under a measured crate and asserts the refusal names it.

`survey/make-logical-source-evidence.py` writes
`survey/logical-source-evidence.json`, where every source claim the harness
makes records its project, commit, path, line, the verbatim line and why. Both
are regenerated rather than edited, so a claim that moves fails its generator
instead of going stale in prose.

## Entry points

The launcher is `dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/survey/run-logical-harness.sh`,
invoked from the worktree root.
It exports `~/.cargo/bin` on `PATH` itself, because the benchmark-window unit
has no login shell.

```
STORY=dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations
$STORY/survey/run-logical-harness.sh build [--isal]
$STORY/survey/run-logical-harness.sh cells --family <family-id> --issue <8-hex> \
    --frozen-utc <YYYY-MM-DDTHH:MM:SSZ> --output <path>
$STORY/survey/run-logical-harness.sh smoke [--isal]
$STORY/survey/run-logical-harness.sh window --family <family-id> \
    --addendum <committed campaign JSON> --run-id <id> [--isal]
```

The launcher drives one Rust tool, `logical-campaign`, which a leaf may also
call directly: `pins` prints the frozen addendum's path, pinned digest,
identity, freeze time and the four family ledgers, and fails when the document's
bytes differ from the pin; `list` prints one family's cells with their ordinals
and seeds; `cells`, `verify` and `plan` are the transcription, the comparison
and the plan projection; `inputs` is the producing-input closure guard.

`build` compiles the gf2 arms `conservative-portable` into
`target/bb769456-arms`, runs the crate's own contract tests and the semantic
oracle, and writes `survey/logical-harness-validation.txt`. With `--isal` it
also compiles `logical-isal-arm` against the pinned ISA-L checkout into
`target/bb769456-isal-arm`.

`logical-profile` is the crate's profile driver: `cases` prints the frozen
profile matrix, and `run --case <id> --seconds <n>` runs one route on one
frozen cell under that cell's cache policy until the duration elapses,
counting its own calls and reporting the route provenance it observed. It
refuses to run outside the benchmark window exactly as the campaign arm does.
`survey/run-logical-profile.sh` is its session launcher; `survey/disassemble-logical.sh`
writes the annotated release disassembly of the measured routes from the same
executables, and times nothing.

`cells` writes one family's campaign JSON addendum. `--family` is one of
`2037941f-logical-isolated-xor`, `2037941f-logical-public-row-xor`,
`2037941f-logical-nr-construction`, `2037941f-logical-isal-base-gap`.

`smoke` is the untimed release smoke described below.

`window` refuses unless `GF2_BENCH_WINDOW=1`, the frozen prose addendum's
SHA-256 equals the pin the harness carries, and the campaign JSON matches
`logical-campaign verify`. It then rebuilds every executable it launches from
the current tree, regenerates the closure with
`make-logical-producing-inputs.py --check`, and only afterwards runs
`logical-campaign inputs`, which refuses unless every path of the
producing-input closure, plus the campaign JSON and the family ledger, is
tracked by git and identical to its committed content. A source added to a
measured crate stops the run either way: the freshness check catches one the
committed closure does not yet name, and the guard catches one it names that
git does not track. The two closure checks are the last steps before the launch:
nothing rebuilds after them, so no executable can carry bytes they never saw. It then projects the plan, prints the execution log path, runs
the runner under `dev/scripts/ccx1-bench-flock.sh --full-host` until the log's
terminal record is `complete`, finalizes the receipt under
`dev/bench_results/2037941f/<family>/<run-id>-pilot`, and evaluates it with
`benchmark-acceptance`. A resumed invocation reuses the stored plan and refuses
when the current projection differs.

`window` queues nothing by itself. A baseline leaf adds its own line to
`dev/active/1a379447-zen3-cpu-performance/bench-window/queue.tsv` in the form
`issue<TAB>worktree<TAB>est_minutes<TAB>command`, where the command is the
`window` invocation above.

### Downstream invocation

| Leaf | Family | Commands |
|---|---|---|
| `c489745b` isolated XOR | `2037941f-logical-isolated-xor` | `build`, `cells`, `smoke`, `window` |
| `8197174d` row XOR | `2037941f-logical-public-row-xor` | `build`, `cells`, `smoke`, `window` |
| `c49e78bc` coding route | `2037941f-logical-nr-construction` | `build`, `cells`, `smoke`, `window` |
| `65c0e13d` ISA-L | `2037941f-logical-isal-base-gap` | `build --isal`, `cells`, `smoke --isal`, `window --isal` |

Each leaf commits its family's campaign JSON addendum before its `window` line
reaches the queue, and each owns its own ledger reservation.

## Untimed release smoke

```
dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/survey/run-logical-harness.sh smoke [--isal]
```

The launcher projects each family's plan and smokes its arms with
`benchmark-ab-runner smoke <plan.json> --record <path>`, whose contract
`tuning_campaign_support::arm::smoke` states. Without `--isal` it covers the
three gf2 families; with it, all four.

Around that contract the launcher adds what belongs to this harness: every
family transcribes twice to identical bytes and passes `logical-campaign
verify`; `logical-oracle` and, with `--isal`, `logical-isal-arm --oracle` report
every case as `PASS`; and `survey/check-smoke.py` refuses a family whose smoke
record does not cover exactly the cells its campaign addendum declares. Each
family contributes two cells, its smallest anchor plus one cell in a second
cache state, so warm, streaming and the frozen cold call count all reach the
wire.

The record is `survey/logical-runner-smoke.txt`, written by `check-smoke.py`
from the oracle output and the families' smoke records. Its `# command:` header
names the invocation that wrote it, so the committed record states whether its
run covered the ISA-L family; no line is a clock reading, so a rerun on the same
executables reproduces it byte for byte. `build` writes
`survey/logical-harness-validation.txt` the same way.

Journal, checkpoint and resume behaviour belongs to the timed path and is bound
in `tuning-campaign-support`'s own suite.
