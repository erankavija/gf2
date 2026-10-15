# Acceptance summary for `9bcc7e73-ldpc-batch-size-sweep-pilot`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 6; resumed true.
Receipt digest `7a7bb874a897e6139601a9b3769a3284480ae7221761499253c6fe6c09fb98f1`.

Family `ldpc-batch-size-sweep-v1`: 1 comparisons; family-wise alpha 0.05 (frozen total), attempt alpha 0.025 (this attempt's sequential allocation), corrected alpha 0.025 per comparison (confidence 0.975000), 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `dvb-t2-r12-lane-w1-b4` | Exploratory | SingleCore | 0 | 12 | 0/120 | 2.7103 | [2.7037, 2.7158] at 0.9750 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.equivalence_margin, effect.worthwhile_speedup, complexity_budget.max_added_source_lines |
| `dvb-t2-r12-lane-w1-b8` | Exploratory | SingleCore | 0 | 12 | 0/120 | 5.0250 | [4.9808, 5.0428] at 0.9750 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.worthwhile_speedup, effect.equivalence_margin, complexity_budget.max_added_source_lines |
| `dvb-t2-r12-lane-w1-b32` | Exploratory | SingleCore | 0 | 12 | 0/120 | 5.0037 | [4.9910, 5.0150] at 0.9750 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.worthwhile_speedup, effect.equivalence_margin, complexity_budget.max_added_source_lines |
| `nr-bg1-z384-lane-w1-b4` | Exploratory | SingleCore | 0 | 12 | 0/120 | 2.7498 | [2.6983, 2.7606] at 0.9750 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.equivalence_margin, effect.worthwhile_speedup, complexity_budget.max_added_source_lines |
| `nr-bg1-z384-lane-w1-b8` | Exploratory | SingleCore | 0 | 12 | 0/120 | 5.3940 | [5.3812, 5.4049] at 0.9750 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.worthwhile_speedup, effect.equivalence_margin, complexity_budget.max_added_source_lines |
| `nr-bg1-z384-lane-w1-b32` | Exploratory | SingleCore | 0 | 12 | 0/120 | 5.0206 | [5.0087, 5.0350] at 0.9750 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.worthwhile_speedup, effect.equivalence_margin, complexity_budget.max_added_source_lines |

## Findings

No findings.
