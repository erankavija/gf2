# Acceptance summary for `bfc2ceab-r1-entry-check-confirmation`

Label `Confirmation`; verdict **Accepted**; qualifies for production selection: **false**; sessions 2; resumed true.
Receipt digest `2b48f95b3dbb1d57201b542ae176a61c1c119ea57efceb139a99acf83d0550ce`.

Family `caller-trusted-entry-check-cost`: 6 comparisons; family-wise alpha 0.05 (frozen total), attempt alpha 0.025 (this attempt's sequential allocation), corrected alpha 0.004166666666666667 per comparison (confidence 0.995833), 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `spmm-small-n16` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.9322 | [0.9308, 0.9332] at 0.9958 | Regressed | **Fail** |  |
| `spmm-medium-n16` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.9371 | [0.9356, 0.9399] at 0.9958 | Regressed | **Fail** |  |
| `permanent-f3-n14` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.9893 | [0.9769, 1.0025] at 0.9958 | NotWorse | **Pass** |  |
| `ple-small-n128` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.9982 | [0.9958, 1.0029] at 0.9958 | NotWorse | **Pass** |  |
| `gemm-small-n32` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.9857 | [0.9817, 0.9891] at 0.9958 | NotWorse | **Pass** |  |
| `gemm-medium-n32` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.9997 | [0.9989, 1.0018] at 0.9958 | NotWorse | **Pass** |  |

## Findings

No findings.
