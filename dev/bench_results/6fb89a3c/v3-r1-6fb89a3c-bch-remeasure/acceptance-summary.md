# Acceptance summary for `6fb89a3c-v3-r1-bch-remeasure`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 3; resumed true.
Receipt digest `73830941185b42b1dcb106fa5a52181c35457178607e6122d4097e2e66bac0e2`.

Family `bch-genmatrix-consumer`: 3 comparisons at family-wise alpha 0.025, per-comparison confidence 0.991667, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `bch-genmatrix-b1-vs-m4ri` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.2063 | [0.2052, 0.2083] at 0.9917 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `bch-genmatrix-b2-vs-m4ri` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.1276 | [0.1274, 0.1279] at 0.9917 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `bch-genmatrix-b3-vs-m4ri` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.0564 | [0.0560, 0.0568] at 0.9917 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `bch-genmatrix-reference-b1-vs-m4ri` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.6402 | [1.6135, 1.6695] at 0.9917 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `bch-genmatrix-reference-b2-vs-m4ri` | Exploratory | SingleCore | 0 | 24 | 0/240 | 7.5645 | [7.3000, 7.6613] at 0.9917 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `bch-genmatrix-reference-b3-vs-m4ri` | Exploratory | SingleCore | 0 | 24 | 0/240 | 7.0731 | [6.9905, 7.1041] at 0.9917 | Improved | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.
