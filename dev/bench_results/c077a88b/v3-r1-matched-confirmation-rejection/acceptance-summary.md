# Acceptance summary for `c077a88b-v3-r1-matched-algorithm-confirmation`

Label `Confirmation`; verdict **Rejected**; qualifies for production selection: **false**; sessions 2; resumed true.
Receipt digest `37fda99e6f956ec92bf3a4525577ef825258308dfc2fac7e4ca558cf8a8683b9`.

Family `ldpc-matched-algorithm-alist-v1`: 2 comparisons at family-wise alpha 0.025, per-comparison confidence 0.987500, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `dvb-t2-r12-matched-single-core` | Confirmatory | SingleCore | 0 | 24 | 3/240 | 0.1421 | [0.1404, 0.1449] at 0.9875 | Regressed | **Fail** |  |
| `nr-bg1-z384-matched-single-core` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.9617 | [0.9515, 0.9682] at 0.9875 | NotWorse | **NotMaterial** |  |

## Findings

- P-03 Error: pilot cell dvb-t2-r12-matched-pilot lacks a declared bootstrap alpha
