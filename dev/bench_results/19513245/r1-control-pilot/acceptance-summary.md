# Acceptance summary for `19513245-r1-control-pilot`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 1; resumed false.
Receipt digest `5e7a93870e0bd3026b17985e5949d65d9f6e3c76ee3ecc4936ad9486778f6392`.

Family `bytefield-consumer-control`: 1 comparisons at family-wise alpha 0.025, per-comparison confidence 0.975000, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `pairwise-4k-batch` | Exploratory | SingleCore | 0 | 12 | 0/120 | 2.3215 | [2.3121, 2.3254] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `pairwise-128k-batch` | Exploratory | SingleCore | 0 | 12 | 0/120 | 2.2854 | [2.2728, 2.3385] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `pairwise-128k-whole-batch` | Exploratory | SingleCore | 0 | 12 | 0/120 | 3.4801 | [3.4682, 3.6072] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.
