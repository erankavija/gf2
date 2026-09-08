# Acceptance summary for `protocol-v2-confirmation-r1`

Label `Smoke`; verdict **Accepted**; qualifies for production selection: **false**; sessions 2; resumed true.
Receipt digest `29df2562dd160080331047c068d4edc70e84258574ee8ba53fc9ae3250f59cf0`.

This is a smoke receipt: it proves the receipt pipeline and claims no performance result.

Family `protocol-smoke-v2`: 2 comparisons at family-wise alpha 0.025, per-comparison confidence 0.987500, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `decoder-pipeline` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.0011 | [0.9991, 1.0036] at 0.9875 | NotWorse | **Pass** |  |
| `cold-first-use` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.5004 | [0.4977, 0.5040] at 0.9875 | Regressed | **Fail** |  |

## Findings

No findings.
