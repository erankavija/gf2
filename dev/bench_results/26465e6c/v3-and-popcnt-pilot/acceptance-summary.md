# Acceptance summary for `26465e6c-v3-and-popcnt-pilot`

Label `Pilot`; verdict **Rejected**; qualifies for production selection: **false**; sessions 3; resumed true.
Receipt digest `7d9c85d6ed9a6eaea537e959df4ff5e28b06280cf1445a5e4668e959cca82137`.

Family `and-popcnt-baselines`: 1 comparisons at family-wise alpha 0.025, per-comparison confidence 0.975000, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `and-popcnt-w4-vs-scalar-control` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.8373 | [0.8342, 0.8409] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `and-popcnt-w4096-vs-scalar-control` | Exploratory | SingleCore | 0 | 12 | 0/120 | n/a | n/a | n/a | **Invalid** |  |
| `and-popcnt-w512k-streaming-vs-scalar-control` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.6486 | [0.6440, 0.6663] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `and-popcnt-w4-vs-two-pass` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.2037 | [0.2031, 0.2044] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `and-popcnt-w4096-vs-two-pass` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.4221 | [0.4204, 0.4236] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `and-popcnt-w512k-streaming-vs-two-pass` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.5982 | [0.5590, 0.6094] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

- P-11 Error `and-popcnt-w4096-vs-scalar-control`: cell started 2 times; completed cells must not repeat
- P-11 Error `and-popcnt-w4096-vs-scalar-control`: cell-start input differs from the saved plan
- P-11 Error `and-popcnt-w4096-vs-scalar-control`: cell has no single cell-start record
