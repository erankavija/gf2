# Acceptance summary for `pilot-f547c394-20260906t230725z`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **true**; sessions 2; resumed true.
Receipt digest `6ff1d8697eb7846b98702302cd258a1ccc72c0ab9575fc8f0bf74401310757bf`.

Family `protocol-smoke-pilot`: 1 comparisons at family-wise alpha 0.05, per-comparison confidence 0.950000, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `xor-fold-double-pass-1core` | Exploratory | SingleCore | 6 | 6 | 0/60 | 2.0542 | [1.9892, 2.1182] at 0.9500 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `xor-fold-identical-1core` | Exploratory | SingleCore | 6 | 6 | 0/60 | 1.0043 | [0.9958, 1.0086] at 0.9500 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.
