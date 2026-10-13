# Acceptance summary for `63bad95d-r1-native-ladder-v2-pilot`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 2; resumed true.
Receipt digest `348a0c3da326e5411069d4cd9920fdcec5754a91b38e78bd60c80beb404bfad2`.

Family `63bad95d-native-ladder-v2`: 1 comparisons; family-wise alpha 0.05 (frozen total), attempt alpha 0.025 (this attempt's sequential allocation), corrected alpha 0.025 per comparison (confidence 0.975000), 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `matvec-r1024-24w-native` | Exploratory | SingleCore | 0 | 12 | 0/120 | 1.0215 | [1.0065, 1.0358] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `bitmatrix-transpose-8192x2048-native` | Exploratory | SingleCore | 0 | 12 | 0/120 | 1.0035 | [0.9480, 1.0232] at 0.9750 | Inconclusive | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-1024w-native` | Exploratory | SingleCore | 0 | 12 | 0/120 | 1.0278 | [1.0253, 1.0287] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `and-popcount-1024w-native` | Exploratory | SingleCore | 0 | 12 | 0/120 | 1.0252 | [1.0239, 1.0280] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `xor-inplace-8w-native` | Exploratory | SingleCore | 0 | 12 | 0/120 | 1.0017 | [0.9530, 1.0248] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `clmul-wide-9w-native` | Exploratory | SingleCore | 0 | 12 | 0/120 | 1.0629 | [1.0620, 1.0645] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.
