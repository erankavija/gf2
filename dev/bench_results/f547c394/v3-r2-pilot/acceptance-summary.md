# Acceptance summary for `protocol-v3-r2-pilot`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 2; resumed true.
Receipt digest `81617be7002434c4a7f0a395eb72805cb7f6e3cca542a17f63899c9aca99c597`.

Family `protocol-smoke-v3-r2`: 1 comparisons at family-wise alpha 0.025, per-comparison confidence 0.975000, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `decoder-pipeline` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.9998 | [0.9973, 1.0025] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `cold-first-use` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.5011 | [0.4978, 0.5050] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.
