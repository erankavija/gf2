# 5a25717c verdict baseline

Benchmark-acceptance verdicts and findings of every committed `receipt.json`
with schema `zen3-benchmark-receipt-v1` under `dev/bench_results/` outside
`inputs/` directories, before and after content-identity matching of the
shared receipt pins. Raw outputs are committed beside this record.

Baseline source: `65a916ade2a38529ddbe40b4bb4a72f1645e17f5`. After source:
`64f1a4f691177b72ce325a964aa3929f2201220a`.

## Commands

Run at each source state. The verifier writes its summary beside the receipt,
so the runner evaluates a scratch copy of each receipt directory and lists the
verdict line and every finding of its summary:

```
./scripts/cargo-budget.sh cargo build -p tuning-campaign-support --bin benchmark-acceptance --profile ci-test
dev/active/fa787f85-documentation-overhaul/5a25717c-verdicts.sh <verdict-file>
```

## Raw outputs

| State | Verdicts and findings |
| --- | --- |
| Baseline | `5a25717c-verdicts-before.txt` |
| After | `5a25717c-verdicts-after.txt` |

## Equality

The listings are identical; this command prints nothing:

```
diff dev/active/fa787f85-documentation-overhaul/5a25717c-verdicts-before.txt dev/active/fa787f85-documentation-overhaul/5a25717c-verdicts-after.txt
```
