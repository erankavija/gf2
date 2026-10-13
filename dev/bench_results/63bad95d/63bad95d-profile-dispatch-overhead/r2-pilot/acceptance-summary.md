# Acceptance summary for `63bad95d-r2-profile-dispatch-overhead-pilot`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 1; resumed false.
Receipt digest `4190be8b73d6730554d4da03c141aeb34efe50bfae20bbe85a872489f62c5a7a`.

Family `63bad95d-profile-dispatch-overhead`: 1 comparisons; family-wise alpha 0.05 (frozen total), attempt alpha 0.025 (this attempt's sequential allocation), corrected alpha 0.025 per comparison (confidence 0.975000), 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `xor-inplace-4w` | Exploratory | SingleCore | 0 | 12 | 0/120 | 1.2782 | [1.1078, 1.3294] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `and-popcount-7w` | Exploratory | SingleCore | 0 | 12 | 0/120 | 0.9850 | [0.9809, 0.9928] at 0.9750 | NotWorse | **Pilot** | unresolved: effect.measurement_resolution |
| `xor-inplace-8w` | Exploratory | SingleCore | 0 | 12 | 0/120 | 1.0073 | [0.9353, 1.7760] at 0.9750 | Inconclusive | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.
