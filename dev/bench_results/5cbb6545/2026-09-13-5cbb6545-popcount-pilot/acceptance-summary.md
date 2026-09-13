# Acceptance summary for `popcount-pilot-5cbb6545-20260913t125443z`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 1; resumed false.
Receipt digest `9979b200a01c807c6936d18e9125c70dcd54f3f1d6406193ab93fe3be05c9b94`.

Family `popcount-route-selection`: 1 comparisons at family-wise alpha 0.025, per-comparison confidence 0.975000, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `popcount-w4-dispatch` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.0181 | [1.0169, 1.0571] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w8-dispatch` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.1624 | [1.1552, 1.2190] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w128-dispatch` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.0448 | [1.0414, 1.0481] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w1024-dispatch` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.3302 | [1.3072, 1.3371] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w16384-dispatch` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.3196 | [1.3146, 1.3248] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w16384-vs-libpopcnt` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.0323 | [1.0297, 1.0345] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.
