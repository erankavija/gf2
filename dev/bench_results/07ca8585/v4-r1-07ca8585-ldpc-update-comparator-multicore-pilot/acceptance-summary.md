# Acceptance summary for `07ca8585-v4-r1-update-comparator-multicore-pilot`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 6; resumed true.
Receipt digest `0a13431a52b6c7e7c9f8cccc890ce874d67c9ec01f631d62517099980544f04a`.

Family `ldpc-update-comparator-multicore-v1`: 1 comparisons at family-wise alpha 0.025, per-comparison confidence 0.975000, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `dvb-t2-r12-update-p6` | Exploratory | PhysicalCores6 | 0,1,2,3,4,5 | 6 | 0/60 | 1.8347 | [1.8228, 1.8459] at 0.9750 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.material_gap_threshold, effect.equivalence_margin |
| `dvb-t2-r12-update-p12` | Exploratory | PhysicalCores12 | 0,1,2,3,4,5,6,7,8,9,10,11 | 6 | 0/60 | 1.7465 | [1.6943, 1.8191] at 0.9750 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.material_gap_threshold, effect.equivalence_margin |
| `dvb-t2-r12-update-l24` | Exploratory | LogicalCpus24 | 0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23 | 6 | 0/60 | 0.8230 | [0.8124, 0.8325] at 0.9750 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.material_gap_threshold, effect.equivalence_margin |
| `nr-bg1-z384-update-p6` | Exploratory | PhysicalCores6 | 0,1,2,3,4,5 | 6 | 0/60 | 1.6795 | [1.6302, 1.7218] at 0.9750 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.material_gap_threshold, effect.equivalence_margin |
| `nr-bg1-z384-update-p12` | Exploratory | PhysicalCores12 | 0,1,2,3,4,5,6,7,8,9,10,11 | 6 | 0/60 | 1.5250 | [1.4961, 1.5783] at 0.9750 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.material_gap_threshold, effect.equivalence_margin |
| `nr-bg1-z384-update-l24` | Exploratory | LogicalCpus24 | 0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23 | 6 | 0/60 | 1.1083 | [1.0554, 1.1358] at 0.9750 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.material_gap_threshold, effect.equivalence_margin |

## Findings

No findings.
