# Acceptance summary for `v4-r1-2037941f-logical-isolated-xor`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 9; resumed true.
Receipt digest `1994686cbee1c972e4e573dd7bd57228218672dd8c4b973ed6e89256bc11015c`.

Family `2037941f-logical-isolated-xor`: 1 comparisons; family-wise alpha 0.05 (frozen total), attempt alpha 0.025 (this attempt's sequential allocation), corrected alpha 0.025 per comparison (confidence 0.975000), 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `xor-8w-a64-warm` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.9997 | [0.9976, 1.0007] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `xor-9w-a64-warm` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.9997 | [0.9955, 1.0037] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `xor-63w-a64-warm` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.0185 | [0.9678, 1.0647] at 0.9750 | Inconclusive | **Pilot** | unresolved: effect.measurement_resolution |
| `xor-64w-a64-warm` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.9890 | [0.9498, 1.0106] at 0.9750 | Inconclusive | **Pilot** | unresolved: effect.measurement_resolution |
| `xor-65w-a64-warm` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.9951 | [0.9485, 1.0007] at 0.9750 | Inconclusive | **Pilot** | unresolved: effect.measurement_resolution |
| `xor-7w-a64-warm` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.0001 | [0.9974, 1.0039] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `xor-66w-a64-warm` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.0091 | [0.9759, 1.0245] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `xor-8w-o8-warm` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.0041 | [0.9981, 1.0144] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `xor-9w-o8-warm` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.9994 | [0.9925, 1.0089] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `xor-63w-o8-warm` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.9962 | [0.9719, 1.0110] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `xor-64w-o8-warm` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.0128 | [0.8693, 1.0186] at 0.9750 | Inconclusive | **Pilot** | unresolved: effect.measurement_resolution |
| `xor-65w-o8-warm` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.0112 | [0.7618, 1.0386] at 0.9750 | Inconclusive | **Pilot** | unresolved: effect.measurement_resolution |
| `xor-8w-a64-streaming` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.0014 | [0.9934, 1.0068] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `xor-9w-a64-streaming` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.0060 | [1.0019, 1.0138] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `xor-63w-a64-streaming` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.9990 | [0.9910, 1.0226] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `xor-64w-a64-streaming` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.0031 | [0.9955, 1.0167] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `xor-65w-a64-streaming` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.0102 | [0.9875, 1.0282] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.
