# Acceptance summary for `fused-confirmation-5cbb6545-20260913t165142z`

Label `Confirmation`; verdict **Accepted**; qualifies for production selection: **false**; sessions 1; resumed false.
Receipt digest `4ca8bab0697b768455960c990b0fc21015fe5a97a7dc9d87b89cfa2ff0dea4bb`.

Family `fused-count-consumers`: 6 comparisons at family-wise alpha 0.025, per-comparison confidence 0.995833, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `and-w128-fused` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.9647 | [0.9609, 0.9696] at 0.9958 | Inconclusive | **Inconclusive** |  |
| `and-w512-fused` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.3145 | [1.3086, 1.3203] at 0.9958 | Improved | **Pass** |  |
| `and-w4096-fused` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.2750 | [1.2671, 1.2811] at 0.9958 | Improved | **Pass** |  |
| `and-w4096-vs-two-pass` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 2.9841 | [2.9702, 3.0088] at 0.9958 | Improved | **Pass** |  |
| `matvec-1024x16384` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.1779 | [1.1479, 1.2107] at 0.9958 | Improved | **Pass** |  |
| `matvec-1024x4096` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.9846 | [0.9532, 0.9957] at 0.9958 | Inconclusive | **Inconclusive** |  |

## Findings

No findings.
