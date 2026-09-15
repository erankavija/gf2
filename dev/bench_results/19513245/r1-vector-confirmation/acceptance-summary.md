# Acceptance summary for `19513245-r1-vector-confirmation`

Label `Confirmation`; verdict **Accepted**; qualifies for production selection: **true**; sessions 4; resumed true.
Receipt digest `53f8c413dd1bf90190c6ad7bd51680d4b231063fb7e5aca7f977ead1e99a3b74`.

Family `bytefield-consumer-vector`: 6 comparisons at family-wise alpha 0.025, per-comparison confidence 0.995833, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `axpy-4k-element` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 2.4261 | [2.4163, 2.4413] at 0.9958 | Improved | **Pass** |  |
| `axpy-128k-element` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 2.4348 | [2.4274, 2.4417] at 0.9958 | Improved | **Pass** |  |
| `axpy-8m-element` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 2.1885 | [2.1831, 2.1927] at 0.9958 | Improved | **Pass** |  |
| `region-128k-element` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 58.6947 | [58.5858, 58.8593] at 0.9958 | Improved | **Pass** |  |
| `axpy-128k-wide` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 111.6025 | [107.7840, 111.9183] at 0.9958 | Improved | **Pass** |  |
| `region-128k-wide` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 162.4598 | [162.2814, 162.7342] at 0.9958 | Improved | **Pass** |  |

## Findings

No findings.
