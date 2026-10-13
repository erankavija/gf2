# Acceptance summary for `v4-r1-63bad95d-native-matvec-vs-m4ri`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 3; resumed true.
Receipt digest `f4c946d334e144fb72a3315ccd159cd73ae215d83649440fcae9fc91ca4ef264`.

Family `63bad95d-native-matvec-vs-m4ri`: 1 comparisons; family-wise alpha 0.05 (frozen total), attempt alpha 0.025 (this attempt's sequential allocation), corrected alpha 0.025 per comparison (confidence 0.975000), 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `native-m4ri-gap-65x512-warm` | Exploratory | SingleCore | 0 | 24 | 1/240 | 0.0037 | [0.0024, 0.0048] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `native-m4ri-gap-65x4096-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.0008 | [0.0007, 0.0010] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `native-m4ri-gap-65x512-retained-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.0897 | [0.0889, 0.0900] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `native-m4ri-gap-65x4096-retained-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.1879 | [0.1836, 0.1916] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `native-m4ri-gap-1x1-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.1149 | [0.1131, 0.1170] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `native-m4ri-gap-64x64-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.0287 | [0.0282, 0.0290] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.
