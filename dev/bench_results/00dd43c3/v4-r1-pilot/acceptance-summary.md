# Acceptance summary for `00dd43c3-residual-bmi2-pilot-v4-r1`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 4; resumed true.
Receipt digest `28c94a7c1cd4584e7fbc7b69d91cf9e1b044a42f23e66b14e739abfa91638905`.

Family `bitvec-residual-bmi2-route`: 1 comparisons; family-wise alpha 0.05 (frozen total), attempt alpha 0.025 (this attempt's sequential allocation), corrected alpha 0.025 per comparison (confidence 0.975000), 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `left-lane-crossing-r7-w64` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.1220 | [1.0933, 1.1255] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `left-byte-residual-r8-w64` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.6485 | [1.6370, 1.6536] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `left-resident-r63-w64` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.9533 | [1.9462, 1.9559] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `left-streaming-r65-w64` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.7652 | [1.7347, 1.7939] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `right-byte-residual-r8-w64` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.6118 | [1.6081, 1.6166] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `right-resident-r63-w64` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.9402 | [1.8749, 1.9429] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `right-streaming-r65-w64` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.9077 | [1.8737, 1.9143] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.
