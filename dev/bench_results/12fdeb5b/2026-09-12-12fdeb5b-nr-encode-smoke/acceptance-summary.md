# Acceptance summary for `nr-encode-smoke-12fdeb5b-20260912t173816z`

Label `Smoke`; verdict **Accepted**; qualifies for production selection: **false**; sessions 1; resumed false.
Receipt digest `d4690db504927a09b6f11746d3c3c1183cfc211fcf53f8f9ef14d6ae7d5be487`.

This is a smoke receipt: it proves the receipt pipeline and claims no performance result.

Family `nr-rate-matched-encode-baselines-v1`: 1 comparisons at family-wise alpha 0.025, per-comparison confidence 0.975000, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `nr-enc-bg2-n1024-k400-null-native-vs-native` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.9990 | [0.9939, 1.0051] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `nr-enc-bg2-n256-k121-gap-native-vs-srsran` | Exploratory | SingleCore | 0 | 6 | 0/60 | 6.9493 | [6.8415, 7.0773] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `nr-enc-bg1-n8448-k4224-gap-native-vs-srsran` | Exploratory | SingleCore | 0 | 6 | 0/60 | 149.2070 | [147.2868, 151.9032] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `nr-enc-bg1-n2560-k2048-gap-native-vs-aff3ct` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.9619 | [1.9521, 1.9768] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `nr-enc-bg1-n2560-k2048-control-portable-vs-native` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.8989 | [0.8844, 0.9646] at 0.9750 | Inconclusive | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.
