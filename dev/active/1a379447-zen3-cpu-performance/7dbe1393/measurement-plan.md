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
3. **Confirmation.** The canonical freezer derives the confirmation addendum
   from the committed pilot receipt when the pilot resolves the family within
   its ceiling; it runs in the window after the pilot's.

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

## Limits

The family measures the candidate against the SIMD lane and not against the
scalar lane. The profile session covers the scalar lane; a timed contrast with
it and the value of `bit_matrix.matvec_simd_min_words` belong to `63bad95d`.
The family holds one cache state, one row count and six strides.
