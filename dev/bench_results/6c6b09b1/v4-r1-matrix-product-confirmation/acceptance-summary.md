# Acceptance summary for `6c6b09b1-v4-r1-matrix-product-confirmation`

Label `Confirmation`; verdict **Accepted**; qualifies for production selection: **true**; sessions 2; resumed true.
Receipt digest `b1126f12db92c26b1d08bcf955e44e2ef08ea6776ae84e389e3a4e66208df9c2`.

Family `byte-field-matrix-product`: 6 comparisons at family-wise alpha 0.025, per-comparison confidence 0.995833, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `matmul-n256-element-vs-m4rie` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 35.2718 | [34.8340, 35.6014] at 0.9958 | Improved | **Pass** |  |
| `matmul-n256-wide-vs-m4rie` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 11.7548 | [11.4089, 11.8052] at 0.9958 | Improved | **Pass** |  |
| `matmul-n256-whole-element-vs-m4rie` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 27.8651 | [27.7174, 28.0342] at 0.9958 | Improved | **Pass** |  |
| `matmul-n256-whole-wide-vs-m4rie` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 9.0363 | [8.9606, 9.0615] at 0.9958 | Improved | **Pass** |  |
| `encode-k10r4-64k-element-vs-isal` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 361.3979 | [360.3794, 362.5983] at 0.9958 | Improved | **Pass** |  |
| `encode-k10r4-64k-wide-vs-isal` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 115.4156 | [115.2164, 115.8774] at 0.9958 | Improved | **Pass** |  |

## Findings

No findings.
