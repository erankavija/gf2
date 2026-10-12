# Measurement plan: the parity kernel against the SIMD lane

> **Diátaxis Type:** Explanation

Issue `@/issue/7dbe1393` measures one candidate, the fused AND-parity kernel
that the [attribution](attribution.md) derives, under the
[frozen addendum](matvec-ragged-tail-addendum.md) of family
`7dbe1393-matvec-ragged-tail`. This page states how the two arms are reached,
what decides, and what each outcome does to the candidate.

## How the arms are reached

Both arms are routes of one executable, the dense harness `dense-arm` built
with the `simd` feature; the plan selects a route through `GF2_DENSE_ROUTE`.

| Arm | Route | Entry | Row kernel |
|---|---|---|---|
| baseline | `matvec-a` | public `BitMatrix::matvec` | bundle `and_popcnt_fn` |
| candidate | `matvec-lane-parity` | public `BitMatrix::matvec_parity_kernel` | bundle `and_parity_fn` |

Production selection is what `main` selects. `matvec` matches on
`matvec_route` and its SIMD lane calls `and_popcnt_fn`; this issue edits
neither. The candidate is reachable only through its own entry and its own
bundle field, both documented as comparators that no automatic dispatch
selects. `MatvecRoute` keeps the dispatcher's two arms: the candidate is no
arm of the dispatcher, as the carry-save count comparators of the bundle are
no variant of `PopcountRoute`.

The shared suites hold every lane to the same bits:
`crates/gf2-core/tests/simd_equiv_matvec.rs` runs the public entry, both
pinned lanes and the parity-kernel entry at column counts 0, 1, 63, 64 and 65
bits past each word and vector boundary and checks zero tail padding, and
`crates/gf2-core/tests/popcount_routes.rs` holds the bundle's parity entry to
the low bit of every fused count at each word offset.
[`survey/boundary-suites.txt`](survey/boundary-suites.txt), written by
[`survey/run-boundary-suites.sh`](survey/run-boundary-suites.sh), records both
suites under Rust 1.95 with the `simd` feature and without it.

## Sessions

Every timed step is a line of the benchmark-window queue; no session runs
one.

1. **Profile session.** `run-dense-profile.sh window --cases
   survey/profile-cases.txt` of the dense story profiles the scalar lane, the
   SIMD lane and the parity-kernel lane at the five ragged strides and the
   control: hardware counters and sampled symbols in each of its repetitions,
   into `dev/bench_results/7dbe1393/ragged-tail-profile`, with a summary its
   generator writes. The scalar lane is reached through the pinned-lane arm of
   the lane-threshold family's cells at the same strides, whose workload seeds
   differ from this family's.
2. **Pilot.** `run-dense-harness.sh window --family
   7dbe1393-matvec-ragged-tail` runs the six cells of
   [`campaigns/matvec-ragged-tail.json`](campaigns/matvec-ragged-tail.json),
   all exploratory.
   [`survey/pilot-plan-smoke.json`](survey/pilot-plan-smoke.json) is the
   shared smoke's record of that plan: one untimed dispatch per arm and cell.
3. **Confirmation.** A pilot that resolves its family within the ceiling gives
   the canonical freezer the receipt to derive a confirmation addendum from;
   neither pilot does.
4. **Repeat pilot.** `run-dense-harness.sh window --family
   7dbe1393-matvec-ragged-tail-v2` runs the same six cells, seeds and arms
   under the [second addendum](matvec-ragged-tail-v2-addendum.md), from
   [`campaigns/matvec-ragged-tail-v2.json`](campaigns/matvec-ragged-tail-v2.json)
   on a ledger of its own. Its queue line starts the launcher only on a
   quiet host. [`survey/pilot-plan-v2-smoke.json`](survey/pilot-plan-v2-smoke.json)
   is the shared smoke's record of that plan.

## Decision

The addendum's *Decision rule* reads the confirmation's acceptance summary:
validity, the control's non-regression, and improvement at every ragged
stride. Its outcome is *adopt* or *no change*.

| Outcome | What happens to the candidate |
|---|---|
| *adopt* | The SIMD lane's row loop calls `and_parity_fn`. `matvec_parity_kernel` and its lane body are removed, the harness route with them, and the bundle field's rustdoc names the lane that selects it. `and_popcnt_fn` stays: `kernels::ops::and_popcount` resolves it. |
| *no change* on a `not-material` cell | Production keeps its kernel. The candidate kernel, its bundle field, its entry and the harness route are removed; the receipts, tables and this family's ledger stay as the recorded no-win. |
| *no change* on a `fail` (regressed) cell | The same removal; the regressed cell is reported as recorded. |
| *no change* on validity, or `resolution-insufficient` | Production keeps its kernel and the candidate stays unselected; the outcome record states the deciding step and whether P-20 admits a further attempt. |

The addendum's complexity budget counts the adopted state: the kernel, its
bundle entry and its bundle field. The public entry and the lane body exist
for the measurement and either outcome removes them.

A pilot that places the candidate behind the baseline is a recorded no-win
without a confirmation: the family spends no reservation on a candidate its
own pilot does not favour, and the removal of the second row applies.

## Recorded outcome

The first four paragraphs record family `7dbe1393-matvec-ragged-tail`; the
last section records the family that repeats its pilot.

**Pilot.** The pilot's
[execution log](../../../bench_results/7dbe1393/7dbe1393-matvec-ragged-tail/v4-r1-pilot/execution.log)
holds a `cell-start` and a `cell-complete` record for each cell of the
campaign addendum, every cell `measured` at the pilot's pair count, and a
terminal `complete` record; the campaign ran as resumed sessions under one
identity. Its
[acceptance summary](../../../bench_results/7dbe1393/7dbe1393-matvec-ragged-tail/v4-r1-pilot/acceptance-summary.md)
records the verdict `accepted` under the label `Pilot`, with each cell's
estimate, pair count and interval; every row is exploratory.

**Host condition.** The runner's observation at each session start is in the
*Host load at each pilot session* table of
[`pilot-resolution.md`](pilot-resolution.md), and the window runner's start
line for the job is the second job of
[`survey/bench-window-jobs.txt`](survey/bench-window-jobs.txt). Both state
load averages of a host running other work during the pilot. The receipt
stands as measured.

**Resolution.** [`pilot-resolution.md`](pilot-resolution.md) applies the
addendum's *Margins and resolution* rule to the committed pilot and records
the outcome `resolution-insufficient`: the pilot-derived resolution exceeds
the family ceiling, and its widest cell is the aligned control. The canonical
freezer, given that resolution and the pilot addendum's margins unchanged,
refuses to write a confirmation addendum;
[`survey/freeze-refusal.txt`](survey/freeze-refusal.txt) holds its command,
its message and its exit status.

**Consequence.** The fourth row of the *Decision* table applies. The family
runs no confirmation and its ledger holds no confirmatory reservation;
production keeps its kernel and the candidate stays unselected behind its own
entry. The deciding step is the resolution rule, before the decision rule
reads any cell. The pilot's rows favour the candidate at every cell and decide
nothing. The P-20 arithmetic in `pilot-resolution.md` states the comparisons
the ledger still admits; the addendum's *Search and stopping* section allows
this family one pilot.

**Repeat.** Family `7dbe1393-matvec-ragged-tail-v2` asks the same question
again because the first pilot ran under the load its receipt records
(`session_hosts[].load_average`). Its pilot is the last attempt at the
question.

- *Pilot.* The
  [execution log](../../../bench_results/7dbe1393/7dbe1393-matvec-ragged-tail-v2/v4-r1-pilot/execution.log)
  holds a `cell-start` and a `cell-complete` record for each of the six cells,
  every cell `measured` at the pilot's pair count, and a terminal `complete`
  record. The
  [acceptance summary](../../../bench_results/7dbe1393/7dbe1393-matvec-ragged-tail-v2/v4-r1-pilot/acceptance-summary.md)
  records the verdict `accepted`, does not qualify the receipt for production
  selection, and tabulates each cell's estimate, pair count and interval; every
  row is exploratory. The speedup is the baseline's time over the candidate's,
  so a value above one favours the parity kernel, and every pilot row favours
  it.
- *Host condition.* [`pilot-resolution-v2.md`](pilot-resolution-v2.md)
  tabulates the load the runner observed at each session start.
- *Resolution.* The same page applies the *Margins and resolution* rule of the
  first addendum to this receipt and records the outcome
  `resolution-insufficient`: the pilot-derived resolution exceeds the family
  ceiling, and the widest cell is the aligned control.
  [`survey/freeze-refusal-v2.txt`](survey/freeze-refusal-v2.txt) records the
  canonical freezer's command and output. The freezer checks the margins
  against one plus the supplied resolution and not against the ceiling, so it
  writes an addendum; the frozen rule governs, and the record states that no
  confirmation addendum is kept.
- *Consequence.* The *no change on validity, or `resolution-insufficient`*
  row of the *Decision* table applies to the repeat. The family runs no
  confirmation and its ledger holds no confirmatory reservation. The question
  has no further attempt: the candidate kernel stays unselected behind
  `BitMatrix::matvec_parity_kernel` and production keeps its kernel.

## Generators

Every generator under `survey/` runs untimed from the worktree root and
reproduces its record byte for byte on the same inputs.

- `record-lane-instruction-counts.py` sends the arm the validation-position
  request of each frozen cell (`dense-campaign validation-request`), which the
  arm answers with one call of the measured product and no timing window, under
  `valgrind --tool=callgrind --dump-instr=yes`. The record keeps the per-offset
  execution counts of the lane symbols, so the arm's digest and the assembly
  record of the same executable identify every counted instruction.
- `make-asm-attribution.py` nests loops by removing each component's entry
  instructions and classifies nothing by hand. Every figure is an execution
  count divided by the cell's row count; the script fails when the two records
  name different executables or a counted offset is not a listed instruction.
- `make-source-evidence.py` records, per claim, the first matching line, the
  verbatim line, the file digest and the commit that last changed the file; a
  fragment that is absent or changes its occurrence count fails the script.
- `make-resolution.py` applies the *Margins and resolution* section of the
  ragged-tail addendum through the dense story's shared `resolution_rule` and
  the lane-threshold family's comparison `budget`. Every pilot cell is
  confirmatory, so the rule's scope is the whole pilot; the standard output
  line carries the resolution and the outcome for the freeze. The repeat family
  writes `pilot-resolution-v2.md`.
- `record-freeze-refusal.sh` hands the canonical freezer the resolution
  `make-resolution.py` derives with every margin of the pilot addendum
  unchanged, and records the command, the freezer's output and exit status and
  whether a confirmation addendum exists afterwards. The repeat family records
  to `freeze-refusal-v2.txt`.
- `record-plan-check.sh` records the runner's `check` of the plan the
  launcher's `window --smoke` stores: it decodes the plan and the campaign
  addendum it names and validates the addendum against the live version-4
  schema.
- `record-window-jobs.sh` keeps the window runner's start line, with the host
  load averages read at that moment, and exit line of each named job key.
- `run-boundary-suites.sh` runs the suites once with every feature, where the
  kernel lanes and the bundle's entries exist, and once with the default
  features, where every lane is the scalar lane; the record holds test names
  and verdicts only.

## Limits

The family measures the candidate against the SIMD lane and not against the
scalar lane. The profile session covers the scalar lane; a timed contrast with
it and the value of `bit_matrix.matvec_simd_min_words` belong to `63bad95d`.
The family holds one cache state, one row count and six strides.
