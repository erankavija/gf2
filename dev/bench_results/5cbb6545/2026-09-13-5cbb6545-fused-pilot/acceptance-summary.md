# Acceptance summary for `fused-pilot-5cbb6545-20260913t125637z`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 1; resumed false.
Receipt digest `cab6f267a58bafb5b62217efb45fd75b526420783df31d80b8f1c1a40166b3a0`.

Family `fused-count-consumers`: 1 comparisons at family-wise alpha 0.025, per-comparison confidence 0.975000, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `and-w128-fused` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.9778 | [0.9744, 0.9800] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `and-w512-fused` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.3046 | [1.2365, 1.3108] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `and-w4096-fused` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.2833 | [1.2812, 1.2863] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `and-w4096-vs-two-pass` | Exploratory | SingleCore | 0 | 6 | 0/60 | 2.9989 | [2.9876, 3.0310] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `matvec-1024x16384` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.1904 | [1.1579, 1.2149] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `matvec-1024x4096` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.9217 | [0.8866, 0.9772] at 0.9750 | Inconclusive | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.
