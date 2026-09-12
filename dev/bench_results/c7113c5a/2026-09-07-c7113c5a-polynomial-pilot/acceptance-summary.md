# Acceptance summary for `pilot-c7113c5a-20260907t191644z`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 2; resumed true.
Receipt digest `22d2663f89f32bc0cccbd6baebad6c1b054ac04f408311a97b79790af7ecaed1`.

Family `polynomial-multiplication-baselines-pilot`: 1 comparisons at family-wise alpha 0.05, per-comparison confidence 0.950000, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `poly-mul-4w-native-pilot` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.5987 | [0.5966, 0.6004] at 0.9500 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `poly-mul-256w-native-pilot` | Exploratory | SingleCore | 0 | 6 | 27/60 | 302.0813 | [301.0115, 317.4622] at 0.9500 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `poly-mul-256w-conservative-pilot` | Exploratory | SingleCore | 0 | 6 | 26/60 | 291.2516 | [290.6860, 291.7950] at 0.9500 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `poly-mul-256w-tuned-pilot` | Exploratory | SingleCore | 0 | 6 | 21/60 | 305.6574 | [304.3941, 307.1526] at 0.9500 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `poly-mul-4w-public-api-pilot` | Exploratory | SingleCore | 0 | 6 | 16/60 | 76.4619 | [74.9924, 79.3813] at 0.9500 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `clmul-batch-1024-native-pilot` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.1576 | [0.1524, 0.1766] at 0.9500 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.
