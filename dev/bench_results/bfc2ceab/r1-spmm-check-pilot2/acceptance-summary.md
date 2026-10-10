# Acceptance summary for `bfc2ceab-r1-spmm-check-pilot2`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 1; resumed false.
Receipt digest `deb649efbf0afe56cf0c15d4f045e499d8c0740741b11d4c726d1df48f7be76b`.

Family `sparse-product-check-cost`: 1 comparisons; family-wise alpha 0.05 (frozen total), attempt alpha 0.025 (this attempt's sequential allocation), corrected alpha 0.025 per comparison (confidence 0.975000), 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `spmm-small-n16` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.0200 | [0.7995, 1.2936] at 0.9750 | Inconclusive | **Pilot** | unresolved: effect.measurement_resolution |
| `spmm-small-n1024` | Exploratory | SingleCore | 0 | 24 | 1/240 | 0.9961 | [0.9003, 1.0065] at 0.9750 | Inconclusive | **Pilot** | unresolved: effect.measurement_resolution |
| `spmm-medium-n16` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.9285 | [0.7339, 1.4047] at 0.9750 | Inconclusive | **Pilot** | unresolved: effect.measurement_resolution |
| `spmm-medium-n1024` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.9795 | [0.9668, 0.9880] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.
