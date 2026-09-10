# Acceptance summary for `nr-derate-confirmation-eda07788-20260910t170957z`

Label `Confirmation`; verdict **Accepted**; qualifies for production selection: **false**; sessions 3; resumed true.
Receipt digest `e1041cde377fa5742b8f613530647222bcfede7c2f47710711f6c832669de03e`.

Family `nr-llr-derate-matching-baselines`: 6 comparisons at family-wise alpha 0.025, per-comparison confidence 0.995833, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `nr-bg2-n256-k121-gap-native-vs-aff3ct` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.3491 | [0.3472, 0.3504] at 0.9958 | Regressed | **Fail** |  |
| `nr-bg2-n1024-k400-gap-native-vs-aff3ct` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.3428 | [0.3418, 0.3476] at 0.9958 | Regressed | **Fail** |  |
| `nr-bg2-n1440-k720-gap-native-vs-aff3ct` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.3197 | [0.3186, 0.3208] at 0.9958 | Regressed | **Fail** |  |
| `nr-bg1-n1320-k1056-gap-native-vs-aff3ct` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.3263 | [0.3251, 0.3274] at 0.9958 | Regressed | **Fail** |  |
| `nr-bg1-n2560-k2048-gap-native-vs-aff3ct` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.3292 | [0.3282, 0.3312] at 0.9958 | Regressed | **Fail** |  |
| `nr-bg1-n2560-k2048-control-portable-vs-native` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.9992 | [0.9905, 1.0031] at 0.9958 | NotWorse | **NotMaterial** |  |

## Findings

No findings.
