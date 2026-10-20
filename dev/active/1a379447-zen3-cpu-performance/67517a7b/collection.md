# Decode selector holdout pilot: collection (jit:67517a7b)

> **Diátaxis Type:** Research

The pilot of family `ldpc-batch-decode-selector-holdout-v1` is measured and
accepted, and it does not qualify for production selection: every pilot cell
records the outcome `pilot`, which decides nothing. Its confirmation addendum is
frozen with the holdout cells attached, and the confirmation waits for the
benchmark window.

## Evidence

The receipt directory
[`37aa7254-ldpc-batch-decode-selector-holdout-pilot`](../../../bench_results/ed3d490e/decode-selector-holdout/37aa7254-ldpc-batch-decode-selector-holdout-pilot/)
holds the receipt, execution log, checkpoints, plan, input snapshots and the
[acceptance summary](../../../bench_results/ed3d490e/decode-selector-holdout/37aa7254-ldpc-batch-decode-selector-holdout-pilot/acceptance-summary.md),
whose cell table lists each cell's pairs, interval and outcome. Its frozen
addendum is [`37aa7254/addendum.json`](../37aa7254/addendum.json). The
[family ledger](../../../bench_results/ed3d490e/decode-selector-holdout/ledger.jsonl)
holds the pilot's exploratory reservation, which spends no comparison.

## Verification

The shared [log verifier](../../../scripts/verify-campaign-log.py), called as
`run-family-campaign.py` calls it, accepts the campaign from its own log:

```sh
d=dev/bench_results/ed3d490e/decode-selector-holdout/37aa7254-ldpc-batch-decode-selector-holdout-pilot
python3 dev/scripts/verify-campaign-log.py --log $d/execution.log --receipt $d/receipt.json --plan $d/plan.json
```

Every declared cell starts, checkpoints and completes once with status
`measured` at its declared `pilot_pairs` of the plan; the first session closes
with `paused` at its cell budget, and the second with the single terminal
`complete` record. `python3 dev/scripts/check-receipt-input-snapshots.py`
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

## Confirmation freeze

The canonical freezer
[`freeze-confirmation.py`](../c7113c5a/survey/freeze-confirmation.py) derives
[`67517a7b/addendum.json`](addendum.json) from the committed pilot receipt,
pinned by path and SHA-256, with purpose `selector-calibration`, a family
description of the confirmatory stage and the cells of
[`37aa7254/holdout-cells.json`](../37aa7254/holdout-cells.json) attached with
the holdout role. The [derivation record](derivation.txt) carries the pilot's
widest relative half-width at its corrected alpha, the frozen resolution, the
replaced margins and the holdout declaration's digest; the freezer refuses
margins that do not lie strictly above one plus that resolution. The holdout
declaration entered the tree in commit `39325e49a`, before the pilot's
`campaign-start` record, so no holdout cell took part in the pilot.

`make-family.py --confirmation` writes the confirmation [plan](plan.json) from
the frozen addendum and the declaration's `confirmation` block in
[`decode-selector-holdout.json`](../../ed3d490e-zen3-ldpc-frame-simd/survey/families/decode-selector-holdout.json);
a second run reproduces it byte for byte. The
[confirmatory bound](../../../bench_results/ed3d490e/decode-selector-holdout/confirmatory-bound.json),
recomputed by `benchmark-acceptance --next-attempt` (`trial_ledger::next_attempt`
under `receipt::MIN_TAIL_DRAWS`) on the post-pilot ledger, admits the declared
confirmatory and holdout cells. The
[untimed smoke](../../../bench_results/ed3d490e/decode-selector-holdout/confirmation-smoke.json)
of that plan, from `run-family-campaign.py --untimed`, drives every arm of every
frozen cell once and opens no timing window.

## Status

The pilot is measured. The confirmation is the `67517a7b` line of
[`bench-window/queue.tsv`](../bench-window/queue.tsv) and waits for the
benchmark window; its decision rule, frozen in the addendum's family
description, retains the envelope's decode selectors only when the
confirmation's acceptance summary qualifies.
