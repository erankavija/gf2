# Acceptance summary for `v4-r2-confirmation-63bad95d-native-matvec-vs-m4ri`

Label `Confirmation`; verdict **Accepted**; qualifies for production selection: **false**; sessions 3; resumed true.
Receipt digest `f2bab12d05671015ec3da75474a8d284df1c2d858d8f7a30ad998e207c00145d`.

Family `63bad95d-native-matvec-vs-m4ri`: 6 comparisons; family-wise alpha 0.05 (frozen total), attempt alpha 0.025 (this attempt's sequential allocation), corrected alpha 0.004166666666666667 per comparison (confidence 0.995833), 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `native-m4ri-gap-65x512-warm` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.0028 | [0.0028, 0.0028] at 0.9958 | Regressed | **Fail** |  |
| `native-m4ri-gap-65x4096-warm` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.0009 | [0.0009, 0.0009] at 0.9958 | Regressed | **Fail** |  |
| `native-m4ri-gap-65x512-retained-warm` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.0900 | [0.0899, 0.0903] at 0.9958 | Regressed | **Fail** |  |
| `native-m4ri-gap-65x4096-retained-warm` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.1865 | [0.1862, 0.1869] at 0.9958 | Regressed | **Fail** |  |
| `native-m4ri-gap-1x1-warm` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.1134 | [0.1125, 0.1159] at 0.9958 | Regressed | **Fail** |  |
| `native-m4ri-gap-64x64-warm` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.0288 | [0.0286, 0.0290] at 0.9958 | Regressed | **Fail** |  |

## Findings

No findings.
