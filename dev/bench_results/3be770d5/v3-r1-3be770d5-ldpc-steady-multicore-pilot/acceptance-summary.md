# Acceptance summary for `3be770d5-v3-r1-steady-multicore-pilot`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 6; resumed true.
Receipt digest `ee9b3fe30d5e62096ad52e045862c47b77903089cfd8f32401027707a8ec8483`.

Family `ldpc-steady-matched-multicore-v1`: 1 comparisons at family-wise alpha 0.025, per-comparison confidence 0.975000, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `dvb-t2-r12-matched-p6` | Exploratory | PhysicalCores6 | 0,1,2,3,4,5 | 6 | 0/60 | 15.2619 | [15.1067, 17.0916] at 0.9750 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.material_gap_threshold, effect.equivalence_margin |
| `dvb-t2-r12-matched-p12` | Exploratory | PhysicalCores12 | 0,1,2,3,4,5,6,7,8,9,10,11 | 6 | 0/60 | 14.9284 | [14.3915, 15.4943] at 0.9750 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.material_gap_threshold, effect.equivalence_margin |
| `dvb-t2-r12-matched-l24` | Exploratory | LogicalCpus24 | 0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23 | 6 | 0/60 | 11.9711 | [11.8848, 12.1564] at 0.9750 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.material_gap_threshold, effect.equivalence_margin |
| `nr-bg1-z384-matched-p6` | Exploratory | PhysicalCores6 | 0,1,2,3,4,5 | 6 | 0/60 | 18.5859 | [18.1627, 19.0987] at 0.9750 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.material_gap_threshold, effect.equivalence_margin |
| `nr-bg1-z384-matched-p12` | Exploratory | PhysicalCores12 | 0,1,2,3,4,5,6,7,8,9,10,11 | 6 | 0/60 | 16.4603 | [14.7032, 16.6357] at 0.9750 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.material_gap_threshold, effect.equivalence_margin |
| `nr-bg1-z384-matched-l24` | Exploratory | LogicalCpus24 | 0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23 | 6 | 0/60 | 15.8976 | [15.0514, 15.9533] at 0.9750 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.material_gap_threshold, effect.equivalence_margin |

## Findings

No findings.
