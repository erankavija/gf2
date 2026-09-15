# Acceptance summary for `pilot-r2-crossover-53c5a8c0-20260913t130716z`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 1; resumed false.
Receipt digest `f6d60673823b66560665942431f61101c5bb34c5d56eb228a2cf2679affc5c6d`.

Family `gf2m-clmul-crossover`: 1 comparisons at family-wise alpha 0.025, per-comparison confidence 0.975000, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `raw-batch-8` | Exploratory | SingleCore | 0 | 24 | 0/240 | 2.2469 | [2.2443, 2.2521] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `field-dot-8` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.0224 | [1.0092, 1.0523] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `field-dot-64` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.9057 | [1.8905, 1.9131] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `field-batch-mul-8` | Exploratory | SingleCore | 0 | 24 | 0/240 | 2.3532 | [2.3504, 2.3648] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.
