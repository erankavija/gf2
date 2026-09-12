# Acceptance summary for `c7113c5a-v3-r1-host-targeting-pilot`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 1; resumed false.
Receipt digest `8b7fa85812f54188057c12b68a713a7fcba4537f2bf540be6305800fcdead4bb`.

Family `polynomial-host-targeting`: 1 comparisons at family-wise alpha 0.025, per-comparison confidence 0.975000, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `poly-mul-4w-conservative-1core` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.6912 | [0.6896, 0.6977] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `poly-mul-4w-tuned-1core` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.6312 | [0.6274, 0.6327] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `poly-mul-256w-conservative-1core` | Exploratory | SingleCore | 0 | 12 | 0/120 | 287.4239 | [286.0666, 288.1338] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `poly-mul-256w-tuned-1core` | Exploratory | SingleCore | 0 | 12 | 0/120 | 304.8862 | [303.1418, 305.9498] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.
