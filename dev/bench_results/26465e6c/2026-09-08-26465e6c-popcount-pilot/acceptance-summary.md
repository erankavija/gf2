# Acceptance summary for `popcount-pilot-26465e6c-20260908`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 1; resumed false.
Receipt digest `f28291bd365cd9ee4104c45e8692a13e1436c78bfafcfc4509d149190c3c718d`.

Family `popcount-baselines-pilot`: 1 comparisons at family-wise alpha 0.05, per-comparison confidence 0.950000, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `popcount-pilot-small-vs-nibble-lut` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.5755 | [1.5264, 1.5874] at 0.9500 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.material_gap_threshold, effect.equivalence_margin |
| `popcount-pilot-alignment-vs-compiler-count-ones` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.3480 | [0.3470, 0.3531] at 0.9500 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.material_gap_threshold, effect.equivalence_margin |
| `popcount-pilot-streaming-vs-mula` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.1123 | [1.0517, 1.1408] at 0.9500 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.material_gap_threshold, effect.equivalence_margin |

## Findings

No findings.
