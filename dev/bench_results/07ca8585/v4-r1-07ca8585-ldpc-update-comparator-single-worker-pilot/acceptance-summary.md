# Acceptance summary for `07ca8585-v4-r1-update-comparator-single-worker-pilot`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 2; resumed true.
Receipt digest `35cee71e99360b3d25cf81e6959734ef1f11480f860e1901403f12a82fd60fd3`.

Family `ldpc-update-comparator-single-worker-v1`: 1 comparisons at family-wise alpha 0.025, per-comparison confidence 0.975000, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `dvb-t2-r12-update-w1` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.8429 | [1.8346, 1.8600] at 0.9750 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.material_gap_threshold, effect.equivalence_margin |
| `nr-bg1-z384-update-w1` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.6572 | [1.6401, 1.7097] at 0.9750 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.material_gap_threshold, effect.equivalence_margin |

## Findings

No findings.
