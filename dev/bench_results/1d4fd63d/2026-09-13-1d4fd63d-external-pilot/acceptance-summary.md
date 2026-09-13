# Acceptance summary for `external-pilot-1d4fd63d-20260913t125750z`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 2; resumed true.
Receipt digest `40237fc93051d234137acc19c8e8eb887ac2af3d4002f463010005313165f3c0`.

Family `transpose-lane-vs-external`: 1 comparisons at family-wise alpha 0.025, per-comparison confidence 0.975000, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `lane-production-block-64-vs-m4ri` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.2947 | [0.2939, 0.2975] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `lane-production-block-64-vs-bitshuffle` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.1744 | [0.1732, 0.1923] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `lane-avx2-ymm6-block-64-vs-m4ri` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.2453 | [0.2442, 0.2463] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `lane-avx2-ymm6-block-64-vs-bitshuffle` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.1448 | [0.1445, 0.1454] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `lane-avx2-pshufb-block-64-vs-m4ri` | Exploratory | SingleCore | 0 | 24 | 0/240 | 20.3065 | [20.2341, 20.4119] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `lane-avx2-pshufb-block-64-vs-bitshuffle` | Exploratory | SingleCore | 0 | 24 | 0/240 | 11.5750 | [11.5306, 11.5961] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `lane-avx2-movemask-block-64-vs-m4ri` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.4111 | [0.4039, 0.4177] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `lane-avx2-movemask-block-64-vs-bitshuffle` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.2392 | [0.2384, 0.2462] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.
