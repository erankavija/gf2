# Acceptance summary for `transpose-pilot-6fb89a3c-20260908t101756z`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 3; resumed true.
Receipt digest `ac8d8d57a6c9319aaf71a7b7afcff7d5770df55c340d4c04b297ac0374c6b7b6`.

Family `transpose-vs-external-pilot`: 1 comparisons at family-wise alpha 0.05, per-comparison confidence 0.950000, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `transpose-64-canonical-vs-m4ri` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.3385 | [0.3219, 0.3846] at 0.9500 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `transpose-64-canonical-vs-bitshuffle` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.1905 | [0.1895, 0.1923] at 0.9500 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `transpose-consumer-63-vs-m4ri` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.0584 | [1.0510, 1.0760] at 0.9500 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `transpose-consumer-64-vs-m4ri` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.7967 | [0.7797, 0.8066] at 0.9500 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `transpose-consumer-65-vs-m4ri` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.8739 | [1.8468, 1.9121] at 0.9500 | Improved | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.
