# Acceptance summary for `d0d2c1bd-ldpc-batch-single-worker-confirmation`

Label `Confirmation`; verdict **Accepted**; qualifies for production selection: **false**; sessions 6; resumed true.
Receipt digest `748c2ff8179fb7eba42de59875b4207fe9101e77e4342de0707704e03e14bf01`.

Family `ldpc-batch-single-worker-v1`: 6 comparisons; family-wise alpha 0.05 (frozen total), attempt alpha 0.025 (this attempt's sequential allocation), corrected alpha 0.004166666666666667 per comparison (confidence 0.995833), 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `dvb-t2-r12-lane-w1-b8` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 5.0510 | [5.0364, 5.0628] at 0.9958 | Improved | **Pass** |  |
| `dvb-t2-r12-lane-w1-b1` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.0207 | [1.0141, 1.0230] at 0.9958 | NotWorse | **Pass** |  |
| `dvb-t2-r12-lane-w1-b11` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 3.7450 | [3.7183, 3.7551] at 0.9958 | Improved | **Pass** |  |
| `nr-bg1-z384-lane-w1-b8` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 5.3765 | [5.3628, 5.3875] at 0.9958 | Improved | **Pass** |  |
| `nr-bg1-z384-lane-w1-b1` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.8910 | [0.8878, 0.8954] at 0.9958 | Regressed | **Fail** |  |
| `nr-bg1-z384-lane-w1-b11` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 3.7479 | [3.7415, 3.7609] at 0.9958 | Improved | **Pass** |  |

## Findings

No findings.
