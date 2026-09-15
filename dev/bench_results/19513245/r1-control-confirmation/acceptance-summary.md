# Acceptance summary for `19513245-r1-control-confirmation`

Label `Confirmation`; verdict **Accepted**; qualifies for production selection: **true**; sessions 1; resumed false.
Receipt digest `6b840723c8b78b80002e76accb90f7c639dbd7f053129b6eb280c7131e37589f`.

Family `bytefield-consumer-control`: 3 comparisons at family-wise alpha 0.025, per-comparison confidence 0.991667, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `pairwise-4k-batch` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 2.3220 | [2.3101, 2.3299] at 0.9917 | Improved | **Pass** |  |
| `pairwise-128k-batch` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 2.2952 | [2.2883, 2.3252] at 0.9917 | Improved | **Pass** |  |
| `pairwise-128k-whole-batch` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 3.5390 | [3.4861, 3.6235] at 0.9917 | Improved | **Pass** |  |

## Findings

No findings.
