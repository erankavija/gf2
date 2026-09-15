# Acceptance summary for `07ca8585-v4-r1-update-multicore-pilot`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 6; resumed true.
Receipt digest `462141bac4d1e45335d19e31389cb02f0eec83757b94d45e596c63929f97d5e3`.

Family `ldpc-update-multicore-v1`: 1 comparisons at family-wise alpha 0.025, per-comparison confidence 0.975000, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `dvb-t2-r12-update-p6` | Exploratory | PhysicalCores6 | 0,1,2,3,4,5 | 6 | 0/60 | 8.3756 | [8.3581, 8.4308] at 0.9750 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.worthwhile_speedup, effect.equivalence_margin |
| `dvb-t2-r12-update-p12` | Exploratory | PhysicalCores12 | 0,1,2,3,4,5,6,7,8,9,10,11 | 6 | 0/60 | 8.6057 | [8.4793, 8.6463] at 0.9750 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.worthwhile_speedup, effect.equivalence_margin |
| `dvb-t2-r12-update-l24` | Exploratory | LogicalCpus24 | 0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23 | 6 | 0/60 | 14.7519 | [14.5964, 14.9440] at 0.9750 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.worthwhile_speedup, effect.equivalence_margin |
| `nr-bg1-z384-update-p6` | Exploratory | PhysicalCores6 | 0,1,2,3,4,5 | 6 | 0/60 | 11.1143 | [11.0587, 11.1565] at 0.9750 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.worthwhile_speedup, effect.equivalence_margin |
| `nr-bg1-z384-update-p12` | Exploratory | PhysicalCores12 | 0,1,2,3,4,5,6,7,8,9,10,11 | 6 | 0/60 | 10.9596 | [10.5808, 10.9969] at 0.9750 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.worthwhile_speedup, effect.equivalence_margin |
| `nr-bg1-z384-update-l24` | Exploratory | LogicalCpus24 | 0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23 | 6 | 0/60 | 14.1436 | [13.7510, 14.5813] at 0.9750 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.worthwhile_speedup, effect.equivalence_margin |

## Findings

No findings.
