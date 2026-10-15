# Acceptance summary for `a7ae2623-ldpc-batch-multicore-pilot`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 6; resumed true.
Receipt digest `9e33fc9584bf899f8c0c28c5a23b4bf0e042a1ca642e4033fa46b2c199476347`.

Family `ldpc-batch-multicore-v1`: 1 comparisons; family-wise alpha 0.05 (frozen total), attempt alpha 0.025 (this attempt's sequential allocation), corrected alpha 0.025 per comparison (confidence 0.975000), 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `dvb-t2-r12-lane-p6-b8` | Exploratory | PhysicalCores6 | 0,1,2,3,4,5 | 12 | 0/120 | 0.8333 | [0.8303, 0.8365] at 0.9750 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.worthwhile_speedup, effect.equivalence_margin |
| `dvb-t2-r12-lane-p12-b8` | Exploratory | PhysicalCores12 | 0,1,2,3,4,5,6,7,8,9,10,11 | 12 | 0/120 | 0.5576 | [0.5556, 0.5671] at 0.9750 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.worthwhile_speedup, effect.equivalence_margin |
| `dvb-t2-r12-lane-l24-b8` | Exploratory | LogicalCpus24 | 0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23 | 12 | 0/120 | 0.3299 | [0.3284, 0.3318] at 0.9750 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.worthwhile_speedup, effect.equivalence_margin |
| `nr-bg1-z384-lane-p6-b8` | Exploratory | PhysicalCores6 | 0,1,2,3,4,5 | 12 | 0/120 | 1.5036 | [1.4243, 1.5589] at 0.9750 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.worthwhile_speedup, effect.equivalence_margin |
| `nr-bg1-z384-lane-p12-b8` | Exploratory | PhysicalCores12 | 0,1,2,3,4,5,6,7,8,9,10,11 | 12 | 0/120 | 1.2076 | [1.1810, 1.2364] at 0.9750 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.worthwhile_speedup, effect.equivalence_margin |
| `nr-bg1-z384-lane-l24-b8` | Exploratory | LogicalCpus24 | 0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23 | 12 | 0/120 | 0.3565 | [0.3555, 0.3583] at 0.9750 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.worthwhile_speedup, effect.equivalence_margin |

## Findings

No findings.
