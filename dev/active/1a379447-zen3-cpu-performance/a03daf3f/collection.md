# Multicore before-after confirmation: collection (jit:a03daf3f)

> **Diátaxis Type:** Research

The confirmatory attempt of family `ldpc-batch-multicore-v1` is accepted and
does not qualify for production selection: the lane route qualifies on the NR
code at six and twelve physical cores and in no other cell; every DVB-T2 cell
and the NR cell at twenty-four logical CPUs record `fail`.

## Evidence

The receipt directory
[`093ae4d0-ldpc-batch-multicore-confirmation`](../../../bench_results/ed3d490e/multicore/093ae4d0-ldpc-batch-multicore-confirmation/)
holds the receipt, execution log, checkpoints, plan, input snapshots and the
[acceptance summary](../../../bench_results/ed3d490e/multicore/093ae4d0-ldpc-batch-multicore-confirmation/acceptance-summary.md).
Its frozen addendum is [`093ae4d0/addendum.json`](../093ae4d0/addendum.json),
derived from the family's pilot receipt by the canonical freezer. The
[family ledger](../../../bench_results/ed3d490e/multicore/ledger.jsonl)
holds the pilot's reservation and this attempt's, chained to it.

## Verification

The shared [log verifier](../../../scripts/verify-campaign-log.py), called as
`run-family-campaign.py` calls it, accepts the campaign from its own log:

```sh
d=dev/bench_results/ed3d490e/multicore/093ae4d0-ldpc-batch-multicore-confirmation
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
the decision and outcome its `cells[]` entry records. Every cell is an
improvement cell of the addendum; the CPU set is the one the runner resolved,
in the summary's *CPUs* column.

| Cell | Core arm | Decision | Outcome | Lane route qualifies |
|---|---|---|---|---|
| `dvb-t2-r12-lane-p6-b8` | six physical cores | `regressed` | `fail` | no |
| `dvb-t2-r12-lane-p12-b8` | twelve physical cores | `regressed` | `fail` | no |
| `dvb-t2-r12-lane-l24-b8` | twenty-four logical CPUs | `regressed` | `fail` | no |
| `nr-bg1-z384-lane-p6-b8` | six physical cores | `improved` | `pass` | yes |
| `nr-bg1-z384-lane-p12-b8` | twelve physical cores | `improved` | `pass` | yes |
| `nr-bg1-z384-lane-l24-b8` | twenty-four logical CPUs | `regressed` | `fail` | no |

`qualifies` requires every confirmatory cell to pass (protocol, *Outcomes and
completeness*), so the receipt does not qualify. The margins stay as frozen
and no further attempt is queued.
