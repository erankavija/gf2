# Acceptance summary for `bc091474-u2-2037941f-logical-public-row-xor`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 9; resumed true.
Receipt digest `2184e36e333fddbab54de4ed8eae659aaa14f4a746c010182a4524afa0fcab5d`.

Family `2037941f-logical-public-row-xor`: 1 comparisons; family-wise alpha 0.05 (frozen total), attempt alpha 0.025 (this attempt's sequential allocation), corrected alpha 0.025 per comparison (confidence 0.975000), 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `row-xor-8w-full-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.9729 | [0.9705, 0.9742] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `row-xor-9w-full-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.9787 | [0.9777, 0.9804] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `row-xor-63w-full-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.9803 | [0.9608, 0.9856] at 0.9750 | Inconclusive | **Pilot** | unresolved: effect.measurement_resolution |
| `row-xor-64w-full-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.9961 | [0.9939, 0.9982] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `row-xor-65w-full-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.9866 | [0.9702, 1.0135] at 0.9750 | Inconclusive | **Pilot** | unresolved: effect.measurement_resolution |
| `row-xor-7w-full-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.9963 | [0.9946, 0.9986] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `row-xor-66w-full-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.9846 | [0.9646, 0.9996] at 0.9750 | Inconclusive | **Pilot** | unresolved: effect.measurement_resolution |
| `row-xor-8w-tail63-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.9742 | [0.9707, 0.9780] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `row-xor-9w-tail63-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.9808 | [0.9794, 0.9818] at 0.9750 | Inconclusive | **Pilot** | unresolved: effect.measurement_resolution |
| `row-xor-63w-tail63-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.9769 | [0.9670, 0.9965] at 0.9750 | Inconclusive | **Pilot** | unresolved: effect.measurement_resolution |
| `row-xor-64w-tail63-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.9966 | [0.9941, 0.9976] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `row-xor-65w-tail63-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.9832 | [0.9610, 0.9968] at 0.9750 | Inconclusive | **Pilot** | unresolved: effect.measurement_resolution |
| `row-xor-8w-full-streaming` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.9953 | [0.9885, 1.0016] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `row-xor-9w-full-streaming` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.9963 | [0.9859, 1.0033] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `row-xor-63w-full-streaming` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.9867 | [0.9775, 1.0065] at 0.9750 | Inconclusive | **Pilot** | unresolved: effect.measurement_resolution |
| `row-xor-64w-full-streaming` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.9814 | [0.9654, 0.9903] at 0.9750 | Inconclusive | **Pilot** | unresolved: effect.measurement_resolution |
| `row-xor-65w-full-streaming` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.9858 | [0.9807, 0.9929] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.
