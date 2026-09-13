# Acceptance summary for `external-confirmation-1d4fd63d-20260913t130401z`

Label `Confirmation`; verdict **Accepted**; qualifies for production selection: **false**; sessions 1; resumed false.
Receipt digest `c2fcd1c8a8c9ec88b9541426f39e569594c71e6f47c0557c354bd70a578a7972`.

Family `transpose-lane-vs-external`: 2 comparisons at family-wise alpha 0.025, per-comparison confidence 0.987500, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `lane-avx2-ymm6-block-64-vs-m4ri` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.2448 | [0.2435, 0.2453] at 0.9875 | Regressed | **Fail** |  |
| `lane-avx2-ymm6-block-64-vs-bitshuffle` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.1447 | [0.1441, 0.1450] at 0.9875 | Regressed | **Fail** |  |

## Findings

No findings.
