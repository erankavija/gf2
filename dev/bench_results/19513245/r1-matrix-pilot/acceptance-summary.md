# Acceptance summary for `19513245-r1-matrix-pilot`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 2; resumed true.
Receipt digest `dc39e3f2d8bc2098a88408342480e655367ec43898ae59220a1eb0da9715a751`.

Family `bytefield-consumer-matrix`: 1 comparisons at family-wise alpha 0.025, per-comparison confidence 0.975000, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `matmul-n64-element` | Exploratory | SingleCore | 0 | 12 | 0/120 | 11.0976 | [11.0479, 11.1352] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `matmul-n256-element` | Exploratory | SingleCore | 0 | 12 | 0/120 | 10.0512 | [9.9843, 10.3006] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `matmul-n512-element` | Exploratory | SingleCore | 0 | 12 | 0/120 | 10.1329 | [10.0917, 10.2109] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `matmul-n256-whole-element` | Exploratory | SingleCore | 0 | 12 | 0/120 | 9.5685 | [9.5217, 9.7862] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `matmul-n64-wide` | Exploratory | SingleCore | 0 | 12 | 0/120 | 3.2177 | [3.2044, 3.2348] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `matmul-n256-wide` | Exploratory | SingleCore | 0 | 12 | 0/120 | 3.0692 | [3.0632, 3.0750] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `matmul-n512-wide` | Exploratory | SingleCore | 0 | 12 | 0/120 | 3.1235 | [3.1114, 3.1286] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `matmul-n256-whole-wide` | Exploratory | SingleCore | 0 | 12 | 0/120 | 3.0457 | [3.0358, 3.0515] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.
