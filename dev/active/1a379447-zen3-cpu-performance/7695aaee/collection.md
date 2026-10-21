# Decode selector holdout confirmation: collection (jit:7695aaee)

> **Diátaxis Type:** Research

The confirmatory attempt of family `ldpc-batch-decode-selector-holdout-v1` is
accepted and qualifies for production selection. Every confirmatory and holdout
cell records the outcome `pass`, so the calibrated decode profile holds on the
held-out NR inputs and, by the frozen decision rule, the envelope's decode
selectors are retained.

## Evidence

The receipt directory
[`67517a7b-ldpc-batch-decode-selector-holdout-confirmation`](../../../bench_results/ed3d490e/decode-selector-holdout/67517a7b-ldpc-batch-decode-selector-holdout-confirmation/)
holds the receipt, execution log, checkpoints, plan, input snapshots and the
[acceptance summary](../../../bench_results/ed3d490e/decode-selector-holdout/67517a7b-ldpc-batch-decode-selector-holdout-confirmation/acceptance-summary.md),
whose cell table lists each cell's pairs, interval and outcome. Its frozen
addendum is [`67517a7b/addendum.json`](../67517a7b/addendum.json), derived from
the family's pilot receipt by the canonical freezer with the holdout cells of
[`37aa7254/holdout-cells.json`](../37aa7254/holdout-cells.json) attached
([derivation record](../67517a7b/derivation.txt)). The
[family ledger](../../../bench_results/ed3d490e/decode-selector-holdout/ledger.jsonl)
holds the pilot's exploratory reservation and this attempt's, chained to it.
The family's pilot has a superseded first window run; its
[attempt record](../67517a7b/superseded-pilot-attempt.md) states the
contradiction with the append-only ledger and shows that the confirmation's
P-20 bound is unchanged by it. The benchmark window ran the `67517a7b` line of
[`bench-window/queue.tsv`](../bench-window/queue.tsv) as job key
`20a7f86392f16b4d`.

## Verification

The shared [log verifier](../../../scripts/verify-campaign-log.py), called as
`run-family-campaign.py` calls it, accepts the campaign from its own log:

```sh
d=dev/bench_results/ed3d490e/decode-selector-holdout/67517a7b-ldpc-batch-decode-selector-holdout-confirmation
python3 dev/scripts/verify-campaign-log.py --log $d/execution.log --receipt $d/receipt.json --plan $d/plan.json
```

Every declared cell starts, checkpoints and completes once with status
`measured` at the protocol's confirmatory pair count (`settings.confirmatory_pairs`
of the receipt); each session that reached its cell budget closes with
`paused`, and the last closes with the single terminal `complete` record. No
cell is abandoned. The samples are fresh: the campaign is distinct from the
pilot, its `campaign_seed` differs from the pilot receipt's, and no holdout
cell is a cell of the pilot. `python3 dev/scripts/check-receipt-input-snapshots.py`
finds every pinned input of the receipt committed.

`benchmark-acceptance` over the committed receipt, with its two summary files
excluded, reproduces both byte for byte:

```sh
s=$(mktemp -d)
git archive HEAD $d | tar -x -C $s --exclude='*/acceptance-summary.*'
target/release/benchmark-acceptance $s/$d
cmp $s/$d/acceptance-summary.json $d/acceptance-summary.json
cmp $s/$d/acceptance-summary.md $d/acceptance-summary.md
```

## Outcome per cell

The generated [outcome table](outcome.md) lists each cell's objective, arms,
decision and outcome as the acceptance summary's `cells[]` entry records them.
Its builds table names each cell's baseline and candidate builds and states, per
holdout cell, whether the calibrated profile holds on the held-out inputs.

The builds stay separate cell by cell. A selector cell compares the
tuned-portable build, which installs the envelope, with the conservative-portable
build, which installs none; a native cell compares the native build with the
tuned-portable build under the same envelope. No cell compares the native build
with the conservative-portable build, and the native results of this family are
the native cells of this receipt.

## Decision

The frozen decision rule in the addendum's family description retains the
envelope's decode selectors only when the confirmation's acceptance summary
qualifies, which requires every confirmatory and holdout cell, native cells
included, to record `pass`. The summary records `qualifies` true, so the decode
selectors of the committed coding owner envelope
[`gf2-0a77ecfc-20261013t011147z-815998.json`](../../../../crates/gf2-coding/data/tuning-profiles/gf2-0a77ecfc-20261013t011147z-815998.json)
are retained as committed. The rule requires no code change. The margins stay
as frozen and no further attempt is queued.
