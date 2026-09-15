# Acceptance summary for `19513245-r1-matrix-confirmation`

Label `Confirmation`; verdict **Accepted**; qualifies for production selection: **true**; sessions 3; resumed true.
Receipt digest `0610976ca4c2b0d04681b29f4ff87949c58edb9f5e9dfdea4d4cc5e81c88d8a2`.

Family `bytefield-consumer-matrix`: 6 comparisons at family-wise alpha 0.025, per-comparison confidence 0.995833, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `matmul-n256-element` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 10.0355 | [10.0110, 10.1399] at 0.9958 | Improved | **Pass** |  |
| `matmul-n512-element` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 10.1061 | [10.0827, 10.1960] at 0.9958 | Improved | **Pass** |  |
| `matmul-n256-whole-element` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 9.5422 | [9.5197, 9.5986] at 0.9958 | Improved | **Pass** |  |
| `matmul-n256-wide` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 3.0726 | [3.0660, 3.0795] at 0.9958 | Improved | **Pass** |  |
| `matmul-n512-wide` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 3.1211 | [3.1165, 3.1260] at 0.9958 | Improved | **Pass** |  |
| `matmul-n256-whole-wide` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 3.0493 | [3.0433, 3.0545] at 0.9958 | Improved | **Pass** |  |

## Findings

No findings.
