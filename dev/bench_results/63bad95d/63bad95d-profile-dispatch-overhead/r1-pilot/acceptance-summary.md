# Acceptance summary for `63bad95d-r1-profile-dispatch-overhead-pilot`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 1; resumed false.
Receipt digest `09e1f1dcbd9e768e5fc8b639d015aca8e5b1b7f61fbdacd3958e9a1236b1c5ea`.

Family `63bad95d-profile-dispatch-overhead`: 1 comparisons; family-wise alpha 0.05 (frozen total), attempt alpha 0.025 (this attempt's sequential allocation), corrected alpha 0.025 per comparison (confidence 0.975000), 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `xor-inplace-4w` | Exploratory | SingleCore | 0 | 12 | 0/120 | 1.3049 | [1.2647, 1.3193] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `and-popcount-7w` | Exploratory | SingleCore | 0 | 12 | 0/120 | 1.0064 | [0.6864, 1.0729] at 0.9750 | Inconclusive | **Pilot** | unresolved: effect.measurement_resolution |
| `xor-inplace-8w` | Exploratory | SingleCore | 0 | 12 | 0/120 | 1.0024 | [0.9796, 1.0290] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.
