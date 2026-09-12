# Acceptance summary for `nr-encode-smoke-12fdeb5b-20260912t174447z`

Label `Smoke`; verdict **Accepted**; qualifies for production selection: **false**; sessions 1; resumed false.
Receipt digest `823a4f08df2fa797a70053721b15d209acea1265625169db0d8c30307716046e`.

This is a smoke receipt: it proves the receipt pipeline and claims no performance result.

Family `nr-rate-matched-encode-baselines-v1`: 1 comparisons at family-wise alpha 0.025, per-comparison confidence 0.975000, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `nr-enc-bg2-n1024-k400-null-native-vs-native` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.9998 | [0.9986, 1.0067] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `nr-enc-bg2-n256-k121-gap-native-vs-srsran` | Exploratory | SingleCore | 0 | 6 | 0/60 | 6.9328 | [6.7788, 7.0305] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `nr-enc-bg1-n8448-k4224-gap-native-vs-srsran` | Exploratory | SingleCore | 0 | 6 | 0/60 | 150.0377 | [148.4871, 152.0705] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `nr-enc-bg1-n2560-k2048-gap-native-vs-aff3ct` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.9703 | [1.9671, 1.9817] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `nr-enc-bg1-n2560-k2048-control-portable-vs-native` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.9100 | [0.7904, 0.9992] at 0.9750 | Inconclusive | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.
