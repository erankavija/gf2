# Acceptance summary for `f63a2464-v4-r1-qc-intra-frame-multicore-confirmation`

Label `Confirmation`; verdict **Accepted**; qualifies for production selection: **true**; sessions 3; resumed true.
Receipt digest `5eb9dbd4a4adf9d04847155ea38e82c738caf6b3a2bae1ba7cba47b94e6e4018`.

Family `ldpc-qc-intra-frame-multicore-v1`: 3 comparisons; family-wise alpha 0.05 (frozen total), attempt alpha 0.025 (this attempt's sequential allocation), corrected alpha 0.008333333333333333 per comparison (confidence 0.991667), 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `nr-bg1-z384-qc-w6` | Confirmatory | PhysicalCores6 | 0,1,2,3,4,5 | 24 | 0/240 | 8.6440 | [8.6221, 8.6772] at 0.9917 | Improved | **Pass** |  |
| `nr-bg1-z384-qc-w12` | Confirmatory | PhysicalCores12 | 0,1,2,3,4,5,6,7,8,9,10,11 | 24 | 0/240 | 8.2970 | [8.1995, 8.4357] at 0.9917 | Improved | **Pass** |  |
| `nr-bg1-z384-qc-w24` | Confirmatory | LogicalCpus24 | 0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23 | 24 | 0/240 | 6.0823 | [6.0123, 6.2494] at 0.9917 | Improved | **Pass** |  |

## Findings

No findings.
