# Acceptance summary for `31727c57-decoder-confirmation`

Label `Confirmation`; verdict **Accepted**; qualifies for production selection: **true**; sessions 6; resumed true.
Receipt digest `ab21e122b1508bcd986cf25421e64fa9f7966065514456785d2db71158ad3fcf`.

Family `1362381c-decoder-route-integration`: 6 comparisons; family-wise alpha 0.05 (frozen total), attempt alpha 0.025 (this attempt's sequential allocation), corrected alpha 0.004166666666666667 per comparison (confidence 0.995833), 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `dvb-t2-r12-integrated-w1-b1` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.0046 | [1.0027, 1.0065] at 0.9958 | NotWorse | **Pass** |  |
| `dvb-t2-r12-integrated-w6-b8` | Confirmatory | PhysicalCores6 | 0,1,2,3,4,5 | 24 | 0/240 | 1.0063 | [1.0042, 1.0093] at 0.9958 | NotWorse | **Pass** |  |
| `nr-bg1-z384-integrated-w12-b8` | Confirmatory | PhysicalCores12 | 0,1,2,3,4,5,6,7,8,9,10,11 | 24 | 0/240 | 1.0064 | [0.9994, 1.0229] at 0.9958 | NotWorse | **Pass** |  |
| `nr-bg1-z384-integrated-w24-b8` | Confirmatory | LogicalCpus24 | 0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23 | 24 | 0/240 | 1.0020 | [0.9976, 1.0066] at 0.9958 | NotWorse | **Pass** |  |
| `nr-bg1-z384-integrated-holdout-w1-b5` | Holdout | SingleCore | 0 | 24 | 0/240 | 3.8706 | [3.8619, 3.8815] at 0.9958 | Improved | **Pass** |  |
| `nr-bg1-z384-integrated-holdout-w1-b13` | Holdout | SingleCore | 0 | 24 | 0/240 | 4.6368 | [4.6193, 4.6548] at 0.9958 | Improved | **Pass** |  |

## Findings

No findings.
