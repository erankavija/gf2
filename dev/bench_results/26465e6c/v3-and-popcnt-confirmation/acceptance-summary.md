# Acceptance summary for `26465e6c-v3-and-popcnt-confirmation`

Label `Confirmation`; verdict **Accepted**; qualifies for production selection: **false**; sessions 2; resumed true.
Receipt digest `2323c8122a9ab87b1d287704fad5b412e7ed4c8aa2af9dfefc0290b5a093540d`.

Family `and-popcnt-baselines`: 6 comparisons at family-wise alpha 0.025, per-comparison confidence 0.995833, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `and-popcnt-w4-vs-scalar-control` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.8372 | [0.8329, 0.8413] at 0.9958 | Regressed | **Fail** |  |
| `and-popcnt-w4096-vs-scalar-control` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.3270 | [0.3257, 0.3284] at 0.9958 | Regressed | **Fail** |  |
| `and-popcnt-w512k-streaming-vs-scalar-control` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.6621 | [0.6354, 0.6851] at 0.9958 | Regressed | **Fail** |  |
| `and-popcnt-w4-vs-two-pass` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.2029 | [0.2021, 0.2036] at 0.9958 | Regressed | **Fail** |  |
| `and-popcnt-w4096-vs-two-pass` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.4228 | [0.4215, 0.4242] at 0.9958 | Regressed | **Fail** |  |
| `and-popcnt-w512k-streaming-vs-two-pass` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.5766 | [0.5652, 0.6063] at 0.9958 | Regressed | **Fail** |  |

## Findings

No findings.
