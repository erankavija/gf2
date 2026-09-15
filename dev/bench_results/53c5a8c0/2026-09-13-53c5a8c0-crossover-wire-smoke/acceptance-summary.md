# Acceptance summary for `smoke-crossover-53c5a8c0-20260913t121110z`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 3; resumed true.
Receipt digest `52bdc042e170657486f1e9948b1edc77dc9f05661defe6ec461cdfb60a3f3320`.

Family `gf2m-clmul-crossover`: 1 comparisons at family-wise alpha 0.025, per-comparison confidence 0.975000, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `raw-batch-8` | Exploratory | SingleCore | 0 | 6 | 0/60 | 2.2423 | [2.2356, 2.2549] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `raw-batch-64` | Exploratory | SingleCore | 0 | 6 | 0/60 | 3.7148 | [3.7090, 3.7209] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `raw-batch-1024` | Exploratory | SingleCore | 0 | 6 | 0/60 | 3.9725 | [3.9208, 3.9898] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `raw-batch-65536-streaming` | Exploratory | SingleCore | 0 | 6 | 0/60 | 3.7471 | [3.6741, 3.9596] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `field-dot-8` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.0327 | [1.0233, 1.0436] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `field-dot-64` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.9144 | [1.8679, 1.9403] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `field-dot-1024` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.8290 | [1.8266, 1.8413] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `field-dot-65536-streaming` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.4027 | [1.4012, 1.4064] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `field-batch-mul-8` | Exploratory | SingleCore | 0 | 6 | 0/60 | 2.4091 | [2.4003, 2.4173] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `field-batch-mul-32` | Exploratory | SingleCore | 0 | 6 | 0/60 | 4.3637 | [4.2969, 4.4029] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `field-batch-mul-1024` | Exploratory | SingleCore | 0 | 6 | 0/60 | 5.5265 | [5.3332, 5.5899] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.
