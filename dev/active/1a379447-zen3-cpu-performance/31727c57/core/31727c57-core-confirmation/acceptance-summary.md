# Acceptance summary for `31727c57-core-confirmation`

Label `Confirmation`; verdict **Accepted**; qualifies for production selection: **true**; sessions 6; resumed true.
Receipt digest `135497a20af294e8e1476be5a6268b16157e2a303f872f5566efd8ec21a92855`.

Family `1362381c-core-route-integration`: 6 comparisons; family-wise alpha 0.05 (frozen total), attempt alpha 0.025 (this attempt's sequential allocation), corrected alpha 0.004166666666666667 per comparison (confidence 0.995833), 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `matvec-r1024-24w-native` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.3375 | [1.1681, 1.3488] at 0.9958 | Improved | **Pass** |  |
| `popcount-1024w-native` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.0228 | [1.0218, 1.0244] at 0.9958 | NotWorse | **Pass** |  |
| `clmul-wide-9w-native` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.0722 | [1.0707, 1.0736] at 0.9958 | NotWorse | **Pass** |  |
| `xor-inplace-5w` | Holdout | SingleCore | 0 | 24 | 0/240 | 1.0499 | [1.0467, 1.0561] at 0.9958 | NotWorse | **Pass** |  |
| `popcount-6w` | Holdout | SingleCore | 0 | 24 | 0/240 | 1.3548 | [1.3490, 1.3575] at 0.9958 | Improved | **Pass** |  |
| `xor-inplace-3w` | Holdout | SingleCore | 0 | 24 | 0/240 | 1.0047 | [0.9970, 1.0067] at 0.9958 | NotWorse | **Pass** |  |

## Findings

No findings.
