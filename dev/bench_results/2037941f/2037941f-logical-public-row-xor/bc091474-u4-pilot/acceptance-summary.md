# Acceptance summary for `bc091474-u4-2037941f-logical-public-row-xor`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 9; resumed true.
Receipt digest `1825736bc4fdfbb49ed2cc19938173d0f170c5c12b89b03c05fe7c9deb08fbb0`.

Family `2037941f-logical-public-row-xor`: 1 comparisons; family-wise alpha 0.05 (frozen total), attempt alpha 0.025 (this attempt's sequential allocation), corrected alpha 0.025 per comparison (confidence 0.975000), 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `row-xor-8w-full-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.9703 | [0.9673, 0.9749] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `row-xor-9w-full-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.9540 | [0.9514, 0.9588] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `row-xor-63w-full-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.9976 | [0.9912, 1.0148] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `row-xor-64w-full-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.9970 | [0.9930, 1.0135] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `row-xor-65w-full-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.9936 | [0.9679, 1.0128] at 0.9750 | Inconclusive | **Pilot** | unresolved: effect.measurement_resolution |
| `row-xor-7w-full-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 1.0027 | [0.9999, 1.0052] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `row-xor-66w-full-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.9878 | [0.9717, 1.0045] at 0.9750 | Inconclusive | **Pilot** | unresolved: effect.measurement_resolution |
| `row-xor-8w-tail63-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.9703 | [0.9584, 0.9779] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `row-xor-9w-tail63-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.9580 | [0.9550, 0.9632] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `row-xor-63w-tail63-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.9942 | [0.9772, 1.0041] at 0.9750 | Inconclusive | **Pilot** | unresolved: effect.measurement_resolution |
| `row-xor-64w-tail63-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.9957 | [0.9934, 0.9972] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `row-xor-65w-tail63-warm` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.9943 | [0.9706, 1.0111] at 0.9750 | Inconclusive | **Pilot** | unresolved: effect.measurement_resolution |
| `row-xor-8w-full-streaming` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.9873 | [0.9788, 0.9982] at 0.9750 | Inconclusive | **Pilot** | unresolved: effect.measurement_resolution |
| `row-xor-9w-full-streaming` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.9958 | [0.9841, 1.0026] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `row-xor-63w-full-streaming` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.9778 | [0.9709, 0.9847] at 0.9750 | Inconclusive | **Pilot** | unresolved: effect.measurement_resolution |
| `row-xor-64w-full-streaming` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.9674 | [0.9547, 0.9764] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `row-xor-65w-full-streaming` | Exploratory | SingleCore | 0 | 24 | 0/240 | 0.9578 | [0.9490, 0.9662] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.
