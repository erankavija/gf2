# Acceptance summary for `63bad95d-r1-native-ladder-v2-confirmation`

Label `Confirmation`; verdict **Accepted**; qualifies for production selection: **true**; sessions 2; resumed true.
Receipt digest `24d93009a4d3609a66fc79972d975b343bda2aae5b9351b7cbdcb7904802c49e`.

Family `63bad95d-native-ladder-v2`: 6 comparisons; family-wise alpha 0.05 (frozen total), attempt alpha 0.025 (this attempt's sequential allocation), corrected alpha 0.004166666666666667 per comparison (confidence 0.995833), 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `matvec-r1024-24w-native` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.0327 | [1.0227, 1.0488] at 0.9958 | NotWorse | **Pass** |  |
| `bitmatrix-transpose-8192x2048-native` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.0000 | [0.9903, 1.0332] at 0.9958 | NotWorse | **Pass** |  |
| `popcount-1024w-native` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.0277 | [1.0264, 1.0297] at 0.9958 | NotWorse | **Pass** |  |
| `and-popcount-1024w-native` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.0253 | [1.0212, 1.0278] at 0.9958 | NotWorse | **Pass** |  |
| `xor-inplace-8w-native` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.9979 | [0.9814, 1.0048] at 0.9958 | NotWorse | **Pass** |  |
| `clmul-wide-9w-native` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.0634 | [1.0619, 1.0650] at 0.9958 | NotWorse | **Pass** |  |

## Findings

No findings.
