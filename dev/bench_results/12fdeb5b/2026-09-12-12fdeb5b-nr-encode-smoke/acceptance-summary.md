# Acceptance summary for `nr-encode-smoke-12fdeb5b-20260912t172031z`

Label `Smoke`; verdict **Accepted**; qualifies for production selection: **false**; sessions 1; resumed false.
Receipt digest `efde67d53962f484cb9b86028c93d3e61e3bf0915feebb8ce6faeb8ffb262221`.

This is a smoke receipt: it proves the receipt pipeline and claims no performance result.

Family `nr-rate-matched-encode-baselines-v1`: 1 comparisons at family-wise alpha 0.025, per-comparison confidence 0.975000, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `nr-enc-bg2-n1024-k400-null-native-vs-native` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.0090 | [0.9378, 1.0772] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `nr-enc-bg2-n256-k121-gap-native-vs-srsran` | Exploratory | SingleCore | 0 | 6 | 0/60 | 7.0261 | [6.9523, 7.1108] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `nr-enc-bg1-n8448-k4224-gap-native-vs-srsran` | Exploratory | SingleCore | 0 | 6 | 0/60 | 162.1477 | [161.2234, 162.5886] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `nr-enc-bg1-n2560-k2048-gap-native-vs-aff3ct` | Exploratory | SingleCore | 0 | 6 | 0/60 | 2.2087 | [2.1856, 2.2725] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `nr-enc-bg1-n2560-k2048-control-portable-vs-native` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.8398 | [0.8192, 0.8651] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.
