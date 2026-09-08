# Acceptance summary for `protocol-v3-pilot-r1`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 2; resumed true.
Receipt digest `0f3972ed0d46fa048800ef362d2c48bb4761cde381e362f7b1feac42f704bb08`.

Family `protocol-smoke-v3`: 1 comparisons at family-wise alpha 0.025, per-comparison confidence 0.975000, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `decoder-pipeline` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.0003 | [0.9990, 1.0043] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `cold-first-use` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.5001 | [0.4957, 0.5030] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.
