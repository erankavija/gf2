# Acceptance summary for `v4-r1-2037941f-dense-matvec-vs-m4ri`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 4; resumed true.
Receipt digest `714223d50a6774a7a793db6848c162e45b29958672fc12d807e94ad7d61ba9ab`.

Family `2037941f-dense-matvec-vs-m4ri`: 1 comparisons; family-wise alpha 0.05 (frozen total), attempt alpha 0.025 (this attempt's sequential allocation), corrected alpha 0.025 per comparison (confidence 0.975000), 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `m4ri-gap-65x512-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.0028 | [0.0028, 0.0028] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `m4ri-gap-65x4096-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.0009 | [0.0009, 0.0009] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `m4ri-gap-1x1-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.1253 | [0.1238, 0.1264] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `m4ri-gap-63x63-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.0304 | [0.0304, 0.0305] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `m4ri-gap-64x64-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.0306 | [0.0305, 0.0306] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `m4ri-gap-65x65-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.0328 | [0.0327, 0.0328] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `m4ri-gap-65x512-retained-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.0900 | [0.0899, 0.0903] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `m4ri-gap-65x4096-retained-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.1976 | [0.1974, 0.1980] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.
