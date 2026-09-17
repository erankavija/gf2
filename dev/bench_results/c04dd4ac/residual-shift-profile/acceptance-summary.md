# Acceptance summary for `residual-shift-profile-85fc5ff4-v4-a2`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 5; resumed true.
Receipt digest `bb70db8ce001425b1cbd8d4b3d144beb04cba5331fb53a9a7dc27e2f60d45616`.

Family `bitvec-residual-shift-profile`: 1 comparisons at family-wise alpha 0.025, per-comparison confidence 0.975000, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `left-small-r1-w64` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.7941 | [0.7918, 0.7958] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `left-lane-crossing-r7-w64` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.3086 | [1.3040, 1.3453] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `left-byte-residual-r8-w64` | Exploratory | SingleCore | 0 | 6 | 0/60 | 4.0083 | [3.9417, 4.0908] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `left-resident-r63-w64` | Exploratory | SingleCore | 0 | 6 | 0/60 | 5.5712 | [5.5543, 5.5821] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `left-streaming-r65-w64` | Exploratory | SingleCore | 0 | 6 | 0/60 | 4.0476 | [4.0173, 4.0710] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `right-small-r1-w64` | Exploratory | SingleCore | 0 | 6 | 0/60 | 0.6505 | [0.6471, 0.6639] at 0.9750 | Regressed | **Pilot** | unresolved: effect.measurement_resolution |
| `right-lane-crossing-r7-w64` | Exploratory | SingleCore | 0 | 6 | 0/60 | 1.2131 | [1.1869, 1.2161] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `right-byte-residual-r8-w64` | Exploratory | SingleCore | 0 | 6 | 0/60 | 4.3438 | [4.1741, 4.3824] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `right-resident-r63-w64` | Exploratory | SingleCore | 0 | 6 | 0/60 | 9.5077 | [9.1922, 9.5216] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `right-streaming-r65-w64` | Exploratory | SingleCore | 0 | 6 | 0/60 | 4.2602 | [4.1442, 4.3331] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.
