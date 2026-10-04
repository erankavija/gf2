# Acceptance summary for `ad2a6a58-r1-axpy-confirmation`

Label `Confirmation`; verdict **Accepted**; qualifies for production selection: **true**; sessions 2; resumed true.
Receipt digest `88ebc629f1583b0a20a7802a02ac154590e964af0a00fe1d695d1514f21c72a0`.

Family `gf256-shipped-axpy-lane`: 6 comparisons; family-wise alpha 0.05 (frozen total), attempt alpha 0.025 (this attempt's sequential allocation), corrected alpha 0.004166666666666667 per comparison (confidence 0.995833), 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `axpy-4k-element` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 12.3727 | [12.3147, 12.4628] at 0.9958 | Improved | **Pass** |  |
| `axpy-128k-element` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 12.9439 | [12.4396, 12.9729] at 0.9958 | Improved | **Pass** |  |
| `axpy-2m-stream-element` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 4.9216 | [4.9087, 4.9311] at 0.9958 | Improved | **Pass** |  |
| `axpy-4k-wide` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 155.6847 | [150.7343, 156.0864] at 0.9958 | Improved | **Pass** |  |
| `axpy-128k-wide` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 168.9019 | [161.9845, 169.1625] at 0.9958 | Improved | **Pass** |  |
| `axpy-2m-stream-wide` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 86.5016 | [86.2104, 86.6870] at 0.9958 | Improved | **Pass** |  |

## Findings

No findings.
