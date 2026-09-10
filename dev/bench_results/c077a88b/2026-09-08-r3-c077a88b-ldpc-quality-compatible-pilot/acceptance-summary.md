# Acceptance summary for `c077a88b-2026-09-08-r3-quality-compatible-pilot`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 6; resumed true.
Receipt digest `e0f5921ad2f2ecd221a7773ddefd9bc419cd67859d63ced48554a91eca88893f`.

Family `ldpc-quality-compatible-pilot`: 1 comparisons at family-wise alpha 0.05, per-comparison confidence 0.950000, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `dvb-t2-r12-f32-scalar-pilot` | Exploratory | SingleCore | 0 | 6 | 0/60 | 16.2130 | [16.0495, 16.2680] at 0.9500 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.material_gap_threshold, effect.equivalence_margin |
| `dvb-t2-r12-f32-inter-pilot` | Exploratory | SingleCore | 0 | 6 | 0/60 | 47.3920 | [47.0045, 47.8444] at 0.9500 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.material_gap_threshold, effect.equivalence_margin |
| `dvb-t2-r12-i16-inter-pilot` | Exploratory | SingleCore | 0 | 6 | 8/60 | 64.2451 | [62.9872, 64.6040] at 0.9500 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.material_gap_threshold, effect.equivalence_margin |
| `nr-bg1-z384-f32-scalar-pilot` | Exploratory | SingleCore | 0 | 6 | 17/60 | 27.7507 | [27.0686, 28.3393] at 0.9500 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.material_gap_threshold, effect.equivalence_margin |
| `nr-bg1-z384-f32-inter-pilot` | Exploratory | SingleCore | 0 | 6 | 23/60 | 91.0446 | [88.5576, 92.7515] at 0.9500 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.material_gap_threshold, effect.equivalence_margin |
| `nr-bg1-z384-i16-inter-pilot` | Exploratory | SingleCore | 0 | 6 | 23/60 | 108.0178 | [104.6944, 111.3716] at 0.9500 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.material_gap_threshold, effect.equivalence_margin |

## Findings

- P-19 Note `dvb-t2-r12-i16-inter-pilot`: candidate FER upper bound exceeds the predeclared tolerance
