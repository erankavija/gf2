# Acceptance summary for `31727c57-core-pilot`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 9; resumed true.
Receipt digest `cb2b2998fe7c52194d1a3e494231384d52c895d0aea07a9fcd2a10c7fa3e2e22`.

Family `1362381c-core-route-integration`: 1 comparisons; family-wise alpha 0.05 (frozen total), attempt alpha 0.025 (this attempt's sequential allocation), corrected alpha 0.025 per comparison (confidence 0.975000), 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `xor-inplace-4w` | Exploratory | SingleCore | 0 | 12 | 0/120 | 1.2848 | [1.2803, 1.2904] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `and-popcount-7w` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.9898 | [0.9858, 0.9921] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `xor-inplace-8w` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.9887 | [0.9850, 0.9915] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `matvec-r1024-24w-native` | Exploratory | SingleCore | 0 | 12 | 0/120 | 1.3376 | [1.2791, 1.3521] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `bitmatrix-transpose-8192x2048-native` | Exploratory | SingleCore | 0 | 12 | 0/120 | 1.0101 | [0.9440, 1.0351] at 0.9750 | Inconclusive | **Pilot** | unresolved: effect.measurement_resolution |
| `popcount-1024w-native` | Exploratory | SingleCore | 0 | 12 | 0/120 | 1.0238 | [1.0224, 1.0242] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `and-popcount-1024w-native` | Exploratory | SingleCore | 0 | 12 | 0/120 | 1.0360 | [1.0306, 1.0455] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `xor-inplace-8w-native` | Exploratory | SingleCore | 0 | 12 | 0/120 | 1.0185 | [0.9568, 1.0295] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `clmul-wide-9w-native` | Exploratory | SingleCore | 0 | 12 | 0/120 | 1.0728 | [1.0711, 1.0749] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.
