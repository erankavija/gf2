# Acceptance summary for `1358d6ab-bch-integration-confirmation`

Label `Confirmation`; verdict **Accepted**; qualifies for production selection: **false**; sessions 6; resumed true.
Receipt digest `07cd723a158c5a7beb0f265875e823cd88cc31d571cce92e872d7d39232906c9`.

Family `1362381c-bch-integration`: 6 comparisons; family-wise alpha 0.05 (frozen total), attempt alpha 0.025 (this attempt's sequential allocation), corrected alpha 0.004166666666666667 per comparison (confidence 0.995833), 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `m14-b1-aff3ct-lfsr-scalar` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.2684 | [0.2674, 0.2690] at 0.9958 | Regressed | **Fail** |  |
| `m14-b256-aff3ct-lfsr-simd-inter` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.4368 | [0.3839, 0.4398] at 0.9958 | Regressed | **Fail** |  |
| `t2s-b1-bchlib-table-remainder` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 105.8335 | [105.4671, 106.5905] at 0.9958 | Improved | **Pass** |  |
| `t2s-b256-bchlib-table-remainder` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 107.3078 | [106.7793, 107.9895] at 0.9958 | Improved | **Pass** |  |
| `t2n-b1-bchlib-table-remainder` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 104.5234 | [104.1588, 105.6429] at 0.9958 | Improved | **Pass** |  |
| `t2n-b256-bchlib-table-remainder` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 106.1817 | [105.9914, 107.3477] at 0.9958 | Improved | **Pass** |  |

## Findings

No findings.
