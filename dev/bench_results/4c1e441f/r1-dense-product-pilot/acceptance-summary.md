# Acceptance summary for `4c1e441f-r1-dense-product-pilot`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 2; resumed true.
Receipt digest `fdba1b601318ed2660225ceca8cf0edb9a96f4ca72759dda46a2ef28c6c7762f`.

Family `gf256-shipped-dense-product`: 1 comparisons; family-wise alpha 0.05 (frozen total), attempt alpha 0.025 (this attempt's sequential allocation), corrected alpha 0.025 per comparison (confidence 0.975000), 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `matmul-n64-element` | Exploratory | SingleCore | 0 | 12 | 0/120 | 7.7713 | [7.7601, 7.8253] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `matmul-n256-element` | Exploratory | SingleCore | 0 | 12 | 0/120 | 8.9409 | [8.9171, 8.9745] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `matmul-n512-element` | Exploratory | SingleCore | 0 | 12 | 0/120 | 10.4562 | [10.4300, 10.5168] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `matmul-n256-whole-element` | Exploratory | SingleCore | 0 | 12 | 0/120 | 8.3118 | [8.2978, 8.3414] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `matmul-n64-wide` | Exploratory | SingleCore | 0 | 12 | 0/120 | 3.5580 | [3.5477, 3.5675] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `matmul-n256-wide` | Exploratory | SingleCore | 0 | 12 | 0/120 | 3.4464 | [3.4371, 3.4521] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `matmul-n512-wide` | Exploratory | SingleCore | 0 | 12 | 0/120 | 3.6817 | [3.6760, 3.6895] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `matmul-n256-whole-wide` | Exploratory | SingleCore | 0 | 12 | 0/120 | 3.4272 | [3.4169, 3.4353] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.
