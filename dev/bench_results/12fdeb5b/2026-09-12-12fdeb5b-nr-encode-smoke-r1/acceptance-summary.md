# Acceptance summary for `nr-encode-smoke-12fdeb5b-20260912t171814z`

Label `Smoke`; verdict **Accepted**; qualifies for production selection: **false**; sessions 1; resumed false.
Receipt digest `8dc2f3f8554c4cf3779b82e6709d26174378ff77a9a34191216b4812deea68e2`.

This is a smoke receipt: it proves the receipt pipeline and claims no performance result.

Family `nr-rate-matched-encode-baselines-v1`: 1 comparisons at family-wise alpha 0.025, per-comparison confidence 0.975000, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `nr-enc-bg2-n1024-k400-null-native-vs-native` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.0815 | [0.9714, 1.0856] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `nr-enc-bg2-n256-k121-gap-native-vs-srsran` | Exploratory | SingleCore | 0 | 6 | 0/60 | 7.0649 | [6.9320, 7.0993] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `nr-enc-bg1-n8448-k4224-gap-native-vs-srsran` | Exploratory | SingleCore | 0 | 6 | 0/60 | 162.6313 | [149.6840, 166.1804] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `nr-enc-bg1-n2560-k2048-gap-native-vs-aff3ct` | Exploratory | SingleCore | 0 | 6 | 0/60 | 2.2019 | [2.1895, 2.2302] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `nr-enc-bg1-n2560-k2048-control-portable-vs-native` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.8336 | [0.7961, 0.9328] at 0.9750 | Inconclusive | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.
