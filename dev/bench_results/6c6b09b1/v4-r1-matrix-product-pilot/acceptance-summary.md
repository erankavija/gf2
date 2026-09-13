# Acceptance summary for `6c6b09b1-v4-r1-matrix-product-pilot`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 3; resumed true.
Receipt digest `a15214d268c9faf7350a026d49cb18474056a096a7df61f916d8fc143c8c88f8`.

Family `byte-field-matrix-product`: 1 comparisons at family-wise alpha 0.025, per-comparison confidence 0.975000, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `matmul-n64-element-vs-m4rie` | Exploratory | SingleCore | 0 | 12 | 0/120 | 7.0256 | [7.0097, 7.0687] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `matmul-n256-element-vs-m4rie` | Exploratory | SingleCore | 0 | 12 | 0/120 | 34.8544 | [33.8892, 36.4677] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `matmul-n512-element-vs-m4rie` | Exploratory | SingleCore | 0 | 12 | 0/120 | 85.6712 | [85.1768, 89.9422] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `matmul-n256-whole-element-vs-m4rie` | Exploratory | SingleCore | 0 | 12 | 0/120 | 28.0721 | [27.9426, 28.3796] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `matmul-n64-wide-vs-m4rie` | Exploratory | SingleCore | 0 | 12 | 0/120 | 2.0812 | [2.0692, 2.0845] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `matmul-n256-wide-vs-m4rie` | Exploratory | SingleCore | 0 | 12 | 0/120 | 11.7692 | [11.6314, 11.8312] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `matmul-n512-wide-vs-m4rie` | Exploratory | SingleCore | 0 | 12 | 0/120 | 28.5644 | [28.3572, 28.6425] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `matmul-n256-whole-wide-vs-m4rie` | Exploratory | SingleCore | 0 | 12 | 0/120 | 9.0297 | [8.9674, 9.0696] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `encode-k10r4-64k-element-vs-isal` | Exploratory | SingleCore | 0 | 12 | 0/120 | 358.1590 | [355.3820, 359.4261] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `encode-k10r4-64k-whole-element-vs-isal` | Exploratory | SingleCore | 0 | 12 | 0/120 | 398.0850 | [395.2856, 402.2387] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `encode-k10r4-64k-wide-vs-isal` | Exploratory | SingleCore | 0 | 12 | 0/120 | 115.5366 | [115.0973, 115.9630] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `encode-k10r4-64k-whole-wide-vs-isal` | Exploratory | SingleCore | 0 | 12 | 0/120 | 115.2543 | [105.0969, 116.3558] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `encode-k10r4-64k-isal-vs-m4rie` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.0051 | [0.0051, 0.0051] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `matmul-n256-element-vs-wide` | Exploratory | SingleCore | 0 | 12 | 0/120 | 3.0096 | [3.0025, 3.0541] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.
