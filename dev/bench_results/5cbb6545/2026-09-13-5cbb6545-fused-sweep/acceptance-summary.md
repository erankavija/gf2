# Acceptance summary for `fused-sweep-5cbb6545-20260913t124244z`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 2; resumed true.
Receipt digest `61a01f19f3693553746d41353f8389a50d37f66c3b8f67501d48e912c37de838`.

Family `fused-count-consumers`: 1 comparisons at family-wise alpha 0.025, per-comparison confidence 0.975000, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `sweep-and-w8-csa` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.5262 | [0.5105, 0.5377] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `sweep-and-w16-csa` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.6209 | [0.6149, 0.6260] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `sweep-and-w32-csa` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.7092 | [0.7037, 0.7168] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `sweep-and-w64-csa` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.8940 | [0.8764, 0.9011] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `sweep-and-w96-csa` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.9373 | [0.9188, 0.9419] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `sweep-and-w128-csa` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.0730 | [1.0621, 1.0963] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `sweep-and-w512-csa` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.3294 | [1.3242, 1.3365] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `sweep-and-w4096-csa` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.3091 | [1.2990, 1.3175] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `sweep-and-w4096-vs-two-pass` | Exploratory | SingleCore | 0 | 6 | 0/60 | 3.2508 | [3.2420, 3.3112] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.
