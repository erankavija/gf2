# Integration route preparation

> **Diátaxis Type:** Reference

Preparation for `@/issue/31727c57` is ready for the overnight window under the
[scorecard design](../1362381c/design.md). No timed result or acceptance verdict
exists for either integrated family. Changed files: shared `63bad95d` tooling, the decoder declaration and this issue's
preparation directory; `COMMIT-PLAN.md` lists them. Links: `target/31727c57-links.tsv`.

## Families and cells

`1362381c-core-route-integration`: [pilot](core/pilot/addendum.json) has nine
exploratory cells comparing tuned-portable with conservative-portable and native
with tuned-portable, on one core. The confirmation allocation in `prepare.py`
selects `matvec-r1024-24w-native`, `popcount-1024w-native` and
`clmul-wide-9w-native`, plus the [declared holdouts](core/holdout-cells.json)
`xor-inplace-5w`, `popcount-6w` and `xor-inplace-3w`. The self-test checks absence from recorded `63bad95d` pilots.

`1362381c-decoder-route-integration`: [pilot](decoder/pilot/addendum.json) has ten
exploratory cells: both codes at one worker with one-frame and full waves, six
and twelve physical workers, and twenty-four logical workers. The confirmation
allocation selects DVB one-frame/one-worker and six-worker/full-wave cells, NR
twelve-worker and twenty-four-worker/full-wave cells, and the
[NR partial-wave holdouts](decoder/holdout-cells.json) at batches five and
thirteen. Confirmation claims apply only to its selected cells. The shared generator verifies holdout code,
seed and batch absence from the calibration inventory.

Both [core](core/next-attempt.json) and
[decoder](decoder/next-attempt.json) `benchmark-acceptance --next-attempt`
outputs report attempt 1, alpha 0.025, max_cells 6, prior_comparisons 0 and
reservations 0. Allocations are 3 + 3 and 4 + 2 respectively; pilots spend no
comparisons. The launcher checks the ledger before freezing.

## Verification and operation

[Core build record](core/arm-builds.json) pins the three Rust 1.95 release arms.
Decoder arms use the live `e5f8b3ae/portable/build-identity.json` record and its
read-only story executables. Bundles rehydrate from the existing archive under `target/`.

Both launchers pass `--untimed` through the shared session runner. Pilot and
confirmation [core smoke](core/confirmation-untimed-smoke.json) and
[decoder smoke](decoder/confirmation-untimed-smoke.json) records report zero
windows; their execution logs end complete. Decoder
[diagnostics](decoder/untimed-diagnostics.json) verify separate `batch_fill_ns`
reporting from every zero-iteration stage probe. Generator checks, preparation self-test, build-record check and shell
syntax pass; plain Python check modes preserve ignored Git status.

Launchers: `core/window.sh`, `decoder/window.sh`. Each requires committed pilot
and holdout inputs, runs pilot → canonical freeze → confirmation, commits only
freeze outputs,. Margins are not
widened when pilot resolution rejects them. Timed receipts and outcomes remain
window work; the lead runs merged-tree CI. Open preparation issues: none.

Queue lines (minutes are estimates using the sibling worker-routing job's
20-minute queue allowance):

```tsv
31727c57	.agents/worktrees/agent-31727c57	20	bash dev/active/1a379447-zen3-cpu-performance/31727c57/core/window.sh
31727c57	.agents/worktrees/agent-31727c57	20	bash dev/active/1a379447-zen3-cpu-performance/31727c57/decoder/window.sh
```
