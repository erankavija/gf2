# Acceptance summary for `63bad95d-r1-native-ladder-pilot`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 2; resumed true.
Receipt digest `64d235e6b11ab384158cee74004baca4be90e379dccf3ef0dba8d9612406934d`.

Family `63bad95d-native-ladder`: 1 comparisons; family-wise alpha 0.05 (frozen total), attempt alpha 0.025 (this attempt's sequential allocation), corrected alpha 0.025 per comparison (confidence 0.975000), 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `matvec-r1024-24w-native` | Exploratory | SingleCore | 0 | 12 | 0/120 | 1.0282 | [1.0008, 1.0323] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `bitmatrix-transpose-8192x2048-native` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.9925 | [0.9877, 1.0089] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-1024w-native` | Exploratory | SingleCore | 0 | 12 | 0/120 | 1.0268 | [1.0236, 1.0410] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `and-popcount-1024w-native` | Exploratory | SingleCore | 0 | 12 | 0/120 | 1.0236 | [1.0211, 1.0261] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `xor-inplace-8w-native` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.9980 | [0.9782, 1.0283] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `clmul-wide-9w-native` | Exploratory | SingleCore | 0 | 12 | 0/120 | 1.0788 | [1.0202, 1.1057] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.
