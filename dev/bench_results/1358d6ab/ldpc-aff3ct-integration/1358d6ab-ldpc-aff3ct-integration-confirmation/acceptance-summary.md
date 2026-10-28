# Acceptance summary for `1358d6ab-ldpc-aff3ct-integration-confirmation`

Label `Confirmation`; verdict **Accepted**; qualifies for production selection: **false**; sessions 6; resumed true.
Receipt digest `6aceec7fa2edd87f4ec43b32ebfb07c040bc9f46f1ce9eabe2ba20727a4b89e3`.

Family `1362381c-ldpc-aff3ct-integration`: 6 comparisons; family-wise alpha 0.05 (frozen total), attempt alpha 0.025 (this attempt's sequential allocation), corrected alpha 0.004166666666666667 per comparison (confidence 0.995833), 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `dvb-t2-r12-integrated-w1-b8` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.3759 | [0.3692, 0.3773] at 0.9958 | Regressed | **Fail** |  |
| `dvb-t2-r12-integrated-w1-b1` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.9720 | [1.9690, 1.9765] at 0.9958 | Improved | **Pass** |  |
| `dvb-t2-r12-integrated-p12-b8` | Confirmatory | PhysicalCores12 | 0,1,2,3,4,5,6,7,8,9,10,11 | 24 | 0/240 | 1.8195 | [1.7726, 1.8412] at 0.9958 | Improved | **Pass** |  |
| `nr-bg1-z384-integrated-w1-b8` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.3249 | [0.3127, 0.3263] at 0.9958 | Regressed | **Fail** |  |
| `nr-bg1-z384-integrated-w1-b1` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.7762 | [1.7696, 1.7824] at 0.9958 | Improved | **Pass** |  |
| `nr-bg1-z384-integrated-p12-b8` | Confirmatory | PhysicalCores12 | 0,1,2,3,4,5,6,7,8,9,10,11 | 24 | 0/240 | 1.5428 | [1.5040, 1.5603] at 0.9958 | Improved | **Pass** |  |

## Findings

No findings.
