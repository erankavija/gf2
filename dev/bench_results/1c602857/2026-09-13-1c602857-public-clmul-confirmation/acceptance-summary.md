# Acceptance summary for `confirmation-1c602857-20260913t063634z`

Label `Confirmation`; verdict **Accepted**; qualifies for production selection: **true**; sessions 3; resumed true.
Receipt digest `2cd31085f5a5f5e71197a4ab182bbb42d5e22c9884d9fb232026882213d3fdfa`.

Family `public-clmul-wide-dispatch`: 6 comparisons at family-wise alpha 0.025, per-comparison confidence 0.995833, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `clmul-wide-4w-owned` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 76.2369 | [74.5799, 77.5504] at 0.9958 | Improved | **Pass** |  |
| `clmul-wide-9w-owned` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 113.7870 | [112.8393, 114.1494] at 0.9958 | Improved | **Pass** |  |
| `clmul-wide-4w-accumulate` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 65.5858 | [65.3403, 65.7658] at 0.9958 | Improved | **Pass** |  |
| `clmul-wide-1w-dispatch-overhead` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.0016 | [0.9992, 1.0042] at 0.9958 | NotWorse | **Pass** |  |
| `clmul-wide-2w-dispatch-overhead` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.9982 | [0.9941, 1.0005] at 0.9958 | NotWorse | **Pass** |  |
| `clmul-wide-16w-dispatch-overhead` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.0001 | [0.9938, 1.0034] at 0.9958 | NotWorse | **Pass** |  |

## Findings

No findings.
