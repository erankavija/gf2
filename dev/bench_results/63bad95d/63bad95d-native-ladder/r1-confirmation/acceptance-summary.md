# Acceptance summary for `63bad95d-r1-native-ladder-confirmation`

Label `Confirmation`; verdict **Accepted**; qualifies for production selection: **false**; sessions 2; resumed true.
Receipt digest `2b9f614a2c11224e737d7df0ab0aab4da139a3798767630bb50f7352c6ae476b`.

Family `63bad95d-native-ladder`: 6 comparisons; family-wise alpha 0.05 (frozen total), attempt alpha 0.025 (this attempt's sequential allocation), corrected alpha 0.004166666666666667 per comparison (confidence 0.995833), 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `matvec-r1024-24w-native` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.1378 | [0.5893, 1.6060] at 0.9958 | Inconclusive | **Inconclusive** |  |
| `bitmatrix-transpose-8192x2048-native` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.0456 | [0.9955, 1.0888] at 0.9958 | NotWorse | **Pass** |  |
| `popcount-1024w-native` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.1884 | [0.7591, 1.5668] at 0.9958 | Inconclusive | **Inconclusive** |  |
| `and-popcount-1024w-native` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.0187 | [1.0022, 1.0325] at 0.9958 | NotWorse | **Pass** |  |
| `xor-inplace-8w-native` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.8389 | [0.5778, 1.3181] at 0.9958 | Inconclusive | **Inconclusive** |  |
| `clmul-wide-9w-native` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.0776 | [1.0597, 1.0924] at 0.9958 | NotWorse | **Pass** |  |

## Findings

No findings.
