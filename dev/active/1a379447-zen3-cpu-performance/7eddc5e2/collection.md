# Matched AFF3CT confirmation: collection (jit:7eddc5e2)

> **Diátaxis Type:** Research

The confirmatory attempt of family `ldpc-batch-aff3ct-matched-v1` is accepted
and does not qualify for production selection. On one worker at the full wave
the gf2 lane route is faster than AFF3CT flooding f32 normalized min-sum
[Cassagne2019] on both codes; in the single-frame cells and the DVB-T2
twelve-core cell AFF3CT shows a material gap in its favour; the NR twelve-core
cell establishes no material gap. The comparison selects nothing.

## Evidence

The receipt directory
[`87a6bb36-ldpc-batch-aff3ct-matched-confirmation`](../../../bench_results/ed3d490e/aff3ct-matched/87a6bb36-ldpc-batch-aff3ct-matched-confirmation/)
holds the receipt, execution log, checkpoints, plan, input snapshots and the
[acceptance summary](../../../bench_results/ed3d490e/aff3ct-matched/87a6bb36-ldpc-batch-aff3ct-matched-confirmation/acceptance-summary.md).
Its frozen addendum is [`87a6bb36/addendum.json`](../87a6bb36/addendum.json),
derived from the family's pilot receipt by the canonical freezer. The
[family ledger](../../../bench_results/ed3d490e/aff3ct-matched/ledger.jsonl)
holds the pilot's reservation and this attempt's, chained to it.

## Verification

The shared [log verifier](../../../scripts/verify-campaign-log.py), called as
`run-family-campaign.py` calls it, accepts the campaign from its own log:

```sh
d=dev/bench_results/ed3d490e/aff3ct-matched/87a6bb36-ldpc-batch-aff3ct-matched-confirmation
python3 dev/scripts/verify-campaign-log.py --log $d/execution.log --receipt $d/receipt.json --plan $d/plan.json
```

Every declared cell starts, checkpoints and completes once with status
`measured` at the protocol's confirmatory pair count (`settings.confirmatory_pairs`
of the receipt); each session that reached its cell budget closes with
`paused`, and the last closes with the single terminal `complete` record. No
cell is abandoned. The samples are fresh: the campaign is distinct from the
pilot and its `campaign_seed` differs from the pilot receipt's.

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

In every cell of the receipt's [plan](../../../bench_results/ed3d490e/aff3ct-matched/87a6bb36-ldpc-batch-aff3ct-matched-confirmation/plan.json)
the baseline arm is the gf2 lane route (`gf2-batch-lane-avx2-<code>`) and the
candidate arm is AFF3CT (`aff3ct-flooding-nms-f32-<code>`). The estimator's
values above one favour the candidate (protocol, *Estimator and interval*), and
every cell is a comparator-gap cell of the addendum, so `improved` and its
`pass` mean a material gap in AFF3CT's favour and `regressed` and its `fail`
mean AFF3CT is slower than the gf2 lane route beyond the equivalence margin
(protocol, *Decision rule*). The generated [outcome table](outcome.md) lists
each cell's objective, arms, decision and outcome as the acceptance summary's
`cells[]` entry records them.

`qualifies` requires every confirmatory cell to pass (protocol, *Outcomes and
completeness*), so the receipt does not qualify. The margins stay as frozen
and no further attempt is queued.
