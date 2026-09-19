# Acceptance summary for `f63a2464-v4-r1-qc-comparator-single-worker-pilot`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 2; resumed true.
Receipt digest `cef1c9ba35d1945295211061f4a15bb957b809d8550006b09d5f5bd964227d8e`.

Family `ldpc-qc-comparator-single-worker-v1`: 1 comparisons; family-wise alpha 0.05 (frozen total), attempt alpha 0.025 (this attempt's sequential allocation), corrected alpha 0.025 per comparison (confidence 0.975000), 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `nr-bg1-z384-qc-comparator-w1` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.1947 | [0.1858, 0.1958] at 0.9750 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.material_gap_threshold, effect.equivalence_margin |
| `nr-bg1-z384-qc-comparator-latency` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.1932 | [0.1926, 0.1947] at 0.9750 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.material_gap_threshold, effect.equivalence_margin |

## Findings

No findings.
