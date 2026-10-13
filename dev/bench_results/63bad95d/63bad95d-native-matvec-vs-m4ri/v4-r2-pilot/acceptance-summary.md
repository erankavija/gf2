# Acceptance summary for `v4-r2-63bad95d-native-matvec-vs-m4ri`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 3; resumed true.
Receipt digest `351e2fa3583e58151f9d9803f06f4a69e3e4d0b24156cb3efab2e1a9e68cfce6`.

Family `63bad95d-native-matvec-vs-m4ri`: 1 comparisons; family-wise alpha 0.05 (frozen total), attempt alpha 0.025 (this attempt's sequential allocation), corrected alpha 0.025 per comparison (confidence 0.975000), 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `native-m4ri-gap-65x512-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.0028 | [0.0028, 0.0028] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `native-m4ri-gap-65x4096-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.0009 | [0.0009, 0.0009] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `native-m4ri-gap-65x512-retained-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.0900 | [0.0899, 0.0902] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `native-m4ri-gap-65x4096-retained-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.1879 | [0.1876, 0.1880] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `native-m4ri-gap-1x1-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.1137 | [0.1125, 0.1149] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `native-m4ri-gap-64x64-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.0287 | [0.0286, 0.0288] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.
