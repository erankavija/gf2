# Acceptance summary for `v4-r1-2037941f-logical-public-row-xor`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 9; resumed true.
Receipt digest `488bfff72b7432c0c8dc9a9e0116ac41ed216d14ca5978b921256aff38422865`.

Family `2037941f-logical-public-row-xor`: 1 comparisons; family-wise alpha 0.05 (frozen total), attempt alpha 0.025 (this attempt's sequential allocation), corrected alpha 0.025 per comparison (confidence 0.975000), 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `row-xor-8w-full-warm` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.9996 | [0.9954, 1.2452] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `row-xor-9w-full-warm` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.0015 | [0.9989, 1.0031] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `row-xor-63w-full-warm` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.9975 | [0.9740, 1.0016] at 0.9750 | Inconclusive | **Pilot** | unresolved: effect.measurement_resolution |
| `row-xor-64w-full-warm` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.9865 | [0.9557, 1.0034] at 0.9750 | Inconclusive | **Pilot** | unresolved: effect.measurement_resolution |
| `row-xor-65w-full-warm` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.0214 | [0.9841, 1.1473] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `row-xor-7w-full-warm` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.9993 | [0.8559, 1.0026] at 0.9750 | Inconclusive | **Pilot** | unresolved: effect.measurement_resolution |
| `row-xor-66w-full-warm` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.9950 | [0.9729, 1.0083] at 0.9750 | Inconclusive | **Pilot** | unresolved: effect.measurement_resolution |
| `row-xor-8w-tail63-warm` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.9993 | [0.9807, 1.0008] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `row-xor-9w-tail63-warm` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.9994 | [0.5683, 1.0017] at 0.9750 | Inconclusive | **Pilot** | unresolved: effect.measurement_resolution |
| `row-xor-63w-tail63-warm` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.9979 | [0.9760, 1.0026] at 0.9750 | Inconclusive | **Pilot** | unresolved: effect.measurement_resolution |
| `row-xor-64w-tail63-warm` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.0095 | [0.9754, 1.0501] at 0.9750 | Inconclusive | **Pilot** | unresolved: effect.measurement_resolution |
| `row-xor-65w-tail63-warm` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.0170 | [1.0000, 1.0928] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `row-xor-8w-full-streaming` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.0040 | [0.9758, 1.0199] at 0.9750 | Inconclusive | **Pilot** | unresolved: effect.measurement_resolution |
| `row-xor-9w-full-streaming` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.9966 | [0.9669, 1.0122] at 0.9750 | Inconclusive | **Pilot** | unresolved: effect.measurement_resolution |
| `row-xor-63w-full-streaming` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.9982 | [0.9666, 1.0073] at 0.9750 | Inconclusive | **Pilot** | unresolved: effect.measurement_resolution |
| `row-xor-64w-full-streaming` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.9958 | [0.9761, 1.0286] at 0.9750 | Inconclusive | **Pilot** | unresolved: effect.measurement_resolution |
| `row-xor-65w-full-streaming` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.9939 | [0.9764, 1.0253] at 0.9750 | Inconclusive | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.
