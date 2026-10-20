# Superseded attempt of the decode selector holdout pilot (jit:67517a7b)

> **Diátaxis Type:** Research

This record covers the first window run of campaign
`37aa7254-ldpc-batch-decode-selector-holdout-pilot` of family
`ldpc-batch-decode-selector-holdout-v1`, addendum
[`37aa7254/addendum.json`](../37aa7254/addendum.json) with SHA-256
`04724a3caaa2e27e72e08fefdcdb055e96fc1dcb5d3d20f77547634c61611b74`. The
attempt is superseded and not voided. Its results were read, so protocol.md
rules out voiding it.

## Preserved evidence

The directory
[`37aa7254-ldpc-batch-decode-selector-holdout-pilot-superseded`](../../../bench_results/ed3d490e/decode-selector-holdout/37aa7254-ldpc-batch-decode-selector-holdout-pilot-superseded/)
holds the attempt's receipt, execution log, checkpoints, plan, input snapshots
and acceptance summary. It also holds the following files:

- `campaign-stage.tar.zst`: the campaign directory's `stage`, `untimed-stage`
  and `untimed-smoke.json`.
- `ledger-line.jsonl`: the attempt's family ledger line.
- `job-output.out`: the window job's output.
- `window-log.txt`: the benchmark window log lines of job key
  `99252f3487f542d0`, which covers both runs of the queue line.

The shared log verifier accepts the attempt from its own log. Every declared
cell started, checkpointed and completed once at its declared pairs, and the
terminal record is `complete`. `benchmark-acceptance`, run over the preserved
directory with its summary files excluded, reproduces both summary files byte
for byte:

```sh
s=dev/bench_results/ed3d490e/decode-selector-holdout/37aa7254-ldpc-batch-decode-selector-holdout-pilot-superseded
python3 dev/scripts/verify-campaign-log.py --log $s/execution.log --receipt $s/receipt.json --plan $s/plan.json
t=$(mktemp -d)
git archive HEAD $s | tar -x -C $t --exclude='*/acceptance-summary.*'
target/release/benchmark-acceptance $t/$s
cmp $t/$s/acceptance-summary.json $s/acceptance-summary.json
cmp $t/$s/acceptance-summary.md $s/acceptance-summary.md
```

## Event

The window job of 19:35:39–19:36:43Z (`window-log.txt`) ran while the lead
ran the story's `survey/test_*.py` files in-session on the same host. It
measured both declared cells. The lead read the acceptance verdict line of
`job-output.out`, then deleted the receipt, the stage, the ledger line and the
window done marker. The window ran the queue line again from 19:37:50Z under
the same campaign ID and campaign seed, and that run is the published pilot
receipt
[`37aa7254-ldpc-batch-decode-selector-holdout-pilot`](../../../bench_results/ed3d490e/decode-selector-holdout/37aa7254-ldpc-batch-decode-selector-holdout-pilot/).

## Recorded contradiction

The ledger section of protocol.md
([`f547c394/protocol.md`](../f547c394/protocol.md)) keeps the family ledger
append-only. It also states that an attempt whose results were read is never
voided and spends its reservation. This attempt's results were read, and its
ledger line was deleted from
[`ledger.jsonl`](../../../bench_results/ed3d490e/decode-selector-holdout/ledger.jsonl).
The deletion contradicts the append-only rule.

`ledger.jsonl` holds one reservation for this campaign, which the rerun
appended. That line is byte-identical to `ledger-line.jsonl` (`cmp`), and the
rerun receipt's `inputs/trial-ledger.jsonl` equals the superseded receipt's.
No line in `ledger.jsonl` belongs to the superseded attempt alone, so the chain
that the rerun receipt pins does not include this attempt as a reservation of
its own. The ledger format cannot hold one either.
`trial_ledger::decode` rejects a chain in which two reservations name the same
campaign. When `trial_ledger::reserve` finds a reservation for its campaign,
it resumes that reservation without appending. If the line had been kept, the
rerun would have resumed on it, and `ledger.jsonl` would hold the same bytes.

## Consequence for the confirmation's P-20 bound

The arithmetic follows `dev/tools/tuning-campaign-support/src/trial_ledger.rs`
(`attempt_budget`, `comparisons`, `next_attempt`) and
`dev/tools/tuning-campaign-support/src/receipt.rs` (`tail_supported`,
`MIN_TAIL_DRAWS`). The family alpha and bootstrap resamples are those that
[`confirmatory-bound.json`](../../../bench_results/ed3d490e/decode-selector-holdout/confirmatory-bound.json)
reads from `protocol::SHARED_SETTINGS`.

- `attempt_budget` counts only the reservations with `comparisons > 0`, and
  `comparisons` sums `comparisons` over the chain. The superseded attempt is
  exploratory: `ledger-line.jsonl` records `comparisons` 0 and no candidate.
  It therefore adds 0 to both counts, whether the chain holds it as its own
  reservation or not.
- On the chain `next_attempt` evaluates, which is the ledger plus a
  reservation of `m` confirmatory and holdout cells, exactly one reservation
  spends. The attempt alpha is therefore `family_alpha / (1 * 2)`, and the
  corrected alpha is that value over `0 + m`.
- `tail_supported` requires `bootstrap_resamples * corrected_alpha / 2 >=
  MIN_TAIL_DRAWS`, which gives `m <= bootstrap_resamples * family_alpha /
  (4 * MIN_TAIL_DRAWS)`. The admitted count is the largest integer `m` that
  satisfies it.

With the superseded attempt spent as a separate exploratory reservation, the
admitted count equals the `max_confirmatory_cells` that `confirmatory-bound.json`
records for the ledger as it stands. The declared cells of
[`67517a7b/addendum.json`](addendum.json) stay within it, and the frozen
confirmation's P-20 bound is unchanged.
