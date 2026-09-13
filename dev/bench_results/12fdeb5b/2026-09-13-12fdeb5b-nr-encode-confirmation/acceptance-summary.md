# Acceptance summary for `nr-encode-confirmation-12fdeb5b-20260913t065640z`

Label `Confirmation`; verdict **Accepted**; qualifies for production selection: **false**; sessions 3; resumed true.
Receipt digest `814343d65ac13891c3fef5da97b19759f6e3576b2fffa22b08e15bd07050abbf`.

Family `nr-rate-matched-encode-baselines-v1`: 6 comparisons at family-wise alpha 0.025, per-comparison confidence 0.995833, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `nr-enc-bg2-n256-k121-gap-native-vs-srsran` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 6.8886 | [6.8263, 6.9789] at 0.9958 | Improved | **Pass** |  |
| `nr-enc-bg2-n1440-k720-gap-native-vs-srsran` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 34.6492 | [34.3817, 34.8561] at 0.9958 | Improved | **Pass** |  |
| `nr-enc-bg1-n2560-k2048-gap-native-vs-srsran` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 80.3690 | [79.9356, 80.7570] at 0.9958 | Improved | **Pass** |  |
| `nr-enc-bg1-n8448-k4224-gap-native-vs-srsran` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 151.3073 | [149.1436, 152.0714] at 0.9958 | Improved | **Pass** |  |
| `nr-enc-bg2-n256-k121-gap-native-vs-aff3ct` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.4751 | [0.4726, 0.4770] at 0.9958 | Regressed | **Fail** |  |
| `nr-enc-bg1-n2560-k2048-gap-native-vs-aff3ct` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.9772 | [1.9708, 1.9891] at 0.9958 | Improved | **Pass** |  |

## Findings

No findings.
