# Acceptance summary for `popcount-pilot-26465e6c-20260907t194956z`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 1; resumed false.
Receipt digest `5bc5ac429c5ce8866a491f5fec5fdba63ea8ced63f05b3b10e5e3419da42e4e4`.

Family `popcount-baselines-pilot`: 1 comparisons at family-wise alpha 0.05, per-comparison confidence 0.950000, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `popcount-pilot-small-vs-nibble-lut` | Exploratory | SingleCore | 6 | 6 | 0/60 | 1.5959 | [1.5409, 1.6051] at 0.9500 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.material_gap_threshold, effect.equivalence_margin |
| `popcount-pilot-streaming-vs-mula` | Exploratory | SingleCore | 6 | 6 | 0/60 | 1.0812 | [1.0706, 1.0856] at 0.9500 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.material_gap_threshold, effect.equivalence_margin |

## Findings

No findings.
