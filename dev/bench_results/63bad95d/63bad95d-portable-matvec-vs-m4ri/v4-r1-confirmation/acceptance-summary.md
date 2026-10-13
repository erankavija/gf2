# Acceptance summary for `v4-r1-confirmation-63bad95d-portable-matvec-vs-m4ri`

Label `Confirmation`; verdict **Accepted**; qualifies for production selection: **false**; sessions 3; resumed true.
Receipt digest `6be4027db75543929ccbf8a66b7f939dd83899bf834526080baa01fa088fcf42`.

Family `63bad95d-portable-matvec-vs-m4ri`: 6 comparisons; family-wise alpha 0.05 (frozen total), attempt alpha 0.025 (this attempt's sequential allocation), corrected alpha 0.004166666666666667 per comparison (confidence 0.995833), 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `portable-m4ri-gap-65x512-warm` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.0026 | [0.0026, 0.0027] at 0.9958 | Regressed | **Fail** |  |
| `portable-m4ri-gap-65x4096-warm` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.0009 | [0.0009, 0.0009] at 0.9958 | Regressed | **Fail** |  |
| `portable-m4ri-gap-65x512-retained-warm` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.0854 | [0.0852, 0.0897] at 0.9958 | Regressed | **Fail** |  |
| `portable-m4ri-gap-65x4096-retained-warm` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.1912 | [0.1907, 0.1917] at 0.9958 | Regressed | **Fail** |  |
| `portable-m4ri-gap-1x1-warm` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.1193 | [0.1188, 0.1215] at 0.9958 | Regressed | **Fail** |  |
| `portable-m4ri-gap-64x64-warm` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.0292 | [0.0292, 0.0293] at 0.9958 | Regressed | **Fail** |  |

## Findings

No findings.
