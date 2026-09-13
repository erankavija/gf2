# Acceptance summary for `3be770d5-v3-r1-steady-fastest-compatible-pilot`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 6; resumed true.
Receipt digest `9c1e056af19695d28b0aa6c1cf5d448e52c841d535256365ad413449d1758037`.

Family `ldpc-steady-fastest-compatible-v1`: 1 comparisons at family-wise alpha 0.025, per-comparison confidence 0.975000, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `dvb-t2-r12-layered-f32-w1` | Exploratory | SingleCore | 0 | 6 | 0/60 | 18.4079 | [18.3230, 18.5471] at 0.9750 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.material_gap_threshold, effect.equivalence_margin |
| `dvb-t2-r12-layered-f32-inter-w1` | Exploratory | SingleCore | 0 | 6 | 0/60 | 96.3222 | [93.6771, 97.1613] at 0.9750 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.material_gap_threshold, effect.equivalence_margin |
| `dvb-t2-r12-layered-i16-inter-w1` | Exploratory | SingleCore | 0 | 6 | 0/60 | 204.0258 | [199.7739, 204.2629] at 0.9750 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.material_gap_threshold, effect.equivalence_margin |
| `nr-bg1-z384-layered-f32-w1` | Exploratory | SingleCore | 0 | 6 | 0/60 | 30.4248 | [30.2955, 30.6855] at 0.9750 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.material_gap_threshold, effect.equivalence_margin |
| `nr-bg1-z384-layered-f32-inter-w1` | Exploratory | SingleCore | 0 | 6 | 0/60 | 279.1185 | [272.2229, 286.1487] at 0.9750 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.material_gap_threshold, effect.equivalence_margin |
| `nr-bg1-z384-layered-i16-inter-w1` | Exploratory | SingleCore | 0 | 6 | 0/60 | 724.8136 | [709.3481, 727.2454] at 0.9750 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.material_gap_threshold, effect.equivalence_margin |

## Findings

- P-19 Note `dvb-t2-r12-layered-f32-w1`: candidate FER upper bound exceeds the predeclared tolerance
- P-19 Note `dvb-t2-r12-layered-f32-inter-w1`: candidate FER upper bound exceeds the predeclared tolerance
- P-19 Note `dvb-t2-r12-layered-i16-inter-w1`: candidate FER upper bound exceeds the predeclared tolerance
- P-19 Note `nr-bg1-z384-layered-f32-w1`: candidate FER upper bound exceeds the predeclared tolerance
- P-19 Note `nr-bg1-z384-layered-f32-inter-w1`: candidate FER upper bound exceeds the predeclared tolerance
- P-19 Note `nr-bg1-z384-layered-i16-inter-w1`: candidate FER upper bound exceeds the predeclared tolerance
