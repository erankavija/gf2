# Acceptance summary for `v4-r1-63bad95d-portable-matvec-vs-m4ri`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 3; resumed true.
Receipt digest `c4e96aa34746dd16fe153532947b94e59ded6df464d824cda8a5661b0573952d`.

Family `63bad95d-portable-matvec-vs-m4ri`: 1 comparisons; family-wise alpha 0.05 (frozen total), attempt alpha 0.025 (this attempt's sequential allocation), corrected alpha 0.025 per comparison (confidence 0.975000), 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `portable-m4ri-gap-65x512-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.0026 | [0.0026, 0.0027] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `portable-m4ri-gap-65x4096-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.0009 | [0.0009, 0.0012] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `portable-m4ri-gap-65x512-retained-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.0860 | [0.0845, 0.0891] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `portable-m4ri-gap-65x4096-retained-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.1910 | [0.1900, 0.1929] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `portable-m4ri-gap-1x1-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.1166 | [0.1161, 0.1173] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `portable-m4ri-gap-64x64-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.0291 | [0.0284, 0.0297] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.
