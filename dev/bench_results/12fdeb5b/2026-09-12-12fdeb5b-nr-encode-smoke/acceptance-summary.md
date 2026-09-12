# Acceptance summary for `nr-encode-smoke-12fdeb5b-20260912t172522z`

Label `Smoke`; verdict **Accepted**; qualifies for production selection: **false**; sessions 1; resumed false.
Receipt digest `729d67b369b05edd99c1b9abbae97ee7536d542f6200bd223c4c655f15fc5834`.

This is a smoke receipt: it proves the receipt pipeline and claims no performance result.

Family `nr-rate-matched-encode-baselines-v1`: 1 comparisons at family-wise alpha 0.025, per-comparison confidence 0.975000, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `nr-enc-bg2-n1024-k400-null-native-vs-native` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.0338 | [0.9293, 1.0813] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `nr-enc-bg2-n256-k121-gap-native-vs-srsran` | Exploratory | SingleCore | 0 | 6 | 0/60 | 7.1326 | [6.9510, 7.1896] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `nr-enc-bg1-n8448-k4224-gap-native-vs-srsran` | Exploratory | SingleCore | 0 | 6 | 0/60 | 160.5030 | [157.9368, 163.4575] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `nr-enc-bg1-n2560-k2048-gap-native-vs-aff3ct` | Exploratory | SingleCore | 0 | 6 | 0/60 | 2.1951 | [2.1822, 2.2348] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `nr-enc-bg1-n2560-k2048-control-portable-vs-native` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.8237 | [0.8028, 0.8630] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.
