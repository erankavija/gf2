# Acceptance summary for `093ae4d0-ldpc-batch-multicore-confirmation`

Label `Confirmation`; verdict **Accepted**; qualifies for production selection: **false**; sessions 6; resumed true.
Receipt digest `11db66df880abec205131fbf9065c9cf8043aee2b267b45c6b992263ba903f26`.

Family `ldpc-batch-multicore-v1`: 6 comparisons; family-wise alpha 0.05 (frozen total), attempt alpha 0.025 (this attempt's sequential allocation), corrected alpha 0.004166666666666667 per comparison (confidence 0.995833), 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `dvb-t2-r12-lane-p6-b8` | Confirmatory | PhysicalCores6 | 0,1,2,3,4,5 | 24 | 0/240 | 0.8269 | [0.8228, 0.8317] at 0.9958 | Regressed | **Fail** |  |
| `dvb-t2-r12-lane-p12-b8` | Confirmatory | PhysicalCores12 | 0,1,2,3,4,5,6,7,8,9,10,11 | 24 | 0/240 | 0.5572 | [0.5543, 0.5601] at 0.9958 | Regressed | **Fail** |  |
| `dvb-t2-r12-lane-l24-b8` | Confirmatory | LogicalCpus24 | 0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23 | 24 | 0/240 | 0.3310 | [0.3292, 0.3338] at 0.9958 | Regressed | **Fail** |  |
| `nr-bg1-z384-lane-p6-b8` | Confirmatory | PhysicalCores6 | 0,1,2,3,4,5 | 24 | 0/240 | 1.5013 | [1.4306, 1.5425] at 0.9958 | Improved | **Pass** |  |
| `nr-bg1-z384-lane-p12-b8` | Confirmatory | PhysicalCores12 | 0,1,2,3,4,5,6,7,8,9,10,11 | 24 | 0/240 | 1.2020 | [1.1716, 1.2332] at 0.9958 | Improved | **Pass** |  |
| `nr-bg1-z384-lane-l24-b8` | Confirmatory | LogicalCpus24 | 0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23 | 24 | 0/240 | 0.3570 | [0.3561, 0.3586] at 0.9958 | Regressed | **Fail** |  |

## Findings

No findings.
