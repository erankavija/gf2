# Acceptance summary for `6fb89a3c-v3-r1-bch-pilot`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 3; resumed true.
Receipt digest `e680c1d1c7a1482afa19a62f83154293e54ebdda8f5763239185321504edc1a7`.

Family `bch-genmatrix-consumer`: 1 comparisons at family-wise alpha 0.025, per-comparison confidence 0.975000, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `bch-genmatrix-b1-vs-m4ri` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.2045 | [0.2023, 0.2065] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `bch-genmatrix-b2-vs-m4ri` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.1271 | [0.1266, 0.1295] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `bch-genmatrix-b3-vs-m4ri` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.0556 | [0.0548, 0.0565] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `bch-genmatrix-reference-b1-vs-m4ri` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.6803 | [1.6618, 1.7020] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `bch-genmatrix-reference-b2-vs-m4ri` | Exploratory | SingleCore | 0 | 24 | 0/240 | 7.7111 | [7.6131, 7.7406] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `bch-genmatrix-reference-b3-vs-m4ri` | Exploratory | SingleCore | 0 | 24 | 0/240 | 7.0311 | [6.9470, 7.0580] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.
