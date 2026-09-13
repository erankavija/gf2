# Acceptance summary for `fused-pilot-r2-5cbb6545-20260913t130716z`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 1; resumed false.
Receipt digest `9a435ae1f563a82faa69e1867fdd3780584a8156570980bd345fa623cae70a0f`.

Family `fused-count-consumers`: 1 comparisons at family-wise alpha 0.025, per-comparison confidence 0.975000, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `and-w128-fused` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.9761 | [0.9719, 0.9782] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `and-w512-fused` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.3072 | [1.3049, 1.3090] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `and-w4096-fused` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.2831 | [1.2818, 1.2861] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `and-w4096-vs-two-pass` | Exploratory | SingleCore | 0 | 24 | 0/240 | 3.0051 | [3.0011, 3.0253] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `matvec-1024x16384` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.1736 | [1.1443, 1.2076] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `matvec-1024x4096` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.9408 | [0.9205, 0.9644] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.
