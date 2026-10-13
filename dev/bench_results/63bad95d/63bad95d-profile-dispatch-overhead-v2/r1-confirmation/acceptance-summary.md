# Acceptance summary for `63bad95d-r1-profile-dispatch-overhead-v2-confirmation`

Label `Confirmation`; verdict **Accepted**; qualifies for production selection: **true**; sessions 2; resumed true.
Receipt digest `e1b2ad71ba4cfa57fddc62534b7178a58c91044c836f58c6abb4db81457759a8`.

Family `63bad95d-profile-dispatch-overhead-v2`: 6 comparisons; family-wise alpha 0.05 (frozen total), attempt alpha 0.025 (this attempt's sequential allocation), corrected alpha 0.004166666666666667 per comparison (confidence 0.995833), 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `xor-inplace-4w` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.3029 | [1.2572, 1.3091] at 0.9958 | Improved | **Pass** |  |
| `and-popcount-7w` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.9857 | [0.9825, 0.9886] at 0.9958 | NotWorse | **Pass** |  |
| `xor-inplace-8w` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.9977 | [0.9924, 1.0014] at 0.9958 | NotWorse | **Pass** |  |
| `xor-inplace-5w` | Holdout | SingleCore | 0 | 24 | 0/240 | 1.1373 | [1.1273, 1.1433] at 0.9958 | Improved | **Pass** |  |
| `popcount-6w` | Holdout | SingleCore | 0 | 24 | 0/240 | 1.3306 | [1.3272, 1.3335] at 0.9958 | Improved | **Pass** |  |
| `xor-inplace-3w` | Holdout | SingleCore | 0 | 24 | 0/240 | 0.9954 | [0.9924, 1.0024] at 0.9958 | NotWorse | **Pass** |  |

## Findings

No findings.
