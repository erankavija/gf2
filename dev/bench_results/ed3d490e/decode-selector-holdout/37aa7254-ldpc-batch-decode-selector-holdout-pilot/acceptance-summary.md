# Acceptance summary for `37aa7254-ldpc-batch-decode-selector-holdout-pilot`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 2; resumed true.
Receipt digest `6ba9992d6aea55d77506e682cd79c727422de4830c7fbef5419bbe997d5d6b01`.

Family `ldpc-batch-decode-selector-holdout-v1`: 1 comparisons; family-wise alpha 0.05 (frozen total), attempt alpha 0.025 (this attempt's sequential allocation), corrected alpha 0.025 per comparison (confidence 0.975000), 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `dvb-t2-r12-tuned-w1-b8` | Exploratory | SingleCore | 0 | 12 | 0/120 | 5.2818 | [5.2160, 5.2927] at 0.9750 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.worthwhile_speedup, effect.equivalence_margin |
| `dvb-t2-r12-native-w1-b8` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.9921 | [0.9666, 1.0015] at 0.9750 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.equivalence_margin, effect.worthwhile_speedup |

## Findings

No findings.
