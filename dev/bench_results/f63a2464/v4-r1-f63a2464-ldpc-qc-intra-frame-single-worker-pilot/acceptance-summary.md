# Acceptance summary for `f63a2464-v4-r1-qc-intra-frame-single-worker-pilot`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 2; resumed true.
Receipt digest `3b032ba8d7df356bcad93c5414a92483890d59e882652fb6c2062adfdb5e3a1f`.

Family `ldpc-qc-intra-frame-single-worker-v1`: 1 comparisons; family-wise alpha 0.05 (frozen total), attempt alpha 0.025 (this attempt's sequential allocation), corrected alpha 0.025 per comparison (confidence 0.975000), 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `nr-bg1-z384-qc-w1` | Exploratory | SingleCore | 0 | 6 | 0/60 | 8.8132 | [8.7793, 8.8607] at 0.9750 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.worthwhile_speedup, effect.equivalence_margin |
| `nr-bg1-z384-qc-latency` | Exploratory | SingleCore | 0 | 6 | 0/60 | 8.8813 | [8.8438, 8.9431] at 0.9750 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.worthwhile_speedup, effect.equivalence_margin |

## Findings

No findings.
