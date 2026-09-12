# Acceptance summary for `3be770d5-v4-r1-steady-multicore-confirmation`

Label `Confirmation`; verdict **Accepted**; qualifies for production selection: **true**; sessions 6; resumed true.
Receipt digest `0358b647f1d5e7a1980c461661ca395118011d57e4dfe3c404609cba0428c477`.

Family `ldpc-steady-matched-multicore-v1`: 6 comparisons at family-wise alpha 0.025, per-comparison confidence 0.995833, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `dvb-t2-r12-matched-p6` | Confirmatory | PhysicalCores6 | 0,1,2,3,4,5 | 24 | 0/240 | 15.5331 | [15.3470, 15.8269] at 0.9958 | Improved | **Pass** |  |
| `dvb-t2-r12-matched-p12` | Confirmatory | PhysicalCores12 | 0,1,2,3,4,5,6,7,8,9,10,11 | 24 | 0/240 | 15.3404 | [15.1440, 15.4642] at 0.9958 | Improved | **Pass** |  |
| `dvb-t2-r12-matched-l24` | Confirmatory | LogicalCpus24 | 0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23 | 24 | 0/240 | 12.0686 | [11.8882, 12.1497] at 0.9958 | Improved | **Pass** |  |
| `nr-bg1-z384-matched-p6` | Confirmatory | PhysicalCores6 | 0,1,2,3,4,5 | 24 | 0/240 | 18.1939 | [18.0147, 18.3786] at 0.9958 | Improved | **Pass** |  |
| `nr-bg1-z384-matched-p12` | Confirmatory | PhysicalCores12 | 0,1,2,3,4,5,6,7,8,9,10,11 | 24 | 0/240 | 16.5324 | [16.3513, 16.6406] at 0.9958 | Improved | **Pass** |  |
| `nr-bg1-z384-matched-l24` | Confirmatory | LogicalCpus24 | 0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23 | 24 | 0/240 | 15.7970 | [15.4956, 15.8951] at 0.9958 | Improved | **Pass** |  |

## Findings

No findings.
