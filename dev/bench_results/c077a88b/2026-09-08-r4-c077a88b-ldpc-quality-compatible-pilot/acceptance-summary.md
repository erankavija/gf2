# Acceptance summary for `c077a88b-2026-09-08-r4-quality-compatible-pilot`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 6; resumed true.
Receipt digest `647cecd904866c9be0598c72325e8d2732fd36b4f8756e2c49ef4549e50987ec`.

Family `ldpc-quality-compatible-alist-v1-pilot`: 1 comparisons at family-wise alpha 0.05, per-comparison confidence 0.950000, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `dvb-t2-r12-f32-scalar-pilot` | Exploratory | SingleCore | 0 | 6 | 0/60 | 16.1212 | [15.8960, 16.2225] at 0.9500 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.material_gap_threshold, effect.equivalence_margin |
| `dvb-t2-r12-f32-inter-pilot` | Exploratory | SingleCore | 0 | 6 | 3/60 | 47.2900 | [46.9368, 47.5180] at 0.9500 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.material_gap_threshold, effect.equivalence_margin |
| `dvb-t2-r12-i16-inter-pilot` | Exploratory | SingleCore | 0 | 6 | 16/60 | 63.8424 | [63.0838, 64.3660] at 0.9500 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.material_gap_threshold, effect.equivalence_margin |
| `nr-bg1-z384-f32-scalar-pilot` | Exploratory | SingleCore | 0 | 6 | 26/60 | 23.2861 | [23.1677, 23.3824] at 0.9500 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.material_gap_threshold, effect.equivalence_margin |
| `nr-bg1-z384-f32-inter-pilot` | Exploratory | SingleCore | 0 | 6 | 25/60 | 75.2414 | [73.5300, 76.2051] at 0.9500 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.material_gap_threshold, effect.equivalence_margin |
| `nr-bg1-z384-i16-inter-pilot` | Exploratory | SingleCore | 0 | 6 | 26/60 | 90.9756 | [89.4851, 91.3391] at 0.9500 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.material_gap_threshold, effect.equivalence_margin |

## Findings

- P-19 Note `dvb-t2-r12-i16-inter-pilot`: candidate FER upper bound exceeds the predeclared tolerance
