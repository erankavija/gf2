# Acceptance summary for `6fb89a3c-v3-r2-transpose-pilot`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 4; resumed true.
Receipt digest `43bc7e386fde8d1c65915d1e9347e60094ff4924d8f202e621186c3a0ec42858`.

Family `transpose-vs-external`: 1 comparisons at family-wise alpha 0.025, per-comparison confidence 0.975000, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `transpose-64-canonical-vs-m4ri` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.3215 | [0.3191, 0.3241] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `transpose-64-canonical-vs-bitshuffle` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.1900 | [0.1891, 0.1904] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `transpose-consumer-63-vs-m4ri` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.0584 | [1.0554, 1.0653] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `transpose-consumer-64-vs-m4ri` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.7967 | [0.7932, 0.8006] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `transpose-consumer-65-vs-m4ri` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.8681 | [1.8606, 1.8827] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `transpose-consumer-64-vs-bitshuffle-adapter` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.4869 | [0.4850, 0.4898] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `transpose-consumer-63-vs-bitshuffle-adapter` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.2356 | [0.2351, 0.2374] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `transpose-consumer-65-vs-bitshuffle-adapter` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.5023 | [0.5007, 0.5086] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.
