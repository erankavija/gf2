# Acceptance summary for `nr-encode-pilot-12fdeb5b-20260913t011403z`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 4; resumed true.
Receipt digest `9e2c362845d5c84bff33fdbeeee8ffab4685e0189ac7896828e496c2435974a5`.

Family `nr-rate-matched-encode-baselines-v1`: 1 comparisons at family-wise alpha 0.025, per-comparison confidence 0.975000, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `nr-enc-bg2-n1024-k400-null-native-vs-native` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.0006 | [0.9992, 1.0021] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `nr-enc-bg2-n256-k121-gap-native-vs-srsran` | Exploratory | SingleCore | 0 | 24 | 0/240 | 6.9139 | [6.8181, 6.9453] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `nr-enc-bg2-n1440-k720-gap-native-vs-srsran` | Exploratory | SingleCore | 0 | 24 | 0/240 | 34.6559 | [34.4440, 34.8123] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `nr-enc-bg1-n2560-k2048-gap-native-vs-srsran` | Exploratory | SingleCore | 0 | 24 | 0/240 | 80.5605 | [80.2720, 80.8918] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `nr-enc-bg1-n8448-k4224-gap-native-vs-srsran` | Exploratory | SingleCore | 0 | 24 | 0/240 | 150.4391 | [149.9527, 151.3757] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `nr-enc-bg2-n256-k121-gap-native-vs-aff3ct` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.4724 | [0.4696, 0.4738] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `nr-enc-bg1-n2560-k2048-gap-native-vs-aff3ct` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.9730 | [1.9677, 1.9803] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `nr-enc-bg1-n2560-k2048-control-portable-vs-native` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.8931 | [0.8840, 0.9250] at 0.9750 | Inconclusive | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.
