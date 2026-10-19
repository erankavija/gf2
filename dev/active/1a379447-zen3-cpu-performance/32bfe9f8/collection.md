# Single-worker before-after confirmation: collection (jit:32bfe9f8)

> **Diátaxis Type:** Research

The confirmatory attempt of family `ldpc-batch-single-worker-v1` is accepted
and does not qualify for production selection: the lane route meets the
improvement margin in both one-worker throughput cells and passes three of
the four non-regression cells, and `nr-bg1-z384-lane-w1-b1` records `fail`.

## Evidence

The receipt directory
[`d0d2c1bd-ldpc-batch-single-worker-confirmation`](../../../bench_results/ed3d490e/single-worker/d0d2c1bd-ldpc-batch-single-worker-confirmation/)
holds the receipt, execution log, checkpoints, plan, input snapshots and the
[acceptance summary](../../../bench_results/ed3d490e/single-worker/d0d2c1bd-ldpc-batch-single-worker-confirmation/acceptance-summary.md).
Its frozen addendum is [`d0d2c1bd/addendum.json`](../d0d2c1bd/addendum.json),
derived from the family's pilot receipt by the canonical freezer. The
[family ledger](../../../bench_results/ed3d490e/single-worker/ledger.jsonl)
holds the pilot's reservation and this attempt's, chained to it.

## Verification

The shared [log verifier](../../../scripts/verify-campaign-log.py), called as
`run-family-campaign.py` calls it, accepts the campaign from its own log:

```sh
d=dev/bench_results/ed3d490e/single-worker/d0d2c1bd-ldpc-batch-single-worker-confirmation
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

Each row names the cell's row in the acceptance summary's *Cells* table and
the decision and outcome its `cells[]` entry records. The objective comes from
the addendum.

| Cell | Objective | Decision | Outcome | Lane route on one worker |
|---|---|---|---|---|
| `dvb-t2-r12-lane-w1-b8` | improvement | `improved` | `pass` | qualifies in sustained throughput |
| `dvb-t2-r12-lane-w1-b1` | non-regression | `not-worse` | `pass` | single-frame latency not regressed |
| `dvb-t2-r12-lane-w1-b11` | non-regression | `improved` | `pass` | incomplete wave not regressed |
| `nr-bg1-z384-lane-w1-b8` | improvement | `improved` | `pass` | qualifies in sustained throughput |
| `nr-bg1-z384-lane-w1-b1` | non-regression | `regressed` | `fail` | single-frame latency regressed |
| `nr-bg1-z384-lane-w1-b11` | non-regression | `improved` | `pass` | incomplete wave not regressed |

`qualifies` requires every confirmatory cell to pass (protocol, *Outcomes and
completeness*), so the single-frame latency regression on the NR code keeps the
receipt from qualifying. The margins stay as frozen and no further attempt is
queued.
