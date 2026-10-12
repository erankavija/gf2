# Acceptance summary for `63bad95d-r1-profile-dispatch-overhead-v2-pilot`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 1; resumed false.
Receipt digest `aef3faa4e8b65df65ece814310c22d8ccafd4819e9d4d01b8ee7c198f6eca820`.

Family `63bad95d-profile-dispatch-overhead-v2`: 1 comparisons; family-wise alpha 0.05 (frozen total), attempt alpha 0.025 (this attempt's sequential allocation), corrected alpha 0.025 per comparison (confidence 0.975000), 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `xor-inplace-4w` | Exploratory | SingleCore | 0 | 12 | 0/120 | 1.3046 | [1.2988, 1.3078] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `and-popcount-7w` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.9836 | [0.9818, 0.9859] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `xor-inplace-8w` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.9976 | [0.9891, 1.0018] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.
