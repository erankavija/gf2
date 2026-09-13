# Acceptance summary for `popcount-pilot-r2-5cbb6545-20260913t130101z`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 1; resumed false.
Receipt digest `3639174e76959ff7187cc3a83ef46ac6f7d175d1c890e2304f0182b639a55f05`.

Family `popcount-route-selection`: 1 comparisons at family-wise alpha 0.025, per-comparison confidence 0.975000, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `popcount-w4-dispatch` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.0238 | [1.0198, 1.0362] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w8-dispatch` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.1555 | [1.1512, 1.1583] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w128-dispatch` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.0453 | [1.0421, 1.0463] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w1024-dispatch` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.3314 | [1.3289, 1.3333] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w16384-dispatch` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.3202 | [1.3145, 1.3226] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-w16384-vs-libpopcnt` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.0341 | [1.0311, 1.0365] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.
