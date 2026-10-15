# Acceptance summary for `caabc4e1-ldpc-batch-aff3ct-fastest-pilot`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 6; resumed true.
Receipt digest `d8dca32cc0df2db732a17adb9bb20dbb492025e1229a0f3f5243e2ff8d4e88be`.

Family `ldpc-batch-aff3ct-fastest-v1`: 1 comparisons; family-wise alpha 0.05 (frozen total), attempt alpha 0.025 (this attempt's sequential allocation), corrected alpha 0.025 per comparison (confidence 0.975000), 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `dvb-t2-r12-layered-f32-inter` | Exploratory | SingleCore | 0 | 12 | 0/120 | 2.6698 | [2.6169, 2.8071] at 0.9750 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.material_gap_threshold, effect.equivalence_margin |
| `dvb-t2-r12-layered-i16-inter` | Exploratory | SingleCore | 0 | 12 | 0/120 | 5.9750 | [5.9468, 6.0165] at 0.9750 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.material_gap_threshold, effect.equivalence_margin |
| `dvb-t2-r12-spa-intra` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.8202 | [0.8178, 0.8238] at 0.9750 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.material_gap_threshold, effect.equivalence_margin |
| `nr-bg1-z384-layered-f32-inter` | Exploratory | SingleCore | 0 | 12 | 0/120 | 5.5558 | [5.3754, 5.8026] at 0.9750 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.material_gap_threshold, effect.equivalence_margin |
| `nr-bg1-z384-layered-i16-inter` | Exploratory | SingleCore | 0 | 12 | 0/120 | 13.7959 | [13.7217, 13.8854] at 0.9750 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.material_gap_threshold, effect.equivalence_margin |
| `nr-bg1-z384-spa-intra` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.6643 | [0.6607, 0.6661] at 0.9750 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.material_gap_threshold, effect.equivalence_margin |

## Findings

- P-19 Note `dvb-t2-r12-layered-f32-inter`: candidate FER upper bound exceeds the predeclared tolerance
- P-19 Note `dvb-t2-r12-layered-i16-inter`: candidate FER upper bound exceeds the predeclared tolerance
- P-19 Note `dvb-t2-r12-spa-intra`: candidate FER upper bound exceeds the predeclared tolerance
- P-19 Note `nr-bg1-z384-layered-f32-inter`: candidate FER upper bound exceeds the predeclared tolerance
- P-19 Note `nr-bg1-z384-layered-i16-inter`: candidate FER upper bound exceeds the predeclared tolerance
- P-19 Note `nr-bg1-z384-spa-intra`: candidate FER upper bound exceeds the predeclared tolerance
