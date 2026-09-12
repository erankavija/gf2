# Acceptance summary for `1d0da41f-v3-confirmation`

Label `Confirmation`; verdict **Accepted**; qualifies for production selection: **false**; sessions 3; resumed true.
Receipt digest `91f11a886f4635c24a99c8913bf9995860eacc38645f3ec8ea7ff267cada20b8`.

Family `ymm-clmul-dispatch`: 10 comparisons at family-wise alpha 0.008333333333333333, per-comparison confidence 0.999167, 10000 bootstrap resamples.

## Cells

| Cell | Role | Arm | CPUs | Pairs | Flagged | Speedup | Interval | Decision | Outcome | Note |
|---|---|---|---|---:|---:|---:|---|---|---|---|
| `raw-batch-small-8-1core` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.7871 | [0.7767, 0.8018] at 0.9992 | Regressed | **NotConfirmatory** |  |
| `raw-batch-odd-tail-65-1core` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.6040 | [0.6012, 0.6091] at 0.9992 | Regressed | **NotConfirmatory** |  |
| `raw-batch-l1-512-1core` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.5821 | [0.5791, 0.5875] at 0.9992 | Regressed | **NotConfirmatory** |  |
| `fieldvec-dot-1024-1core` | Confirmatory | SingleCore | 0 | 24 | 0/240 | 0.7756 | [0.7682, 0.7796] at 0.9992 | Regressed | **NotConfirmatory** |  |
| `raw-batch-l1-512-streaming-6core` | Confirmatory | PhysicalCores6 | 0,1,2,3,4,5 | 24 | 0/240 | 0.5849 | [0.5801, 0.5889] at 0.9992 | Regressed | **NotConfirmatory** |  |

## Findings

- P-20 Note `raw-batch-small-8-1core`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `raw-batch-odd-tail-65-1core`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `raw-batch-l1-512-1core`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `fieldvec-dot-1024-1core`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
- P-20 Note `raw-batch-l1-512-streaming-6core`: bootstrap endpoints lack declared numerical resolution or twenty tail replicates
