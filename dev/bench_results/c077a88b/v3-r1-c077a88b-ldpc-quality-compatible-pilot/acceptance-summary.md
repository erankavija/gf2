# Acceptance summary for `c077a88b-v3-r1-quality-compatible-pilot`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 6; resumed true.
Receipt digest `4492f89587101660306d97f4aba05a609028c431d3a77024868352bd40c756c2`.

Family `ldpc-quality-compatible-alist-v1`: 1 comparisons at family-wise alpha 0.025, per-comparison confidence 0.975000, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `dvb-t2-r12-f32-scalar-pilot` | Exploratory | SingleCore | 0 | 6 | 0/60 | 16.5175 | [16.4311, 16.7321] at 0.9750 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.material_gap_threshold, effect.equivalence_margin |
| `dvb-t2-r12-f32-inter-pilot` | Exploratory | SingleCore | 0 | 6 | 0/60 | 48.9434 | [47.8504, 49.4879] at 0.9750 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.material_gap_threshold, effect.equivalence_margin |
| `dvb-t2-r12-i16-inter-pilot` | Exploratory | SingleCore | 0 | 6 | 0/60 | 65.5460 | [64.3558, 66.2881] at 0.9750 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.material_gap_threshold, effect.equivalence_margin |
| `nr-bg1-z384-f32-scalar-pilot` | Exploratory | SingleCore | 0 | 6 | 1/60 | 23.5193 | [18.4694, 24.3037] at 0.9750 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.material_gap_threshold, effect.equivalence_margin |
| `nr-bg1-z384-f32-inter-pilot` | Exploratory | SingleCore | 0 | 6 | 0/60 | 75.5580 | [72.2198, 76.2817] at 0.9750 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.material_gap_threshold, effect.equivalence_margin |
| `nr-bg1-z384-i16-inter-pilot` | Exploratory | SingleCore | 0 | 6 | 0/60 | 89.5572 | [87.6321, 90.3144] at 0.9750 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.material_gap_threshold, effect.equivalence_margin |

## Findings

- P-19 Note `dvb-t2-r12-f32-scalar-pilot`: candidate FER upper bound exceeds the predeclared tolerance
- P-19 Note `dvb-t2-r12-f32-inter-pilot`: candidate FER upper bound exceeds the predeclared tolerance
- P-19 Note `dvb-t2-r12-i16-inter-pilot`: candidate FER upper bound exceeds the predeclared tolerance
- P-19 Note `nr-bg1-z384-f32-scalar-pilot`: candidate FER upper bound exceeds the predeclared tolerance
- P-19 Note `nr-bg1-z384-f32-inter-pilot`: candidate FER upper bound exceeds the predeclared tolerance
- P-19 Note `nr-bg1-z384-i16-inter-pilot`: candidate FER upper bound exceeds the predeclared tolerance
