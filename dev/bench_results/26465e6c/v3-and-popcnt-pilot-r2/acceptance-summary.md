# Acceptance summary for `26465e6c-v3-and-popcnt-pilot-r2`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 2; resumed true.
Receipt digest `8148ca916fcf48ea004b2ec54ecbfe69302b3da3a149cd64ed1aa61bc982043b`.

Family `and-popcnt-baselines`: 1 comparisons at family-wise alpha 0.025, per-comparison confidence 0.975000, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `and-popcnt-w4-vs-scalar-control` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.8381 | [0.8356, 0.8408] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `and-popcnt-w4096-vs-scalar-control` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.3277 | [0.3273, 0.3286] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `and-popcnt-w512k-streaming-vs-scalar-control` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.6608 | [0.6294, 0.6714] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `and-popcnt-w4-vs-two-pass` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.2028 | [0.2023, 0.2034] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `and-popcnt-w4096-vs-two-pass` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.4230 | [0.4213, 0.4244] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `and-popcnt-w512k-streaming-vs-two-pass` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.5753 | [0.5521, 0.6006] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.
