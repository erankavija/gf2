# Acceptance summary for `bfc2ceab-r1-spmm-check-pilot`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 1; resumed false.
Receipt digest `0afedb9bc20a30107230af72a9e5666057fe41490cb4335b932eca59270c803e`.

Family `sparse-product-check-cost`: 1 comparisons; family-wise alpha 0.05 (frozen total), attempt alpha 0.025 (this attempt's sequential allocation), corrected alpha 0.025 per comparison (confidence 0.975000), 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `spmm-small-n16` | Exploratory | SingleCore | 0 | 12 | 0/120 | 1.0868 | [1.0750, 1.0892] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `spmm-small-n1024` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.9797 | [0.8907, 1.0734] at 0.9750 | Inconclusive | **Pilot** | unresolved: effect.measurement_resolution |
| `spmm-medium-n16` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.9911 | [0.9880, 0.9936] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `spmm-medium-n1024` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.9713 | [0.9485, 0.9822] at 0.9750 | Inconclusive | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.
