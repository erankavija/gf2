# Acceptance summary for `confirmation-crossover-53c5a8c0-20260913t131257z`

Label `Confirmation`; verdict **Accepted**; qualifies for production selection: **false**; sessions 2; resumed true.
Receipt digest `abebe7a353ddabc5be259feb5ee6ac44c147b0317488a16e3b32ea3fc573fb5c`.

Family `gf2m-clmul-crossover`: 6 comparisons at family-wise alpha 0.025, per-comparison confidence 0.995833, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `raw-batch-8` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 2.2467 | [2.2424, 2.2526] at 0.9958 | Improved | **Pass** |  |
| `field-dot-8` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.0157 | [1.0068, 1.0389] at 0.9958 | NotWorse | **NotMaterial** |  |
| `field-dot-64` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 1.9007 | [1.8868, 1.9081] at 0.9958 | Improved | **Pass** |  |
| `field-batch-mul-8` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 2.3560 | [2.3530, 2.3619] at 0.9958 | Improved | **Pass** |  |
| `field-dot-16-holdout` | Holdout | SingleCore | 0 | 24 | 0/240 | 0.7452 | [0.7377, 0.7527] at 0.9958 | Regressed | **Fail** |  |
| `field-dot-512-holdout` | Holdout | SingleCore | 0 | 24 | 0/240 | 1.9482 | [1.9338, 1.9652] at 0.9958 | Improved | **Pass** |  |

## Findings

No findings.
