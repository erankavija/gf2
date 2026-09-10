# Acceptance summary for `nr-derate-pilot-eda07788-20260910t170155z`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 4; resumed true.
Receipt digest `8843eeab9b4d95d2740658973c7e0c9b0dafadbbabe645b0743ec94c4797c713`.

Family `nr-llr-derate-matching-baselines`: 1 comparisons at family-wise alpha 0.025, per-comparison confidence 0.975000, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `nr-bg2-n1024-k400-null-native-vs-native` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.9975 | [0.9948, 1.0019] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `nr-bg2-n256-k121-gap-native-vs-aff3ct` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.3499 | [0.3486, 0.3509] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `nr-bg2-n1024-k400-gap-native-vs-aff3ct` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.3439 | [0.3428, 0.3450] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `nr-bg2-n1440-k720-gap-native-vs-aff3ct` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.3188 | [0.3179, 0.3201] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `nr-bg1-n1200-k900-gap-native-vs-aff3ct` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.3330 | [0.3327, 0.3345] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `nr-bg1-n1320-k1056-gap-native-vs-aff3ct` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.3266 | [0.3256, 0.3271] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `nr-bg1-n2560-k2048-gap-native-vs-aff3ct` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.3300 | [0.3295, 0.3325] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `nr-bg1-n2560-k2048-control-portable-vs-native` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.9997 | [0.9966, 1.0035] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.
