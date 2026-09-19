# Acceptance summary for `f63a2464-v4-r1-qc-intra-frame-multicore-pilot`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 3; resumed true.
Receipt digest `82610c2d8c31f7bac799478d3fb1b40ada916d6848904e2304dbe5ac4e9fac12`.

Family `ldpc-qc-intra-frame-multicore-v1`: 1 comparisons; family-wise alpha 0.05 (frozen total), attempt alpha 0.025 (this attempt's sequential allocation), corrected alpha 0.025 per comparison (confidence 0.975000), 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `nr-bg1-z384-qc-w6` | Exploratory | PhysicalCores6 | 0,1,2,3,4,5 | 6 | 0/60 | 8.6550 | [8.6450, 8.7074] at 0.9750 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.worthwhile_speedup, effect.equivalence_margin |
| `nr-bg1-z384-qc-w12` | Exploratory | PhysicalCores12 | 0,1,2,3,4,5,6,7,8,9,10,11 | 6 | 0/60 | 8.2037 | [8.1766, 8.3497] at 0.9750 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.worthwhile_speedup, effect.equivalence_margin |
| `nr-bg1-z384-qc-w24` | Exploratory | LogicalCpus24 | 0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23 | 6 | 0/60 | 6.1791 | [6.0471, 6.4757] at 0.9750 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.worthwhile_speedup, effect.equivalence_margin |

## Findings

No findings.
