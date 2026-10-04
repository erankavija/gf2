# Acceptance summary for `4c1e441f-r1-dense-product-confirmation`

Label `Confirmation`; verdict **Accepted**; qualifies for production selection: **true**; sessions 2; resumed true.
Receipt digest `a1f53ad3e194915bd3c31410515dff71df2897539a587c908a0bee98b51396e6`.

Family `gf256-shipped-dense-product`: 6 comparisons; family-wise alpha 0.05 (frozen total), attempt alpha 0.025 (this attempt's sequential allocation), corrected alpha 0.004166666666666667 per comparison (confidence 0.995833), 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `matmul-n256-element` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 8.9453 | [8.9328, 8.9891] at 0.9958 | Improved | **Pass** |  |
| `matmul-n512-element` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 10.4679 | [10.4498, 10.5139] at 0.9958 | Improved | **Pass** |  |
| `matmul-n256-whole-element` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 8.3223 | [8.3115, 8.3388] at 0.9958 | Improved | **Pass** |  |
| `matmul-n256-wide` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 3.4428 | [3.4366, 3.4491] at 0.9958 | Improved | **Pass** |  |
| `matmul-n512-wide` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 3.6778 | [3.6715, 3.6826] at 0.9958 | Improved | **Pass** |  |
| `matmul-n256-whole-wide` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 3.4207 | [3.4161, 3.4259] at 0.9958 | Improved | **Pass** |  |

## Findings

No findings.
