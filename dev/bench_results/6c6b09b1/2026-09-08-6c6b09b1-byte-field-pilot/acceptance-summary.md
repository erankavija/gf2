# Acceptance summary for `pilot-6c6b09b1-20260908t092546z`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 2; resumed true.
Receipt digest `b15308aad470be693d0f0afff09fd418c074fde58230fdb25ae8ef3fab928425`.

Family `byte-field-arms-pilot`: 1 comparisons at family-wise alpha 0.05, per-comparison confidence 0.950000, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `axpy-isal-l2-1core` | Exploratory | SingleCore | 0 | 6 | 19/60 | 469.4537 | [468.6690, 471.4861] at 0.9500 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.material_gap_threshold |
| `matmul-m4rie-n128-1core` | Exploratory | SingleCore | 0 | 6 | 0/60 | 16.7644 | [16.6433, 17.0417] at 0.9500 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.material_gap_threshold |
| `pairwise-gfcomplete-l2-1core` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.6026 | [0.5966, 0.6147] at 0.9500 | n/a | **Pilot** | unresolved: effect.measurement_resolution, effect.material_gap_threshold |

## Findings

No findings.
