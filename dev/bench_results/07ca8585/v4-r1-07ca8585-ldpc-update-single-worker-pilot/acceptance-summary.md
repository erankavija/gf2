# Acceptance summary for `07ca8585-v4-r1-update-single-worker-pilot`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 2; resumed true.
Receipt digest `b437c5cbc5d98b8c146d2add99f7f7b56ad2678ea5afc0b02bf0b55abc5ee160`.

Family `ldpc-update-single-worker-v1`: 1 comparisons at family-wise alpha 0.025, per-comparison confidence 0.975000, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `dvb-t2-r12-update-w1` | Exploratory | SingleCore | 0 | 6 | 0/60 | 6.7684 | [6.7485, 6.7922] at 0.9750 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.worthwhile_speedup, effect.equivalence_margin |
| `nr-bg1-z384-update-w1` | Exploratory | SingleCore | 0 | 6 | 0/60 | 10.0134 | [9.9806, 10.0916] at 0.9750 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.worthwhile_speedup, effect.equivalence_margin |

## Findings

No findings.
