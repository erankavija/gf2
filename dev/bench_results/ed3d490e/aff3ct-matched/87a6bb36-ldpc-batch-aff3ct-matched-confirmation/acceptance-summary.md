# Acceptance summary for `87a6bb36-ldpc-batch-aff3ct-matched-confirmation`

Label `Confirmation`; verdict **Accepted**; qualifies for production selection: **false**; sessions 6; resumed true.
Receipt digest `51bf2ddbb59c7f35f2683cc0f528df5fa5ceb75aff29f2ec767eddfb42a2a894`.

Family `ldpc-batch-aff3ct-matched-v1`: 6 comparisons; family-wise alpha 0.05 (frozen total), attempt alpha 0.025 (this attempt's sequential allocation), corrected alpha 0.004166666666666667 per comparison (confidence 0.995833), 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `dvb-t2-r12-matched-w1-b8` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.3810 | [0.3789, 0.3824] at 0.9958 | Regressed | **Fail** |  |
| `dvb-t2-r12-matched-w1-b1` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.8966 | [1.8920, 1.9021] at 0.9958 | Improved | **Pass** |  |
| `dvb-t2-r12-matched-p12-b8` | Confirmatory | PhysicalCores12 | 0,1,2,3,4,5,6,7,8,9,10,11 | 24 | 0/240 | 3.1584 | [3.1078, 3.2187] at 0.9958 | Improved | **Pass** |  |
| `nr-bg1-z384-matched-w1-b8` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.3135 | [0.3069, 0.3277] at 0.9958 | Regressed | **Fail** |  |
| `nr-bg1-z384-matched-w1-b1` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.8827 | [1.8639, 1.9743] at 0.9958 | Improved | **Pass** |  |
| `nr-bg1-z384-matched-p12-b8` | Confirmatory | PhysicalCores12 | 0,1,2,3,4,5,6,7,8,9,10,11 | 24 | 0/240 | 1.2873 | [1.2418, 1.3419] at 0.9958 | NotWorse | **NotMaterial** |  |

## Findings

No findings.
