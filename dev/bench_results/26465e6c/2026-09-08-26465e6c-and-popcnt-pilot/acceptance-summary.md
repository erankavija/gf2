# Acceptance summary for `and-popcnt-pilot-26465e6c-20260908`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 1; resumed false.
Receipt digest `26498454137e2091ea42aa537b3f6fe9d3532eb6fa8fdb591636a935959509ef`.

Family `and-popcnt-baselines-pilot`: 1 comparisons at family-wise alpha 0.05, per-comparison confidence 0.950000, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `and-popcnt-pilot-cache-resident` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.3252 | [0.3241, 0.3253] at 0.9500 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.material_gap_threshold, effect.equivalence_margin |
| `and-popcnt-pilot-streaming` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.6533 | [0.6436, 0.6719] at 0.9500 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.material_gap_threshold, effect.equivalence_margin |
| `and-popcnt-pilot-two-pass-consumer` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.3869 | [0.3793, 0.3925] at 0.9500 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.material_gap_threshold, effect.equivalence_margin |

## Findings

No findings.
