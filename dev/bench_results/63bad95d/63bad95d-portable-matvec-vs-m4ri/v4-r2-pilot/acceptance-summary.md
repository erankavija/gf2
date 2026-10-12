# Acceptance summary for `v4-r2-63bad95d-portable-matvec-vs-m4ri`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 3; resumed true.
Receipt digest `e1e4022e2c5c237bb4396e3c053790acc43813ec4a3f1c647f3bcf3d7a028a35`.

Family `63bad95d-portable-matvec-vs-m4ri`: 1 comparisons; family-wise alpha 0.05 (frozen total), attempt alpha 0.025 (this attempt's sequential allocation), corrected alpha 0.025 per comparison (confidence 0.975000), 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `portable-m4ri-gap-65x512-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.0026 | [0.0026, 0.0027] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `portable-m4ri-gap-65x4096-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.0009 | [0.0009, 0.0009] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `portable-m4ri-gap-65x512-retained-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.0855 | [0.0853, 0.0860] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `portable-m4ri-gap-65x4096-retained-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.1915 | [0.1912, 0.1918] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `portable-m4ri-gap-1x1-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.1192 | [0.1188, 0.1220] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `portable-m4ri-gap-64x64-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.0293 | [0.0292, 0.0293] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.
