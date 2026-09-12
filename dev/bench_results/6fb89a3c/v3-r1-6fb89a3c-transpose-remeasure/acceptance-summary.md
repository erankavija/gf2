# Acceptance summary for `6fb89a3c-v3-r1-transpose-remeasure`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 4; resumed true.
Receipt digest `03f249f17c2bd024dfab4c48931e921b0e019c3fb590cbd382c1956d0f54bcb6`.

Family `transpose-vs-external`: 8 comparisons at family-wise alpha 0.025, per-comparison confidence 0.996875, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `transpose-64-canonical-vs-m4ri` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.3215 | [0.3190, 0.3239] at 0.9969 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `transpose-64-canonical-vs-bitshuffle` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.1898 | [0.1895, 0.1911] at 0.9969 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `transpose-consumer-63-vs-m4ri` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.0606 | [1.0535, 1.0652] at 0.9969 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `transpose-consumer-64-vs-m4ri` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.7997 | [0.7957, 0.8282] at 0.9969 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `transpose-consumer-65-vs-m4ri` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.8765 | [1.8664, 1.9105] at 0.9969 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `transpose-consumer-64-vs-bitshuffle-adapter` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.4938 | [0.4911, 0.5010] at 0.9969 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `transpose-consumer-63-vs-bitshuffle-adapter` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.2313 | [0.2298, 0.2323] at 0.9969 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `transpose-consumer-65-vs-bitshuffle-adapter` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.4989 | [0.4975, 0.5105] at 0.9969 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.
