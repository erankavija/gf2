# Acceptance summary for `popcount-sweep-5cbb6545-20260913t123255z`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 3; resumed true.
Receipt digest `c0eb6e5e9329d125c3fd908247167c9b3a6b07bf6ee945f07b9ecb825c447cd1`.

Family `popcount-route-selection`: 1 comparisons at family-wise alpha 0.025, per-comparison confidence 0.975000, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `sweep-popcount-w1-scalar-popcnt` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.2271 | [1.2212, 1.2308] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `sweep-popcount-w2-scalar-popcnt` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.4831 | [1.4811, 1.4882] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `sweep-popcount-w4-scalar-popcnt` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.2370 | [1.2273, 1.2435] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `sweep-popcount-w7-scalar-popcnt` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.6489 | [1.6415, 1.6553] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `sweep-popcount-w8-nibble-vs-scalar` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.0470 | [1.0385, 1.0585] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `sweep-popcount-w12-nibble-vs-scalar` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.2320 | [1.2205, 1.2450] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `sweep-popcount-w16-nibble-vs-scalar` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.2334 | [1.2114, 1.2391] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `sweep-popcount-w16-csa` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.4933 | [0.4908, 0.4950] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `sweep-popcount-w32-csa` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.7266 | [0.7190, 0.7339] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `sweep-popcount-w64-csa` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.8030 | [0.7843, 0.8264] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `sweep-popcount-w96-csa` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.8257 | [0.8230, 0.8289] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `sweep-popcount-w128-csa` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.9461 | [0.9092, 0.9503] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `sweep-popcount-w256-csa` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.1074 | [1.1036, 1.1113] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `sweep-popcount-w1024-csa` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.3151 | [1.3113, 1.3160] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `sweep-popcount-w16384-csa` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.3298 | [1.3236, 1.3366] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `sweep-popcount-w1024-off3-csa` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.3085 | [1.2896, 1.3110] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `sweep-popcount-w1024-ones-csa` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.3105 | [1.3060, 1.3136] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `sweep-popcount-w1024-zeros-csa` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.3250 | [1.3218, 1.3310] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `sweep-popcount-w1024-vs-libpopcnt` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.0613 | [1.0576, 1.0684] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `sweep-popcount-w1024-vs-mula-avx2-harley-seal` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.9673 | [0.9548, 0.9892] at 0.9750 | Inconclusive | **Pilot** | unresolved: effect.measurement_resolution |
| `sweep-popcount-w16384-vs-libpopcnt` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.0365 | [1.0313, 1.0420] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `sweep-popcount-w16384-vs-mula-avx2-harley-seal` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.9710 | [0.9679, 0.9774] at 0.9750 | Inconclusive | **Pilot** | unresolved: effect.measurement_resolution |
| `sweep-popcount-w1m-streaming-csa` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.0699 | [1.0660, 1.0790] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.
