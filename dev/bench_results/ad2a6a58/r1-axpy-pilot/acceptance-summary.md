# Acceptance summary for `ad2a6a58-r1-axpy-pilot`

Label `Pilot`; verdict **Accepted**; qualifies for production selection: **false**; sessions 2; resumed true.
Receipt digest `60b5311ca7655d61be5bba58e6b28c99efb002b5791dfac8cf6322e1cdd1ce9c`.

Family `gf256-shipped-axpy-lane`: 1 comparisons; family-wise alpha 0.05 (frozen total), attempt alpha 0.025 (this attempt's sequential allocation), corrected alpha 0.025 per comparison (confidence 0.975000), 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `axpy-1k-cold-element` | Exploratory | SingleCore | 0 | 12 | 12/120 | 10.7715 | [8.6851, 13.8462] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `axpy-4k-element` | Exploratory | SingleCore | 0 | 12 | 0/120 | 12.3595 | [12.3377, 12.3936] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `axpy-128k-element` | Exploratory | SingleCore | 0 | 12 | 0/120 | 12.9974 | [12.9884, 13.0145] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `axpy-8m-element` | Exploratory | SingleCore | 0 | 12 | 0/120 | 5.8992 | [5.8873, 5.9515] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `axpy-2m-stream-element` | Exploratory | SingleCore | 0 | 12 | 0/120 | 4.9673 | [4.9535, 4.9740] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `axpy-1k-cold-wide` | Exploratory | SingleCore | 0 | 12 | 12/120 | 145.6872 | [112.0000, 167.0133] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `axpy-4k-wide` | Exploratory | SingleCore | 0 | 12 | 0/120 | 155.5005 | [151.1276, 156.2428] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `axpy-128k-wide` | Exploratory | SingleCore | 0 | 12 | 0/120 | 169.1186 | [168.2848, 169.6750] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `axpy-8m-wide` | Exploratory | SingleCore | 0 | 12 | 0/120 | 87.8626 | [86.5984, 88.9413] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |
| `axpy-2m-stream-wide` | Exploratory | SingleCore | 0 | 12 | 0/120 | 86.3791 | [86.2981, 86.4777] at 0.9750 | Improved | **Pilot** | unresolved: effect.measurement_resolution |

## Findings

No findings.
