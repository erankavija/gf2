# Acceptance summary for `37aa7254-ldpc-batch-decode-selector-holdout-pilot`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 2; resumed true.
Receipt digest `96229931881ed6a2a12f6d2140883c41b4aa831eb37951e67bab41c7d1216256`.

Family `ldpc-batch-decode-selector-holdout-v1`: 1 comparisons; family-wise alpha 0.05 (frozen total), attempt alpha 0.025 (this attempt's sequential allocation), corrected alpha 0.025 per comparison (confidence 0.975000), 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `dvb-t2-r12-tuned-w1-b8` | Exploratory | SingleCore | 0 | 12 | 0/120 | 5.2329 | [5.2229, 5.2563] at 0.9750 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.worthwhile_speedup, effect.equivalence_margin |
| `dvb-t2-r12-native-w1-b8` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.9943 | [0.9851, 0.9998] at 0.9750 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.equivalence_margin, effect.worthwhile_speedup |

## Findings

No findings.
