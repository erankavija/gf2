# Acceptance summary for `protocol-v2-pilot-r1`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 2; resumed true.
Receipt digest `42b93aa349bff167866e453f3d50efccfec316acc9f3a2ee1af058442902eb1f`.

Family `protocol-smoke-v2`: 1 comparisons at family-wise alpha 0.025, per-comparison confidence 0.975000, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `decoder-pipeline` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.0008 | [0.9964, 1.0030] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `cold-first-use` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.5021 | [0.4981, 0.5036] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.
